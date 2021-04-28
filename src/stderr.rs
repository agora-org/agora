use std::{
  fmt::Debug,
  io::{self, Write},
};

#[cfg(test)]
use std::{
  io::Cursor,
  sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub(crate) enum Stderr {
  Production,
  #[cfg(test)]
  Test(Arc<Mutex<Cursor<Vec<u8>>>>),
}

impl Stderr {
  pub fn production() -> Stderr {
    Stderr::Production
  }

  #[cfg(test)]
  pub fn test() -> Stderr {
    Stderr::Test(Arc::new(Mutex::new(Cursor::new(vec![]))))
  }

  #[cfg(test)]
  pub fn contents(&self) -> String {
    match self {
      Stderr::Production => panic!("can't get contents of production stderr"),
      Stderr::Test(arc) => String::from_utf8(arc.lock().unwrap().clone().into_inner()).unwrap(),
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
