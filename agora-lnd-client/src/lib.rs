mod https_service;
mod millisatoshi;
mod lnd;

pub use millisatoshi::Millisatoshi;
pub use lnd::Client;

use std::fmt;
use std::error::Error;


pub trait LightningInvoice {
    fn value_msat(&self) -> Millisatoshi;

    fn is_settled(&self) -> bool;
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
