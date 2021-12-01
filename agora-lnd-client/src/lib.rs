use {
  crate::https_service::HttpsService,
  http::uri::Authority,
  lnrpc::{
    lightning_client::LightningClient, AddInvoiceResponse, Invoice, ListInvoiceRequest, PaymentHash,
  },
  openssl::x509::X509,
  std::convert::TryInto,
  tonic::{
    metadata::AsciiMetadataValue,
    service::interceptor::{InterceptedService, Interceptor},
    Code, Request, Status,
  },
};

#[cfg(test)]
use {lnd_test_context::LndTestContext, std::sync::Arc};

pub use millisatoshi::Millisatoshi;

mod https_service;
mod millisatoshi;

pub mod lnrpc {
  use crate::millisatoshi::Millisatoshi;
  use std::convert::TryInto;

  tonic::include_proto!("lnrpc");

  impl Invoice {
    pub fn value_msat(&self) -> Millisatoshi {
      Millisatoshi::new(
        self
          .value_msat
          .try_into()
          .expect("value_msat is always positive"),
      )
    }
  }
}

#[derive(Clone)]
struct MacaroonInterceptor {
  macaroon: Option<AsciiMetadataValue>,
}

impl Interceptor for MacaroonInterceptor {
  fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
    if let Some(macaroon) = &self.macaroon {
      request.metadata_mut().insert("macaroon", macaroon.clone());
    }
    Ok(request)
  }
}

#[derive(Debug, Clone)]
pub struct Client {
  inner: LightningClient<InterceptedService<HttpsService, MacaroonInterceptor>>,
  #[cfg(test)]
  _lnd_test_context: Arc<LndTestContext>,
}

impl Client {
  pub async fn new(
    authority: Authority,
    certificate: Option<X509>,
    macaroon: Option<Vec<u8>>,
    #[cfg(test)] lnd_test_context: LndTestContext,
  ) -> Result<Client, openssl::error::ErrorStack> {
    let grpc_service = HttpsService::new(authority, certificate)?;

    let macaroon = macaroon.map(|macaroon| {
      hex::encode_upper(macaroon)
        .parse::<AsciiMetadataValue>()
        .expect("Client::new: hex characters are valid metadata values")
    });

    let inner = LightningClient::with_interceptor(grpc_service, MacaroonInterceptor { macaroon });

    Ok(Client {
      inner,
      #[cfg(test)]
      _lnd_test_context: Arc::new(lnd_test_context),
    })
  }

  pub async fn ping(&mut self) -> Result<(), Status> {
    let request = tonic::Request::new(ListInvoiceRequest {
      index_offset: 0,
      num_max_invoices: 0,
      pending_only: false,
      reversed: false,
    });

    self.inner.list_invoices(request).await?;

    Ok(())
  }

  pub async fn add_invoice(
    &mut self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<AddInvoiceResponse, Status> {
    let request = tonic::Request::new(Invoice {
      memo: memo.to_owned(),
      value_msat: value_msat.value().try_into().map_err(|source| {
        Status::new(
          Code::InvalidArgument,
          format!("invalid value for `value_msat`: {}", source),
        )
      })?,
      ..Invoice::default()
    });
    Ok(self.inner.add_invoice(request).await?.into_inner())
  }

  pub async fn lookup_invoice(&mut self, r_hash: [u8; 32]) -> Result<Option<Invoice>, Status> {
    let request = tonic::Request::new(PaymentHash {
      r_hash: r_hash.to_vec(),
      ..PaymentHash::default()
    });
    match self.inner.lookup_invoice(request).await {
      Ok(response) => Ok(Some(response.into_inner())),
      Err(status) => {
        if status.code() == Code::Unknown
          && (status.message() == "there are no existing invoices"
            || status.message() == "unable to locate invoice")
        {
          Ok(None)
        } else {
          Err(status)
        }
      }
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
  async fn with_test_context(lnd_test_context: LndTestContext) -> Self {
    let cert = std::fs::read_to_string(lnd_test_context.cert_path()).unwrap();
    Self::with_cert(lnd_test_context, &cert).await
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
    let response = client
      .add_invoice("", Millisatoshi::new(1_000))
      .await
      .unwrap();
    assert!(
      !response.payment_request.is_empty(),
      "Bad response: {:?}",
      response
    );
  }

  #[tokio::test]
  async fn add_invoice_memo_and_value() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let r_hash = client
      .add_invoice("test-memo", Millisatoshi::new(42_000))
      .await
      .unwrap()
      .r_hash;
    let invoice = client
      .lookup_invoice(r_hash.try_into().unwrap())
      .await
      .unwrap()
      .unwrap();
    assert_eq!(invoice.memo, "test-memo");
    assert_eq!(invoice.value, 42);
  }

  #[tokio::test]
  async fn lookup_invoice() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let _ignored1 = client
      .add_invoice("foo", Millisatoshi::new(1_000))
      .await
      .unwrap();
    let created = client
      .add_invoice("bar", Millisatoshi::new(2_000))
      .await
      .unwrap();
    let _ignored2 = client
      .add_invoice("baz", Millisatoshi::new(3_000))
      .await
      .unwrap();
    let retrieved = client
      .lookup_invoice(created.r_hash.as_slice().try_into().unwrap())
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
    assert_eq!(retrieved.memo, "bar");
    assert_eq!(retrieved.value, 2);
  }

  #[tokio::test]
  async fn lookup_invoice_not_found_no_invoices() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    assert_eq!(client.lookup_invoice([0; 32]).await.unwrap(), None);
  }

  #[tokio::test]
  async fn lookup_invoice_not_found_some_invoices() {
    let mut client = Client::with_test_context(LndTestContext::new().await).await;
    let _ignored1 = client
      .add_invoice("foo", Millisatoshi::new(1_000))
      .await
      .unwrap();
    assert_eq!(client.lookup_invoice([0; 32]).await.unwrap(), None);
  }
}
