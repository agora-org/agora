use reqwest::{Certificate, Result};
use std::{
  fs::File,
  io::{self, Read},
  path::Path,
};

#[cfg(test)]
mod owned_child;
#[cfg(test)]
mod test_context;

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
  use crate::test_context::TestContext;
  use cradle::*;
  use pretty_assertions::assert_eq;
  use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command},
    sync::Once,
  };
  use tempfile::TempDir;

  #[test]
  fn state() {
    assert_eq!(
      TestContext::new().client().state().unwrap(),
      r#"{"state":"RPC_ACTIVE"}"#
    );
  }

  #[test]
  #[ignore]
  fn returns_error_when_response_status_code_is_not_2xx() {}
}
