# syntax=docker/dockerfile:1
# SyncFiles V1 server — build multi-stage.
#
# Stage 1 (builder): compila syncfiles-server en release con Rust toolchain.
# Stage 2 (runtime): imagen mínima Debian bookworm-slim con solo el binario,
# libssl/libcrypto y los archivos estáticos del dashboard.
#
# La BD (SQLite) y el storage viven en volúmenes persistentes montados en
# /data, por lo que el contenedor es stateless salvo por esos mount points.

# ---------------------------------------------------------------------------
# Stage 1: build
# ---------------------------------------------------------------------------
FROM rust:1.83-bookworm AS builder

WORKDIR /build

# Instalar dependencias de compilación de ssl-sys (necesario para sqlx/tokio-postgres
# y rustls). En V1 solo usamos SQLite, pero se deja openssl por si se migra a V2.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
        gcc \
        g++ \
        libc6-dev \
    && rm -rf /var/lib/apt/lists/*

# Copiar manifests primero para cachear la capa de dependencias.
COPY syncfiles-models/Cargo.toml syncfiles-models/Cargo.lock ./syncfiles-models/
COPY syncfiles-server/Cargo.toml syncfiles-server/Cargo.lock ./syncfiles-server/

# Capa de falsas fuentes para que `cargo build` cachee las dependencias.
RUN mkdir -p syncfiles-models/src syncfiles-server/src \
    && echo 'pub fn _stub(){}' > syncfiles-models/src/lib.rs \
    && echo 'fn main(){}' > syncfiles-server/src/main.rs \
    && cd syncfiles-server && cargo build --release --locked 2>/dev/null || true

# Copiar el código real y reconstruir solo lo necesario.
COPY syncfiles-models/src ./syncfiles-models/src
COPY syncfiles-server/src ./syncfiles-server/src
COPY syncfiles-server/migrations ./syncfiles-server/migrations
COPY syncfiles-server/docs ./syncfiles-server/docs
COPY syncfiles-server/static ./syncfiles-server/static

# Construir release (binario + assets embebidos en el exe por include_str!).
RUN cd syncfiles-server && cargo build --release --locked

# ---------------------------------------------------------------------------
# Stage 2: runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        sqlite3 \
        tar \
        curl \
        bash \
    && rm -rf /var/lib/apt/lists/*

# Crear usuario no-root para ejecutar el server.
RUN groupadd -r syncfiles && useradd -r -g syncfiles -d /data -s /sbin/nologin syncfiles

WORKDIR /app

# Copiar binario y assets construidos.
COPY --from=builder /build/syncfiles-server/target/release/syncfiles-server /app/syncfiles-server
COPY --from=builder /build/syncfiles-server/migrations /app/migrations
COPY --from=builder /build/syncfiles-server/static /app/static
COPY --from=builder /build/syncfiles-server/docs /app/docs
COPY scripts/backup.sh /scripts/backup.sh
COPY scripts/restore.sh /scripts/restore.sh
RUN chmod 0755 /scripts/backup.sh /scripts/restore.sh

# Directorios de datos (volúmenes persistentes).
RUN mkdir -p /data/storage /data/backups /data/db \
    && chown -R syncfiles:syncfiles /app /data

# Variables de entorno por defecto (sobrescribibles).
# - Bind: 0.0.0.0:8080 para que funcione detrás de un reverse proxy y en LAN.
# - DB y storage bajo /data (persistencia del volumen).
ENV     SF_BIND_ADDRESS=0.0.0.0:8080 \
    SF_MIGRATIONS_DIR=/app/migrations \
    SF_STATIC_DIR=/app/static \
    SF_DATABASE_URL=sqlite:/data/db/syncfiles.db \

    SF_SERVER_URL=http://localhost:8080 \
    SF_STORAGE_ROOT=/data/storage \
    BACKUP_DIR=/data/backups \
    SF_BACKUP_LOG=/data/backup.log \
    RUST_LOG=info

# Health checks (liveness/readiness) en /health/live y /health/ready.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8080/health/live || exit 1

EXPOSE 8080

USER syncfiles

# El server resuelve migrations/static/CWD desde el binario, no del host.
CMD ["/app/syncfiles-server"]