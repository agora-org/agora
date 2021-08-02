use std::{
  io::{self, Write},
  sync::{Arc, Mutex},
};
use termcolor::{ColorSpec, WriteColor};

#[derive(Clone)]
pub(crate) enum Stderr {
  Production(Arc<Mutex<termcolor::StandardStream>>),
  #[cfg(test)]
  Test(Arc<Mutex<termcolor::Buffer>>),
}

impl Stderr {
  pub fn production() -> Stderr {
    Stderr::Production(Arc::new(Mutex::new(termcolor::StandardStream::stderr(
      termcolor::ColorChoice::Auto,
    ))))
  }

  #[cfg(test)]
  pub fn test() -> Stderr {
    // fixme: use something else on windows
    Stderr::Test(Arc::new(Mutex::new(termcolor::Buffer::ansi())))
  }

  #[cfg(test)]
  pub fn contents(&self) -> String {
    match self {
      Stderr::Production(_) => panic!("can't get contents of production stderr"),
      Stderr::Test(arc) => String::from_utf8(arc.lock().unwrap().as_slice().to_vec()).unwrap(),
    }
  }
}

impl Write for Stderr {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Stderr::Production(arc) => {
        if let Ok(mut guard) = arc.lock() {
          return guard.write(buf);
        } else {
          std::io::stderr().write(buf)
        }
      }
      #[cfg(test)]
      Stderr::Test(arc) => arc.lock().unwrap().write(buf),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Stderr::Production(arc) => {
        if let Ok(mut guard) = arc.lock() {
          guard.flush()?;
        }
        Ok(())
      }
      #[cfg(test)]
      Stderr::Test(arc) => arc.lock().unwrap().flush(),
    }
  }
}

impl WriteColor for Stderr {
  fn supports_color(&self) -> bool {
    match self {
      Stderr::Production(stream) => match stream.lock() {
        Ok(guard) => guard.supports_color(),
        Err(_) => termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).supports_color(),
      },
      #[cfg(test)]
      Stderr::Test(cursor) => {
        let guard = cursor.lock().unwrap();
        guard.supports_color()
      }
    }
  }

  fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
    match self {
      Stderr::Production(stream) => {
        if let Ok(mut guard) = stream.lock() {
          guard.set_color(spec)
        } else {
          termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).set_color(spec)
        }
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
      Stderr::Production(stream) => match stream.lock() {
        Ok(mut guard) => guard.reset(),
        Err(_) => termcolor::StandardStream::stderr(termcolor::ColorChoice::Auto).reset(),
      },
      #[cfg(test)]
      Stderr::Test(cursor) => {
        let mut guard = cursor.lock().unwrap();
        guard.reset()
      }
    }
  }
}
