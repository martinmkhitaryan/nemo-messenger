# MIT. Builds only nemo-server (no libsignal / nemo-core).
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/nemo-wire ./crates/nemo-wire
COPY crates/nemo-server ./crates/nemo-server
# Workspace still lists nemo-core; replace so this image never fetches libsignal.
RUN printf '%s\n' \
    '[workspace]' \
    'resolver = "2"' \
    'members = ["crates/nemo-wire", "crates/nemo-server"]' \
    '' \
    '[workspace.package]' \
    'edition = "2021"' \
    'license = "MIT"' \
    'repository = "https://github.com/martinmkhitaryan/nemo-messenger"' \
    > Cargo.toml
RUN cargo build --release -p nemo-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/nemo-server /usr/local/bin/nemo-server
ENV NEMO_LISTEN=0.0.0.0:8787
EXPOSE 8787
USER nobody
CMD ["nemo-server"]
