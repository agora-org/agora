use hyper::client::connect::HttpConnector;
use hyper::http::Uri;
use hyper_openssl::HttpsConnector;
use openssl::ssl::{SslConnector, SslMethod};
use openssl::x509::X509;
use std::task::{Context, Poll};
use tonic::body::BoxBody;

pub(crate) struct GrpcService {
  base_uri: Uri,
  hyper_client: hyper::Client<HttpsConnector<HttpConnector>, BoxBody>,
}

impl GrpcService {
  pub(crate) fn new(base_uri: Uri, certificate: X509) -> GrpcService {
    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.cert_store_mut().add_cert(certificate).unwrap();
    connector.set_alpn_protos(b"\x02h2").unwrap();

    let mut http = HttpConnector::new();
    http.enforce_http(false);

    let https = HttpsConnector::with_connector(http, connector).unwrap();
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
