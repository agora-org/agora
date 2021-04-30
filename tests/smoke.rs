use executable_path::executable_path;
use hyper::StatusCode;
use std::{
  fs,
  io::{BufRead, BufReader},
  process::{Command, Stdio},
};

#[test]
fn server_listens_on_localhost_8080() {
  let tempdir = tempfile::tempdir().unwrap();

  fs::create_dir(tempdir.path().join("www")).unwrap();

  let child = Command::new(executable_path("foo"))
    .current_dir(&tempdir)
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  BufReader::new(child.stderr.unwrap())
    .read_line(&mut String::new())
    .unwrap();

  assert_eq!(
    reqwest::blocking::get("http://localhost:8080")
      .unwrap()
      .status(),
    StatusCode::OK
  );
}
