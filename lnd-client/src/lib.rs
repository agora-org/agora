use lnrpc::lightning_client::LightningClient;
use lnrpc::{GetInfoRequest, GetInfoResponse};
use rustls::ClientConfig;
use std::sync::Arc;
use tonic::transport::channel::Channel;
use tonic::transport::Certificate;
use tonic::transport::ClientTlsConfig;
use tonic::Status;

#[cfg(test)]
mod owned_child;
#[cfg(test)]
mod test_context;

pub struct Client {
  client: LightningClient<Channel>,
}

impl Client {
  pub async fn new(
    certificate: &[u8],
    rpc_port: u16,
  ) -> Result<Client, Box<dyn std::error::Error>> {
    let certificate = Certificate::from_pem(&certificate);

    const ALPN_H2: &str = "h2";

    let mut config = ClientConfig::new();
    config.set_protocols(&[Vec::from(&ALPN_H2[..])]);
    config
      .dangerous()
      .set_certificate_verifier(Arc::new(NoCertVerifier {}));

    // we might be able to use openssl for tonic
    let channel = Channel::builder(format!("https://localhost:{}", rpc_port).parse().unwrap())
      .tls_config(ClientTlsConfig::new().rustls_client_config(config))?
      .connect()
      .await?;
    let client = LightningClient::new(channel);
    Ok(Client { client })
  }

  pub async fn get_info(&mut self) -> Result<GetInfoResponse, Status> {
    Ok(self.client.get_info(GetInfoRequest {}).await?.into_inner())
  }
}

struct NoCertVerifier {}

impl rustls::ServerCertVerifier for NoCertVerifier {
  fn verify_server_cert(
    &self,
    _: &rustls::RootCertStore,
    _: &[rustls::Certificate],
    _: webpki::DNSNameRef,
    _: &[u8],
  ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
    Ok(rustls::ServerCertVerified::assertion())
  }
}

#[cfg(test)]
mod tests {
  use crate::test_context::TestContext;
  use pretty_assertions::assert_eq;

  #[tokio::test]
  async fn info() {
    let response = TestContext::new().client().await.get_info().await.unwrap();
    assert_eq!(response.version, "0.13.0-beta commit=0.0.1-12-ge7e246d");
  }
}

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}
