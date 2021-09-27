use crate::{
  arguments::Arguments,
  error::{self, Result},
  https_request_handler::HttpsRequestHandler,
  redirect::redirect,
};
use http::uri::Authority;
use hyper::server::conn::AddrIncoming;
use hyper::{header, Body, Request, Response};
use snafu::ResultExt;
use std::{
  future,
  net::ToSocketAddrs,
  task::{Context, Poll},
};
use tower::make::Shared;
use tower::Service;

#[derive(Clone)]
pub(crate) struct HttpsRedirectService {
  https_port: u16,
}

impl HttpsRedirectService {
  pub(crate) fn new_server(
    arguments: &Arguments,
    tls_request_handler: &HttpsRequestHandler,
  ) -> Result<Option<hyper::Server<AddrIncoming, Shared<HttpsRedirectService>>>> {
    match arguments.https_redirect_port {
      Some(https_redirect_port) => {
        let socket_addr = (arguments.address.as_str(), https_redirect_port)
          .to_socket_addrs()
          .context(error::AddressResolutionIo {
            input: &arguments.address,
          })?
          .next()
          .ok_or_else(|| {
            error::AddressResolutionNoAddresses {
              input: arguments.address.clone(),
            }
            .build()
          })?;

        let server = hyper::Server::bind(&socket_addr).serve(Shared::new(HttpsRedirectService {
          https_port: tls_request_handler.https_port(),
        }));

        Ok(Some(server))
      }
      None => Ok(None),
    }
  }
}

impl Service<Request<Body>> for HttpsRedirectService {
  type Response = Response<Body>;
  type Error = http::Error;
  type Future = future::Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let authority = request.headers().get(header::HOST).unwrap();

    let authority = Authority::from_maybe_shared(authority.to_str().unwrap().to_string()).unwrap();

    future::ready(Ok(
      redirect(format!(
        "https://{}:{}{}",
        authority.host(),
        self.https_port,
        request.uri()
      ))
      .unwrap(),
    ))
  }
}
