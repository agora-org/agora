use crate::stderr::Stderr;
use anyhow::Result;
use std::{env, ffi::OsString, path::PathBuf};
use structopt::StructOpt;

#[cfg(test)]
use tempfile::TempDir;

const DEFAULT_PORT: &str = if cfg!(test) { "0" } else { "8080" };

#[derive(StructOpt)]
pub(crate) struct Arguments {
  #[structopt(long, default_value = DEFAULT_PORT)]
  pub(crate) port: u16,
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
      working_directory: env::current_dir()?,
      #[cfg(test)]
      _working_directory_tempdir: TempDir::new().unwrap(),
    })
  }

  #[cfg(test)]
  pub(crate) fn test() -> Self {
    let tempdir = tempfile::Builder::new()
      .prefix("foo-test")
      .tempdir()
      .unwrap();

    Environment {
      arguments: vec!["foo".into()],
      stderr: Stderr::test(),
      working_directory: tempdir.path().to_owned(),
      _working_directory_tempdir: tempdir,
    }
  }

  pub(crate) fn arguments(&self) -> Result<Arguments> {
    Ok(Arguments::from_iter_safe(&self.arguments)?)
  }
}
