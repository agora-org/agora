use futures::future::{self, FutureExt};
use http::{
  header::{HeaderName, HeaderValue},
  Request, Response,
};
use std::fmt::Debug;
use std::future::Future;
use std::task;
use tower::{Layer, Service};

pub struct LogHeaders;

impl<S> Layer<S> for LogHeaders {
  type Service = WithLogHeaders<S>;

  fn layer(&self, inner: S) -> Self::Service {
    WithLogHeaders { inner }
  }
}

#[derive(Clone, Debug)]
pub struct WithLogHeaders<Inner> {
  inner: Inner,
}

impl<Inner> WithLogHeaders<Inner> {
  fn log_headers<'a>(kind: &str, headers: impl Iterator<Item = (&'a HeaderName, &'a HeaderValue)>) {
    eprintln!("[{}-HEADERS]:", kind);
    for (key, value) in headers {
      eprintln!("  {}: {:?}", key, value);
    }
  }
}

impl<Inner, Body> Service<Request<Body>> for WithLogHeaders<Inner>
where
  Body: Debug,
  Inner: Service<Request<Body>, Response = Response<Body>>,
  Inner::Future: Future,
  Inner::Error: Debug,
  Inner::Response: Debug,
{
  type Response = Inner::Response;
  type Error = Inner::Error;
  type Future = future::Inspect<Inner::Future, fn(&<Inner::Future as Future>::Output)>;

  fn poll_ready(&mut self, context: &mut task::Context) -> task::Poll<Result<(), Self::Error>> {
    self.inner.poll_ready(context)
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    Self::log_headers("REQUEST", request.headers().iter());
    self.inner.call(request).inspect(|result| match result {
      Ok(response) => {
        Self::log_headers("RESPONSE", response.headers().iter());
      }
      Err(_) => {}
    })
  }
}
