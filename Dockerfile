# League of Vibes — image de production : serveur autoritaire Rust (lov-server) + client web statique.
#
# Le client web (packages/client) est encore servi en HTTP jusqu'à la bascule vers le client natif ;
# le serveur, lui, est celui de `crates/lov-server`.

# --- Client web statique
FROM node:24-slim AS client
WORKDIR /app
COPY package.json package-lock.json ./
COPY packages/shared/package.json packages/shared/
COPY packages/server/package.json packages/server/
COPY packages/client/package.json packages/client/
RUN npm ci --no-audit --no-fund
COPY tsconfig.base.json ./
COPY packages packages
RUN npm run build -w @lov/client

# --- Serveur Rust (seul lov-server et ses dépendances : ni Bevy ni le client natif)
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
COPY --from=client /app/packages/client/dist /app/client
ENV PORT=3000 CLIENT_DIST=/app/client
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s CMD curl -sf "http://127.0.0.1:${PORT}/health" || exit 1
USER nobody
CMD ["lov-server"]
