# Reference deploy

MIT. Caddy terminates TLS; `nemo-server` listens on localhost (or `0.0.0.0` in the container). PostgreSQL is started with the frozen DDL ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). With `DATABASE_URL` set, the process loads that schema on start and write-throughs after successful mutating requests. Unset `DATABASE_URL` to keep the in-memory engine.

```text
docker compose -f deploy/compose.yml up --build
```

Default HTTPS port on the host: **8443**. Federation mTLS is **9443** inside the compose network (`NEMO_S2S_LISTEN`), not through Caddy. Optional coturn: `docker compose -f deploy/compose.yml --profile calls up`. The app is not identity; fetch `/v1/bundle` and check HPKE against the contact card. Pin peer bundles as `NEMO_PEERS_DIR/*.cbor`.
