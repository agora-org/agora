use crate::{
  request_handler::{run_server, RequestHandler},
  stderr::Stderr,
};
use anyhow::Result;
use std::env;

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
  let stderr = Stderr::production();
  let server = RequestHandler::bind(&stderr, &env::current_dir()?, Some(8080))?;
  run_server(server).await
}
