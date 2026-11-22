// build.rs - generate the gRPC service when the grpc feature is on (NF-10).
//
// The generated code goes to OUT_DIR. The build script finds protoc in the
// repository tools directory first, then on PATH. Without the feature the
// crate keeps its zero-dependency default build.

#[cfg(feature = "grpc")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let protoc = manifest.join("../../tools/protoc/bin/protoc.exe");
    if protoc.is_file() {
        std::env::set_var("PROTOC", &protoc);
    }
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["src/grpc/proto/sheaf.proto"], &["src/grpc/proto"])?;
    println!("cargo:rerun-if-changed=src/grpc/proto/sheaf.proto");
    Ok(())
}

#[cfg(not(feature = "grpc"))]
fn main() {
    println!("cargo:rustc-check-cfg=cfg(sheaf_native_rdma)");}
