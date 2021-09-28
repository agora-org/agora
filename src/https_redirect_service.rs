use crate::{
  arguments::Arguments,
  environment::Environment,
  error::{self, Result},
  error_page,
  https_request_handler::HttpsRequestHandler,
  redirect::redirect,
  stderr::Stderr,
};
use http::uri::Authority;
use hyper::server::conn::AddrIncoming;
use hyper::{header, Body, Request, Response, StatusCode};
use snafu::ResultExt;
use std::{
  convert::Infallible,
  future,
  net::ToSocketAddrs,
  task::{Context, Poll},
};
use tower::make::Shared;
use tower::Service;

#[derive(Clone)]
pub(crate) struct HttpsRedirectService {
  https_port: u16,
  stderr: Stderr,
}

impl HttpsRedirectService {
  pub(crate) fn new_server(
    environment: &Environment,
    arguments: &Arguments,
    https_request_handler: &HttpsRequestHandler,
  ) -> Result<Option<hyper::Server<AddrIncoming, Shared<HttpsRedirectService>>>> {
    match arguments.https_redirect_port {
      None => Ok(None),
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

        Ok(Some(hyper::Server::bind(&socket_addr).serve(Shared::new(
          HttpsRedirectService {
            https_port: https_request_handler.https_port(),
            stderr: environment.stderr.clone(),
          },
        ))))
      }
    }
  }

  fn response(&mut self, request: Request<Body>) -> Result<Response<Body>> {
    let authority = request.headers().get(header::HOST).ok_or_else(|| {
      error::Custom {
        message: "Missing HOST header",
        status_code: StatusCode::BAD_REQUEST,
      }
      .build()
    })?;

    let authority =
      Authority::from_maybe_shared(authority.as_bytes().to_vec()).map_err(|error| {
        error::Custom {
          message: format!(
            "Invalid HOST header `{}`: {}",
            String::from_utf8_lossy(authority.as_bytes()),
            error
          ),
          status_code: StatusCode::BAD_REQUEST,
        }
        .build()
      })?;

    redirect(format!(
      "https://{}:{}{}",
      authority.host(),
      self.https_port,
      request.uri()
    ))
  }
}

impl Service<Request<Body>> for HttpsRedirectService {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = future::Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let result = self.response(request);
    future::ready(error_page::map_error(self.stderr.clone(), result))
  }
}
