use executable_path::executable_path;
use hyper::StatusCode;
use std::{
  fs,
  io::{BufRead, BufReader},
  process::{Command, Stdio},
};

#[test]
fn server_listens_on_all_ip_addresses_port_8080() {
  let tempdir = tempfile::tempdir().unwrap();

  fs::create_dir(tempdir.path().join("www")).unwrap();

  let child = Command::new(executable_path("foo"))
    .current_dir(&tempdir)
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  let mut line = String::new();

  BufReader::new(child.stderr.unwrap())
    .read_line(&mut line)
    .unwrap();

  assert!(line.contains("0.0.0.0:8080"));

  assert_eq!(
    reqwest::blocking::get("http://localhost:8080")
      .unwrap()
      .status(),
    StatusCode::OK
  );
}
