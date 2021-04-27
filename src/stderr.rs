use std::{
  fmt::Debug,
  io::{self, Cursor, Write},
  sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub(crate) enum Stderr {
  #[allow(dead_code)]
  Test(Arc<Mutex<Cursor<Vec<u8>>>>),
  Production,
}

impl Stderr {
  pub fn production() -> Stderr {
    Stderr::Production
  }
}

impl Write for Stderr {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Stderr::Production => std::io::stderr().write(buf),
      Stderr::Test(arc) => arc.lock().unwrap().write(buf),
    }
  }

  fn flush(&mut self) -> io::Result<()> {
    match self {
      Stderr::Production => std::io::stderr().flush(),
      Stderr::Test(arc) => arc.lock().unwrap().flush(),
    }
  }
}
