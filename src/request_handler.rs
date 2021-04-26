use futures::future::{self, Ready};
use hyper::{server::conn::AddrIncoming, service::Service, Body, Request, Response, Server};
use maud::{html, DOCTYPE};
use std::{
  fs, io,
  net::SocketAddr,
  path::{Path, PathBuf},
  task::{Context, Poll},
};
use tower::make::Shared;

pub(crate) type RequestHandlerServer = Server<AddrIncoming, Shared<RequestHandler>>;

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn bind(
    working_directory: &Path,
    port: Option<u16>,
  ) -> io::Result<RequestHandlerServer> {
    fs::read_dir(working_directory.join("www"))?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let server = Server::bind(&socket_addr).serve(Shared::new(RequestHandler {
      working_directory: working_directory.to_owned(),
    }));
    eprintln!("Listening on port {}", server.local_addr().port());
    Ok(server)
  }

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
