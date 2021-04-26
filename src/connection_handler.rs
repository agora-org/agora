use crate::request_handler::RequestHandler;
use futures::future::{self, Ready};
use hyper::{
  server::conn::{AddrIncoming, AddrStream},
  service::Service,
  Server,
};
use std::{
  convert::Infallible,
  net::SocketAddr,
  path::{Path, PathBuf},
  task::{Context, Poll},
};

pub(crate) type ConnectionHandlerServer = Server<AddrIncoming, ConnectionHandler>;

pub(crate) struct ConnectionHandler {
  working_directory: PathBuf,
}

impl ConnectionHandler {
  pub(crate) fn bind(working_directory: &Path, port: Option<u16>) -> ConnectionHandlerServer {
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let connection_handler = Self {
      working_directory: working_directory.to_owned(),
    };
    let server = Server::bind(&socket_addr).serve(connection_handler);
    eprintln!("Listening on port {}", server.local_addr().port());
    server
  }
}

impl Service<&AddrStream> for ConnectionHandler {
  type Response = RequestHandler;
  type Error = Infallible;
  type Future = Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: &AddrStream) -> Self::Future {
    future::ok(RequestHandler {
      working_directory: self.working_directory.clone(),
    })
  }
}
