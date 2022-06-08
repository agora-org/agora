#![allow(clippy::all)]

use {
  async_trait::async_trait, core::fmt::Debug, dyn_clone::DynClone, std::error::Error, std::fmt,
};

pub use core_lightning::CoreLightningClient;
pub use lnd::LndClient;
pub use millisatoshi::Millisatoshi;

mod core_lightning;
mod https_service;
mod lnd;
mod millisatoshi;

#[derive(Debug, Clone)]
pub struct LightningInvoice {
  pub value_msat: Millisatoshi,
  pub is_settled: bool,
  pub memo: std::string::String,
  pub payment_hash: Vec<u8>,
  pub payment_request: std::string::String,
}

#[derive(Debug, Clone)]
pub struct AddLightningInvoiceResponse {
  pub payment_hash: Vec<u8>,
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
    "failed lightning node request"
  }
}

#[async_trait]
pub trait LightningNodeClient: DynClone + Send + Sync + 'static {
  async fn ping(&self) -> Result<(), LightningError>;

  async fn add_invoice(
    &self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<AddLightningInvoiceResponse, LightningError>;

  async fn lookup_invoice(
    &self,
    r_hash: [u8; 32],
  ) -> Result<Option<LightningInvoice>, LightningError>;
}

dyn_clone::clone_trait_object!(LightningNodeClient);

impl Debug for dyn LightningNodeClient {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    write!(f, "Hi")
  }
}
