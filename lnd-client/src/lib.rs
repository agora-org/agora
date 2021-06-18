use reqwest::{Certificate, Result};
use std::{
  fs::File,
  io::{self, Read},
  path::Path,
};

pub struct Client {
  client: reqwest::blocking::Client,
  base_url: String,
}

impl Client {
  pub fn new(pem_file: &Path, base_url: String) -> io::Result<Result<Client>> {
    let certificate = {
      let mut buf = Vec::new();
      File::open(pem_file)?.read_to_end(&mut buf)?;
      buf
    };
    Ok(Certificate::from_pem(&certificate).and_then(|certificate| {
      reqwest::blocking::Client::builder()
        .add_root_certificate(certificate)
        .build()
        .map(|client| Client { client, base_url })
    }))
  }

  pub fn state(&self) -> Result<String> {
    let response = self.client.execute(
      self
        .client
        .get(format!("{}/v1/state", self.base_url))
        .build()?,
    )?;
    response.error_for_status_ref()?;
    Ok(response.text()?)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use cradle::*;
  use httpmock::{Method::GET, MockServer, Then, When};
  use pretty_assertions::assert_eq;
  use reqwest::StatusCode;
  use std::path::PathBuf;

  fn test<Setup, Test>(setup: Setup, test: Test)
  where
    Setup: FnOnce(When, Then),
    Test: FnOnce(Client),
  {
    let server = MockServer::start();
    server.mock(setup);
    let client = Client::new(Path::new("tests/test-cert.pem"), server.base_url())
      .unwrap()
      .unwrap();
    test(client);
  }

  #[test]
  fn state() {
    test(
      |when, then| {
        when.method(GET).path("/v1/state");
        then.status(200).body("test-state");
      },
      |client| assert_eq!(client.state().unwrap(), "test-state"),
    );
  }

  #[test]
  fn returns_error_when_response_status_code_is_not_2xx() {
    test(
      |when, then| {
        when.any_request();
        then.status(404);
      },
      |client| {
        assert_eq!(
          client.state().unwrap_err().status(),
          Some(StatusCode::NOT_FOUND)
        )
      },
    );
  }

  fn bitcoind_tarball(target_dir: &Path) -> PathBuf {
    let tarball_path = target_dir.join("bitcoin-0.21.1-x86_64-linux-gnu.tar.gz");
    if !tarball_path.exists() {
      let mut response = reqwest::blocking::get(
        "https://bitcoin.org/bin/bitcoin-core-0.21.1/bitcoin-0.21.1-x86_64-linux-gnu.tar.gz",
      )
      .unwrap();
      let mut tarball_file = std::fs::File::create(&tarball_path).unwrap();
      std::io::copy(&mut response, &mut tarball_file).unwrap();
    }
    tarball_path
  }

  fn bitcoind_executable() -> PathBuf {
    let target_dir = Path::new("../target");
    let binary = target_dir.join("bitcoind");
    if !binary.exists() {
      let tarball_path = bitcoind_tarball(target_dir);
      cmd_unit!(
        Stdin(
          format!(
            "366eb44a7a0aa5bd342deea215ec19a184a11f2ca22220304ebb20b9c8917e2b {}",
            tarball_path.to_str().unwrap()
          ).as_str()
        ),
        %"sha256sum -c -"
      );
      cmd_unit!(
        %"tar -xzvf",
        tarball_path.to_str().unwrap(),
        "-C", target_dir.to_str().unwrap(),
        %"--strip-components=2 bitcoin-0.21.1/bin/bitcoin-cli bitcoin-0.21.1/bin/bitcoind"
      );
    }
    binary
  }

  #[test]
  fn installs_bitcoind_test_executable() {
    let StdoutTrimmed(version) = cmd!(bitcoind_executable().to_str().unwrap(), "--version");
    assert!(version.contains("v0.21.1"));
  }
}
