use crate::{
  environment::Environment,
  error::{self, Error, Result},
  request_handler::RequestHandler,
};
use hyper::server::conn::AddrIncoming;
use snafu::ResultExt;
use std::{fmt::Debug, fs, net::ToSocketAddrs};
use tower::make::Shared;

#[derive(Debug)]
pub(crate) struct Server {
  inner: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
}

impl Server {
  pub(crate) fn setup(environment: &Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    let directory = environment.working_directory.join(&arguments.directory);
    fs::read_dir(&directory).context(error::FilesystemIo { path: &directory })?;

    let socket_addr = (arguments.address.as_str(), arguments.port)
      .to_socket_addrs()
      .context(error::AddressResolutionIo {
        input: arguments.address.clone(),
      })?
      .next()
      .ok_or(Error::AddressResolutionNoAddresses {
        input: arguments.address,
      })?;

    let inner = hyper::Server::bind(&socket_addr).serve(Shared::new(RequestHandler::new(
      &environment,
      &arguments.directory,
    )));

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
  fn listen_on_localhost_by_default_in_tests() {
    let environment = Environment::test(&[]);

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = Server::setup(&environment).unwrap();
        let ip = server.inner.local_addr().ip();
        assert!(
          ip == IpAddr::from([127, 0, 0, 1])
            || ip == IpAddr::from([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
          "Server address is not loopback address: {}",
          ip,
        );
      });
  }

  #[test]
  fn address_resolution_failure_error() {
    let mut environment = Environment::test(&[]);
    environment.arguments = vec!["foo".into(), "--address".into(), "host.invalid".into()];

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&environment).unwrap_err();
        assert_matches!(error, Error::AddressResolutionIo { input, ..} if input == "host.invalid");
      });
  }
}
