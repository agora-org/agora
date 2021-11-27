use crate::common::*;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod arguments;
mod common;
mod config;
mod display_size;
mod environment;
mod error;
mod error_page;
mod file_stream;
mod files;
mod html;
mod https_redirect_service;
mod https_request_handler;
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
  env_logger::init();
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
