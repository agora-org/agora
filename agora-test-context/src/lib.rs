use {
  executable_path::executable_path,
  reqwest::{blocking::Response, header, redirect::Policy, StatusCode, Url},
  scraper::Html,
  std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, Stdio},
  },
  tempfile::TempDir,
};

pub struct AgoraTestContext {
  base_url: Url,
  child: Child,
  collected_stderr: String,
  files_directory: PathBuf,
  files_url: Url,
  port: u16,
  stderr: ChildStderr,
  tempdir: TempDir,
}

impl AgoraTestContext {
  pub fn base_url(&self) -> &Url {
    &self.base_url
  }

  pub fn builder() -> Builder {
    Builder::new()
  }

  pub fn create_dir_all(&self, path: &str) {
    std::fs::create_dir_all(self.files_directory().join(path)).unwrap();
  }

  pub fn files_directory(&self) -> &Path {
    &self.files_directory
  }

  pub fn current_dir(&self) -> &Path {
    self.tempdir.path()
  }

  pub fn files_url(&self) -> &Url {
    &self.files_url
  }

  pub fn get(&self, url: impl AsRef<str>) -> Response {
    let response = self.response(url);
    assert_eq!(response.status(), StatusCode::OK);
    response
  }

  pub fn html(&self, url: impl AsRef<str>) -> Html {
    Html::parse_document(&self.text(url))
  }

  pub fn kill(mut self) -> String {
    self.child.kill().unwrap();
    self.child.wait().unwrap();
    self
      .stderr
      .read_to_string(&mut self.collected_stderr)
      .unwrap();
    self.collected_stderr
  }

  pub fn port(&self) -> u16 {
    self.port
  }

  pub fn redirect_url(&self, url: &str) -> Url {
    let client = reqwest::blocking::Client::builder()
      .redirect(Policy::none())
      .build()
      .unwrap();
    let request = client
      .get(self.base_url().join(url).unwrap())
      .build()
      .unwrap();
    let response = client.execute(request).unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    self
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

  pub fn response(&self, url: impl AsRef<str>) -> Response {
    reqwest::blocking::get(self.base_url.join(url.as_ref()).unwrap()).unwrap()
  }

  pub fn status(&self, url: impl AsRef<str>) -> StatusCode {
    self.response(url).status()
  }

  pub fn text(&self, url: impl AsRef<str>) -> String {
    self.get(url).text().unwrap()
  }

  pub fn write(&self, path: &str, content: &str) -> PathBuf {
    let path = self.files_directory().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
  }
}

pub struct Builder {
  address: Option<String>,
  args: Vec<String>,
  backtraces: bool,
  current_dir: Option<String>,
  files_directory: String,
  http_port: Option<u16>,
  tempdir: TempDir,
}

impl Builder {
  pub fn address(self, address: Option<&str>) -> Self {
    Self {
      address: address.map(str::to_owned),
      ..self
    }
  }

  pub fn args(self, args: &[&str]) -> Self {
    Self {
      args: self
        .args
        .into_iter()
        .chain(args.iter().cloned().map(str::to_owned))
        .collect(),
      ..self
    }
  }

  pub fn backtraces(self, backtraces: bool) -> Self {
    Self { backtraces, ..self }
  }

  pub fn build(self) -> AgoraTestContext {
    let mut command = Command::new(executable_path("agora"));

    let current_dir = if let Some(current_dir) = self.current_dir {
      self.tempdir.path().join(current_dir)
    } else {
      self.tempdir.path().to_owned()
    };

    let files_directory = current_dir.join(&self.files_directory);

    fs::create_dir_all(&files_directory).unwrap();

    if let Some(address) = self.address {
      command.arg("--address");
      command.arg(address);
    }

    if let Some(http_port) = self.http_port {
      command.arg("--http-port");
      command.arg(&http_port.to_string());
    }

    command
      .args(self.args)
      .arg("--directory")
      .arg(dbg!(self.files_directory))
      .current_dir(dbg!(current_dir))
      .stderr(Stdio::piped());

    if !self.backtraces {
      command.env("AGORA_SUPPRESS_BACKTRACE", "");
    }

    let mut child = dbg!(command).spawn().unwrap();

    let mut first_line = String::new();
    let child_stderr = child.stderr.take().unwrap();
    let mut child_stderr = BufReader::new(child_stderr);
    child_stderr.read_line(&mut first_line).unwrap();
    eprintln!("First line: {}", first_line);
    let port_string = first_line
      .trim()
      .trim_end_matches('`')
      .split(':')
      .last()
      .unwrap_or_else(|| {
        panic!(
          "first line to stderr does not contain `:` and port: {}",
          first_line
        )
      });
    let port: u16 = port_string
      .parse()
      .unwrap_or_else(|_| panic!("port should be an integer: {}", port_string));

    let base_url = Url::parse(&format!("http://localhost:{}", port)).unwrap();

    let files_url = base_url.join("files/").unwrap();

    AgoraTestContext {
      base_url,
      child,
      collected_stderr: first_line,
      files_directory,
      files_url,
      port,
      stderr: child_stderr.into_inner(),
      tempdir: self.tempdir,
    }
  }

  pub fn files_directory(self, files_directory: &str) -> Self {
    Self {
      files_directory: files_directory.to_owned(),
      ..self
    }
  }

  pub fn http_port(self, http_port: Option<u16>) -> Self {
    Self { http_port, ..self }
  }

  fn new() -> Self {
    Self {
      address: Some("localhost".to_owned()),
      args: Vec::new(),
      backtraces: false,
      files_directory: "files".to_owned(),
      http_port: Some(0),
      tempdir: tempfile::tempdir().unwrap(),
      current_dir: None,
    }
  }

  pub fn current_dir(self, current_dir: &str) -> Self {
    Self {
      current_dir: Some(current_dir.to_owned()),
      ..self
    }
  }

  pub fn write(self, path: &str, content: &str) -> Self {
    let path = self.tempdir.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    self
  }
}
