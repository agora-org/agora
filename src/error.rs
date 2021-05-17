use crate::file_path::FilePath;
use hyper::{StatusCode, Uri};
use snafu::Snafu;
use std::{
  fmt::Debug,
  io,
  path::{Path, PathBuf},
};
use structopt::clap;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility = "pub(crate)")]
pub(crate) enum Error {
  #[snafu(display("Failed to resolve `{}` to an IP address: {}", input, source))]
  AddressResolutionIo { input: String, source: io::Error },
  #[snafu(display("`{}` did not resolve to an IP address", input))]
  AddressResolutionNoAddresses { input: String },
  #[snafu(
    context(false),
    display("Failed to parse command line arguments: {}", source)
  )]
  Clap { source: clap::Error },
  #[snafu(display("Failed to retrieve current directory: {}", source))]
  CurrentDir { source: io::Error },
  #[snafu(display("IO error accessing filesystem at `{}`: {}", path.display(), source))]
  FilesystemIo { source: io::Error, path: PathBuf },
  #[snafu(display(
    "Internal error, this is probably a bug in foo: {}\n\
      Consider filing an issue: https://github.com/soenkehahn/foo/issues/new/",
    message
  ))]
  Internal { message: String },
  #[snafu(display("Invalid URL file path: {}", uri))]
  InvalidPath { uri: Uri },
  #[snafu(display("Failed running HTTP server: {}", source))]
  ServerRun { source: hyper::Error },
}

impl Error {
  pub(crate) fn status(&self) -> StatusCode {
    use Error::*;
    match self {
      FilesystemIo { source, .. } if source.kind() == io::ErrorKind::NotFound => {
        StatusCode::NOT_FOUND
      }
      InvalidPath { .. } => StatusCode::BAD_REQUEST,
      AddressResolutionIo { .. }
      | AddressResolutionNoAddresses { .. }
      | Clap { .. }
      | CurrentDir { .. }
      | FilesystemIo { .. }
      | Internal { .. }
      | ServerRun { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  pub(crate) fn internal(message: impl Into<String>) -> Self {
    Self::Internal {
      message: message.into(),
    }
  }

  pub(crate) fn filesystem_io(file_path: &FilePath) -> FilesystemIo<&Path> {
    FilesystemIo {
      path: file_path.display_path(),
    }
  }
}
