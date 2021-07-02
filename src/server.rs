use crate::{
  arguments::Arguments,
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
  request_handler: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
  #[cfg(test)]
  directory: std::path::PathBuf,
  lnd_client: Option<lnd_client::Client>,
}

impl Server {
  async fn setup_lnd_client(
    environment: &mut Environment,
    arguments: &Arguments,
  ) -> Result<Option<lnd_client::Client>> {
    match &arguments.lnd_rpc_authority {
      Some(lnd_rpc_authority) => {
        let lnd_rpc_cert = match &arguments.lnd_rpc_cert_path {
          Some(path) => {
            let pem = tokio::fs::read_to_string(&path)
              .await
              .context(error::FilesystemIo { path })?;
            Some(X509::from_pem(pem.as_bytes()).context(error::LndGrpcCertificateParse)?)
          }
          None => None,
        };

        let lnd_rpc_macaroon = match &arguments.lnd_rpc_macaroon_path {
          Some(path) => Some(
            tokio::fs::read(&path)
              .await
              .context(error::FilesystemIo { path })?,
          ),
          None => None,
        };

        let mut client =
          lnd_client::Client::new(lnd_rpc_authority.clone(), lnd_rpc_cert, lnd_rpc_macaroon)
            .await
            .context(error::LndGrpcConnect)?;

        client.ping().await.context(error::LndGrpcStatus)?;

        writeln!(
          environment.stderr,
          "Connected to LND RPC server at {}",
          lnd_rpc_authority
        )
        .context(error::StderrWrite)?;

        Ok(Some(client))
      }
      None => Ok(None),
    }
  }

  fn setup_request_handler(
    environment: &mut Environment,
    arguments: &Arguments,
  ) -> Result<hyper::Server<AddrIncoming, Shared<RequestHandler>>> {
    let socket_addr = (arguments.address.as_str(), arguments.port)
      .to_socket_addrs()
      .context(error::AddressResolutionIo {
        input: &arguments.address,
      })?
      .next()
      .ok_or_else(|| Error::AddressResolutionNoAddresses {
        input: arguments.address.clone(),
      })?;

    let request_handler = hyper::Server::bind(&socket_addr).serve(Shared::new(
      RequestHandler::new(&environment, &arguments.directory),
    ));
    writeln!(
      environment.stderr,
      "Listening on {}",
      request_handler.local_addr()
    )
    .context(error::StderrWrite)?;
    Ok(request_handler)
  }

  pub(crate) async fn setup(environment: &mut Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    let directory = environment.working_directory.join(&arguments.directory);
    let _: tokio::fs::ReadDir = tokio::fs::read_dir(&directory)
      .await
      .context(error::FilesystemIo { path: &directory })?;

    let lnd_client = Self::setup_lnd_client(environment, &arguments).await?;

    let request_handler = Self::setup_request_handler(environment, &arguments)?;

    Ok(Self {
      request_handler,
      #[cfg(test)]
      directory,
      lnd_client,
    })
  }

  pub(crate) async fn run(self) -> Result<()> {
    self.request_handler.await.context(error::ServerRun)
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

    let lnd_rpc_authority = format!("localhost:{}", lnd_test_context.lnd_rpc_port);

    let stderr = test_with_arguments(
      &[
        "--lnd-rpc-authority",
        &lnd_rpc_authority,
        "--lnd-rpc-cert-path",
        lnd_test_context.cert_path().to_str().unwrap(),
        "--lnd-rpc-macaroon-path",
        lnd_test_context.invoice_macaroon_path().to_str().unwrap(),
      ],
      |_context| async move {},
    );

    assert_contains(
      &stderr,
      &format!("Connected to LND RPC server at {}", lnd_rpc_authority),
    );
  }
}
