use ::{
  executable_path::executable_path,
  reqwest::{StatusCode, Url},
  std::{
    fs,
    io::{BufRead, BufReader, Read},
    process::{Child, ChildStderr, Command, Stdio},
  },
  tempfile::TempDir,
};

pub async fn get(url: &Url) -> reqwest::Response {
  let response = reqwest::get(url.clone()).await.unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  response
}

pub struct AgoraInstance {
  pub tempdir: TempDir,
  child: Child,
  pub port: u16,
  stderr: ChildStderr,
  collected_stderr: String,
}

impl AgoraInstance {
  pub fn new(tempdir: TempDir, additional_flags: Vec<&str>, print_backtraces: bool) -> Self {
    let mut command = Command::new(executable_path("agora"));

    fs::create_dir(tempdir.path().join("www")).unwrap();

    command
      .args(additional_flags)
      .arg("--directory=www")
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

  pub fn base_url(&self) -> Url {
    Url::parse(&format!("http://localhost:{}", self.port)).unwrap()
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
}
