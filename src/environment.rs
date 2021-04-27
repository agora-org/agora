use crate::stderr::Stderr;
use anyhow::Result;
use std::{env, ffi::OsString, path::PathBuf};

#[cfg(test)]
use tempfile::TempDir;

pub(crate) struct Environment {
  pub(crate) arguments: Vec<OsString>,
  pub(crate) working_directory: PathBuf,
  pub(crate) stderr: Stderr,
  #[cfg(test)]
  pub(crate) tempdir: TempDir,
}

impl Environment {
  pub(crate) fn production() -> Result<Self> {
    Ok(Environment {
      arguments: env::args_os().into_iter().collect(),
      stderr: Stderr::production(),
      working_directory: env::current_dir()?,
      #[cfg(test)]
      tempdir: TempDir::new().unwrap(),
    })
  }

  pub(crate) fn test() -> Self {
    let tempdir = tempfile::Builder::new()
      .prefix("foo-test")
      .tempdir()
      .unwrap();

    Environment {
      arguments: vec!["foo".into()],
      stderr: Stderr::test(),
      working_directory: tempdir.path().to_owned(),
      tempdir,
    }
  }
}
