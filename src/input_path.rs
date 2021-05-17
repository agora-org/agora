use crate::{
  environment::Environment,
  error::{Error, Result},
};
use hyper::Uri;
use mime_guess::MimeGuess;
use percent_encoding::percent_decode_str;
use std::path::{Component, Path, PathBuf};
use std::{borrow::Cow, fmt::Debug};

#[derive(Debug, Clone)]
pub(crate) struct InputPath {
  full_path: PathBuf,
  display_path: PathBuf,
}

impl InputPath {
  pub(crate) fn new(environment: &Environment, display_path: &Path) -> Self {
    Self {
      full_path: environment.working_directory.join(display_path),
      display_path: display_path.to_owned(),
    }
  }

  pub(crate) fn join(&self, path: &Path) -> Self {
    Self {
      full_path: self.full_path.join(path),
      display_path: self.display_path.join(path),
    }
  }

  pub(crate) fn join_uri(&self, uri: &Uri) -> Result<Self> {
    self
      .join_uri_option(uri)
      .ok_or_else(|| Error::InvalidPath { uri: uri.clone() })
  }

  fn join_uri_option(&self, uri: &Uri) -> Option<Self> {
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
      full_path: self.full_path.join(&relative_path),
      display_path: self.display_path.join(&relative_path),
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

impl AsRef<Path> for InputPath {
  fn as_ref(&self) -> &Path {
    self.full_path.as_ref()
  }
}
