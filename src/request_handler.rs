use crate::stderr::Stderr;
use anyhow::{Context, Result};
use futures::{future::BoxFuture, FutureExt};
use hyper::{
  server::conn::AddrIncoming, service::Service, Body, Request, Response, Server, StatusCode,
};
use maud::{html, DOCTYPE};
use std::{
  convert::Infallible,
  fmt::Debug,
  fs,
  io::Write,
  net::SocketAddr,
  path::{Path, PathBuf},
  task::{self, Poll},
};
use tower::make::Shared;

pub(crate) type RequestHandlerServer = Server<AddrIncoming, Shared<RequestHandler>>;

pub(crate) async fn run_server(server: RequestHandlerServer) -> Result<()> {
  Ok(server.await?)
}

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  stderr: Stderr,
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn bind(
    stderr: &Stderr,
    working_directory: &Path,
    port: Option<u16>,
  ) -> Result<RequestHandlerServer> {
    fs::read_dir(working_directory.join("www")).context("cannot access `www`")?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let server = Server::bind(&socket_addr).serve(Shared::new(RequestHandler {
      stderr: stderr.clone(),
      working_directory: working_directory.to_owned(),
    }));
    eprintln!("Listening on port {}", server.local_addr().port());
    Ok(server)
  }

  async fn response(mut self) -> Response<Body> {
    match self.list_www().await.context("cannot access `www`") {
      Ok(response) => response,
      Err(error) => {
        writeln!(self.stderr, "{:?}", error).unwrap();
        Response::builder()
          .status(StatusCode::INTERNAL_SERVER_ERROR)
          .body(Body::empty())
          .unwrap()
      }
    }
  }

  async fn list_www(&self) -> Result<Response<Body>> {
    let mut read_dir = tokio::fs::read_dir(self.working_directory.join("www")).await?;
    let body = html! {
      (DOCTYPE)
      html {
        head {
          title {
            "foo"
          }
        }
        body {
          @while let Some(entry) = read_dir.next_entry().await? {
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

  fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: Request<Body>) -> Self::Future {
    self.clone().response().map(Ok).boxed()
  }
}
