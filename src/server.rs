use crate::{
  arguments::Arguments,
  environment::Environment,
  error::{self, Result},
  https_redirect_service::HttpsRedirectService,
  request_handler::RequestHandler,
};
use futures::{future::BoxFuture, future::OptionFuture, FutureExt};
use hyper::server::conn::{AddrIncoming, Http};
use openssl::x509::X509;
use rustls_acme::{acme::LETS_ENCRYPT_PRODUCTION_DIRECTORY, acme::LETS_ENCRYPT_STAGING_DIRECTORY};
use snafu::ResultExt;
use std::{io::Write, net::ToSocketAddrs, path::Path};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tower::make::Shared;

pub(crate) struct Server {
  request_handler: hyper::Server<AddrIncoming, Shared<RequestHandler>>,
  tls_request_handler: Option<BoxFuture<'static, Result<()>>>,
  https_redirect_server: Option<hyper::Server<AddrIncoming, Shared<HttpsRedirectService>>>,
  #[cfg(test)]
  tls_port: Option<u16>,
  #[cfg(test)]
  directory: std::path::PathBuf,
}

impl Server {
  pub(crate) async fn setup(environment: &mut Environment) -> Result<Self> {
    let arguments = environment.arguments()?;

    let directory = environment.working_directory.join(&arguments.directory);
    let _ = tokio::fs::read_dir(&directory)
      .await
      .context(error::FilesystemIo { path: &directory })?;

    let request_handler = Self::setup_request_handler(environment, &arguments).await?;

    let (tls_port, tls_request_handler) = if let Some(https_port) = arguments.https_port {
      let acme_cache_directory = arguments.acme_cache_directory.as_ref().unwrap();
      let (a, b) = Self::foo(environment, &arguments, acme_cache_directory, https_port).await?;
      (Some(a), Some(b))
    } else {
      (None, None)
    };

    let https_redirect_server = HttpsRedirectService::new_server(&arguments, tls_port)?;

    Ok(Self {
      request_handler,
      tls_request_handler,
      #[cfg(test)]
      tls_port,
      https_redirect_server,
      #[cfg(test)]
      directory,
    })
  }

  async fn foo(
    environment: &mut Environment,
    arguments: &Arguments,
    acme_cache_directory: &Path,
    https_port: u16,
  ) -> Result<(u16, BoxFuture<'static, Result<()>>)> {
    let lnd_client = Self::setup_lnd_client(environment, arguments).await?;
    let request_handler = RequestHandler::new(environment, &arguments.directory, lnd_client);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", https_port))
      .await
      .expect("fixme");
    simple_logger::SimpleLogger::new()
      .with_level(log::LevelFilter::Info)
      .init()
      .ok();

    Ok((
      listener.local_addr().expect("fixme").port(),
      crate::bind_listen_serve::bind_listen_serve(
        listener,
        if cfg!(test) {
          LETS_ENCRYPT_STAGING_DIRECTORY
        } else {
          LETS_ENCRYPT_PRODUCTION_DIRECTORY
        },
        vec!["test.agora.download".to_string()],
        Some(environment.working_directory.join(acme_cache_directory)),
        request_handler,
      )
      .map(|x| x.map_err(|_| todo!()))
      .boxed(),
    ))
  }

  async fn setup_request_handler(
    environment: &mut Environment,
    arguments: &Arguments,
  ) -> Result<hyper::Server<AddrIncoming, Shared<RequestHandler>>> {
    let lnd_client = Self::setup_lnd_client(environment, arguments).await?;

    let socket_addr = (arguments.address.as_str(), arguments.http_port)
      .to_socket_addrs()
      .context(error::AddressResolutionIo {
        input: &arguments.address,
      })?
      .next()
      .ok_or_else(|| {
        error::AddressResolutionNoAddresses {
          input: arguments.address.clone(),
        }
        .build()
      })?;

    let request_handler = hyper::Server::bind(&socket_addr).serve(Shared::new(
      RequestHandler::new(environment, &arguments.directory, lnd_client),
    ));

    writeln!(
      environment.stderr,
      "Listening on {}",
      request_handler.local_addr()
    )
    .context(error::StderrWrite)?;
    Ok(request_handler)
  }

  async fn setup_lnd_client(
    environment: &mut Environment,
    arguments: &Arguments,
  ) -> Result<Option<agora_lnd_client::Client>> {
    match &arguments.lnd_rpc_authority {
      Some(lnd_rpc_authority) => {
        let lnd_rpc_cert = match &arguments.lnd_rpc_cert_path {
          Some(path) => {
            let pem = tokio::fs::read_to_string(&path)
              .await
              .context(error::FilesystemIo { path })?;
            Some(X509::from_pem(pem.as_bytes()).context(error::LndRpcCertificateParse)?)
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
          agora_lnd_client::Client::new(lnd_rpc_authority.clone(), lnd_rpc_cert, lnd_rpc_macaroon)
            .await
            .context(error::LndRpcConnect)?;

        match client.ping().await.context(error::LndRpcStatus) {
          Err(error) => {
            writeln!(
              environment.stderr,
              "warning: Cannot connect to LND gRPC server at `{}`: {}",
              lnd_rpc_authority, error,
            )
            .context(error::StderrWrite)?;
          }
          Ok(()) => {
            writeln!(
              environment.stderr,
              "Connected to LND RPC server at {}",
              lnd_rpc_authority
            )
            .context(error::StderrWrite)?;
          }
        }

        Ok(Some(client))
      }
      None => Ok(None),
    }
  }

  pub(crate) async fn run(self) -> Result<()> {
    futures::try_join!(
      self.request_handler.map(|x| x.context(error::ServerRun)),
      OptionFuture::from(self.tls_request_handler).map(|option| option.unwrap_or(Ok(()))),
      OptionFuture::from(self.https_redirect_server)
        .map(|option| option.unwrap_or(Ok(())).context(error::ServerRun)),
    )?;

    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn port(&self) -> u16 {
    self.request_handler.local_addr().port()
  }

  #[cfg(test)]
  pub(crate) fn tls_port(&self) -> Option<u16> {
    self.tls_port
  }

  #[cfg(test)]
  pub(crate) fn https_redirect_port(&self) -> Option<u16> {
    self
      .https_redirect_server
      .as_ref()
      .map(|server| server.local_addr().port())
  }

  #[cfg(test)]
  pub(crate) fn directory(&self) -> &std::path::Path {
    &self.directory
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::Error;
  use std::net::IpAddr;

  #[test]
  fn listen_on_localhost_by_default_in_tests() {
    let mut environment = Environment::test();

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = Server::setup(&mut environment).await.unwrap();
        let ip = server.request_handler.local_addr().ip();
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
    let mut environment = Environment::test();
    environment.arguments = vec![
      "agora".into(),
      "--address=host.invalid".into(),
      "--http-port=0".into(),
      "--directory=www".into(),
    ];

    let www = environment.working_directory.join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&mut environment).await.err().unwrap();
        assert_matches!(error, Error::AddressResolutionIo { input, ..} if input == "host.invalid");
      });
  }
}

#[cfg(all(test, feature = "slow-tests"))]
mod slow_tests {
  use crate::test_utils::{assert_contains, test_with_lnd};
  use lnd_test_context::LndTestContext;

  #[test]
  fn connect_to_lnd() {
    let lnd_test_context = LndTestContext::new_blocking();
    let stderr = test_with_lnd(&lnd_test_context, |_context| async move {});

    assert_contains(
      &stderr,
      &format!(
        "Connected to LND RPC server at {}",
        lnd_test_context.lnd_rpc_authority()
      ),
    );
  }
}
