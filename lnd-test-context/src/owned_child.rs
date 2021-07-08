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
    if let Ok(mut guard) = self.inner.lock() {
      let _ = guard.kill();
      let _ = guard.wait();
    }
  }
}
