# League of Vibes — image de production : le serveur autoritaire (crates/lov-server).
# Les joueurs lancent le client natif (releases GitHub) et s'y connectent : LOV_URL=ws://<hôte>:3000/ws.

# --- Serveur (seul lov-server et ses dépendances : ni Bevy ni le client natif)
FROM rust:slim-bookworm AS server
WORKDIR /app
# rustup installe la version épinglée par rust-toolchain.toml.
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain || rustup toolchain install
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo build --release --locked -p lov-server

# --- Image finale
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /app/target/release/lov-server /usr/local/bin/lov-server
ENV PORT=3000
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s CMD curl -sf "http://127.0.0.1:${PORT}/health" || exit 1
USER nobody
CMD ["lov-server"]
