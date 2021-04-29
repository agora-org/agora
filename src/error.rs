use hyper::{StatusCode, Uri};
use snafu::Snafu;
use std::{fmt::Debug, io};
use structopt::clap;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub(crate) enum Error {
  #[snafu(
    context(false),
    display("Failed to parse command line arguments: {}", source)
  )]
  Clap { source: clap::Error },
  #[snafu(display("IO error accessing `www`: {}", source))]
  WwwIo { source: io::Error },
  #[snafu(display("IO error accessing file: {}", source))]
  FileIo { source: io::Error },
  #[snafu(display("Invalid URL file path: {}", uri))]
  InvalidPath { uri: Uri },
  #[snafu(display("Failed to retrieve current directory: {}", source))]
  CurrentDir { source: io::Error },
  #[snafu(display("Failed running HTTP server: {}", source))]
  ServerRun { source: hyper::Error },
}

impl Error {
  pub(crate) fn status(&self) -> StatusCode {
    use Error::*;
    match self {
      Clap { .. } | WwwIo { .. } | FileIo { .. } | CurrentDir { .. } | ServerRun { .. } => {
        StatusCode::INTERNAL_SERVER_ERROR
      }
      InvalidPath { .. } => StatusCode::BAD_REQUEST,
    }
  }
}
