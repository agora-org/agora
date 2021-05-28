use crate::{error::Result, server::Server};
use environment::Environment;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod environment;
mod error;
mod file_stream;
mod files;
mod input_path;
mod redirect;
mod request_handler;
mod server;
mod static_assets;
mod stderr;

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    if let crate::error::Error::Clap { source } = error {
      source.exit();
    } else {
      eprintln!("{}", error);
      std::process::exit(1);
    }
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

    test_with_arguments(args, |_| async move {
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
