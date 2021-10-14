use crate::common::*;
use mime_guess::MimeGuess;
use std::path::Component;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InputPath {
  full_path: PathBuf,
  display_path: PathBuf,
}

impl InputPath {
  pub(crate) fn new(environment: &Environment, display_path: &Path) -> Self {
    Self {
      full_path: environment.working_directory.join(display_path).lexiclean(),
      display_path: display_path.lexiclean(),
    }
  }

  pub(crate) fn join_relative(&self, path: &Path) -> Result<Self> {
    if path.is_absolute() {
      return Err(Error::internal(format!(
        "join_relative: {} is absolute",
        path.display()
      )));
    }
    Ok(Self {
      full_path: self.full_path.join(path).lexiclean(),
      display_path: self.display_path.join(path).lexiclean(),
    })
  }

  pub(crate) fn join_file_path(&self, uri_path: &str) -> Result<Self> {
    self
      .join_file_path_option(uri_path)
      .transpose()?
      .ok_or_else(|| {
        error::InvalidFilePath {
          uri_path: uri_path.to_owned(),
        }
        .build()
      })
  }

  fn join_file_path_option(&self, uri_path: &str) -> Option<Result<Self>> {
    for component in Path::new(&uri_path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return None,
      }
    }

    if uri_path.contains("//") {
      return None;
    }

    Some(self.join_relative(Path::new(&uri_path)))
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

  pub(crate) fn iter_prefixes<'a>(
    &'a self,
    tail: &'a [&str],
  ) -> impl Iterator<Item = Result<InputPath>> + 'a {
    (0..tail.len()).map(move |i| self.join_file_path(&tail[..i + 1].join("")))
  }
}

impl AsRef<Path> for InputPath {
  fn as_ref(&self) -> &Path {
    &self.full_path
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn new_removes_trailing_slashes() {
    let environment = Environment::test();
    let input_path = InputPath::new(&environment, Path::new("foo/"));
    assert_eq!(
      input_path,
      InputPath {
        full_path: environment.working_directory.join("foo"),
        display_path: "foo".into()
      }
    );
  }

  #[test]
  fn join_relative_removes_trailing_slashes() {
    let environment = Environment::test();
    let base = InputPath::new(&environment, Path::new("foo"));
    let input_path = base.join_relative(Path::new("bar/")).unwrap();
    assert_eq!(
      input_path,
      InputPath {
        full_path: environment.working_directory.join("foo").join("bar"),
        display_path: Path::new("foo").join("bar")
      }
    );
  }

  #[test]
  fn join_file_path_removes_trailing_slashes() {
    let environment = Environment::test();
    let base = InputPath::new(&environment, Path::new("foo"));
    let input_path = base.join_file_path("bar/").unwrap();
    assert_eq!(
      input_path,
      InputPath {
        full_path: environment.working_directory.join("foo").join("bar"),
        display_path: Path::new("foo").join("bar")
      }
    );
  }

  #[test]
  fn iter_prefixes_iterates_from_base_dir_to_file() {
    let environment = Environment::test();
    let base = InputPath::new(&environment, Path::new("www"));
    let dirs: Result<Vec<InputPath>> = base.iter_prefixes(&["foo/", "bar/", "baz"]).collect();
    assert_eq!(
      dirs.unwrap(),
      ["foo", "foo/bar", "foo/bar/baz"]
        .iter()
        .map(|x| base.join_file_path(x).unwrap())
        .collect::<Vec<_>>()
    );
  }

  #[test]
  fn iter_prefixes_for_empty_inputs() {
    let environment = Environment::test();
    let base = InputPath::new(&environment, Path::new("www"));
    let mut dirs = base.iter_prefixes(&[]);
    assert!(dirs.next().is_none());
  }
}
