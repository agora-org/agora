use {
  crate::common::*,
  termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor},
};

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
mod vfs;

#[tokio::main]
async fn main() {
  env_logger::init();
  if let Err(error) = run().await {
    if let crate::error::Error::Clap { source, .. } = error {
      source.exit();
    } else {
      let mut stderr = StandardStream::stderr(ColorChoice::Auto);

      error.print_backtrace(&mut stderr);

      stderr
        .set_color(ColorSpec::new().set_fg(Some(Color::Red)))
        .ok();
      write!(&mut stderr, "error").ok();

      stderr.set_color(ColorSpec::new().set_bold(true)).ok();
      writeln!(&mut stderr, ": {}", error).ok();

      std::process::exit(1);
    }
  }
}

async fn run() -> Result<()> {
  let mut environment = Environment::production()?;
  let server = Server::setup(&mut environment).await?;
  server.run().await
}
