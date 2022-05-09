
pub trait LightningInvoice {
    fn is_settled(&self) -> bool;
}
