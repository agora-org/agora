use futures::{future::BoxFuture, FutureExt};
use hyper::{
  server::conn::AddrIncoming, service::Service, Body, Request, Response, Server, StatusCode,
};
use maud::{html, DOCTYPE};
use std::fmt::Debug;
#[cfg(test)]
use std::io::Cursor;
use std::{
  convert::Infallible,
  fs,
  io::{self, Write},
  net::SocketAddr,
  path::{Path, PathBuf},
  sync::Arc,
  task::{Context, Poll},
};
use tokio::sync::Mutex;
use tower::make::Shared;

#[cfg(test)]
pub(crate) type Stderr = Cursor<Vec<u8>>;
#[cfg(not(test))]
pub(crate) type Stderr = std::io::Stderr;

pub(crate) fn stderr() -> Stderr {
  #[cfg(test)]
  return Cursor::new(vec![]);
  #[cfg(not(test))]
  return std::io::stderr();
}

pub(crate) type RequestHandlerServer = Server<AddrIncoming, Shared<RequestHandler>>;

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  stderr: Arc<Mutex<Box<Stderr>>>,
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn bind(
    stderr: &Arc<Mutex<Box<Stderr>>>,
    working_directory: &Path,
    port: Option<u16>,
  ) -> io::Result<RequestHandlerServer> {
    fs::read_dir(working_directory.join("www"))?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let server = Server::bind(&socket_addr).serve(Shared::new(RequestHandler {
      stderr: stderr.clone(),
      working_directory: working_directory.to_owned(),
    }));
    eprintln!("Listening on port {}", server.local_addr().port());
    Ok(server)
  }

  async fn response(self) -> Response<Body> {
    match self.list_www().await {
      Ok(response) => response,
      Err(error) => {
        writeln!(self.stderr.lock().await, "{}", error).unwrap();
        Response::builder()
          .status(StatusCode::INTERNAL_SERVER_ERROR)
          .body(Body::empty())
          .unwrap()
      }
    }
  }

  async fn list_www(&self) -> Result<Response<Body>, String> {
    let mut read_dir = tokio::fs::read_dir(self.working_directory.join("www"))
      .await
      .map_err(|error| format!("{}: `www`", error))?;
    let body = html! {
      (DOCTYPE)
      html {
        head {
          title {
            "foo"
          }
        }
        body {
          @while let Some(entry) = read_dir.next_entry().await.map_err(|error|format!("{}: `www`",error))? {
            (entry.file_name().to_string_lossy())
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
  type Error = Infallible;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: Request<Body>) -> Self::Future {
    self.clone().response().map(Ok).boxed()
  }
}
