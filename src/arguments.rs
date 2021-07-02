use http::uri::Authority;
use std::path::PathBuf;
use structopt::StructOpt;

const DEFAULT_PORT: &str = if cfg!(test) { "0" } else { "8080" };

#[derive(StructOpt)]
pub(crate) struct Arguments {
  #[structopt(long, default_value = "0.0.0.0", help = "Address to listen on")]
  pub(crate) address: String,
  #[structopt(long, default_value = "www", help = "Directory of files to serve")]
  pub(crate) directory: PathBuf,
  #[structopt(long, default_value = DEFAULT_PORT, help = "Port to listen on")]
  pub(crate) port: u16,
  #[structopt(
    long,
    help = "Host and port of the LND gRPC server, e.g., `localhost:10009`"
  )]
  pub(crate) lnd_rpc_authority: Option<Authority>,
  #[structopt(
    long,
    help = "Path to LND's TLS certificate, e.g., `~/.lnd/tls.cert`, needed if LND is using a self-signed certificate",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_cert_path: Option<PathBuf>,
  #[structopt(
    long,
    help = "Path to an LND gPRC macaroon, e.g., `~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon`, needed if LND requires macaroon authentication",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_macaroon_path: Option<PathBuf>,
}
