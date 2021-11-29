use crate::common::*;

#[derive(Debug, Clone)]
pub(crate) struct Vfs {
  base_directory: InputPath,
}

impl Vfs {
  pub(crate) fn new(base_directory: InputPath) -> Self {
    Self { base_directory }
  }

  /// If an `.index.md` file exists in this directory, return its contents as a string.
  pub(crate) fn directory_markdown_file(&self, dir_path: &InputPath) -> Result<Option<String>> {
    let file = dir_path.join_relative(".index.md".as_ref())?;
    match fs::read_to_string(&file) {
      Ok(markdown) => Ok(Some(markdown)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(source) => return Err(Error::filesystem_io(&file).into_error(source)),
    }
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

  pub(crate) fn file_path(&self, path: &str) -> Result<InputPath> {
    self.base_directory.join_file_path(path)
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

  pub(crate) fn check_path(&self, path: &InputPath) -> Result<()> {
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

  pub(crate) async fn read_dir(&self, path: &InputPath) -> Result<Vec<DirectoryEntry>> {
    let mut read_dir = tokio::fs::read_dir(path)
      .await
      .with_context(|| Error::filesystem_io(path))?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir
      .next_entry()
      .await
      .with_context(|| Error::filesystem_io(path))?
    {
      let input_path = path.join_relative(Path::new(&entry.file_name()))?;
      if self.check_path(&input_path).is_err() {
        continue;
      }
      let metadata = entry
        .metadata()
        .await
        .with_context(|| Error::filesystem_io(&input_path))?;
      let file_type = metadata.file_type();
      let file_size = if metadata.is_dir() {
        None
      } else {
        Some(metadata.len())
      };
      entries.push(DirectoryEntry {
        file_name: entry.file_name(),
        file_type,
        file_size,
      });
    }
    entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(entries)
  }
}

pub(crate) struct DirectoryEntry {
  pub(crate) file_name: OsString,
  pub(crate) file_type: FileType,
  pub(crate) file_size: Option<u64>,
}
