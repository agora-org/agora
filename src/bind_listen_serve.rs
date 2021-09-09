use async_rustls::rustls::{NoClientAuth, ServerConfig};
use async_rustls::server::TlsStream;
use futures::StreamExt;
use rustls_acme::{ResolvesServerCertUsingAcme, TlsAcceptor};
use std::{future::Future, io, path::Path};
use tokio::{
  net::{TcpListener, TcpStream},
  task,
};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
use tokio_util::compat::{FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};

pub async fn bind_listen_serve<P, F, Fut>(
  listener: TcpListener,
  directory_url: impl AsRef<str>,
  domains: Vec<String>,
  cache_dir: Option<P>,
  f: F,
) -> io::Result<()>
where
  P: AsRef<Path>,
  F: 'static + Clone + Sync + Send + Fn(TlsStream<Compat<TcpStream>>) -> Fut,
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
    let tcp = tcp.compat();
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
