use crate::{
  arguments::Arguments, environment::Environment, error::Result, request_handler::RequestHandler,
  server::Server,
};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use hyper::server::conn::Http;
use rustls_acme::acme::ACME_TLS_ALPN_NAME;
use rustls_acme::ResolvesServerCertUsingAcme;
use rustls_acme::{acme::LETS_ENCRYPT_PRODUCTION_DIRECTORY, acme::LETS_ENCRYPT_STAGING_DIRECTORY};
use std::path::Path;
use std::sync::Arc;
use tokio::{net::TcpListener, task};
use tokio_rustls::rustls::{NoClientAuth, ServerConfig, Session};
use tokio_rustls::server::TlsStream;
use tokio_stream::wrappers::TcpListenerStream;

pub(crate) struct TlsRequestHandler {
  port: u16,
  run: BoxFuture<'static, ()>,
}

impl TlsRequestHandler {
  pub(crate) async fn new(
    environment: &mut Environment,
    arguments: &Arguments,
    acme_cache_directory: &Path,
    https_port: u16,
  ) -> Result<TlsRequestHandler> {
    // fixme: pass this in?
    let lnd_client = Server::setup_lnd_client(environment, arguments).await?;
    let request_handler = RequestHandler::new(environment, &arguments.directory, lnd_client);
    // fixme: bind on different address?
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", https_port))
      .await
      .expect("fixme");
    simple_logger::SimpleLogger::new()
      .with_level(log::LevelFilter::Info)
      .init()
      .ok();

    Ok(TlsRequestHandler {
      port: listener.local_addr().expect("fixme").port(),
      run: bind_listen_serve(
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
      .await,
    })
  }

  pub(crate) fn port(&self) -> u16 {
    self.port
  }

  pub(crate) async fn run(self) {
    self.run.await;
  }
}

async fn bind_listen_serve(
  listener: TcpListener,
  directory_url: impl AsRef<str>,
  domains: Vec<String>,
  cache_dir: Option<impl AsRef<Path>>,
  request_handler: RequestHandler,
) -> BoxFuture<'static, ()> {
  let resolver = ResolvesServerCertUsingAcme::new();
  let config = ServerConfig::new(NoClientAuth::new());
  let acceptor = TlsAcceptor::new(config, resolver.clone());

  let directory_url = directory_url.as_ref().to_string();
  let cache_dir = cache_dir.map(|p| p.as_ref().to_path_buf());
  task::spawn(async move {
    resolver.run(directory_url, domains, cache_dir).await;
  });

  let mut listener = TcpListenerStream::new(listener);
  (async move {
    while let Some(tcp) = listener.next().await {
      let tcp = match tcp {
        Ok(tcp) => tcp,
        Err(err) => {
          log::error!("tcp accept error: {:?}", err);
          continue;
        }
      };
      let acceptor = acceptor.clone();
      let request_handler = request_handler.clone();
      match acceptor.accept(tcp).await {
        Ok(Some(tls_stream)) => {
          Http::new()
            .serve_connection(tls_stream, request_handler)
            .await
            .ok();
        }
        Ok(None) => {}
        Err(err) => log::error!("tls accept error: {:?}", err),
      }
    }
  })
  .boxed()
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
