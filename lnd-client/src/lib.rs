use lnrpc::state_client::StateClient;
use lnrpc::{GetStateRequest, GetStateResponse};
use rustls::ClientConfig;
use std::sync::Arc;
use tonic::transport::channel::Channel;
use tonic::transport::Certificate;
use tonic::transport::ClientTlsConfig;
use tonic::Response;

#[cfg(test)]
mod owned_child;
#[cfg(test)]
mod test_context;

pub struct Client {
  client: StateClient<Channel>,
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

    let channel = Channel::builder(format!("https://localhost:{}", rpc_port).parse().unwrap())
    // we might be able to use openssl for tonic
        .tls_config(ClientTlsConfig::new().rustls_client_config(config))?
        // .timeout(Duration::from_secs(5))
        // .rate_limit(5, Duration::from_secs(1))
        // .concurrency_limit(256)
        .connect()
        .await?;
    let client = StateClient::new(channel);
    Ok(Client { client })
  }

  pub async fn state(&mut self) -> Result<String, Box<dyn std::error::Error>> {
    let foo: GetStateResponse = self
      .client
      .get_state(GetStateRequest {})
      .await?
      .into_inner();
    Ok(format!("{:?}", foo))
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
  async fn state() {
    assert_eq!(
      TestContext::new().client().await.state().await.unwrap(),
      r#"GetStateResponse { state: RpcActive }"#
    );
  }
}

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}
