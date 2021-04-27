use crate::request_handler::{run_server, Environment, RequestHandler};
use anyhow::Result;

mod request_handler;
mod stderr;

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    eprintln!("{:?}", error);
    std::process::exit(1);
  }
}

async fn run() -> Result<()> {
  let environment = Environment::production()?;
  let server = RequestHandler::bind(environment)?;
  run_server(server).await
}

#[cfg(test)]
mod tests {
  use crate::request_handler::tests::test_with_arguments;
  use std::net::TcpListener;

  #[test]
  fn configure_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let port_str = port.as_str();
    drop(listener);

    let args = &["foo", "--port", port_str];

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
