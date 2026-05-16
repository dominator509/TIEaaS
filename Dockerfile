

ARG RUST_VERSION=1.75
ARG DEBIAN_VERSION=bookworm
ARG APP_NAME=tie
ARG APP_FEATURES=swagger-ui

FROM public.ecr.aws/docker/library/rust:${RUST_VERSION}-slim-${DEBIAN_VERSION} AS builder
ARG APP_NAME
ARG APP_FEATURES
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    clang \
    cmake \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml rust-toolchain.toml ./
COPY src ./src
COPY migrations ./migrations
COPY config ./config
COPY openapi.yaml ./openapi.yaml

RUN cargo build --release --features "${APP_FEATURES}" --bin ${APP_NAME}

FROM public.ecr.aws/debian/debian:${DEBIAN_VERSION}-slim AS runtime
ARG APP_NAME
ENV APP_NAME=${APP_NAME}
ENV TIE_HOST=0.0.0.0
ENV TIE_PORT=8080
ENV TIE_DATABASE_URL=sqlite:///var/lib/tie/tie.db
ENV TIE_CONFIG_PATH=/app/config/default.toml
ENV RUST_LOG=info

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    sqlite3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /home/tie --shell /usr/sbin/nologin tie \
    && mkdir -p /app /var/lib/tie \
    && chown -R tie:tie /app /var/lib/tie /home/tie

WORKDIR /app
COPY --from=builder /app/target/release/${APP_NAME} /usr/local/bin/${APP_NAME}
COPY --from=builder /app/config ./config
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/openapi.yaml ./openapi.yaml

USER tie
EXPOSE 8080
VOLUME ["/var/lib/tie"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${TIE_PORT}/healthz" || exit 1

ENTRYPOINT ["/usr/local/bin/tie"]
CMD ["serve"]
