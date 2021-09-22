use http::uri::Authority;
use std::path::PathBuf;
use structopt::{
  clap::{AppSettings, ArgGroup},
  StructOpt,
};

#[derive(Debug, StructOpt)]
#[structopt(
  group = ArgGroup::with_name("port").multiple(true).required(true),
  settings = if cfg!(test) { &[AppSettings::ColorNever] } else { &[] })
]
pub(crate) struct Arguments {
  #[structopt(
    long,
    help = "Cache directory for TLS certificates fetched via ACME protocol. Let's Encrypt is the only supported ACME provider."
  )]
  pub(crate) acme_cache_directory: Option<PathBuf>,
  #[structopt(
    long,
    help = "Request TLS certificate for <acme-domain>. This agora instance must be reachable at <acme-domain>:443 to respond to ACME challenges."
  )]
  pub(crate) acme_domain: Vec<String>,
  #[structopt(long, default_value = "0.0.0.0", help = "Address to listen on")]
  pub(crate) address: String,
  #[structopt(long, help = "Directory of files to serve")]
  pub(crate) directory: PathBuf,
  #[structopt(
    long,
    group = "port",
    help = "Port to listen on for incoming HTTP requests"
  )]
  pub(crate) http_port: Option<u16>,
  #[structopt(
    long,
    group = "port",
    help = "Port to listen on for incoming HTTPS requests",
    requires_all = &["acme-cache-directory", "acme-domain"]
  )]
  pub(crate) https_port: Option<u16>,
  #[structopt(
    long,
    help = "Port to redirect to HTTPS through HTTP",
    requires = "https-port"
  )]
  pub(crate) https_redirect_port: Option<u16>,
  #[structopt(
    long,
    help = "Host and port of the LND gRPC server. By default a locally running LND instance will expose its gRPC API on `localhost:10009`."
  )]
  pub(crate) lnd_rpc_authority: Option<Authority>,
  #[structopt(
    long,
    help = "Path to LND's TLS certificate. Needed if LND uses a self-signed certificate. By default LND writes its TLS certificate to `~/.lnd/tls.cert`.",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_cert_path: Option<PathBuf>,
  #[structopt(
    long,
    help = "Path to an LND gRPC macaroon, that allows creating and querying invoices. Needed if LND requires macaroon authentication. By default LND writes its invoice macaroon to `~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon`.",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_macaroon_path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::assert_contains;
  use unindent::Unindent;

  #[test]
  fn https_redirect_port_requires_https_port() {
    assert_contains(
      &Arguments::from_iter_safe(&["agora", "--directory=www", "--https-redirect-port=0"])
        .unwrap_err()
        .to_string(),
      &"
        The following required arguments were not provided:
            <--http-port <http-port>|--https-port <https-port>>
      "
      .unindent(),
    );
  }

  #[test]
  fn https_port_requires_acme_cache_directory() {
    assert_contains(
      &Arguments::from_iter_safe(&["agora", "--directory=www", "--https-port=0"])
        .unwrap_err()
        .to_string(),
      &"
        The following required arguments were not provided:
            --acme-cache-directory <acme-cache-directory>
      "
      .unindent(),
    );
  }

  #[test]
  fn https_port_requires_acme_domain() {
    assert_contains(
      &Arguments::from_iter_safe(&[
        "agora",
        "--directory=www",
        "--https-port=0",
        "--acme-cache-directory=cache",
      ])
      .unwrap_err()
      .to_string(),
      &"
        The following required arguments were not provided:
            --acme-domain <acme-domain>...
      "
      .unindent(),
    );
  }

  #[test]
  fn require_at_least_one_port_argument() {
    assert_contains(
      &Arguments::from_iter_safe(&["agora", "--directory=www"])
        .unwrap_err()
        .to_string(),
      &"
        The following required arguments were not provided:
            <--http-port <http-port>|--https-port <https-port>>
      "
      .unindent(),
    );
  }
}
