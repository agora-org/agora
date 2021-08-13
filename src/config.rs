use crate::error::{self, Result};
use regex::Regex;
use serde::{
  de::{self, Visitor},
  Deserialize, Deserializer, Serialize, Serializer,
};
use snafu::{IntoError, ResultExt};
use std::{fmt, fs, io, path::Path};

#[derive(PartialEq, Debug)]
struct Millisatoshi(u64);

impl<'de> Deserialize<'de> for Millisatoshi {
  fn deserialize<D>(deserializer: D) -> Result<Millisatoshi, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_str(MillisatoshiVisitor)
  }
}

struct MillisatoshiVisitor;

impl<'de> Visitor<'de> for MillisatoshiVisitor {
  type Value = Millisatoshi;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("a string, e.g. \"1000 sat\"")
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
    dbg!(&value);
    let regex = Regex::new(r"^([0-9]*) sat$").expect("regex is valid");
    let captures = regex.captures(value).ok_or_else(|| {
      de::Error::invalid_value(
        de::Unexpected::Str(value),
        &"integer number of satoshis, including unit, e.g. \"1000 sat\"",
      )
    })?;
    let value = captures[1].parse::<u64>().unwrap();
    Ok(Millisatoshi(value * 1000))
  }
}

impl Serialize for Millisatoshi {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    todo!();
    // serializer.serialize_str(*self)
  }
}

#[derive(PartialEq, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Config {
  pub(crate) paid: bool,
  base_price: Option<Millisatoshi>,
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
    assert_eq!(config.base_price, Some(Millisatoshi(3000)));
  }

  fn case(input: &str) {
    let expected = format!(
      "invalid value: string \"{}\", expected integer number of satoshis, including unit, e.g. \"1000 sat\" at line 1 column 1",
      serde_yaml::from_str::<String>(input).unwrap(),
    );
    assert_eq!(
      serde_yaml::from_str::<Millisatoshi>(input)
        .unwrap_err()
        .to_string(),
      expected
    );
  }

  #[test]
  fn leading_space() {
    case("\" 1 sat\"");
  }

  #[test]
  fn trailing_space() {
    case("\"1 sat \"");
  }

  #[test]
  fn wrong_unit() {
    case("1 msat");
  }

  #[test]
  fn missing_unit() {
    case("\"1\"");
  }

  #[test]
  fn number_type() {
    case("1");
  }

  #[test]
  fn decimal_point() {
    case("1.1 sat");
  }

  #[test]
  fn negative_number() {
    case("-1 sat");
  }

  #[test]
  fn list_input() {
    assert_eq!(
      serde_yaml::from_str::<Millisatoshi>("[1]")
        .unwrap_err()
        .to_string(),
      "invalid type: sequence, expected a string, e.g. \"1000 sat\" at line 1 column 1"
    );
  }
}
