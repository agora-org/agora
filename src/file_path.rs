use crate::error::{Error, Result};
use hyper::Uri;
use mime_guess::MimeGuess;
use percent_encoding::percent_decode_str;
use std::path::{Component, Path, PathBuf};
use std::{borrow::Cow, fmt::Debug};

#[derive(Debug, Clone)]
pub(crate) struct FilePath {
  full_path: PathBuf,
  display_path: PathBuf,
}

impl FilePath {
  pub(crate) fn new(base_directory: &Path, dir: &Path, uri: &Uri) -> Result<Self> {
    Self::new_option(base_directory, dir, uri)
      .ok_or_else(|| Error::InvalidPath { uri: uri.clone() })
  }

  fn new_option(base_directory: &Path, dir: &Path, uri: &Uri) -> Option<Self> {
    let relative_path = Self::percent_decode(uri.path().strip_prefix('/')?)?;

    for component in Path::new(&relative_path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return None,
      }
    }

    if relative_path.contains("//") {
      return None;
    }

    Some(Self {
      full_path: dir.join(&relative_path),
      display_path: base_directory.join(&relative_path),
    })
  }

  fn percent_decode(path: &str) -> Option<String> {
    percent_decode_str(path)
      .decode_utf8()
      .ok()
      .map(Cow::into_owned)
  }

  pub(crate) fn display_path(&self) -> &Path {
    &self.display_path
  }

  pub(crate) fn mime_guess(&self) -> MimeGuess {
    mime_guess::from_path(&self.display_path)
  }

  #[cfg(test)]
  pub(crate) fn new_unchecked(dir: &Path, inner: &str) -> Self {
    Self {
      full_path: dir.join(inner),
      display_path: Path::new("www").join(inner),
    }
  }
}

impl AsRef<Path> for FilePath {
  fn as_ref(&self) -> &Path {
    self.full_path.as_ref()
  }
}
