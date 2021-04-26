use futures::future::{self, Ready};
use hyper::{service::Service, Body, Request, Response};
use maud::{html, DOCTYPE};
use std::{
  fs, io,
  path::PathBuf,
  task::{Context, Poll},
};

pub(crate) struct RequestHandler {
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  fn response(&self) -> io::Result<Response<Body>> {
    let body = html! {
      (DOCTYPE)
      html {
        head {
          title {
            "foo"
          }
        }
        body {
          @for result in fs::read_dir(self.working_directory.join("www"))? {
            (result?.file_name().to_string_lossy())
            br;
          }
        }
      }
    };

    Ok(Response::new(Body::from(body.into_string())))
  }
}

impl Service<Request<Body>> for RequestHandler {
  type Response = Response<Body>;
  type Error = io::Error;
  type Future = Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: Request<Body>) -> Self::Future {
    future::ready(self.response())
  }
}
