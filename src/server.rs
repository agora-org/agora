use crate::common::*;
use openssl::x509::X509;
use tower::make::Shared;

pub(crate) struct Server {
  http_request_handler: Option<hyper::Server<AddrIncoming, Shared<RequestHandler>>>,
  https_request_handler: Option<HttpsRequestHandler>,
  https_redirect_server: Option<hyper::Server<AddrIncoming, Shared<HttpsRedirectService>>>,
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

    let http_request_handler = match arguments.http_port {
      Some(http_port) => {
        Some(Self::setup_http_request_handler(environment, &arguments, http_port).await?)
      }
      None => None,
    };

    let (https_request_handler, https_redirect_server) =
      if let Some(https_port) = arguments.https_port {
        let acme_cache_directory = arguments
          .acme_cache_directory
          .as_ref()
          .expect("<https-port> requires <acme-cache-directory>");
        let lnd_client = Self::setup_lnd_client(environment, &arguments).await?;
        let https_request_handler = HttpsRequestHandler::new(
          environment,
          &arguments,
          acme_cache_directory,
          https_port,
          lnd_client,
        )
        .await?;
        let https_redirect_server =
          HttpsRedirectService::new_server(environment, &arguments, &https_request_handler)?;
        (Some(https_request_handler), https_redirect_server)
      } else {
        (None, None)
      };

    Ok(Self {
      http_request_handler,
      https_request_handler,
      https_redirect_server,
      #[cfg(test)]
      directory,
    })
  }

  async fn setup_http_request_handler(
    environment: &mut Environment,
    arguments: &Arguments,
    http_port: u16,
  ) -> Result<hyper::Server<AddrIncoming, Shared<RequestHandler>>> {
    let lnd_client = Self::setup_lnd_client(environment, arguments).await?;

    let socket_addr = (arguments.address.as_str(), http_port)
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
      "Listening for HTTP connections on `{}`",
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
      OptionFuture::from(self.http_request_handler)
        .map(|option| option.unwrap_or(Ok(())).context(error::ServerRun)),
      OptionFuture::from(self.https_request_handler.map(|x| x.run())).map(Ok),
      OptionFuture::from(self.https_redirect_server)
        .map(|option| option.unwrap_or(Ok(())).context(error::ServerRun)),
    )?;

    Ok(())
  }

  #[cfg(test)]
  pub(crate) fn test_context(&self, environment: &Environment) -> TestContext {
    let http_url = reqwest::Url::parse(&format!(
      "http://localhost:{}",
      self
        .http_request_handler
        .as_ref()
        .unwrap()
        .local_addr()
        .port()
    ))
    .unwrap();
    let working_directory = environment.working_directory.clone();
    TestContext {
      base_url: http_url.clone(),
      files_url: http_url.join("files/").unwrap(),
      https_files_url: self
        .https_request_handler
        .as_ref()
        .map(|handler| handler.https_port())
        .map(|https_port| {
          let mut url = http_url.join("files/").unwrap();
          url.set_scheme("https").unwrap();
          url.set_port(Some(https_port)).unwrap();
          url
        }),
      https_redirect_port: self
        .https_redirect_server
        .as_ref()
        .map(|server| server.local_addr().port()),
      working_directory,
      files_directory: self.directory.to_owned(),
    }
  }
}

#[cfg(test)]
pub(crate) struct TestContext {
  base_url: reqwest::Url,
  files_directory: std::path::PathBuf,
  files_url: reqwest::Url,
  https_files_url: Option<reqwest::Url>,
  https_redirect_port: Option<u16>,
  working_directory: std::path::PathBuf,
}

#[cfg(test)]
impl TestContext {
  pub(crate) fn files_url(&self) -> &reqwest::Url {
    &self.files_url
  }

  pub(crate) fn https_files_url(&self) -> &reqwest::Url {
    self.https_files_url.as_ref().unwrap()
  }

  pub(crate) fn https_redirect_port(&self) -> u16 {
    self.https_redirect_port.unwrap()
  }

  pub(crate) fn files_directory(&self) -> &std::path::Path {
    &self.files_directory
  }

  pub(crate) fn base_url(&self) -> &reqwest::Url {
    &self.base_url
  }

  pub(crate) fn working_directory(&self) -> &std::path::Path {
    &self.working_directory
  }

  pub(crate) fn write(&self, path: &str, content: &str) -> std::path::PathBuf {
    let path = self.files_directory.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
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
        let ip = server.http_request_handler.unwrap().local_addr().ip();
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
