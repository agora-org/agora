mod https_service;
mod millisatoshi;
mod lnd;

pub use millisatoshi::Millisatoshi;
pub use lnd::Client;


pub trait LightningInvoice {
    fn value_msat(&self) -> Millisatoshi;

    fn is_settled(&self) -> bool;
}
