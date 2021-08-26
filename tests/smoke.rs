use executable_path::executable_path;
use hyper::StatusCode;
use std::{
  fs,
  io::{BufRead, BufReader},
  process::{Command, Stdio},
};

#[test]
fn server_listens_on_all_ip_addresses() {
  let tempdir = tempfile::tempdir().unwrap();

  fs::create_dir(tempdir.path().join("www")).unwrap();

  let mut child = Command::new(executable_path("agora"))
    .arg("--http-port=8080")
    .arg("--directory=www")
    .current_dir(&tempdir)
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  let child_stderr = child.stderr.take().unwrap();
  let result = std::panic::catch_unwind(|| {
    let mut line = String::new();

    BufReader::new(child_stderr).read_line(&mut line).unwrap();
    eprintln!("stderr: {}", line);

    assert!(line.contains("0.0.0.0:8080"));

    assert_eq!(
      reqwest::blocking::get("http://localhost:8080")
        .unwrap()
        .status(),
      StatusCode::OK
    );
  });
  child.kill().unwrap();
  child.wait().unwrap();
  result.unwrap();
}
