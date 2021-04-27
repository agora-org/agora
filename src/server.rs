use crate::{environment::Environment, request_handler::RequestHandler};
use anyhow::{Context, Result};
use hyper::server::conn::AddrIncoming;
use std::{fmt::Debug, fs, net::SocketAddr};
use tower::make::Shared;

#[derive(Debug)]
pub(crate) struct Server {
  inner: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
}

impl Server {
  pub(crate) fn setup(environment: &Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    fs::read_dir(environment.working_directory.join("www")).context("cannot access `www`")?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], arguments.port.unwrap_or(0)));
    let inner =
      hyper::Server::bind(&socket_addr).serve(Shared::new(RequestHandler::new(&environment)));

    eprintln!("Listening on port {}", inner.local_addr().port());
    Ok(Self { inner })
  }

  pub(crate) async fn run(self) -> Result<()> {
    Ok(self.inner.await?)
  }

  pub(crate) fn port(&self) -> u16 {
    self.inner.local_addr().port()
  }
}
