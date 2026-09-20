# Reference deploy

MIT. Caddy terminates TLS; `nemo-server` listens on localhost (or `0.0.0.0` in the container). PostgreSQL is started with the frozen DDL ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). The process still uses the in-memory mailbox engine until sqlx is wired; the database is there so that adapter can attach without a schema change.

```text
docker compose -f deploy/compose.yml up --build
```

Default HTTPS port on the host: **8443**. The app is not identity; fetch `/v1/bundle` and check HPKE against the contact card.
