FROM rust:1.84.1-bookworm AS builder


RUN \
    --mount=type=cache,sharing=locked,target=/var/lib/apt/lists \
    --mount=type=cache,sharing=locked,target=/var/cache/apt \
<<EOF
    set -eux
    rm -f /etc/apt/apt.conf.d/docker-clean
    apt-get update
    apt-get -y --no-install-recommends install \
        protobuf-compiler
EOF


WORKDIR /build
RUN \
    --mount=from=buildSrc,target=/build \
    --mount=type=cache,sharing=locked,target=/build/target \
    --mount=type=cache,sharing=locked,target=/build/src/protos \
    --mount=type=cache,sharing=locked,target=$CARGO_HOME/git \
    --mount=type=cache,sharing=locked,target=$CARGO_HOME/registry \
<<EOF
    set -eux

    # Error if this wasn't set:
    echo "$CARGO_HOME"

    cargo build --release

    mkdir /result
    cp target/release/diskuto /result
    ls -lh /result
EOF

# ------------------------
FROM debian:bookworm AS runtime

# We'll find/write a diskuto.sqlite3 file in CWD by default.
# We expect a volume to be mounted here for data longevity:
WORKDIR /data

COPY --from=builder /result/diskuto /usr/local/bin