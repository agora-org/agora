use reqwest::{Certificate, Result};

#[cfg(test)]
mod owned_child;
#[cfg(test)]
mod test_context;

pub struct Client {
  client: reqwest::blocking::Client,
  base_url: String,
}

impl Client {
  pub fn new(certificate: &[u8], base_url: String) -> Result<Client> {
    let certificate = Certificate::from_pem(&certificate)?;
    let client = reqwest::blocking::Client::builder()
      .add_root_certificate(certificate)
      .build()?;
    Ok(Client { client, base_url })
  }

  pub fn state(&self) -> Result<String> {
    let response = self.client.execute(
      self
        .client
        .get(format!("{}/v1/state", self.base_url))
        .build()?,
    )?;
    Ok(response.text()?)
  }
}

#[cfg(test)]
mod tests {
  use crate::test_context::TestContext;
  use pretty_assertions::assert_eq;

  #[test]
  fn state() {
    assert_eq!(
      TestContext::new().client().state().unwrap(),
      r#"{"state":"RPC_ACTIVE"}"#
    );
  }
}
