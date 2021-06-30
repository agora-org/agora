use crate::{
  environment::Environment,
  error::{self, Error, Result},
  request_handler::RequestHandler,
};
use hyper::server::conn::AddrIncoming;
use openssl::x509::X509;
use snafu::ResultExt;
use std::{fmt::Debug, io::Write, net::ToSocketAddrs};
use tower::make::Shared;

#[derive(Debug)]
pub(crate) struct Server {
  inner: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
  #[cfg(test)]
  directory: std::path::PathBuf,
  lnd_client: Option<lnd_client::Client>,
}

impl Server {
  pub(crate) async fn setup(environment: &mut Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    let directory = environment.working_directory.join(&arguments.directory);

    let _ = tokio::fs::read_dir(&directory)
      .await
      .context(error::FilesystemIo { path: &directory })?;

    let lnd_client = if let Some(lnd_rpc_url) = arguments.lnd_rpc_url {
      let lnd_rpc_cert = if let Some(path) = arguments.lnd_rpc_cert_path {
        tokio::fs::read_to_string(&path)
          .await
          .context(error::FilesystemIo { path })?
      } else {
        todo!("need cert");
      };

      let certificate = X509::from_pem(lnd_rpc_cert.as_bytes()).unwrap();
      let client = lnd_client::Client::new(lnd_rpc_url.clone(), certificate)
        .await
        .unwrap();

      write!(
        environment.stderr,
        "Connected to LND RPC server at {}",
        lnd_rpc_url
      )
      .context(error::StderrWrite)?;

      Some(client)
    } else {
      None
    };

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

    write!(environment.stderr, "Listening on {}", inner.local_addr())
      .context(error::StderrWrite)?;

    Ok(Self {
      inner,
      #[cfg(test)]
      directory,
      lnd_client,
    })
  }

  pub(crate) async fn run(self) -> Result<()> {
    self.inner.await.context(error::ServerRun)
  }

  #[cfg(test)]
  pub(crate) fn port(&self) -> u16 {
    self.inner.local_addr().port()
  }

  #[cfg(test)]
  pub(crate) fn directory(&self) -> &std::path::Path {
    &self.directory
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::{assert_contains, test_with_arguments};
  use lnd_test_context::LndTestContext;
  use std::net::IpAddr;

  #[test]
  fn listen_on_localhost_by_default_in_tests() {
    let mut environment = Environment::test(&[]);

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = Server::setup(&mut environment).await.unwrap();
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
    environment.arguments = vec!["agora".into(), "--address".into(), "host.invalid".into()];

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&mut environment).await.unwrap_err();
        assert_matches!(error, Error::AddressResolutionIo { input, ..} if input == "host.invalid");
      });
  }

  #[test]
  fn connect_to_lnd() {
    let lnd_test_context = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async { LndTestContext::new().await });

    let lnd_rpc_url = format!("https://localhost:{}", lnd_test_context.lnd_rpc_port);

    let stderr = test_with_arguments(
      &[
        "--lnd-rpc-url",
        &lnd_rpc_url,
        "--lnd-rpc-cert-path",
        lnd_test_context
          .lnd_dir()
          .join("tls.cert")
          .to_str()
          .unwrap(),
      ],
      |_context| async move {},
    );

    assert_contains(
      &stderr,
      &format!("Connected to LND RPC server at {}", lnd_rpc_url),
    );
  }
}
