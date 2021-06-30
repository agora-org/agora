use crate::grpc_service::GrpcService;
use hyper::http::Uri;
use lnrpc::lightning_client::LightningClient;
use lnrpc::{GetInfoRequest, GetInfoResponse};
use openssl::x509::X509;
use tonic::Status;

mod grpc_service;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug)]
pub struct Client {
  client: LightningClient<GrpcService>,
}

impl Client {
  pub async fn new(base_uri: Uri, certificate: &str) -> Result<Client, tonic::transport::Error> {
    let certificate = X509::from_pem(certificate.as_bytes()).unwrap();
    Ok(Client {
      client: LightningClient::new(GrpcService::new(base_uri, certificate)),
    })
  }

  pub async fn get_info(&mut self) -> Result<GetInfoResponse, Status> {
    Ok(self.client.get_info(GetInfoRequest {}).await?.into_inner())
  }
}

#[cfg(test)]
mod tests {
  use lnd_test_context::LndTestContext;

  #[tokio::test]
  async fn info() {
    let response = LndTestContext::new()
      .await
      .client()
      .await
      .get_info()
      .await
      .unwrap();
    assert!(
      response.version.starts_with("0.13.0-beta "),
      "Unexpected LND version: {}",
      response.version
    );
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
    let mut client = LndTestContext::new()
      .await
      .client_with_cert(INVALID_TEST_CERT)
      .await
      .unwrap();
    let error = client.get_info().await.unwrap_err();
    let expected = "error trying to connect: tcp connect error: Connection refused";
    assert!(
      error.to_string().contains(expected),
      "{}\ndidn't contain\n{}",
      error,
      expected
    );
  }
}
