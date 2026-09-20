//! Listen on localhost. Caddy (or another reverse proxy) terminates TLS.

#[tokio::main]
async fn main() {
    let addr = std::env::var("NEMO_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    eprintln!("nemo-server http://{addr}");
    nemo_server::serve(&addr, nemo_server::AppState::new())
        .await
        .expect("listen");
}
