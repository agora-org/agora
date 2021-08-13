use crate::error::{self, Result};
use agora_lnd_client::Millisatoshi;
use serde::Deserialize;
use snafu::{IntoError, ResultExt};
use std::{fs, io, path::Path};

#[derive(PartialEq, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Config {
  pub(crate) paid: bool,
  pub(crate) base_price: Option<Millisatoshi>,
}

impl Config {
  pub(crate) fn for_dir(path: &Path) -> Result<Self> {
    path.read_dir().context(error::FilesystemIo { path })?;
    let file_path = path.join(".agora.yaml");
    match fs::read_to_string(&file_path) {
      Ok(yaml) => serde_yaml::from_str(&yaml).context(error::ConfigDeserialize { path: file_path }),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
      Err(source) => Err(error::FilesystemIo { path: file_path }.into_error(source)),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::error::Error;
  use pretty_assertions::assert_eq;
  use tempfile::TempDir;
  use unindent::Unindent;

  #[test]
  fn test_default_config() {
    assert_eq!(
      Config {
        paid: false,
        base_price: None
      },
      Config::default()
    );
  }

  #[test]
  fn loads_the_default_config_when_no_files_given() {
    let temp_dir = TempDir::new().unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config, Config::default());
  }

  #[test]
  fn loads_config_from_files() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "paid: true").unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(
      config,
      Config {
        paid: true,
        base_price: None
      }
    );
  }

  #[test]
  fn directory_does_not_exist() {
    let temp_dir = TempDir::new().unwrap();
    let result = Config::for_dir(&temp_dir.path().join("does-not-exist"));
    assert_matches!(
      result,
      Err(Error::FilesystemIo { path, source, .. })
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
    fs::write(temp_dir.path().join(".agora.yaml"), "unknown_field: foo").unwrap();
    let result = Config::for_dir(temp_dir.path());
    assert_matches!(
      result,
      Err(Error::ConfigDeserialize { path, source, .. })
        if path == temp_dir.path().join(".agora.yaml")
           && source.to_string().contains("unknown field `unknown_field`")
    );
  }

  #[test]
  fn paid_is_optional() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "{}").unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config, Config::default());
  }

  #[test]
  fn parses_base_price_in_satoshi() {
    let temp_dir = TempDir::new().unwrap();
    let yaml = "
      paid: true
      base-price: 3 sat
    "
    .unindent();
    fs::write(temp_dir.path().join(".agora.yaml"), yaml).unwrap();
    let config = Config::for_dir(temp_dir.path()).unwrap();
    assert_eq!(config.base_price, Some(Millisatoshi::new(3000)));
  }
}
