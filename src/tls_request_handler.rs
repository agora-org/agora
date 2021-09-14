use crate::{
  arguments::Arguments,
  environment::Environment,
  error::{self, Result},
  request_handler::RequestHandler,
};
use futures::StreamExt;
use hyper::server::conn::Http;
use rustls_acme::{
  acme::{ACME_TLS_ALPN_NAME, LETS_ENCRYPT_PRODUCTION_DIRECTORY, LETS_ENCRYPT_STAGING_DIRECTORY},
  ResolvesServerCertUsingAcme,
};
use snafu::ResultExt;
use std::{io::Write, net::ToSocketAddrs, path::Path, sync::Arc};
use tokio::task;
use tokio_rustls::{
  rustls::{NoClientAuth, ServerConfig, Session},
  server::TlsStream,
};
use tokio_stream::wrappers::TcpListenerStream;

pub(crate) struct TlsRequestHandler {
  request_handler: RequestHandler,
  https_port: u16,
  tcp_listener_stream: TcpListenerStream,
  resolver: Arc<ResolvesServerCertUsingAcme>,
}

impl TlsRequestHandler {
  pub(crate) async fn new(
    environment: &mut Environment,
    arguments: &Arguments,
    acme_cache_directory: &Path,
    https_port: u16,
    lnd_client: Option<agora_lnd_client::Client>,
  ) -> Result<TlsRequestHandler> {
    simple_logger::SimpleLogger::new()
      .with_level(log::LevelFilter::Info)
      .init()
      .ok();
    let request_handler = RequestHandler::new(environment, &arguments.directory, lnd_client);
    let socket_addr = (arguments.address.as_str(), https_port)
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
    let listener = tokio::net::TcpListener::bind(socket_addr)
      .await
      .expect("fixme");
    writeln!(
      environment.stderr,
      "Listening on {} (https)",
      listener.local_addr().expect("fixme")
    )
    .context(error::StderrWrite)?;
    let https_port = listener.local_addr().expect("fixme").port();
    let cache_dir = environment.working_directory.join(acme_cache_directory);
    let resolver = ResolvesServerCertUsingAcme::new();
    let resolver_clone = resolver.clone();
    let acme_domains = arguments.acme_domain.clone();
    task::spawn(async move {
      resolver_clone
        .run(
          if cfg!(test) {
            LETS_ENCRYPT_STAGING_DIRECTORY
          } else {
            LETS_ENCRYPT_PRODUCTION_DIRECTORY
          },
          acme_domains,
          Some(cache_dir),
        )
        .await;
    });
    Ok(TlsRequestHandler {
      request_handler,
      https_port,
      tcp_listener_stream: TcpListenerStream::new(listener),
      resolver,
    })
  }

  pub(crate) async fn run(mut self) {
    let tls_acceptor = TlsAcceptor::new(ServerConfig::new(NoClientAuth::new()), self.resolver);
    while let Some(tcp) = self.tcp_listener_stream.next().await {
      let tcp = match tcp {
        Ok(tcp) => tcp,
        Err(err) => {
          log::error!("tcp accept error: {:?}", err);
          continue;
        }
      };
      match tls_acceptor.accept(tcp).await {
        Ok(Some(tls_stream)) => {
          Http::new()
            .serve_connection(tls_stream, self.request_handler.clone())
            .await
            .ok();
        }
        Ok(None) => {}
        Err(err) => log::error!("tls accept error: {:?}", err),
      }
    }
  }

  pub(crate) fn https_port(&self) -> u16 {
    self.https_port
  }
}

#[derive(Clone)]
pub(crate) struct TlsAcceptor {
  config: Arc<ServerConfig>,
}

impl TlsAcceptor {
  pub(crate) fn new(mut config: ServerConfig, resolver: Arc<ResolvesServerCertUsingAcme>) -> Self {
    config.alpn_protocols.push(ACME_TLS_ALPN_NAME.to_vec());
    config.cert_resolver = resolver;
    let config = Arc::new(config);
    TlsAcceptor { config }
  }

  pub(crate) async fn accept<IO>(&self, stream: IO) -> std::io::Result<Option<TlsStream<IO>>>
  where
    IO: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
  {
    let tls = tokio_rustls::TlsAcceptor::from(self.config.clone())
      .accept(stream)
      .await?;
    if tls.get_ref().1.get_alpn_protocol() == Some(ACME_TLS_ALPN_NAME) {
      log::debug!("completed acme-tls/1 handshake");
      return Ok(None);
    }
    Ok(Some(tls))
  }
}
