use {executable_path::executable_path, std::process::Command};

#[test]
fn help_returns_success() {
  assert!(Command::new(executable_path("agora"))
    .arg("--help")
    .output()
    .unwrap()
    .status
    .success());
}

#[test]
fn version_returns_success() {
  assert!(Command::new(executable_path("agora"))
    .arg("--version")
    .output()
    .unwrap()
    .status
    .success());
}
