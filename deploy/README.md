# Reference deploy

MIT. Caddy terminates TLS; `nemo-server` listens on localhost (or `0.0.0.0` in the container). PostgreSQL is started with the frozen DDL ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). With `DATABASE_URL` set, the process loads that schema on start and write-throughs after successful mutating requests. Unset `DATABASE_URL` to keep the in-memory engine.

There is no cloud vendor API. Operators run this compose file (or an equivalent Caddy + Postgres + `nemo-server` layout) on their own machines.

## Compose

From the repository root:

```text
docker compose -f deploy/compose.yml up --build
```

Default HTTPS port on the host: **8443**. Caddy uses `tls internal` (a local CA, not public Web PKI) and issues that cert on first handshake (`on_demand`, because the site is a catch-all `:443` published as host port 8443). That certificate is **not** identity: clients fetch `/v1/bundle` and check HPKE against the contact card ([ADR-0033](../docs/decisions/0033-local-http-api-and-postgres-schema.md)). The Nemo client accepts this transport TLS on purpose.

```text
curl -k https://localhost:8443/v1/bundle
```

Federation mTLS is **9443** inside the compose network (`NEMO_S2S_LISTEN`), not through Caddy. Pin peer bundles as `NEMO_PEERS_DIR/*.cbor`.

Optional 1:1 media relay:

```text
docker compose -f deploy/compose.yml --profile calls up --build
```

TURN credentials `nemo:nemo` and ports `3478` / `49152–49200` are a **reference** default, not identity. Point the desktop client at them with `NEMO_TURN_URL` / `NEMO_TURN_USER` / `NEMO_TURN_PASS` if you enable the profile.

## Postgres password

The compose file defaults `POSTGRES_PASSWORD` and the matching `DATABASE_URL` to `nemo`. That value is a **reference** so a fresh clone boots. Change it before any real deployment:

```text
export NEMO_POSTGRES_PASSWORD='a-password-you-chose'
docker compose -f deploy/compose.yml up --build
```

Do not reuse `nemo` on a host that is reachable beyond localhost.

## Two clients on this home

The Compose Multiplatform shell is AGPL (it links libsignal through `nemo-ffi`). JVM run is the supported desktop path; `jpackage` is optional.

```text
cargo build -p nemo-ffi
cd apps/compose
./gradlew run
```

`./gradlew run` builds `nemo-ffi` first and sets `jna.library.path` to `target/debug`. Create two vaults (two process windows, or two data directories), set **Home server** to `https://localhost:8443`, and Register. Local cargo without Caddy still uses `http://127.0.0.1:8787`.

Optional native installer (needs a JDK with `jpackage`, and still needs `libnemo_ffi.so` on `jna.library.path`):

```text
cd apps/compose
./gradlew packageDeb
```

## Licence split

`deploy/`, `nemo-wire`, and `nemo-server` are MIT. The desktop binary is AGPL-3.0-only. See [LICENSE.md](../LICENSE.md).

## Rate limits (v1 defaults)

These are capability- and peer-scoped. Excess is a generic failure, never "rate limited for Alice" ([docs/protocol/04-delivery-protocol.md](../docs/protocol/04-delivery-protocol.md), [05-federation.md](../docs/protocol/05-federation.md)).

| Limit | Default |
| --- | --- |
| Contact-capability mailbox appends | 30 / minute / capability |
| Federation inners accepted | 100 / second / peer |
| Mailbox owner `ts` | rejected if older than 120 s or more than 120 s in the future |
| Mailbox retention | 14 days / 500 MiB |
| Group stream | 30 days / 2 GiB |

Operators MAY tighten these; they MUST NOT advertise unbounded retention. There is no sender identity to rate-limit.
