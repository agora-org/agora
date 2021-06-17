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
  use httpmock::{Method::GET, MockServer, Then, When};
  use pretty_assertions::assert_eq;
  use reqwest::StatusCode;

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
}
