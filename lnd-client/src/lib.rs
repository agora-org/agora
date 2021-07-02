use crate::grpc_service::GrpcService;
use http::uri::Authority;
use lnrpc::lightning_client::LightningClient;
use lnrpc::ListInvoiceRequest;
use openssl::x509::X509;
use tonic::{metadata::AsciiMetadataValue, Status};

mod grpc_service;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug)]
pub struct Client {
  client: LightningClient<GrpcService>,
  macaroon: Option<AsciiMetadataValue>,
}

impl Client {
  pub async fn new(
    authority: Authority,
    certificate: Option<X509>,
    macaroon: Option<Vec<u8>>,
  ) -> Result<Client, openssl::error::ErrorStack> {
    Ok(Client {
      client: LightningClient::new(GrpcService::new(authority, certificate)?),
      macaroon: macaroon.map(|macaroon| {
        hex::encode_upper(macaroon)
          .parse()
          .expect("Client::new: hex characters are valid metadata values")
      }),
    })
  }

  pub async fn ping(&mut self) -> Result<(), Status> {
    let mut request = tonic::Request::new(ListInvoiceRequest {
      index_offset: 0,
      num_max_invoices: 0,
      pending_only: false,
      reversed: false,
    });

    if let Some(macaroon) = &self.macaroon {
      request.metadata_mut().insert("macaroon", macaroon.clone());
    }

    self.client.list_invoices(request).await?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use lnd_test_context::LndTestContext;

  #[tokio::test]
  async fn ping() {
    LndTestContext::new()
      .await
      .client()
      .await
      .ping()
      .await
      .unwrap();
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
    let error = LndTestContext::new()
      .await
      .client_with_cert(INVALID_TEST_CERT)
      .await
      .ping()
      .await
      .unwrap_err();
    #[track_caller]
    fn assert_contains(input: &str, expected: &str) {
      assert!(
        input.contains(expected),
        "assert_contains:\n{}\ndidn't contain\n{}",
        input,
        expected
      );
    }
    assert_contains(&error.to_string(), "error trying to connect: ");
    assert_contains(&error.to_string(), "certificate verify failed");
    assert_contains(&error.to_string(), "self signed certificate");
  }
}
