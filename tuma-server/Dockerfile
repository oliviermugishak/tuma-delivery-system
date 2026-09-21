# Tuma API — the deployable image.
#
# Docker (not a native Rust buildpack) is deliberate: rustls pulls in
# `aws-lc-sys`, a C dependency that needs cmake and a C toolchain at build
# time. Pinning the toolchain here makes the build reproducible and removes
# every "works locally, fails on the host" surprise.
#
# The image holds ONLY the API. It serves no static files — the platform
# is a separate deploy.

# ---- build -----------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder

# aws-lc-sys (via rustls) is the reason for cmake/perl; the rest is the
# standard floor for a SQLx workspace.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential cmake perl pkg-config \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# The committed `.sqlx` cache (151 queries) means the compile-time-checked
# macros resolve WITHOUT a database — so a build never needs a live DB.
ENV SQLX_OFFLINE=true

COPY . .

# `--bin` skips the four auxiliary binaries (openapi export, seeding,
# simulator) — only the server ships.
RUN cargo build --release --locked --bin tuma-server

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates is load-bearing: rustls-platform-verifier reads the
# system trust store, so HTTPS to Google/Neon/R2 fails without it. The
# builder image already carries an up-to-date bundle — copying it beats
# an apt round trip (no package-index dependency in this stage at all).
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

WORKDIR /app

COPY --from=builder /app/target/release/tuma-server ./tuma-server
# Layered config: base.yml + production.yml, read relative to the workdir.
COPY --from=builder /app/configuration ./configuration

EXPOSE 8080

# The host hands us PORT; the server reads APP_APPLICATION__PORT (its own
# layered-config convention). One line of glue keeps both contracts honest.
CMD ["sh", "-c", "APP_APPLICATION__PORT=${PORT:-8080} exec ./tuma-server"]