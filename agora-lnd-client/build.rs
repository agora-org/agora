use std::process::{Command, Stdio};

fn main() {
  tonic_build::configure()
    .build_server(false)
    .format(
      Command::new("rustfmt")
        .arg("--version")
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false),
    )
    .compile(&["proto/rpc.proto"], &["proto"])
    .unwrap();
}
