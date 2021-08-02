use crate::input_path::InputPath;
use color_backtrace::BacktracePrinter;
use hyper::StatusCode;
use snafu::{Backtrace, ErrorCompat, Snafu};
use std::{fmt::Debug, io, path::PathBuf};
use structopt::clap;
use termcolor::WriteColor;
use tokio::task::JoinError;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
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
  #[snafu(display("Failed to deserialize config file at `{}`: {}", path.display(), source))]
  ConfigDeserialize {
    source: serde_yaml::Error,
    path: PathBuf,
  },
  #[snafu(display("Failed to retrieve current directory: {}", source))]
  CurrentDir { source: io::Error },
  #[snafu(display("IO error accessing filesystem at `{}`: {}", path.display(), source))]
  FilesystemIo { source: io::Error, path: PathBuf },
  #[snafu(display("Forbidden access to hidden file: {}", path.display()))]
  HiddenFileAccess { path: PathBuf, backtrace: Backtrace },
  #[snafu(display(
    "Internal error, this is probably a bug in agora: {}\n\
      Consider filing an issue: https://github.com/soenkehahn/agora/issues/new/",
    message
  ))]
  Internal { message: String },
  #[snafu(display("Invalid URI file path: {}", uri_path))]
  InvalidFilePath { uri_path: String },
  #[snafu(display("Invalid invoice ID: {}", source))]
  InvoiceId { source: hex::FromHexError },
  #[snafu(display("Invoice not found: {}", hex::encode(r_hash)))]
  InvoiceNotFound { r_hash: [u8; 32] },
  #[snafu(display("Invoice request requires LND client configuration: {}", uri_path))]
  LndNotConfiguredInvoiceRequest { uri_path: String },
  #[snafu(display("Paid file request requires LND client configuration: `{}`", path.display()))]
  LndNotConfiguredPaidFileRequest { path: PathBuf },
  #[snafu(display("OpenSSL error parsing LND RPC certificate: {}", source))]
  LndRpcCertificateParse { source: openssl::error::ErrorStack },
  #[snafu(display("OpenSSL error connecting to LND RPC server: {}", source))]
  LndRpcConnect { source: openssl::error::ErrorStack },
  #[snafu(display("LND RPC call failed: {}", source))]
  LndRpcStatus { source: tonic::Status },
  #[snafu(display("Request handler panicked: {}", source))]
  RequestHandlerPanic { source: JoinError },
  #[snafu(display("URI path did not match any route: {}", uri_path))]
  RouteNotFound { uri_path: String },
  #[snafu(display("Failed running HTTP server: {}", source))]
  ServerRun { source: hyper::Error },
  #[snafu(display("Static asset not found: {}", uri_path))]
  StaticAssetNotFound { uri_path: String },
  #[snafu(display("IO error writing to stderr: {}", source))]
  StderrWrite { source: io::Error },
  #[snafu(display("Forbidden access to symlink: {}", path.display()))]
  SymlinkAccess { path: PathBuf },
}

impl Error {
  pub(crate) fn status(&self) -> StatusCode {
    use Error::*;
    match self {
      FilesystemIo { source, .. } if source.kind() == io::ErrorKind::NotFound => {
        StatusCode::NOT_FOUND
      }
      InvalidFilePath { .. } | InvoiceId { .. } => StatusCode::BAD_REQUEST,
      HiddenFileAccess { .. }
      | InvoiceNotFound { .. }
      | LndNotConfiguredInvoiceRequest { .. }
      | RouteNotFound { .. }
      | StaticAssetNotFound { .. }
      | SymlinkAccess { .. } => StatusCode::NOT_FOUND,
      AddressResolutionIo { .. }
      | AddressResolutionNoAddresses { .. }
      | Clap { .. }
      | ConfigDeserialize { .. }
      | CurrentDir { .. }
      | FilesystemIo { .. }
      | Internal { .. }
      | LndNotConfiguredPaidFileRequest { .. }
      | LndRpcCertificateParse { .. }
      | LndRpcConnect { .. }
      | LndRpcStatus { .. }
      | RequestHandlerPanic { .. }
      | ServerRun { .. }
      | StderrWrite { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  pub(crate) fn internal(message: impl Into<String>) -> Self {
    Self::Internal {
      message: message.into(),
    }
  }

  pub(crate) fn filesystem_io(file_path: &InputPath) -> FilesystemIo<PathBuf> {
    FilesystemIo {
      path: file_path.display_path().to_owned(),
    }
  }

  pub(crate) fn print_backtrace(&self, write_color: &mut impl WriteColor) {
    if let Some(backtrace) = ErrorCompat::backtrace(self) {
      BacktracePrinter::new()
        .add_frame_filter(Box::new(|frames| {
          frames.retain(
            |frame| match frame.filename.as_ref().and_then(|x| x.to_str()) {
              Some(file) => !(file.starts_with("/rustc/") || file.contains("/.cargo/registry/")),
              None => false,
            },
          );
        }))
        .print_trace(backtrace, write_color)
        .ok();
    }
  }
}
