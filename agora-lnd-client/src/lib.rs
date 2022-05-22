mod https_service;
mod millisatoshi;
mod lnd;

mod core_lightning;

pub use millisatoshi::Millisatoshi;
pub use lnd::LndClient;

use async_trait::async_trait;
use std::fmt;
use std::error::Error;
use core::fmt::Debug;

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

    async fn lookup_invoice(&self, r_hash: [u8; 32]) -> Result<Option<Box<dyn LightningInvoice + Send>>, LightningError>;

}

dyn_clone::clone_trait_object!(LightningNodeClient);


impl Debug for dyn LightningNodeClient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Hi")
    }
}
