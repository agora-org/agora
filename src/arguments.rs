use http::uri::Authority;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
pub(crate) struct Arguments {
  #[structopt(
    long,
    help = "Cache directory for TLS certificates fetched via ACME protocol. Let's Encrypt is the only supported ACME provider."
  )]
  pub(crate) acme_cache_directory: Option<PathBuf>,
  #[structopt(long, default_value = "0.0.0.0", help = "Address to listen on")]
  pub(crate) address: String,
  #[structopt(long, help = "Directory of files to serve")]
  pub(crate) directory: PathBuf,
  // fixme: make this optional
  #[structopt(long, help = "Port to listen on for incoming HTTP requests")]
  pub(crate) http_port: u16,
  #[structopt(long, help = "Port to listen on for incoming HTTPS requests")]
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
