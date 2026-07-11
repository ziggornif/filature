# syntax=docker/dockerfile:1

# ---- builder ---------------------------------------------------------------
# Non-slim image: ships the C toolchain the final link step needs. Pinned to
# the toolchain the repo is developed against (edition 2024 needs >= 1.85).
FROM rust:1.97-bookworm AS builder
WORKDIR /app

# Build against the committed .sqlx query cache — no database needed at build
# time (the query! macros are verified offline).
ENV SQLX_OFFLINE=true

# The workspace is two small crates; copy it whole and build the release
# binary. Templates, static assets, i18n catalogs and migrations are all
# embedded into the binary (rust-embed + sqlx::migrate!), so nothing else is
# needed at runtime.
COPY . .
RUN cargo build --release -p filature

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates is not strictly required today (sqlx is built without TLS and
# the server makes no outbound HTTPS) but is cheap and future-proofs an
# external/TLS database. Run as an unprivileged user.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --user-group filature

COPY --from=builder /app/target/release/filature /usr/local/bin/filature

USER filature
# The port the app listens on inside the container; the actual bind address is
# set via FILATURE_SERVER__BIND (see docker-compose.yml).
EXPOSE 8080

# All configuration comes from FILATURE_* env vars — no filature.toml is shipped
# in the image (Config::load treats a missing file as "env only").
ENTRYPOINT ["filature"]
