FROM docker.io/library/rust:1.94.0-trixie@sha256:f17e723020f87c1b4ac4ff6d73c9dfbb7d5cb978754c76641e47337d65f61e12 AS build

RUN apt-get update && apt-get install -y --no-install-recommends clang libclang-dev libpam0g-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src/backend
COPY VERSION ../VERSION
COPY backend/Cargo.toml backend/Cargo.lock backend/build.rs ./
COPY backend/src/ ./src/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/backend/target \
    cargo build --locked --release && install -Dm755 target/release/infra-av-backend /out/infra-av-backend

FROM docker.io/library/debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2

LABEL org.opencontainers.image.title="VANTAGE Antivirus"
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates clamav clamav-freshclam curl libpam0g util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && install -d -m 0755 /etc/infra-av /var/lib/infra-av/temp
COPY --from=build /out/infra-av-backend /usr/local/bin/infra-av-backend
COPY containers/freshclam.conf /etc/clamav/infra-av.conf
ENV INFRA_AV_HOST=127.0.0.1 INFRA_AV_PORT=8090 INFRA_AV_DATA_DIR=/var/lib/infra-av \
    INFRA_AV_USERS_FILE=/etc/infra-av/users.json INFRA_AV_SPLIT_WORKER=1 \
    INFRA_AV_FRESHCLAM_CONF=/etc/clamav/infra-av.conf TMPDIR=/var/lib/infra-av/temp \
    INFRA_AV_READ_ONLY_SCAN_ROOTS=/srv/vantage-storage-mounts
EXPOSE 8090
ENTRYPOINT ["infra-av-backend"]
CMD ["server"]
