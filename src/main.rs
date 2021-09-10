use crate::{error::Result, server::Server};
use environment::Environment;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod arguments;
mod bind_listen_serve;
mod config;
mod environment;
mod error;
mod file_stream;
mod files;
mod html;
mod https_redirect_service;
mod input_path;
mod redirect;
mod request_handler;
mod server;
mod static_assets;
mod stderr;
#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    if let crate::error::Error::Clap { source, .. } = error {
      source.exit();
    } else {
      error.print_backtrace(&mut termcolor::StandardStream::stderr(
        termcolor::ColorChoice::Auto,
      ));
      eprintln!("{}", error);
      std::process::exit(1);
    }
  }
}

async fn run() -> Result<()> {
  let mut environment = Environment::production()?;
  let server = Server::setup(&mut environment).await?;
  server.run().await
}
