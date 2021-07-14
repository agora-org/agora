use std::{
  process::{Child, Command},
  sync::{Arc, Mutex},
};

pub(crate) trait CommandExt {
  fn spawn_owned(&mut self) -> std::io::Result<OwnedChild>;
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedChild {
  pub(crate) inner: Arc<Mutex<Child>>,
}

impl CommandExt for Command {
  fn spawn_owned(&mut self) -> std::io::Result<OwnedChild> {
    Ok(OwnedChild {
      inner: Arc::new(Mutex::new(self.spawn()?)),
    })
  }
}

impl Drop for OwnedChild {
  fn drop(&mut self) {
    let mut child = self.inner.lock().unwrap();
    let _ = child.kill();
    let _ = child.wait();
  }
}
