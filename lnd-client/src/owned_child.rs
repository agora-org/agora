use super::*;
use cradle::*;
use pretty_assertions::assert_eq;
use std::{
  net::TcpListener,
  path::PathBuf,
  process::{Child, Command},
  sync::Once,
};
use tempfile::TempDir;

pub(crate) trait CommandExt {
  fn spawn_owned(&mut self) -> std::io::Result<OwnedChild>;
}

pub(crate) struct OwnedChild {
  pub(crate) inner: Child,
}

impl CommandExt for Command {
  fn spawn_owned(&mut self) -> std::io::Result<OwnedChild> {
    Ok(OwnedChild {
      inner: self.spawn()?,
    })
  }
}

impl Drop for OwnedChild {
  fn drop(&mut self) {
    let _ = self.inner.kill();
    let _ = self.inner.wait();
  }
}
