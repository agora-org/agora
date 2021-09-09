use async_rustls::rustls::{NoClientAuth, ServerConfig, Session};
use futures::StreamExt;
use rustls_acme::acme::ACME_TLS_ALPN_NAME;
use rustls_acme::ResolvesServerCertUsingAcme;
use std::sync::Arc;
use std::{future::Future, io, path::Path};
use tokio::{
  net::{TcpListener, TcpStream},
  task,
};
use tokio_rustls::server::TlsStream;
use tokio_stream::wrappers::TcpListenerStream;

pub async fn bind_listen_serve<P, F, Fut>(
  listener: TcpListener,
  directory_url: impl AsRef<str>,
  domains: Vec<String>,
  cache_dir: Option<P>,
  f: F,
) -> io::Result<()>
where
  P: AsRef<Path>,
  F: 'static + Clone + Sync + Send + Fn(TlsStream<TcpStream>) -> Fut,
  Fut: Future<Output = ()> + Send,
{
  let resolver = ResolvesServerCertUsingAcme::new();
  let config = ServerConfig::new(NoClientAuth::new());
  let acceptor = TlsAcceptor::new(config, resolver.clone());

  let directory_url = directory_url.as_ref().to_string();
  let cache_dir = cache_dir.map(|p| p.as_ref().to_path_buf());
  task::spawn(async move {
    resolver.run(directory_url, domains, cache_dir).await;
  });

  // let f = Arc::new(f);
  let mut listener = TcpListenerStream::new(listener);
  while let Some(tcp) = listener.next().await {
    let tcp = match tcp {
      Ok(tcp) => tcp,
      Err(err) => {
        log::error!("tcp accept error: {:?}", err);
        continue;
      }
    };
    let f = f.clone();
    let acceptor = acceptor.clone();
    task::spawn(async move {
      match acceptor.accept(tcp).await {
        Ok(Some(tls)) => f(tls).await,
        Ok(None) => {}
        Err(err) => log::error!("tls accept error: {:?}", err),
      }
    });
  }
  Ok(())
}

#[derive(Clone)]
pub struct TlsAcceptor {
  config: Arc<ServerConfig>,
}

impl TlsAcceptor {
  pub fn new(mut config: ServerConfig, resolver: Arc<ResolvesServerCertUsingAcme>) -> Self {
    config.alpn_protocols.push(ACME_TLS_ALPN_NAME.to_vec());
    config.cert_resolver = resolver;
    let config = Arc::new(config);
    TlsAcceptor { config }
  }

  pub async fn accept<IO>(&self, stream: IO) -> std::io::Result<Option<TlsStream<IO>>>
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
