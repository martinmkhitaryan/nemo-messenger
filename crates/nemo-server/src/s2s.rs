//! mTLS listener and outbound hop (phase 5, ADR-0032). In-process `pump` remains for tests.

use std::path::Path;
use std::time::Duration;

use nemo_wire::ids::ServerId;
use nemo_wire::{hello_payload, parse_payload, S2sFrame, S2sPayload, ServerBundle};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

use crate::error::{Result, ServerError};
use crate::federation::{
    accept_frame, pin, reset_link, take_frame, PumpStats, BACKOFF_CAP_SECS, OUTBOUND_MAX_AGE_SECS,
};
use crate::home::HomeServer;
use crate::http::AppState;
use crate::tls;

const MAX_FRAME: usize = 18_000_000;

pub async fn listen(addr: &str, state: AppState) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    eprintln!("nemo-server s2s {addr}");
    accept_loop(listener, state).await
}

pub async fn accept_loop(listener: TcpListener, state: AppState) -> std::io::Result<()> {
    let cfg = tls::server_config(state.home.clone())
        .await
        .map_err(std::io::Error::other)?;
    let acceptor = TlsAcceptor::from(cfg);
    loop {
        let (tcp, _) = listener.accept().await?;
        let acc = acceptor.clone();
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_inbound(st, acc, tcp).await {
                eprintln!("s2s inbound: {e}");
            }
        });
    }
}

async fn handle_inbound(
    state: AppState,
    acceptor: TlsAcceptor,
    tcp: TcpStream,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tls = acceptor.accept(tcp).await?;
    let (_, conn) = tls.get_ref();
    let cert = conn
        .peer_certificates()
        .and_then(|c| c.first())
        .ok_or("no client cert")?;
    let pk = tls::ed25519_spki(cert)?;
    let peer = {
        let home = state.home.lock().await;
        home.peer_id_for_sign_key(&pk).ok_or(ServerError::NotPinned)?
    };
    let mut stream = tls;
    let hello = read_frame(&mut stream).await?;
    let our_hello = {
        let mut home = state.home.lock().await;
        accept_frame(&mut home, peer, &hello)?;
        if let Some(p) = home.peers.get_mut(&peer) {
            p.hello_done = true;
        }
        let us = home.server_id();
        take_frame(&mut home, peer, hello_payload(&us, &peer))?
    };
    write_frame(&mut stream, &our_hello).await?;
    loop {
        let frame = read_frame(&mut stream).await?;
        let reply = {
            let mut home = state.home.lock().await;
            match accept_frame(&mut home, peer, &frame) {
                Ok(payload) if !payload.is_empty() => {
                    Some(take_frame(&mut home, peer, payload)?)
                }
                Ok(_) => None,
                Err(ServerError::CounterGap) => {
                    reset_link(&mut home, peer);
                    return Err(Box::new(ServerError::CounterGap));
                }
                Err(e) => return Err(Box::new(e)),
            }
        };
        if let Some(reply) = reply {
            write_frame(&mut stream, &reply).await?;
        }
        state.persist().await;
    }
}

/// Dial a pinned peer and forward due outbound rows.
pub async fn dial_and_pump(state: &AppState, dest: ServerId) -> Result<PumpStats> {
    let (bundle, cfg) = {
        let home = state.home.lock().await;
        let peer = home.peers.get(&dest).ok_or(ServerError::NotPinned)?;
        if peer.refused {
            return Err(ServerError::PeerRefused);
        }
        let bundle = peer.bundle.clone();
        let cfg = tls::client_config(&home, bundle.server_sign_public_key)
            .map_err(|_| ServerError::NotPinned)?;
        (bundle, cfg)
    };
    let host = if bundle.host.is_empty() {
        "localhost".into()
    } else {
        bundle.host.clone()
    };
    let port = bundle.s2s_port as u16;
    let tcp = TcpStream::connect((host.as_str(), port))
        .await
        .map_err(|_| ServerError::Denied)?;
    let server_name = rustls::pki_types::ServerName::try_from(host)
        .map_err(|_| ServerError::Denied)?
        .to_owned();
    let connector = tokio_rustls::TlsConnector::from(cfg);
    let mut tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|_| ServerError::Denied)?;

    let hello = {
        let mut home = state.home.lock().await;
        let us = home.server_id();
        take_frame(&mut home, dest, hello_payload(&us, &dest))?
    };
    write_frame(&mut tls, &hello)
        .await
        .map_err(|_| ServerError::Denied)?;
    let peer_hello = read_frame(&mut tls).await.map_err(|_| ServerError::Denied)?;
    {
        let mut home = state.home.lock().await;
        accept_frame(&mut home, dest, &peer_hello)?;
        if let Some(p) = home.peers.get_mut(&dest) {
            p.hello_done = true;
        }
    }

    let mut stats = PumpStats::default();
    loop {
        let row = {
            let mut home = state.home.lock().await;
            next_due(&mut home, dest)
        };
        let Some(row) = row else {
            break;
        };
        match forward_row(state, dest, &mut tls, &row).await {
            Ok(()) => {
                let mut home = state.home.lock().await;
                remove_matching_outbound(&mut home, &row);
                stats.delivered += 1;
            }
            Err(ServerError::Denied) | Err(ServerError::PeerRefused) => {
                let mut home = state.home.lock().await;
                remove_matching_outbound(&mut home, &row);
                stats.failed += 1;
            }
            Err(ServerError::CounterGap) => {
                let mut home = state.home.lock().await;
                reset_link(&mut home, dest);
                backoff_row(&mut home, &row);
                stats.failed += 1;
                break;
            }
            Err(_) => {
                let mut home = state.home.lock().await;
                backoff_row(&mut home, &row);
                stats.failed += 1;
                break;
            }
        }
        state.persist().await;
    }
    Ok(stats)
}

