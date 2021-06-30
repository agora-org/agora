use hyper::http::Uri;
use lnrpc::lightning_client::LightningClient;
use lnrpc::{GetInfoRequest, GetInfoResponse};
use std::task::{Context, Poll};
use tonic::body::BoxBody;
use tonic::Status;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug)]
pub struct Client {
  client: LightningClient<GrpcService>,
}

struct GrpcService {
  base_uri: Uri,
  hyper_client:
    hyper::Client<hyper_openssl::HttpsConnector<hyper::client::connect::HttpConnector>, BoxBody>,
}

impl GrpcService {
  fn new(base_uri: Uri, certificate: &str) -> GrpcService {
    let mut connector =
      openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls()).unwrap();
    let ca = openssl::x509::X509::from_pem(certificate.as_bytes()).unwrap();
    connector.cert_store_mut().add_cert(ca).unwrap();
    const ALPN_H2_WIRE: &[u8] = b"\x02h2";
    connector.set_alpn_protos(ALPN_H2_WIRE).unwrap();

    let mut http = hyper::client::connect::HttpConnector::new();
    http.enforce_http(false);

    let https = hyper_openssl::HttpsConnector::with_connector(http, connector).unwrap();
    let hyper_client = hyper::Client::builder().http2_only(true).build(https);
    GrpcService {
      base_uri,
      hyper_client,
    }
  }
}

impl tonic::client::GrpcService<BoxBody> for GrpcService {
  type ResponseBody = hyper::Body;
  type Error = hyper::Error;
  type Future = hyper::client::ResponseFuture;

  fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, mut req: hyper::Request<BoxBody>) -> Self::Future {
    let uri = Uri::builder()
      .scheme(self.base_uri.scheme().unwrap().clone())
      .authority(self.base_uri.authority().unwrap().clone())
      .path_and_query(req.uri().path_and_query().unwrap().clone())
      .build()
      .unwrap();
    *req.uri_mut() = uri;
    self.hyper_client.request(req)
  }
}

impl Client {
  pub async fn new(base_uri: Uri, certificate: &str) -> Result<Client, tonic::transport::Error> {
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
