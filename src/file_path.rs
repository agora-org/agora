use crate::error::{Error, Result};
use hyper::Uri;
use std::fmt::{self, Debug, Display, Formatter};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct FilePath {
  full_path: PathBuf,
  file_path: String,
}

impl FilePath {
  pub(crate) fn new(dir: &Path, uri: &Uri) -> Result<Self> {
    let invalid_path = || Error::InvalidPath { uri: uri.clone() };
    let file_path = uri.path().strip_prefix('/').ok_or_else(invalid_path)?;

    for component in Path::new(file_path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return Err(invalid_path()),
      }
    }

    for component in file_path.split('/') {
      if component.is_empty() {
        return Err(invalid_path());
      }
    }

    Ok(Self {
      full_path: dir.join(file_path),
      file_path: file_path.to_owned(),
    })
  }

  #[cfg(test)]
  pub(crate) fn new_unchecked(dir: &Path, inner: &str) -> Self {
    Self {
      full_path: dir.join(inner),
      file_path: inner.to_owned(),
    }
  }
}

impl AsRef<Path> for FilePath {
  fn as_ref(&self) -> &Path {
    self.full_path.as_ref()
  }
}

impl Display for FilePath {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.file_path)
  }
}
