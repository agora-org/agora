use {
  crate::common::*,
  color_backtrace::BacktracePrinter,
  snafu::{ErrorCompat, Snafu},
  std::{path::MAIN_SEPARATOR, str::Utf8Error},
  structopt::clap,
  termcolor::WriteColor,
  tokio::task::JoinError,
};

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum Error {
  #[snafu(display("Failed to resolve `{}` to an IP address: {}", input, source))]
  AddressResolutionIo {
    backtrace: Backtrace,
    input: String,
    source: io::Error,
  },
  #[snafu(display("`{}` did not resolve to an IP address", input))]
  AddressResolutionNoAddresses { input: String, backtrace: Backtrace },
  #[snafu(context(false), display("{}", source))]
  Clap {
    backtrace: Backtrace,
    source: clap::Error,
  },
  #[snafu(display("Failed to deserialize config file at `{}`: {}", path.display(), source))]
  ConfigDeserialize {
    backtrace: Backtrace,
    path: PathBuf,
    source: serde_yaml::Error,
  },
  #[snafu(display("Missing base price for paid file `{}`", path.display()))]
  ConfigMissingBasePrice { path: PathBuf, backtrace: Backtrace },
  #[snafu(display("Failed to retrieve current directory: {}", source))]
  CurrentDir {
    backtrace: Backtrace,
    source: io::Error,
  },
  #[snafu(display("{}", message))]
  Custom {
    backtrace: Backtrace,
    status_code: StatusCode,
    message: String,
  },
  #[snafu(display("IO error accessing filesystem at `{}`: {}", path.display(), source))]
  FilesystemIo {
    backtrace: Backtrace,
    path: PathBuf,
    source: io::Error,
  },
  #[snafu(display("Forbidden access to hidden file: {}", path.display()))]
  HiddenFileAccess { backtrace: Backtrace, path: PathBuf },
  #[snafu(display(
    "Internal error, this is probably a bug in agora: {}\n\
      Consider filing an issue: https://github.com/soenkehahn/agora/issues/new/",
    message
  ))]
  Internal {
    backtrace: Backtrace,
    message: String,
  },
  #[snafu(display("Invalid URI file path: {}", uri_path))]
  InvalidFilePath {
    backtrace: Backtrace,
    uri_path: String,
  },
  #[snafu(display("Invalid URI path: {}", uri_path))]
  InvalidUriPath {
    backtrace: Backtrace,
    source: Utf8Error,
    uri_path: String,
  },
  #[snafu(display("Invalid invoice ID: {}", source))]
  InvoiceId {
    backtrace: Backtrace,
    source: hex::FromHexError,
  },
  #[snafu(display("Invoice not found: {}", hex::encode(r_hash)))]
  InvoiceNotFound {
    backtrace: Backtrace,
    r_hash: [u8; 32],
  },
  #[snafu(display(
    "Request path `{}` did not match invoice path `{}` for invoice: {}",
    request_tail,
    invoice_tail,
    hex::encode(r_hash),
  ))]
  InvoicePathMismatch {
    backtrace: Backtrace,
    invoice_tail: String,
    r_hash: [u8; 32],
    request_tail: String,
  },
  #[snafu(display("Invoice request requires LND client configuration: {}", uri_path))]
  LndNotConfiguredInvoiceRequest {
    backtrace: Backtrace,
    uri_path: String,
  },
  #[snafu(display("Paid file request requires LND client configuration: `{}`", path.display()))]
  LndNotConfiguredPaidFileRequest { path: PathBuf, backtrace: Backtrace },
  #[snafu(display("OpenSSL error parsing LND RPC certificate: {}", source))]
  LndRpcCertificateParse {
    backtrace: Backtrace,
    source: openssl::error::ErrorStack,
  },
  #[snafu(display("OpenSSL error connecting to LND RPC server: {}", source))]
  LndRpcConnect {
    backtrace: Backtrace,
    source: openssl::error::ErrorStack,
  },
  #[snafu(display("LND RPC call failed: {}", source))]
  LndRpcStatus {
    backtrace: Backtrace,
    source: tonic::Status,
  },
  #[snafu(display(
    "Payment request `{}` too long for QR code: {}",
    payment_request,
    source
  ))]
  PaymentRequestTooLongForQrCode {
    backtrace: Backtrace,
    payment_request: String,
    source: qrcodegen::DataTooLong,
  },
  #[snafu(display("Request handler panicked: {}", source))]
  RequestHandlerPanic {
    backtrace: Backtrace,
    source: JoinError,
  },
  #[snafu(display("URI path did not match any route: {}", uri_path))]
  RouteNotFound { uri_path: String },
  #[snafu(display("Failed running HTTP server: {}", source))]
  ServerRun {
    backtrace: Backtrace,
    source: hyper::Error,
  },
  #[snafu(display("I/O error on socket address `{}`: {}", socket_addr, source))]
  SocketIo {
    backtrace: Backtrace,
    socket_addr: SocketAddr,
    source: io::Error,
  },
  #[snafu(display("Static asset not found: {}", uri_path))]
  StaticAssetNotFound {
    backtrace: Backtrace,
    uri_path: String,
  },
  #[snafu(display("IO error writing to stderr: {}", source))]
  StderrWrite {
    backtrace: Backtrace,
    source: io::Error,
  },
  #[snafu(display("Forbidden access to escaping symlink: `{}`", path.display()))]
  SymlinkAccess { backtrace: Backtrace, path: PathBuf },
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
      | SocketIo { .. }
      | StderrWrite { .. } => StatusCode::INTERNAL_SERVER_ERROR,
      Custom { status_code, .. } => *status_code,
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
      inner: if cfg!(test) || env::var_os("AGORA_SUPPRESS_BACKTRACE").is_some() {
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
