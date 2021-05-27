use executable_path::executable_path;
use std::process::Command;

#[test]
fn help_returns_success() {
  assert!(Command::new(executable_path("foo"))
    .arg("--help")
    .output()
    .unwrap()
    .status
    .success());
}

#[test]
fn version_returns_success() {
  assert!(Command::new(executable_path("foo"))
    .arg("--version")
    .output()
    .unwrap()
    .status
    .success());
}
