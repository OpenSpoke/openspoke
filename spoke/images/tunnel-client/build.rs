// Generate Rust code from proto/tunnel.proto with tonic-build.
//
// The generated file lands in $OUT_DIR/openspoke.tunnel.v1.rs, which
// src/main.rs picks up via `tonic::include_proto!("openspoke.tunnel.v1")`.
// It is not checked in (same convention as the previous Go build).
//
// tunnel-server is a separate binary, so the server-side stubs are
// not needed here.
//
// build_transport(false):
//   When an RPC is named `Connect`, tonic-build's default output has
//   TunnelClient::connect(dst) (a URI-based channel factory) that
//   collides with the connect() method generated from the proto's
//   `rpc Connect`. We do not use the factory; we build Endpoint::
//   from_shared -> Channel -> TunnelClient::new(channel) ourselves,
//   so the transport factory is unnecessary.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .build_transport(false)
        .compile_protos(&["proto/tunnel.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/tunnel.proto");
    Ok(())
}
