use crate::request_handler::RequestHandler;
use futures::StreamExt;
use hyper::server::conn::Http;
use rustls_acme::acme::ACME_TLS_ALPN_NAME;
use rustls_acme::ResolvesServerCertUsingAcme;
use std::sync::Arc;
use std::{io, path::Path};
use tokio::{net::TcpListener, task};
use tokio_rustls::rustls::{NoClientAuth, ServerConfig, Session};
use tokio_rustls::server::TlsStream;
use tokio_stream::wrappers::TcpListenerStream;

pub(crate) async fn bind_listen_serve(
  listener: TcpListener,
  directory_url: impl AsRef<str>,
  domains: Vec<String>,
  cache_dir: Option<impl AsRef<Path>>,
  request_handler: RequestHandler,
) -> io::Result<()> {
  let resolver = ResolvesServerCertUsingAcme::new();
  let config = ServerConfig::new(NoClientAuth::new());
  let acceptor = TlsAcceptor::new(config, resolver.clone());

  let directory_url = directory_url.as_ref().to_string();
  let cache_dir = cache_dir.map(|p| p.as_ref().to_path_buf());
  task::spawn(async move {
    resolver.run(directory_url, domains, cache_dir).await;
  });

  let mut listener = TcpListenerStream::new(listener);
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
    task::spawn(async move {
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
    });
  }
  Ok(())
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
