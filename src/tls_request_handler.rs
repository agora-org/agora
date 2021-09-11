use crate::{
  arguments::Arguments, environment::Environment, error::Result, request_handler::RequestHandler,
};
use futures::StreamExt;
use hyper::server::conn::Http;
use rustls_acme::acme::ACME_TLS_ALPN_NAME;
use rustls_acme::ResolvesServerCertUsingAcme;
use rustls_acme::{acme::LETS_ENCRYPT_PRODUCTION_DIRECTORY, acme::LETS_ENCRYPT_STAGING_DIRECTORY};
use std::path::Path;
use std::sync::Arc;
use tokio::task;
use tokio_rustls::rustls::{NoClientAuth, ServerConfig, Session};
use tokio_rustls::server::TlsStream;
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
    // fixme: bind on different address?
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", https_port))
      .await
      .expect("fixme");
    let https_port = listener.local_addr().expect("fixme").port();
    let cache_dir = environment.working_directory.join(acme_cache_directory);
    let resolver = ResolvesServerCertUsingAcme::new();
    let resolver_clone = resolver.clone();
    task::spawn(async move {
      resolver_clone
        .run(
          if cfg!(test) {
            LETS_ENCRYPT_STAGING_DIRECTORY
          } else {
            LETS_ENCRYPT_PRODUCTION_DIRECTORY
          },
          vec!["test.agora.download".to_string()],
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
      match tls_acceptor.clone().accept(tcp).await {
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
