// Generate Rust code from proto/tunnel.proto with tonic-build.
//
// The generated file lands in $OUT_DIR/openspoke.tunnel.v1.rs, which
// src/main.rs picks up via `tonic::include_proto!("openspoke.tunnel.v1")`.
// It is not checked into the repository.
//
// Unlike the tunnel-client build we need the server side of the
// generated code:
//   build_server(true)     - the Tunnel trait to impl
//   build_client(false)    - no client factory
//   build_transport(true)  - to use Server::builder().add_service(...)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["proto/tunnel.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/tunnel.proto");
    Ok(())
}
