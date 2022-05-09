use {
  crate::millisatoshi::Millisatoshi,
};

pub trait LightningInvoice {
    fn value_msat(&self) -> Millisatoshi;

    fn is_settled(&self) -> bool;
}
