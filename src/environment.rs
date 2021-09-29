use crate::common::*;

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
  pub(crate) fn test() -> Self {
    let tempdir = tempfile::Builder::new()
      .prefix("agora-test")
      .tempdir()
      .unwrap();

    Environment {
      arguments: vec![
        "agora".into(),
        "--address=localhost".into(),
        "--http-port=0".into(),
        "--directory=www".into(),
      ],
      stderr: Stderr::test(),
      working_directory: tempdir.path().to_owned(),
      _working_directory_tempdir: tempdir,
    }
  }

  pub(crate) fn arguments(&self) -> Result<Arguments> {
    Ok(Arguments::from_iter_safe(&self.arguments)?)
  }
}
