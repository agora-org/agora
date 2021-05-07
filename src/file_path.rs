use crate::error::{Error, Result};
use hyper::Uri;
use mime_guess::MimeGuess;
use percent_encoding::percent_decode_str;
use std::path::{Component, Path, PathBuf};
use std::{
  borrow::Cow,
  fmt::{self, Debug, Display, Formatter},
};

#[derive(Debug, Clone)]
pub(crate) struct FilePath {
  full_path: PathBuf,
  file_path: String,
}

impl FilePath {
  pub(crate) fn new(dir: &Path, uri: &Uri) -> Result<Self> {
    Self::new_option(dir, uri).ok_or_else(|| Error::InvalidPath { uri: uri.clone() })
  }

  fn new_option(dir: &Path, uri: &Uri) -> Option<Self> {
    let file_path = Self::percent_decode(uri.path().strip_prefix('/')?)?;

    for component in Path::new(&file_path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return None,
      }
    }

    for component in file_path.split('/') {
      if component.is_empty() {
        return None;
      }
    }

    Some(Self {
      full_path: dir.join(&file_path),
      file_path,
    })
  }

  fn percent_decode(path: &str) -> Option<String> {
    percent_decode_str(path)
      .decode_utf8()
      .ok()
      .map(Cow::into_owned)
  }

  pub(crate) fn mime_guess(&self) -> MimeGuess {
    mime_guess::from_path(&self.file_path)
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
