use executable_path::executable_path;
use hyper::StatusCode;
use reqwest::Url;
use std::{
  fs,
  io::{BufRead, BufReader, Read},
  process::{Child, ChildStderr, Command, Stdio},
};
use tempfile::TempDir;

struct AgoraInstance {
  _tempdir: TempDir,
  child: Child,
  port: u16,
  stderr: ChildStderr,
  collected_stderr: String,
}

impl AgoraInstance {
  fn new(tempdir: TempDir) -> Self {
    let mut child = Command::new(executable_path("agora"))
      .arg("--port=0")
      .arg("--directory=.")
      .current_dir(&tempdir)
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();

    let mut first_line = String::new();
    let child_stderr = child.stderr.take().unwrap();
    let mut child_stderr = BufReader::new(child_stderr);
    child_stderr.read_line(&mut first_line).unwrap();
    let port: u16 = first_line
      .strip_prefix("Listening on 0.0.0.0:")
      .unwrap()
      .trim_end()
      .parse()
      .unwrap();

    AgoraInstance {
      _tempdir: tempdir,
      child,
      port,
      collected_stderr: first_line,
      stderr: child_stderr.into_inner(),
    }
  }

  fn base_url(&self) -> Url {
    Url::parse(&format!("http://localhost:{}", self.port)).unwrap()
  }

  fn kill(mut self) -> String {
    self.child.kill().unwrap();
    self.child.wait().unwrap();
    self
      .stderr
      .read_to_string(&mut self.collected_stderr)
      .unwrap();
    self.collected_stderr
  }
}

#[test]
fn server_listens_on_all_ip_addresses() {
  let tempdir = tempfile::tempdir().unwrap();
  let agora = AgoraInstance::new(tempdir);
  assert_eq!(
    reqwest::blocking::get(agora.base_url()).unwrap().status(),
    StatusCode::OK
  );
  let stderr = agora.kill();
  assert!(stderr.contains("0.0.0.0:"));
}

#[test]
fn errors_contain_backtraces() {
  let tempdir = tempfile::tempdir().unwrap();
  fs::write(tempdir.path().join(".hidden"), "").unwrap();
  let agora = AgoraInstance::new(tempdir);
  let status = reqwest::blocking::get(agora.base_url().join("/files/.hidden").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::NOT_FOUND);
  let rest = agora.kill();
  assert!(rest.contains("agora::files::Files::check_path"));
}
