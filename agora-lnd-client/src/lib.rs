mod core_lightning;
mod https_service;
mod lnd;
mod millisatoshi;

pub use core_lightning::CoreLightningClient;
pub use lnd::LndClient;
pub use millisatoshi::Millisatoshi;

use async_trait::async_trait;
use core::fmt::Debug;
use std::error::Error;
use std::fmt;

use dyn_clone::DynClone;

pub trait LightningInvoice {
  fn value_msat(&self) -> Millisatoshi;

  fn is_settled(&self) -> bool;

  fn memo(&self) -> &std::string::String;

  fn payment_hash(&self) -> &Vec<u8>;

  fn payment_request(&self) -> &std::string::String;
}

pub trait AddLightningInvoiceResponse {
  fn payment_hash(&self) -> &Vec<u8>;
}

impl Debug for dyn AddLightningInvoiceResponse + Send {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("AddLightningInvoiceResponse")
      .field("payment_hash", &self.payment_hash())
      .finish()
  }
}

#[derive(Debug, Clone)]
pub struct LightningError;

impl fmt::Display for LightningError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "failed lightning node request")
  }
}

impl Error for LightningError {
  fn description(&self) -> &str {
    // TODO: replace with actual description from error status.
    &"failed lightning node request"
  }
}

#[async_trait]
pub trait LightningNodeClient: DynClone + Send + Sync + 'static {
  async fn ping(&self) -> Result<(), LightningError>;

  async fn add_invoice(
    &self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<Box<dyn AddLightningInvoiceResponse + Send>, LightningError>;

  async fn lookup_invoice(
    &self,
    r_hash: [u8; 32],
  ) -> Result<Option<Box<dyn LightningInvoice + Send>>, LightningError>;
}

dyn_clone::clone_trait_object!(LightningNodeClient);

impl Debug for dyn LightningNodeClient {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "Hi")
  }
}
