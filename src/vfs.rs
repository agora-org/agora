use crate::common::*;

#[derive(Debug, Clone)]
pub(crate) struct Vfs {
  base_directory: InputPath,
}

impl Vfs {
  pub(crate) fn new(base_directory: InputPath) -> Self {
    Self { base_directory }
  }

  pub(crate) fn paid(&self, path: &InputPath) -> bool {
    let config = Config::for_dir(
      self.base_directory.as_ref(),
      path.as_ref().parent().unwrap(),
    )
    .unwrap();
    config.paid()
  }

    pub(crate) fn base_price(&self, path: &InputPath) -> Option<Millisatoshi> {
    let config = Config::for_dir(
      self.base_directory.as_ref(),
      path.as_ref().parent().unwrap(),
    )
    .unwrap();

        config.base_price
    }

    pub(crate) fn file_type(&self, tail: &[&str]) -> Result<FileType> {

        for result in self.base_directory.iter_prefixes(tail) {
            let prefix = result?;
            self.check_path(&prefix)?;
        }

        let file_path = self.base_directory.join_file_path(&tail.join(""))?;
        let file_type = file_path
            .as_ref()
            .metadata()
            .with_context(|| Error::filesystem_io(&file_path))?
            .file_type();
        Ok(file_type)
    }

  fn check_path(&self, path: &InputPath) -> Result<()> {
    if path
      .as_ref()
      .symlink_metadata()
      .with_context(|| Error::filesystem_io(path))?
      .file_type()
      .is_symlink()
    {
      let link = fs::read_link(path.as_ref()).with_context(|| Error::filesystem_io(path))?;

      let destination = path
        .as_ref()
        .parent()
        .expect("Input paths are always absolute, and thus have parents or are `/`, and `/` cannot be a symlink.")
        .join(link)
        .lexiclean();

      if !destination.starts_with(&self.base_directory) {
        return Err(
          error::SymlinkAccess {
            path: path.display_path().to_owned(),
          }
          .build(),
        );
      }
    }

    if path
      .as_ref()
      .file_name()
      .map(|file_name| file_name.to_string_lossy().starts_with('.'))
      .unwrap_or(false)
    {
      return Err(
        error::HiddenFileAccess {
          path: path.as_ref().to_owned(),
        }
        .build(),
      );
    }

    Ok(())

  }
}
