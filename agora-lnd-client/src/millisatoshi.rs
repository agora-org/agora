use {
  regex::Regex,
  serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
  },
  std::fmt::{self, Display, Formatter},
};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Millisatoshi(u64);

impl Millisatoshi {
  pub(crate) fn value(self) -> u64 {
    self.0
  }

  pub fn new(value: u64) -> Self {
    Self(value)
  }
}

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

  fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
    formatter.write_str("a string, e.g. \"1000 sat\"")
  }

  fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
  where
    E: de::Error,
  {
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

impl Display for Millisatoshi {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    use num_format::{Locale, ToFormattedString};

    write!(f, "{}", (self.0 / 1000).to_formatted_string(&Locale::en))?;

    let millisatoshis = self.0 % 1000;

    if millisatoshis > 0 {
      write!(
        f,
        ".{}",
        ((millisatoshis as f64) / 1000.0)
          .to_string()
          .strip_prefix("0.")
          .expect("float string always starts with `0.`")
      )?;
    }

    if self.0 == 1_000 {
      write!(f, " satoshi")?;
    } else {
      write!(f, " satoshis")?;
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  fn invalid_value(input: &str) {
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
    invalid_value("\" 1 sat\"");
  }

  #[test]
  fn trailing_space() {
    invalid_value("\"1 sat \"");
  }

  #[test]
  fn wrong_unit() {
    invalid_value("1 msat");
  }

  #[test]
  fn missing_unit() {
    invalid_value("\"1\"");
  }

  #[test]
  fn number_type() {
    invalid_value("1");
  }

  #[test]
  fn decimal_point() {
    invalid_value("1.1 sat");
  }

  #[test]
  fn negative_number() {
    invalid_value("-1 sat");
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

  #[test]
  fn display_singular() {
    assert_eq!(Millisatoshi::new(1000).to_string(), "1 satoshi");
  }

  #[test]
  fn display_plural() {
    assert_eq!(Millisatoshi::new(0).to_string(), "0 satoshis");
  }

  #[test]
  fn display_millisatoshis() {
    assert_eq!(Millisatoshi::new(1).to_string(), "0.001 satoshis");
  }

  #[test]
  fn display_millisatoshis_no_trailing_zeros() {
    assert_eq!(Millisatoshi::new(10).to_string(), "0.01 satoshis");
  }

  #[test]
  fn display_satoshis_with_comma() {
    assert_eq!(Millisatoshi::new(1_000_000).to_string(), "1,000 satoshis");
  }

  #[test]
  fn display_millisatoshis_with_comma() {
    assert_eq!(
      Millisatoshi::new(1_000_123).to_string(),
      "1,000.123 satoshis"
    );
  }
}
