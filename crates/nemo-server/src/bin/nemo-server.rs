//! Listen on localhost. Caddy (or another reverse proxy) terminates TLS.

#[tokio::main]
async fn main() {
    let addr = std::env::var("NEMO_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let host = std::env::var("NEMO_HOST").unwrap_or_else(|_| "local".into());
    let s2s_port: u16 = std::env::var("NEMO_S2S_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8443);

    let state = if let Ok(url) = std::env::var("DATABASE_URL") {
        let pool = nemo_server::pg::connect(&url).await.expect("postgres");
        let (home, groups) = nemo_server::pg::load_or_init(&pool, &host, s2s_port)
            .await
            .expect("load");
        eprintln!(
            "nemo-server http://{addr} host={host} s2s_port={s2s_port} db=postgres"
        );
        nemo_server::AppState::from_parts(home, groups).with_db(pool)
    } else {
        eprintln!("nemo-server http://{addr} host={host} (in-memory; set DATABASE_URL to persist)");
        nemo_server::AppState::from_parts(
            nemo_server::HomeServer::advertise(host, s2s_port),
            nemo_server::GroupHost::new(),
        )
    };

    nemo_server::serve(&addr, state).await.expect("listen");
}
