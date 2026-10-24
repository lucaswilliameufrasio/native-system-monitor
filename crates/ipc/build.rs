fn main() {
    let proto = "../../proto/native_monitor/v1/monitor.proto";
    let proto_root = "../../proto";
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("the vendored protoc binary must be available for this build");

    prost_build::Config::new()
        .protoc_executable(protoc)
        .compile_protos(&[proto], &[proto_root])
        .expect("the versioned IPC schema must compile");

    println!("cargo:rerun-if-changed={proto}");
}
