use http::uri::{Authority, Scheme, Uri};
use hyper::client::connect::HttpConnector;
use hyper_openssl::HttpsConnector;
use openssl::ssl::{SslConnector, SslMethod};
use openssl::x509::X509;
use std::task::{Context, Poll};
use tonic::body::BoxBody;

pub(crate) struct GrpcService {
  base_uri_scheme: Scheme,
  base_uri_authority: Authority,
  hyper_client: hyper::Client<HttpsConnector<HttpConnector>, BoxBody>,
}

impl GrpcService {
  pub(crate) fn new(
    authority: Authority,
    certificate: X509,
  ) -> Result<GrpcService, openssl::error::ErrorStack> {
    let mut ssl_connector = SslConnector::builder(SslMethod::tls_client())?;
    ssl_connector.cert_store_mut().add_cert(certificate)?;

    let mut http_connector = HttpConnector::new();
    http_connector.enforce_http(false);

    let hyper_client =
      hyper::Client::builder()
        .http2_only(true)
        .build(HttpsConnector::with_connector(
          http_connector,
          ssl_connector,
        )?);
    Ok(GrpcService {
      base_uri_scheme: Scheme::HTTPS,
      base_uri_authority: authority,
      hyper_client,
    })
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
    let mut builder = Uri::builder()
      .scheme(self.base_uri_scheme.clone())
      .authority(self.base_uri_authority.clone());
    if let Some(path_and_query) = req.uri().path_and_query() {
      builder = builder.path_and_query(path_and_query.clone());
    }
    *req.uri_mut() = builder
      .build()
      .expect("GrpcService::call: Uri constructed from valid parts cannot fail");
    self.hyper_client.request(req)
  }
}
