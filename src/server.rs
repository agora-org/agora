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

    let directory = environment.working_directory.join(arguments.directory);
    fs::read_dir(&directory).context(error::WwwIo)?;
    let socket_addr = SocketAddr::from(([0, 0, 0, 0], arguments.port));
    let inner = hyper::Server::bind(&socket_addr)
      .serve(Shared::new(RequestHandler::new(&environment, &directory)));

    eprintln!("Listening on {}", inner.local_addr());
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

#[cfg(test)]
mod tests {
  use super::*;

  use std::net::IpAddr;

  #[test]
  fn listen_on_all_local_ip_addresses() {
    let environment = Environment::test(&[]);

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = Server::setup(&environment).unwrap();
        assert_eq!(server.inner.local_addr().ip(), IpAddr::from([0, 0, 0, 0]));
      });
  }
}
