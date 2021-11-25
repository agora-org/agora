use crate::common::*;

pub(crate) trait DisplaySize {
  fn display_size(self) -> Wrapper;
}

impl DisplaySize for u64 {
  fn display_size(self) -> Wrapper {
    Wrapper(self)
  }
}

pub(crate) struct Wrapper(u64);

impl Display for Wrapper {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    const SUFFIXES: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

    let value = self.0;

    let (power, suffix) = SUFFIXES
      .iter()
      .enumerate()
      .map(|(i, suffix)| (2u64.pow(i as u32 * 10), suffix))
      .filter(|(power, _)| value >= power - 1)
      .last()
      .unwrap();

    let quotient = self.0 / power;
    let remainder = self.0 % power;

    write!(f, "{}", quotient)?;

    if remainder > 0 {
      write!(f, ".{}", (remainder * 10 / power * 10) / 10)?;
    }

    write!(f, " {}", suffix)?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn zero() {
    assert_eq!(0u64.display_size().to_string(), "0 B");
  }

  #[test]
  fn kib() {
    assert_eq!(2u64.pow(10).display_size().to_string(), "1 KiB");
  }

  #[test]
  fn half() {
    assert_eq!((1024 + 512).display_size().to_string(), "1.5 KiB");
  }

  #[test]
  fn remainder() {
    assert_eq!(1025.display_size().to_string(), "1.0 KiB");
  }

  #[test]
  fn max() {
    assert_eq!(u64::MAX.display_size().to_string(), "15.9 EiB");
  }
}
