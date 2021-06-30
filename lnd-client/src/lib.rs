use crate::single_cert_verifier::SingleCertVerifier;
use lnrpc::lightning_client::LightningClient;
use lnrpc::{GetInfoRequest, GetInfoResponse};
use rustls::internal::pemfile;
use rustls::ClientConfig;
use std::io::Cursor;
use std::sync::Arc;
use tonic::codegen::http::Uri;
use tonic::transport::channel::Channel;
use tonic::transport::ClientTlsConfig;
use tonic::Status;

mod single_cert_verifier;

pub mod lnrpc {
  tonic::include_proto!("lnrpc");
}

#[derive(Debug)]
pub struct Client {
  client: LightningClient<MyBox>,
}

struct Foo;

type MyBox = Box<
  dyn tower::Service<
    hyper::Request<tonic::body::BoxBody>,
    Response = hyper::Response<hyper::Body>,
    Error = hyper::Error,
    Future = hyper::client::ResponseFuture,
  >,
>;

impl Client {
  pub async fn new(url: &Uri, certificate: &str) -> Result<Client, tonic::transport::Error> {
    // let mut certificates = pemfile::certs(&mut Cursor::new(certificate)).unwrap();
    // assert_eq!(certificates.len(), 1);
    // let certificate = certificates.pop().unwrap();

    const ALPN_H2: &str = "h2";

    let mut config = ClientConfig::new();
    config.set_protocols(&[Vec::from(&ALPN_H2[..])]);
    // config
    //   .dangerous()
    //   .set_certificate_verifier(Arc::new(SingleCertVerifier::new(certificate)));
    // let pem = tokio::fs::read("example/tls/ca.pem").await.unwrap();
    let ca = openssl::x509::X509::from_pem(certificate.as_bytes()).unwrap();
    let mut connector =
      openssl::ssl::SslConnector::builder(openssl::ssl::SslMethod::tls()).unwrap();
    connector.cert_store_mut().add_cert(ca).unwrap();
    const ALPN_H2_WIRE: &[u8] = b"\x02h2";
    connector.set_alpn_protos(ALPN_H2_WIRE).unwrap();

    let mut http = hyper::client::connect::HttpConnector::new();
    http.enforce_http(false);

    let mut https = hyper_openssl::HttpsConnector::with_connector(http, connector).unwrap();
    let hyper = hyper::Client::builder().http2_only(true).build(https);
    // let channel = Channel::builder(url.clone())
    //   .tls_config(ClientTlsConfig::new().rustls_client_config(config))?
    //   .connect()
    //   .await?;
    let url_clone = dbg!(url.clone());
    let service: MyBox = Box::new(tower::service_fn(
      move |mut req: hyper::Request<tonic::body::BoxBody>| -> hyper::client::ResponseFuture {
        let uri = Uri::builder()
          .scheme(url_clone.scheme().unwrap().clone())
          .authority(url_clone.authority().unwrap().clone())
          .path_and_query(req.uri().path_and_query().unwrap().clone())
          .build()
          .unwrap();
        *req.uri_mut() = uri;
        dbg!(&req);
        hyper.request(req)
      },
    ));
    let client = LightningClient::new(service);
    Ok(Client { client })
  }

  pub async fn get_info(&mut self) -> Result<GetInfoResponse, Status> {
    let foo = dbg!(self.client.get_info(GetInfoRequest {}).await);
    Ok(foo?.into_inner())
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
    let error = dbg!(client.get_info().await).unwrap_err();
    let expected = "error trying to connect: tcp connect error: Connection refused";
    assert!(
      error.to_string().contains(expected),
      "{}\ndidn't contain\n{}",
      error,
      expected
    );
  }
}
