use crate::grpc_service::GrpcService;
use http::uri::Authority;
#[cfg(test)]
use lnd_test_context::LndTestContext;
use lnrpc::lightning_client::LightningClient;
use lnrpc::{AddInvoiceResponse, Invoice, ListInvoiceRequest};
use openssl::x509::X509;
#[cfg(test)]
use std::sync::Arc;
use tonic::{metadata::AsciiMetadataValue, Status};

mod grpc_service;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug, Clone)]
pub struct Client {
  client: LightningClient<GrpcService>,
  macaroon: Option<AsciiMetadataValue>,
  #[cfg(test)]
  lnd_test_context: Arc<LndTestContext>,
}

impl Client {
  pub async fn new(
    authority: Authority,
    certificate: Option<X509>,
    macaroon: Option<Vec<u8>>,
    #[cfg(test)] lnd_test_context: LndTestContext,
  ) -> Result<Client, openssl::error::ErrorStack> {
    Ok(Client {
      client: LightningClient::new(GrpcService::new(authority, certificate)?),
      macaroon: macaroon.map(|macaroon| {
        hex::encode_upper(macaroon)
          .parse()
          .expect("Client::new: hex characters are valid metadata values")
      }),
      #[cfg(test)]
      lnd_test_context: Arc::new(lnd_test_context),
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

  pub async fn add_invoice(&mut self) -> Result<AddInvoiceResponse, Status> {
    let mut request = tonic::Request::new(Invoice {
      ..Invoice::default()
    });
    // fixme: dry up
    if let Some(macaroon) = &self.macaroon {
      request.metadata_mut().insert("macaroon", macaroon.clone());
    }
    Ok(self.client.add_invoice(request).await?.into_inner())
  }

  async fn get_invoice(&mut self, invoice_index: u64) -> Result<Option<Invoice>, Status> {
    let mut request = tonic::Request::new(ListInvoiceRequest {
      index_offset: invoice_index - 1,
      num_max_invoices: 1,
      pending_only: false,
      reversed: false,
    });
    if let Some(macaroon) = &self.macaroon {
      request.metadata_mut().insert("macaroon", macaroon.clone());
    }
    let response = self.client.list_invoices(request).await?.into_inner();
    match response.invoices.as_slice() {
      [x] => Ok(Some(x.clone())),
      [] => Ok(None),
      _ => Err(Status::internal(
        "lnd-client::Client::get_invoice: LND returned more results than specified in num_max_invoices",
      )),
    }
  }

  #[cfg(test)]
  async fn with_cert(lnd_test_context: LndTestContext, cert: &str) -> Self {
    Self::new(
      format!("localhost:{}", lnd_test_context.lnd_rpc_port)
        .parse()
        .unwrap(),
      Some(X509::from_pem(cert.as_bytes()).unwrap()),
      Some(
        tokio::fs::read(lnd_test_context.invoice_macaroon_path())
          .await
          .unwrap(),
      ),
      lnd_test_context,
    )
    .await
    .unwrap()
  }

  #[cfg(test)]
  async fn with_test_context(context: LndTestContext) -> Self {
    let cert = std::fs::read_to_string(context.cert_path()).unwrap();
    Self::with_cert(context, &cert).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn ping() {
    Client::with_test_context(LndTestContext::new().await)
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
    let error = Client::with_cert(LndTestContext::new().await, INVALID_TEST_CERT)
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

  #[tokio::test]
  async fn add_invoice() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let invoice = client.add_invoice().await.unwrap();
    assert_eq!(invoice.add_index, 1);
  }

  #[tokio::test]
  async fn get_invoice() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let _ignored = client.add_invoice().await.unwrap();
    let created = client.add_invoice().await.unwrap();
    let retrieved = client
      .get_invoice(created.add_index)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(
      (
        created.add_index,
        created.r_hash,
        created.payment_request,
        created.payment_addr
      ),
      (
        retrieved.add_index,
        retrieved.r_hash,
        retrieved.payment_request,
        retrieved.payment_addr
      )
    );
  }

  #[tokio::test]
  async fn get_invoice_not_found() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let retrieved = client.get_invoice(42).await.unwrap();
    assert_eq!(retrieved, None);
  }
}
