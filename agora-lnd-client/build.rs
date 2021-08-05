fn main() {
  tonic_build::configure()
    .build_server(false)
    .compile(&["proto/rpc.proto"], &["proto"])
    .unwrap();
}
