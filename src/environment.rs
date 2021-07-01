use crate::{
  error::{self, Result},
  stderr::Stderr,
};
use http::uri::Authority;
use snafu::ResultExt;
use std::{env, ffi::OsString, path::PathBuf};
use structopt::StructOpt;

#[cfg(test)]
use tempfile::TempDir;

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
    help = "Path to LND's TLS certificate, e.g., `~/.lnd/tls.cert`, needed if LND is using a self-signed certificate"
  )]
  pub(crate) lnd_rpc_cert_path: Option<PathBuf>,
  #[structopt(
    long,
    help = "Path to an LND gPRC macaroon, e.g., `~/.lnd/data/chain/bitcoin/mainnet/invoice.macaroon`, needed if LND requires macaroon authentication"
  )]
  pub(crate) lnd_rpc_macaroon_path: Option<PathBuf>,
}

pub(crate) struct Environment {
  pub(crate) arguments: Vec<OsString>,
  pub(crate) working_directory: PathBuf,
  pub(crate) stderr: Stderr,
  #[cfg(test)]
  _working_directory_tempdir: TempDir,
}

impl Environment {
  pub(crate) fn production() -> Result<Self> {
    Ok(Environment {
      arguments: env::args_os().into_iter().collect(),
      stderr: Stderr::production(),
      working_directory: env::current_dir().context(error::CurrentDir)?,
      #[cfg(test)]
      _working_directory_tempdir: TempDir::new().unwrap(),
    })
  }

  #[cfg(test)]
  pub(crate) fn test(arguments: &[&str]) -> Self {
    let tempdir = tempfile::Builder::new()
      .prefix("agora-test")
      .tempdir()
      .unwrap();

    Environment {
      arguments: ["agora", "--address", "localhost"]
        .iter()
        .chain(arguments)
        .map(OsString::from)
        .collect(),
      stderr: Stderr::test(),
      working_directory: tempdir.path().to_owned(),
      _working_directory_tempdir: tempdir,
    }
  }

  pub(crate) fn arguments(&self) -> Result<Arguments> {
    Ok(Arguments::from_iter_safe(&self.arguments)?)
  }
}
