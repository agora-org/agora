fn main() {
  tonic_build::configure()
    .build_server(false)
    .format(false)
    .compile(&["proto/rpc.proto"], &["proto"])
    .unwrap();
}
