use http::uri::Authority;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
pub(crate) struct Arguments {
  #[structopt(
    long,
    default_value = "0.0.0.0",
    help = "Listen on <address> for incoming requests"
  )]
  pub(crate) address: String,
  #[structopt(long, help = "Serve files from <directory>")]
  pub(crate) directory: PathBuf,
  #[structopt(long, help = "Listen on <port> for incoming HTTP requests")]
  pub(crate) port: u16,
  #[structopt(
    long,
    help = "Connect to LND gRPC server with host and port <lnd-rpc-authority>. By default a locally running LND instance will expose its gRPC API on `localhost:10009`."
  )]
  pub(crate) lnd_rpc_authority: Option<Authority>,
  #[structopt(
    long,
    help = "Read LND's TLS certificate from <lnd-rpc-cert-path>. Needed if LND uses a self-signed certificate. By default LND writes its TLS certificate to `~/.lnd/tls.cert`.",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_cert_path: Option<PathBuf>,
  #[structopt(
    long,
    help = "Read LND gRPC macaroon from <lnd-rpc-macaroon-path>. Needed if LND requires macaroon authentication. The macaroon must include permissions for creating and querying invoices. By default LND writes its invoice macaroon to `~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon`.",
    requires = "lnd-rpc-authority"
  )]
  pub(crate) lnd_rpc_macaroon_path: Option<PathBuf>,
}
