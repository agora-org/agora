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

    let (power, suffix) = SUFFIXES
      .iter()
      .enumerate()
      .map(|(i, suffix)| (1024u64.pow(i as u32), suffix))
      .filter(|(power, _)| self.0 >= power - 1)
      .last()
      .unwrap();

    if power == 1 {
      write!(f, "{} {}", self.0, suffix)?;
    } else {
      write!(f, "{:.1} {}", self.0 as f64 / power as f64, suffix)?;
    }

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
  fn halfway_to_1kib() {
    assert_eq!(512.display_size().to_string(), "512 B");
  }

  #[test]
  fn kib() {
    assert_eq!(2u64.pow(10).display_size().to_string(), "1.0 KiB");
  }

  #[test]
  fn halfway_to_2kib() {
    assert_eq!((1024 + 512).display_size().to_string(), "1.5 KiB");
  }

  #[test]
  fn remainder() {
    assert_eq!(1025.display_size().to_string(), "1.0 KiB");
  }

  #[test]
  fn max() {
    assert_eq!(u64::MAX.display_size().to_string(), "16.0 EiB");
  }
}
