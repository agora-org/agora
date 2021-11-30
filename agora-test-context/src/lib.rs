use ::{
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

pub struct Builder {
  backtraces: bool,
  tempdir: TempDir,
  directory: String,
  address: Option<String>,
}

impl Builder {
  fn new() -> Self {
    Self {
      backtraces: false,
      tempdir: tempfile::tempdir().unwrap(),
      directory: "files".to_owned(),
      address: Some("localhost".to_owned()),
    }
  }

  pub fn build(self) -> AgoraTestContext {
    AgoraTestContext::new(
      self.tempdir,
      vec!["--http-port=0"],
      self.backtraces,
      &self.directory,
      self.address,
    )
  }

  pub fn address(self, address: Option<&str>) -> Self {
    Self {
      address: address.map(str::to_owned),
      ..self
    }
  }

  pub fn directory(self, directory: &str) -> Self {
    Self {
      directory: directory.to_owned(),
      ..self
    }
  }

  pub fn backtraces(self, backtraces: bool) -> Self {
    Self { backtraces, ..self }
  }

  pub fn write(self, path: &str, content: &str) -> Self {
    let path = self.tempdir.path().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    self
  }
}

pub struct AgoraTestContext {
  _tempdir: TempDir,
  child: Child,
  port: u16,
  stderr: ChildStderr,
  collected_stderr: String,
  files_directory: PathBuf,
  base_url: Url,
  files_url: Url,
}

impl AgoraTestContext {
  pub fn builder() -> Builder {
    Builder::new()
  }

  pub fn new(
    tempdir: TempDir,
    additional_flags: Vec<&str>,
    print_backtraces: bool,
    directory: &str,
    address: Option<String>,
  ) -> Self {
    let mut command = Command::new(executable_path("agora"));

    let files_directory = tempdir.path().join(directory);

    fs::create_dir_all(&files_directory).unwrap();

    if let Some(address) = address {
      command.arg("--address");
      command.arg(address);
    }

    command
      .args(additional_flags)
      .arg("--directory")
      .arg(directory)
      .current_dir(&tempdir)
      .stderr(Stdio::piped());

    if !print_backtraces {
      command.env("AGORA_SUPPRESS_BACKTRACE", "");
    }

    let mut child = command.spawn().unwrap();

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
      .expect(&format!(
        "first line to stderr does not contain `:` and port: {}",
        first_line
      ));
    let port: u16 = port_string
      .parse()
      .expect(&format!("port should be an integer: {}", port_string));

    let base_url = Url::parse(&format!("http://localhost:{}", port)).unwrap();

    let files_url = base_url.join("files/").unwrap();

    Self {
      base_url,
      child,
      collected_stderr: first_line,
      files_directory,
      files_url,
      port,
      stderr: child_stderr.into_inner(),
      _tempdir: tempdir,
    }
  }

  pub fn port(&self) -> u16 {
    self.port
  }

  pub fn files_directory(&self) -> &Path {
    &self.files_directory
  }

  pub fn base_url(&self) -> &Url {
    &self.base_url
  }

  pub fn files_url(&self) -> &Url {
    &self.files_url
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

  pub fn write(&self, path: &str, content: &str) -> PathBuf {
    let path = self.files_directory().join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
  }

  pub fn create_dir_all(&self, path: &str) {
    std::fs::create_dir_all(self.files_directory().join(path)).unwrap();
  }

  pub fn status(&self, url: impl AsRef<str>) -> StatusCode {
    self.response(url).status()
  }

  pub fn response(&self, url: impl AsRef<str>) -> Response {
    reqwest::blocking::get(self.base_url.join(url.as_ref()).unwrap()).unwrap()
  }

  pub fn get(&self, url: impl AsRef<str>) -> Response {
    let response = self.response(url);
    assert_eq!(response.status(), StatusCode::OK);
    response
  }

  pub fn text(&self, url: impl AsRef<str>) -> String {
    self.get(url).text().unwrap()
  }

  pub fn html(&self, url: impl AsRef<str>) -> Html {
    Html::parse_document(&self.text(url))
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
}
