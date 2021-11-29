use crate::common::*;

#[derive(Debug, Clone)]
pub(crate) struct Vfs {
  base_directory: InputPath,
}

impl Vfs {
  pub(crate) fn new(base_directory: InputPath) -> Self {
    Self { base_directory }
  }

  pub(crate) fn paid(&self, path: &InputPath) -> bool {
    let config = Config::for_dir(
      self.base_directory.as_ref(),
      path.as_ref().parent().unwrap(),
    )
    .unwrap();
    config.paid()
  }

    pub(crate) fn base_price(&self, path: &InputPath) -> Option<Millisatoshi> {
    let config = Config::for_dir(
      self.base_directory.as_ref(),
      path.as_ref().parent().unwrap(),
    )
    .unwrap();

        config.base_price
    }
}
