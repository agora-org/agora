use crate::error::{self, Error, Result};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::{fs, io, path::Path};

#[derive(PartialEq, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
  #[serde(default)]
  pub(crate) paid: bool,
}

impl Config {
  pub(crate) fn for_dir(path: &Path) -> Result<Self> {
    path.read_dir().context(error::FilesystemIo { path })?;
    let file_path = path.join(".agora.yaml");
    match fs::read_to_string(&file_path) {
      Ok(yaml) => serde_yaml::from_str(&yaml).context(error::ConfigDeserialize { path: file_path }),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self { paid: false }),
      Err(source) => Err(Error::FilesystemIo {
        path: file_path,
        source,
      }),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn loads_the_default_config_when_no_files_given() {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config, Config { paid: false });
  }

  #[test]
  fn loads_config_from_files() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "paid: true").unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config, Config { paid: true });
  }

  #[test]
  fn directory_does_not_exist() {
    let temp_dir = TempDir::new().unwrap();
    let result = Config::for_dir(&temp_dir.path().join("does-not-exist"));
    assert_matches!(
      result,
      Err(Error::FilesystemIo { path, source })
        if path == temp_dir.path().join("does-not-exist") && source.kind() == io::ErrorKind::NotFound
    );
  }

  #[test]
  fn io_error_when_reading_config_file() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir(temp_dir.path().join(".agora.yaml")).unwrap();
    let result = Config::for_dir(temp_dir.path());
    assert_matches!(
      result,
      Err(Error::FilesystemIo { path, .. })
        if path == temp_dir.path().join(".agora.yaml")
    );
  }

  #[test]
  fn invalid_config() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "{{{").unwrap();
    let result = Config::for_dir(temp_dir.path());
    assert_matches!(
      result,
      Err(Error::ConfigDeserialize { path, .. })
        if path == temp_dir.path().join(".agora.yaml")
    );
  }

  #[test]
  fn unknown_fields() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = serde_yaml::to_value(Config { paid: false }).unwrap();
    config["unknown_field"] = serde_yaml::Value::Null;
    fs::write(
      temp_dir.path().join(".agora.yaml"),
      serde_yaml::to_string(&config).unwrap(),
    )
    .unwrap();
    let result = Config::for_dir(temp_dir.path());
    assert_matches!(
      result,
      Err(Error::ConfigDeserialize { path, .. })
        if path == temp_dir.path().join(".agora.yaml")
    );
  }

  #[test]
  fn paid_is_optional() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "{}").unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config, Config { paid: false });
  }
}
