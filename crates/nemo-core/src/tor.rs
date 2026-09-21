//! Client→home via a SOCKS5 proxy (Arti or equivalent). Not a `tor` binary.
//!
//! Loopback homes stay direct: Tor exits cannot reach 127.0.0.1.
//! Set `NEMO_TOR_SOCKS` (default `127.0.0.1:9150`) when High/Maximum talks to a
//! public home.

use std::sync::Arc;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::error::{CoreError, Result};
use crate::home::{HttpRequest, HttpResponse};

pub fn is_loopback_origin(base: &str) -> bool {
    let host = origin_host(base);
    host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]"
}

pub fn socks_addr() -> String {
    std::env::var("NEMO_TOR_SOCKS").unwrap_or_else(|_| "127.0.0.1:9150".into())
}

pub async fn call(
    base: &str,
    req: &HttpRequest,
    tls: Arc<rustls::ClientConfig>,
) -> Result<HttpResponse> {
    let (host, port, https) = parse_origin(base)?;
    let mut stream = socks_connect(&socks_addr(), &host, port).await?;
    if https {
        let connector = TlsConnector::from(tls);
        let name = ServerName::try_from(host.clone())
            .map_err(|_| CoreError::Transport("home host is not a DNS name".into()))?
            .to_owned();
        let mut tls_stream = connector
            .connect(name, stream)
            .await
            .map_err(|e| CoreError::Transport(e.to_string()))?;
        http1(&mut tls_stream, &host, req).await
    } else {
        http1(&mut stream, &host, req).await
    }
}

fn origin_host(base: &str) -> &str {
    let rest = base
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(base)
        .trim_end_matches('/');
    rest.split(['/', ':']).next().unwrap_or(rest)
}

fn parse_origin(base: &str) -> Result<(String, u16, bool)> {
    let https = base.starts_with("https://");
    let rest = base
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(base)
        .trim_end_matches('/');
    let (host_port, _) = rest.split_once('/').unwrap_or((rest, ""));
    if let Some(stripped) = host_port.strip_prefix('[') {
        let (host, rest) = stripped
            .split_once(']')
            .ok_or_else(|| CoreError::Transport("bad home origin".into()))?;
        let port = rest
            .strip_prefix(':')
            .and_then(|p| p.parse().ok())
            .unwrap_or(if https { 443 } else { 80 });
        return Ok((format!("[{host}]"), port, https));
    }
    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse()
                .map_err(|_| CoreError::Transport("bad home port".into()))?,
        ),
        None => (host_port.to_string(), if https { 443 } else { 80 }),
    };
    Ok((host, port, https))
}

async fn socks_connect(proxy: &str, host: &str, port: u16) -> Result<TcpStream> {
    let mut s = TcpStream::connect(proxy).await.map_err(|e| {
        CoreError::Transport(format!(
            "Tor SOCKS at {proxy} is not reachable ({e}). Run Arti (not a system tor binary)."
        ))
    })?;
    s.write_all(&[0x05, 0x01, 0x00])
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    let mut greet = [0u8; 2];
    s.read_exact(&mut greet)
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    if greet != [0x05, 0x00] {
        return Err(CoreError::Transport("Tor SOCKS greeting refused".into()));
    }
    let host_bytes = host.as_bytes();
    if host_bytes.len() > 255 {
        return Err(CoreError::Transport("home host too long for SOCKS".into()));
    }
    let mut req = Vec::with_capacity(7 + host_bytes.len());
    req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03, host_bytes.len() as u8]);
    req.extend_from_slice(host_bytes);
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req)
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    let mut hdr = [0u8; 4];
    s.read_exact(&mut hdr)
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    if hdr[0] != 0x05 || hdr[1] != 0x00 {
        return Err(CoreError::Transport(format!(
            "Tor SOCKS connect failed ({})",
            hdr[1]
        )));
    }
    match hdr[3] {
        0x01 => {
            let mut skip = [0u8; 6];
            s.read_exact(&mut skip)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
        }
        0x03 => {
            let mut ln = [0u8; 1];
            s.read_exact(&mut ln)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
            let mut skip = vec![0u8; ln[0] as usize + 2];
            s.read_exact(&mut skip)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
        }
        0x04 => {
            let mut skip = [0u8; 18];
            s.read_exact(&mut skip)
                .await
                .map_err(|e| CoreError::Transport(e.to_string()))?;
        }
        _ => return Err(CoreError::Transport("Tor SOCKS address type".into())),
    }
    Ok(s)
}

async fn http1<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    host: &str,
    req: &HttpRequest,
) -> Result<HttpResponse> {
    let mut msg = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        req.method, req.path, host
    );
    for (k, v) in &req.headers {
        msg.push_str(&format!("{k}: {v}\r\n"));
    }
    if req.method == "POST" {
        msg.push_str("content-type: application/cbor\r\n");
        msg.push_str(&format!("content-length: {}\r\n", req.body.len()));
    }
    msg.push_str("\r\n");
    stream
        .write_all(msg.as_bytes())
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    if req.method == "POST" && !req.body.is_empty() {
        stream
            .write_all(&req.body)
            .await
            .map_err(|e| CoreError::Transport(e.to_string()))?;
    }
    stream
        .flush()
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .map_err(|e| CoreError::Transport(e.to_string()))?;
    parse_http_response(&buf)
}

fn parse_http_response(raw: &[u8]) -> Result<HttpResponse> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| CoreError::Transport("Tor HTTP response has no header".into()))?;
    let head = std::str::from_utf8(&raw[..sep])
        .map_err(|_| CoreError::Transport("Tor HTTP header is not UTF-8".into()))?;
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| CoreError::Transport("Tor HTTP status".into()))?;
    Ok(HttpResponse {
        status,
        body: raw[sep + 4..].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::is_loopback_origin;

    #[test]
    fn loopback_origins() {
        assert!(is_loopback_origin("http://127.0.0.1:8787"));
        assert!(is_loopback_origin("https://localhost:8443"));
        assert!(!is_loopback_origin("https://home.example:8443"));
    }
}
