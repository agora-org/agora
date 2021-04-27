use crate::request_handler::Server;
use anyhow::Result;
use environment::Environment;

mod environment;
mod request_handler;
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
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let port_str = port.as_str();
    drop(listener);

    let args = &["--port", port_str];

    test_with_arguments(args, |_port, _dir| async move {
      assert_eq!(
        reqwest::get(format!("http://localhost:{}", port_str))
          .await
          .unwrap()
          .status(),
        200
      )
    });
  }
}
