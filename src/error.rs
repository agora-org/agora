use crate::input_path::InputPath;
use color_backtrace::BacktracePrinter;
use hyper::StatusCode;
use snafu::{ErrorCompat, Snafu};
use std::{
  fmt::Debug,
  io,
  path::{PathBuf, MAIN_SEPARATOR},
  str::Utf8Error,
};
use structopt::clap;
use termcolor::WriteColor;
use tokio::task::JoinError;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum Error {
  #[snafu(display("Failed to resolve `{}` to an IP address: {}", input, source))]
  AddressResolutionIo {
    input: String,
    source: io::Error,
    backtrace: Backtrace,
  },
  #[snafu(display("`{}` did not resolve to an IP address", input))]
  AddressResolutionNoAddresses { input: String, backtrace: Backtrace },
  #[snafu(context(false), display("{}", source))]
  Clap {
    source: clap::Error,
    backtrace: Backtrace,
  },
  #[snafu(display("Failed to deserialize config file at `{}`: {}", path.display(), source))]
  ConfigDeserialize {
    source: serde_yaml::Error,
    path: PathBuf,
    backtrace: Backtrace,
  },
  #[snafu(display("Missing base price for paid file `{}`", path.display()))]
  ConfigMissingBasePrice { path: PathBuf, backtrace: Backtrace },
  #[snafu(display("Failed to retrieve current directory: {}", source))]
  CurrentDir {
    source: io::Error,
    backtrace: Backtrace,
  },
  #[snafu(display("IO error accessing filesystem at `{}`: {}", path.display(), source))]
  FilesystemIo {
    source: io::Error,
    path: PathBuf,
    backtrace: Backtrace,
  },
  #[snafu(display("Forbidden access to hidden file: {}", path.display()))]
  HiddenFileAccess { path: PathBuf, backtrace: Backtrace },
  #[snafu(display(
    "Internal error, this is probably a bug in agora: {}\n\
      Consider filing an issue: https://github.com/soenkehahn/agora/issues/new/",
    message
  ))]
  Internal {
    message: String,
    backtrace: Backtrace,
  },
  #[snafu(display("Invalid URI file path: {}", uri_path))]
  InvalidFilePath {
    uri_path: String,
    backtrace: Backtrace,
  },
  #[snafu(display("Invalid URI path: {}", uri_path))]
  InvalidUriPath {
    source: Utf8Error,
    uri_path: String,
    backtrace: Backtrace,
  },
  #[snafu(display("Invalid invoice ID: {}", source))]
  InvoiceId {
    source: hex::FromHexError,
    backtrace: Backtrace,
  },
  #[snafu(display("Invoice not found: {}", hex::encode(r_hash)))]
  InvoiceNotFound {
    r_hash: [u8; 32],
    backtrace: Backtrace,
  },
  #[snafu(display(
    "Request path `{}` did not match invoice path `{}` for invoice: {}",
    request_tail,
    invoice_tail,
    hex::encode(r_hash),
  ))]
  InvoicePathMismatch {
    invoice_tail: String,
    r_hash: [u8; 32],
    request_tail: String,
    backtrace: Backtrace,
  },
  #[snafu(display("Invoice request requires LND client configuration: {}", uri_path))]
  LndNotConfiguredInvoiceRequest {
    uri_path: String,
    backtrace: Backtrace,
  },
  #[snafu(display("Paid file request requires LND client configuration: `{}`", path.display()))]
  LndNotConfiguredPaidFileRequest { path: PathBuf, backtrace: Backtrace },
  #[snafu(display("OpenSSL error parsing LND RPC certificate: {}", source))]
  LndRpcCertificateParse {
    source: openssl::error::ErrorStack,
    backtrace: Backtrace,
  },
  #[snafu(display("OpenSSL error connecting to LND RPC server: {}", source))]
  LndRpcConnect {
    source: openssl::error::ErrorStack,
    backtrace: Backtrace,
  },
  #[snafu(display("LND RPC call failed: {}", source))]
  LndRpcStatus {
    source: tonic::Status,
    backtrace: Backtrace,
  },
  #[snafu(display(
    "Payment request `{}` too long for QR code: {}",
    payment_request,
    source
  ))]
  PaymentRequestTooLongForQrCode {
    payment_request: String,
    source: qrcodegen::DataTooLong,
  },
  #[snafu(display("Request handler panicked: {}", source))]
  RequestHandlerPanic {
    source: JoinError,
    backtrace: Backtrace,
  },
  #[snafu(display("URI path did not match any route: {}", uri_path))]
  RouteNotFound { uri_path: String },
  #[snafu(display("Failed running HTTP server: {}", source))]
  ServerRun {
    source: hyper::Error,
    backtrace: Backtrace,
  },
  #[snafu(display("Static asset not found: {}", uri_path))]
  StaticAssetNotFound {
    uri_path: String,
    backtrace: Backtrace,
  },
  #[snafu(display("IO error writing to stderr: {}", source))]
  StderrWrite {
    source: io::Error,
    backtrace: Backtrace,
  },
  #[snafu(display("Forbidden access to escaping symlink: `{}`", path.display()))]
  SymlinkAccess { path: PathBuf, backtrace: Backtrace },
}

impl Error {
  pub(crate) fn status(&self) -> StatusCode {
    use Error::*;
    match self {
      FilesystemIo { source, .. } if source.kind() == io::ErrorKind::NotFound => {
        StatusCode::NOT_FOUND
      }
      InvalidFilePath { .. }
      | InvalidUriPath { .. }
      | InvoiceId { .. }
      | InvoicePathMismatch { .. } => StatusCode::BAD_REQUEST,
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
      | ConfigMissingBasePrice { .. }
      | CurrentDir { .. }
      | FilesystemIo { .. }
      | Internal { .. }
      | LndNotConfiguredPaidFileRequest { .. }
      | LndRpcCertificateParse { .. }
      | LndRpcConnect { .. }
      | LndRpcStatus { .. }
      | PaymentRequestTooLongForQrCode { .. }
      | RequestHandlerPanic { .. }
      | ServerRun { .. }
      | StderrWrite { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  pub(crate) fn internal(message: impl Into<String>) -> Self {
    Internal { message }.build()
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
              Some(file) => {
                !(file.starts_with("/rustc/")
                  || file.contains(&format!(
                    "{}.cargo{}registry{}",
                    MAIN_SEPARATOR, MAIN_SEPARATOR, MAIN_SEPARATOR
                  )))
              }
              None => false,
            },
          );
        }))
        .print_trace(backtrace, write_color)
        .ok();
    }
  }
}

#[derive(Debug)]
pub(crate) struct Backtrace {
  inner: Option<snafu::Backtrace>,
}

impl snafu::GenerateBacktrace for Backtrace {
  fn generate() -> Self {
    Self {
      inner: if cfg!(test) {
        None
      } else {
        Some(snafu::Backtrace::generate())
      },
    }
  }

  fn as_backtrace(&self) -> Option<&snafu::Backtrace> {
    self.inner.as_ref()
  }
}
