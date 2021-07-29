use serde::Deserialize;
use std::{fs, io, path::Path};

#[derive(PartialEq, Debug, Deserialize)]
pub(crate) struct Config {
  pub(crate) paid: bool,
}

impl Config {
  pub(crate) fn for_dir(path: &Path) -> Self {
    match fs::read_to_string(path.join(".agora.yaml")) {
      Ok(yaml) => serde_yaml::from_str(&yaml).expect("fixme"),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Self { paid: true },
      _ => todo!(),
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
    let config = Config::for_dir(&temp_dir.path());
    assert_eq!(config, Config { paid: true });
  }

  #[test]
  fn loads_config_from_files() {
    let temp_dir = TempDir::new().unwrap();
    fs::write(temp_dir.path().join(".agora.yaml"), "paid: false").unwrap();
    let config = Config::for_dir(&temp_dir.path());
    assert_eq!(config, Config { paid: false });
  }

  // fixme: what if the directory doesn't exist?
  // fixme: io error when reading config file
}
