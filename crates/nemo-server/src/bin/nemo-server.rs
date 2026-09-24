//! Listen on all interfaces by default (`0.0.0.0`). Caddy (or another reverse proxy) terminates TLS.
//! Override with `NEMO_LISTEN`. S2S mTLS is a separate port (ADR-0032).

use std::path::Path;

#[tokio::main]
async fn main() {
    nemo_server::tls_install();
    let addr = std::env::var("NEMO_LISTEN").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let host = std::env::var("NEMO_HOST").unwrap_or_else(|_| "local".into());
    let s2s_port: u16 = std::env::var("NEMO_S2S_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9443);
    let s2s_addr =
        std::env::var("NEMO_S2S_LISTEN").unwrap_or_else(|_| format!("127.0.0.1:{s2s_port}"));

    let state = if let Ok(url) = std::env::var("DATABASE_URL") {
        let pool = match nemo_server::pg::connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                nemo_server::log_ops("nemo-server postgres", e);
                std::process::exit(1);
            }
        };
        let (mut home, groups) = match nemo_server::pg::load_or_init(&pool, &host, s2s_port).await {
            Ok(v) => v,
            Err(e) => {
                nemo_server::log_ops("nemo-server load", e);
                std::process::exit(1);
            }
        };
        pin_dir(&mut home);
        if let Err(e) = nemo_server::pg::flush(&pool, &home, &groups).await {
            nemo_server::log_ops("nemo-server persist pins", e);
            std::process::exit(1);
        }
        eprintln!("nemo-server http://{addr} s2s={s2s_addr} host={host} db=postgres");
        nemo_server::AppState::from_parts(home, groups).with_db(pool)
    } else {
        let mut home = nemo_server::HomeServer::advertise(host.clone(), s2s_port);
        pin_dir(&mut home);
        eprintln!("nemo-server http://{addr} s2s={s2s_addr} host={host} (in-memory)");
        nemo_server::AppState::from_parts(home, nemo_server::GroupHost::new())
    };

    let st = state.clone();
    let s2s_bind = s2s_addr.clone();
    tokio::spawn(async move {
        if let Err(e) = nemo_server::s2s::listen(&s2s_bind, st).await {
            nemo_server::log_ops("s2s listen", e);
        }
    });
    tokio::spawn(nemo_server::s2s::pump_loop(state.clone()));

    nemo_server::serve(&addr, state).await.expect("listen");
}

fn pin_dir(home: &mut nemo_server::HomeServer) {
    if let Ok(dir) = std::env::var("NEMO_PEERS_DIR") {
        match nemo_server::s2s::load_peers_dir(home, Path::new(&dir)) {
            Ok(n) => eprintln!("nemo-server pinned {n} peer bundles from {dir}"),
            Err(e) => nemo_server::log_ops("nemo-server NEMO_PEERS_DIR", e),
        }
    }
}
