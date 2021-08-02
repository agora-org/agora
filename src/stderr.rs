use std::io::{self, Write};
use termcolor::{ColorSpec, WriteColor};

#[cfg(test)]
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) enum Stderr {
  Production,
  #[cfg(test)]
  Test(Arc<Mutex<termcolor::Buffer>>),
}

impl Stderr {
  pub fn production() -> Stderr {
    Stderr::Production
  }

  #[cfg(test)]
  pub fn test() -> Stderr {
    #[cfg(windows)]
    return Stderr::Test(Arc::new(Mutex::new(termcolor::Buffer::console())));
    #[cfg(not(windows))]
    return Stderr::Test(Arc::new(Mutex::new(termcolor::Buffer::ansi())));
  }

  #[cfg(test)]
  pub fn contents(&self) -> String {
    match self {
      Stderr::Production => panic!("can't get contents of production stderr"),
      Stderr::Test(arc) => String::from_utf8(arc.lock().unwrap().as_slice().to_vec()).unwrap(),
    }
  }
}

impl Write for Stderr {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Stderr::Production => std::io::stderr().write(buf),
      #[cfg(test)]
      Stderr::Test(arc) => arc.lock().unwrap().write(buf),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Stderr::Production => std::io::stderr().flush(),
      #[cfg(test)]
      Stderr::Test(arc) => arc.lock().unwrap().flush(),
    }
  }
}

impl WriteColor for Stderr {
  fn supports_color(&self) -> bool {
    match self {
      Stderr::Production => {
        termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).supports_color()
      }
      #[cfg(test)]
      Stderr::Test(cursor) => {
        let guard = cursor.lock().unwrap();
        guard.supports_color()
      }
    }
  }

  fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
    match self {
      Stderr::Production => {
        termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).set_color(spec)
      }
      #[cfg(test)]
      Stderr::Test(cursor) => {
        let mut guard = cursor.lock().unwrap();
        guard.set_color(spec)
      }
    }
  }

  fn reset(&mut self) -> io::Result<()> {
    match self {
      Stderr::Production => termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).reset(),
      #[cfg(test)]
      Stderr::Test(cursor) => {
        let mut guard = cursor.lock().unwrap();
        guard.reset()
      }
    }
  }
}
