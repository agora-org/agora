use lnrpc::lightning_client::LightningClient;
use lnrpc::{GetInfoRequest, GetInfoResponse};
use rustls::internal::pemfile;
use rustls::Certificate;
use rustls::ClientConfig;
use std::io::Cursor;
use std::sync::Arc;
use tonic::transport::channel::Channel;
use tonic::transport::ClientTlsConfig;
use tonic::Status;

#[cfg(test)]
mod owned_child;
#[cfg(test)]
mod test_context;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug)]
pub struct Client {
  client: LightningClient<Channel>,
}

impl Client {
  pub async fn new(certificate: &str, rpc_port: u16) -> Result<Client, tonic::transport::Error> {
    let mut certificates = pemfile::certs(&mut Cursor::new(certificate)).unwrap();
    assert_eq!(certificates.len(), 1);
    let certificate = certificates.pop().unwrap();

    const ALPN_H2: &str = "h2";

    let mut config = ClientConfig::new();
    config.set_protocols(&[Vec::from(&ALPN_H2[..])]);
    config
      .dangerous()
      .set_certificate_verifier(Arc::new(SingleCertVerifier::new(certificate)));

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

struct SingleCertVerifier {
  certificate: Certificate,
}

impl SingleCertVerifier {
  fn new(certificate: Certificate) -> SingleCertVerifier {
    SingleCertVerifier { certificate }
  }
}

impl rustls::ServerCertVerifier for SingleCertVerifier {
  fn verify_server_cert(
    &self,
    _: &rustls::RootCertStore,
    certificates: &[rustls::Certificate],
    _: webpki::DNSNameRef,
    _: &[u8],
  ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
    match certificates {
      [end_entity_cert] => {
        if end_entity_cert == &self.certificate {
          Ok(rustls::ServerCertVerified::assertion())
        } else {
          Err(rustls::TLSError::General(
            "unexpected certificate presented".to_owned(),
          ))
        }
      }
      [] => Err(rustls::TLSError::NoCertificatesPresented),
      [..] => Err(rustls::TLSError::General(
        "more than one certificate presented".to_owned(),
      )),
    }
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

  #[tokio::test]
  async fn fails_on_wrong_lnd_certificate() {
    const INVALID_TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
MIICTDCCAfGgAwIBAgIQdJJBvsv1/V23RMoX9fOOuTAKBggqhkjOPQQDAjAwMR8w
HQYDVQQKExZsbmQgYXV0b2dlbmVyYXRlZCBjZXJ0MQ0wCwYDVQQDEwRwcmFnMB4X
DTIxMDYyNzIxMTg1NloXDTIyMDgyMjIxMTg1NlowMDEfMB0GA1UEChMWbG5kIGF1
dG9nZW5lcmF0ZWQgY2VydDENMAsGA1UEAxMEcHJhZzBZMBMGByqGSM49AgEGCCqG
SM49AwEHA0IABL4lYBbOPVAtglBKPV3LwB7eC1j/Y6Nt0O23M1dSrcLdrNHUP87n
5clDvrur4EaJTmnZHI2141usNs/pljzMHmqjgewwgekwDgYDVR0PAQH/BAQDAgKk
MBMGA1UdJQQMMAoGCCsGAQUFBwMBMA8GA1UdEwEB/wQFMAMBAf8wHQYDVR0OBBYE
FIQ2zY1Z6g9NRGbMtXbSZEesaIqhMIGRBgNVHREEgYkwgYaCBHByYWeCCWxvY2Fs
aG9zdIIEdW5peIIKdW5peHBhY2tldIIHYnVmY29ubocEfwAAAYcQAAAAAAAAAAAA
AAAAAAAAAYcEwKgBDocErBEAAYcErBIAAYcErBMAAYcEwKgBC4cQ/oAAAAAAAAA2
6QIJT4EyIocQ/oAAAAAAAABD0/8gsXGsVzAKBggqhkjOPQQDAgNJADBGAiEA3lrs
qmJp1luuw/ElVG3DdHtz4Lx8iK8EanRdHA3T+78CIQDfuWGMe0IGtwLuDpDixvGy
jlZBq5hr8Nv2qStFfw9qzw==
-----END CERTIFICATE-----
";
    let error = TestContext::new()
      .client_with_cert(INVALID_TEST_CERT)
      .await
      .unwrap_err();
    let expected = "unexpected certificate presented";
    assert!(
      error.to_string().contains(expected),
      "{}\ndidn't contain\n{}",
      error,
      expected
    );
  }
}
