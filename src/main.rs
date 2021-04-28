use crate::server::Server;
use anyhow::Result;
use environment::Environment;

mod environment;
mod request_handler;
mod server;
mod stderr;

#[cfg(test)]
mod test_utils;

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    eprintln!("{:?}", error);
    std::process::exit(1);
  }
}

async fn run() -> Result<()> {
  let environment = Environment::production()?;
  let server = Server::setup(&environment)?;
  server.run().await
}

#[cfg(test)]
mod tests {
  use crate::test_utils::test_with_arguments;
  use std::net::TcpListener;

  #[test]
  fn configure_port() {
    let free_port = {
      TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
    };

    let args = &["--port", &free_port.to_string()];

    test_with_arguments(args, |_port, _dir| async move {
      assert_eq!(
        reqwest::get(format!("http://localhost:{}", free_port))
          .await
          .unwrap()
          .status(),
        200
      )
    });
  }
}
