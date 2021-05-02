use crate::error::{Error, Result};
use hyper::Uri;
use std::fmt::{self, Debug, Display, Formatter};
use std::path::{Component, Path};

#[derive(Debug, Clone)]
pub(crate) struct FilePath {
  inner: String,
}

impl FilePath {
  pub(crate) fn from_uri(uri: &Uri) -> Result<Self> {
    let invalid_path = || Error::InvalidPath { uri: uri.clone() };
    let path = uri.path().strip_prefix('/').ok_or_else(invalid_path)?;

    for component in Path::new(path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return Err(invalid_path()),
      }
    }

    for component in path.split('/') {
      if component.is_empty() {
        return Err(invalid_path());
      }
    }

    Ok(Self {
      inner: path.to_owned(),
    })
  }

  #[cfg(test)]
  pub(crate) fn new(inner: &str) -> Self {
    Self {
      inner: inner.to_owned(),
    }
  }
}

impl AsRef<Path> for FilePath {
  fn as_ref(&self) -> &Path {
    self.inner.as_ref()
  }
}

impl Display for FilePath {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.inner)
  }
}
