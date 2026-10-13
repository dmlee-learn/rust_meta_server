# Phase 13: 멀티 스테이지 Dockerfile
# rquickjs-sys C 컴파일 지원 (pkg-config, libssl-dev 필요)

FROM rust:1.88-slim AS builder
WORKDIR /app

# C 빌드 도구 + SSL
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY scripts/ scripts/
COPY migrations/ migrations/

RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/meta_server /usr/local/bin/
COPY migrations/ /app/migrations/
COPY scripts/ /app/scripts/
COPY templates/ /app/templates/
COPY docs/ /app/docs/

ENV PORT=3000
EXPOSE 3000

# Phase 13: 헬스체크
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

CMD ["meta_server"]
