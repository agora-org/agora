use executable_path::executable_path;
use hyper::StatusCode;
use reqwest::Url;
use std::{
  fs,
  future::Future,
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
  fn new(tempdir: TempDir, additional_flags: Vec<&str>) -> Self {
    let mut child = Command::new(executable_path("agora"))
      .args(additional_flags)
      .arg("--directory=.")
      .arg("--address=localhost")
      .current_dir(&tempdir)
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();

    let mut first_line = String::new();
    let child_stderr = child.stderr.take().unwrap();
    let mut child_stderr = BufReader::new(child_stderr);
    child_stderr.read_line(&mut first_line).unwrap();
    eprintln!("First line: {}", first_line);
    let port: u16 = first_line
      .strip_suffix("`\n")
      .unwrap()
      .split(':')
      .last()
      .unwrap()
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
fn server_listens_on_all_ip_addresses_http() {
  let tempdir = tempfile::tempdir().unwrap();
  let agora = AgoraInstance::new(tempdir, vec!["--http-port=0"]);
  let port = agora.port;
  assert_eq!(
    reqwest::blocking::get(agora.base_url()).unwrap().status(),
    StatusCode::OK
  );
  let stderr = agora.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTP connections on `[::1]:{}`",
    port
  )));
}

#[test]
fn server_listens_on_all_ip_addresses_https() {
  let tempdir = tempfile::tempdir().unwrap();
  let agora = AgoraInstance::new(
    tempdir,
    vec![
      "--https-port=0",
      "--acme-cache-directory=cache",
      "--acme-domain=foo",
    ],
  );
  let port = agora.port;
  let stderr = agora.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTPS connections on `[::1]:{}`",
    port
  )));
}

#[test]
fn errors_contain_backtraces() {
  let tempdir = tempfile::tempdir().unwrap();
  fs::write(tempdir.path().join(".hidden"), "").unwrap();
  let agora = AgoraInstance::new(tempdir, vec!["--http-port=0"]);
  let status = reqwest::blocking::get(agora.base_url().join("/files/.hidden").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::NOT_FOUND);
  let stderr = agora.kill();
  assert!(stderr.contains("agora::files::Files::check_path"));
}

struct TestContext {
  base_url: reqwest::Url,
}

impl TestContext {
  pub(crate) fn base_url(&self) -> &reqwest::Url {
    &self.base_url
  }
}

fn test<Function, F>(f: Function)
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
{
  let tempdir = tempfile::tempdir().unwrap();

  let agora = AgoraInstance::new(tempdir, vec!["--http-port=0"]);

  f(TestContext {
    base_url: agora.base_url().clone(),
  });
}

#[test]
fn index_route_status_code_is_200() {
  test(|context| async move {
    assert_eq!(
      reqwest::get(context.base_url().clone())
        .await
        .unwrap()
        .status(),
      200
    )
  });
}