pub async fn pump_due(state: &AppState) {
    let dests = {
        let home = state.home.lock().await;
        due_destinations(&home)
    };
    for dest in dests {
        if let Err(e) = dial_and_pump(state, dest).await {
            eprintln!("s2s pump {e}");
        }
    }
}

pub fn load_peers_dir(home: &mut HomeServer, dir: &Path) -> Result<usize> {
    let mut n = 0;
    for ent in std::fs::read_dir(dir).map_err(|_| ServerError::Denied)? {
        let path = ent.map_err(|_| ServerError::Denied)?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("cbor") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|_| ServerError::Denied)?;
        let bundle = ServerBundle::decode(&bytes)?;
        pin(home, bundle)?;
        n += 1;
    }
    Ok(n)
}

fn due_destinations(home: &HomeServer) -> Vec<ServerId> {
    let now = home.now;
    let mut out = Vec::new();
    for row in &home.outbound {
        if row.next_attempt <= now && !out.contains(&row.dest) {
            if home.peers.contains_key(&row.dest) && !home.peer_refused(row.dest) {
                out.push(row.dest);
            }
        }
    }
    out
}

fn next_due(home: &mut HomeServer, dest: ServerId) -> Option<crate::federation::OutboundRow> {
    let now = home.now;
    home.outbound.iter().find(|r| {
        r.dest == dest
            && r.next_attempt <= now
            && now.saturating_sub(r.enqueued_at) <= OUTBOUND_MAX_AGE_SECS
    }).cloned()
}

fn remove_matching_outbound(home: &mut HomeServer, row: &crate::federation::OutboundRow) {
    if let Some(i) = home.outbound.iter().position(|r| {
        r.dest == row.dest && r.outer.hpke_ciphertext == row.outer.hpke_ciphertext
    }) {
        home.outbound.remove(i);
    }
}

fn backoff_row(home: &mut HomeServer, row: &crate::federation::OutboundRow) {
    let now = home.now;
    if let Some(r) = home.outbound.iter_mut().find(|r| {
        r.dest == row.dest && r.outer.hpke_ciphertext == row.outer.hpke_ciphertext
    }) {
        r.backoff_secs = (r.backoff_secs.saturating_mul(2)).min(BACKOFF_CAP_SECS);
        r.next_attempt = now.saturating_add(r.backoff_secs);
    }
}

async fn forward_row<S: AsyncRead + AsyncWrite + Unpin>(
    state: &AppState,
    dest: ServerId,
    stream: &mut S,
    row: &crate::federation::OutboundRow,
) -> Result<()> {
    let frame = {
        let mut home = state.home.lock().await;
        take_frame(&mut home, dest, row.outer.hpke_ciphertext.clone())?
    };
    write_frame(stream, &frame)
        .await
        .map_err(|_| ServerError::Denied)?;
    let reply = read_frame(stream).await.map_err(|_| ServerError::Denied)?;
    let payload = {
        let mut home = state.home.lock().await;
        accept_frame(&mut home, dest, &reply)?
    };
    match parse_payload(&payload)? {
        S2sPayload::Fail => Err(ServerError::Denied),
        S2sPayload::Ok { .. } => Ok(()),
        _ => {
            if payload == nemo_wire::fail_payload() {
                Err(ServerError::Denied)
            } else {
                Ok(())
            }
        }
    }
}

async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    frame: &S2sFrame,
) -> std::io::Result<()> {
    let bytes = frame.encode().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "s2s frame")
    })?;
    w.write_all(&bytes).await?;
    w.flush().await
}

async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<S2sFrame> {
    let mut hdr = [0u8; 12];
    r.read_exact(&mut hdr).await?;
    let n = u32::from_be_bytes(hdr[8..12].try_into().unwrap()) as usize;
    if n > MAX_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "s2s frame too large",
        ));
    }
    let mut payload = vec![0u8; n];
    r.read_exact(&mut payload).await?;
    Ok(S2sFrame {
        counter: u64::from_be_bytes(hdr[0..8].try_into().unwrap()),
        payload,
    })
}

pub async fn pump_loop(state: AppState) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        {
            let mut home = state.home.lock().await;
            home.now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(home.now);
        }
        pump_due(&state).await;
    }
}
