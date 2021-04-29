use crate::{
  environment::Environment,
  error::{self, Result},
  request_handler::RequestHandler,
};
use hyper::server::conn::AddrIncoming;
use snafu::ResultExt;
use std::{fmt::Debug, fs, net::SocketAddr};
use tower::make::Shared;

#[derive(Debug)]
pub(crate) struct Server {
  inner: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
}

impl Server {
  pub(crate) fn setup(environment: &Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    fs::read_dir(environment.working_directory.join("www")).context(error::WwwIo)?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], arguments.port));
    let inner =
      hyper::Server::bind(&socket_addr).serve(Shared::new(RequestHandler::new(&environment)));

    eprintln!("Listening on port {}", inner.local_addr().port());
    Ok(Self { inner })
  }

  pub(crate) async fn run(self) -> Result<()> {
    self.inner.await.context(error::ServerRun)
  }

  #[cfg(test)]
  pub(crate) fn port(&self) -> u16 {
    self.inner.local_addr().port()
  }
}
