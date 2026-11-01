# tunnel-server: OpenSpoke hub-side reverse tunnel server (Rust).
#
# Requires Rust 1.88+. The reqwest 0.12 upstream started pulling in
# icu_* / idna_adapter, which require rustc 1.88+.
FROM rust:1.90-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
      protobuf-compiler ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/tunnel.proto ./proto/
COPY src ./src

# Cargo.lock is committed, so --locked pins the dependency graph.
RUN cargo build --release --locked --bin tunnel-server

# tunnel-server is cluster-internal plaintext so TLS is not
# required, but axum reaches libc through hyper-util (TCP
# keepalive), so distroless/cc-debian12 is used instead of
# distroless/static.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/tunnel-server /tunnel-server
USER nonroot:nonroot
ENTRYPOINT ["/tunnel-server"]
