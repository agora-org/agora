use executable_path::executable_path;
use hyper::{header, StatusCode};
use reqwest::{redirect::Policy, Client, Url};
use std::{
  fs,
  future::Future,
  io::{BufRead, BufReader, Read},
  path::PathBuf,
  process::{Child, ChildStderr, Command, Stdio},
};
use tempfile::TempDir;

struct AgoraInstance {
  tempdir: TempDir,
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
      .trim()
      .trim_end_matches('`')
      .split(':')
      .last()
      .unwrap()
      .parse()
      .unwrap();

    AgoraInstance {
      child,
      collected_stderr: first_line,
      port,
      stderr: child_stderr.into_inner(),
      tempdir,
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
    "Listening for HTTP connections on `0.0.0.0:{}`",
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
    "Listening for HTTPS connections on `0.0.0.0:{}`",
    port
  )));
}

#[test]
fn errors_contain_backtraces() {
  let tempdir = tempfile::tempdir().unwrap();
  fs::write(tempdir.path().join(".hidden"), "").unwrap();
  let agora = AgoraInstance::new(tempdir, vec!["--address=localhost", "--http-port=0"]);
  let status = reqwest::blocking::get(agora.base_url().join("/files/.hidden").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::NOT_FOUND);
  let stderr = agora.kill();
  assert!(stderr.contains("agora::files::Files::check_path"));
}

struct TestContext {
  base_url: reqwest::Url,
  files_url: reqwest::Url,
  files_directory: PathBuf,
}

impl TestContext {
  pub(crate) fn base_url(&self) -> &reqwest::Url {
    &self.base_url
  }

  pub(crate) fn files_url(&self) -> &reqwest::Url {
    &self.files_url
  }

  pub(crate) fn files_directory(&self) -> &std::path::Path {
    &self.files_directory
  }
}

fn test<Function, F>(f: Function)
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
{
  let tempdir = tempfile::tempdir().unwrap();

  let agora = AgoraInstance::new(tempdir, vec!["--address=localhost", "--http-port=0"]);

  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      f(TestContext {
        base_url: agora.base_url().clone(),
        files_url: agora.base_url().join("files/").unwrap(),
        files_directory: agora.tempdir.path().to_owned(),
      })
      .await;
    });
}

async fn redirect_url(context: &TestContext, url: &Url) -> Url {
  let client = Client::builder().redirect(Policy::none()).build().unwrap();
  let request = client.get(url.clone()).build().unwrap();
  let response = client.execute(request).await.unwrap();
  assert_eq!(response.status(), StatusCode::FOUND);
  context
    .base_url()
    .join(
      response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap(),
    )
    .unwrap()
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

#[test]
fn index_route_redirects_to_files() {
  test(|context| async move {
    let redirect_url = redirect_url(&context, context.base_url()).await;
    assert_eq!(&redirect_url, context.files_url());
  });
}

#[test]
fn no_trailing_slash_redirects_to_trailing_slash() {
  test(|context| async move {
    fs::create_dir(context.files_directory().join("foo")).unwrap();
    let redirect_url = redirect_url(&context, &context.files_url().join("foo").unwrap()).await;
    assert_eq!(redirect_url, context.files_url().join("foo/").unwrap());
  });
}

#[test]
fn files_route_without_trailing_slash_redirects_to_files() {
  test(|context| async move {
    let redirect_url = redirect_url(&context, &context.base_url().join("files").unwrap()).await;
    assert_eq!(&redirect_url, context.files_url());
  });
}
