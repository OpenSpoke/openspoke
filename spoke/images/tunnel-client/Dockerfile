# tunnel-client: OpenSpoke spoke-side reverse tunnel client (Rust).
#
# Requires Rust 1.88+. The reqwest 0.12 upstream started pulling in
# icu_* / idna_adapter, which require rustc 1.88+; the same
# requirement applies here for symmetry with tunnel-server.
FROM rust:1.90-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
      protobuf-compiler ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# Copy sources and build. The proto is generated during the build
# by tonic-build (see build.rs).
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto/tunnel.proto ./proto/
COPY src ./src

# Cargo.lock is committed, so --locked pins the dependency graph.
RUN cargo build --release --locked --bin tunnel-client

# distroless/cc-debian12 provides glibc / libgcc, the minimum
# runtime needed by tokio and hyper. TLS is handled entirely by
# rustls + webpki-roots, so no system CA store is required.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/tunnel-client /tunnel-client
USER nonroot:nonroot
ENTRYPOINT ["/tunnel-client"]
