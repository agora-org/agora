use {
  crate::millisatoshi::Millisatoshi,
  crate::LightningError,
  crate::LightningInvoice,
  crate::AddLightningInvoiceResponse,
  crate::LightningNodeClient,
  clightningrpc::LightningRPC,
  clightningrpc::responses::ListInvoice,
  clightningrpc::responses::Invoice,
  async_trait::async_trait,
  std::str,
  futures::future,
};


impl LightningInvoice for ListInvoice {
    fn value_msat(&self) -> Millisatoshi {
	Millisatoshi::new(self.amount_msat.unwrap().0)
    }

    fn is_settled(&self) -> bool {
	self.status == "paid"
    }

    fn memo(&self) -> &std::string::String {
	&self.label
    }

    fn payment_hash(&self) -> &Vec<u8> {
        &self.payment_hash.as_bytes().to_vec()
    }

    fn payment_request(&self) -> &std::string::String {
	&self.bolt11
    }

}


impl AddLightningInvoiceResponse for Invoice {

    fn payment_hash(&self) -> &Vec<u8> {
        &self.payment_hash.as_bytes().to_vec()
    }

}



#[derive(Debug, Clone)]
pub struct CoreLightningClient {
  inner: String,
  #[cfg(test)]
  _lnd_test_context: Arc<LndTestContext>,
}

#[async_trait]
impl LightningNodeClient for CoreLightningClient {

  async fn ping(&self) -> Result<(), LightningError> {

    let client = LightningRPC::new(&self.inner);

    let list_invoices_resp = client.getinfo();

    match list_invoices_resp {
        Ok(_) => future::ok(()),
        Err(_) => future::err(LightningError),
    }.await

  }

  async fn add_invoice(
    &self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<Box<dyn AddLightningInvoiceResponse + Send>, LightningError> {

      let value_msat_num = value_msat.value();

      let client = LightningRPC::new(&self.inner);

      let invoice_res = client.invoice(
	  value_msat_num,
	  memo,
	  "",
	  None,
      );

      match invoice_res {
          Ok(invoice_resp) => {
	      let boxed_invoice = Box::new(invoice_resp) as _;
	      future::ok(boxed_invoice)
	  },
          Err(_) => future::err(LightningError),
      }.await

  }

  async fn lookup_invoice(&self, r_hash: [u8; 32]) -> Result<Option<Box<dyn LightningInvoice + Send>>, LightningError> {

      let payment_hash_str = str::from_utf8(&r_hash).unwrap();

      let client = LightningRPC::new(&self.inner);

      let list_invoices_res = client.listinvoices(Some(payment_hash_str));

      match list_invoices_res {
          Ok(list_invoices_resp) => {
	      let invoices = list_invoices_resp.invoices;
	      let maybe_invoice = invoices.get(0);
	      let boxed_maybe = maybe_invoice.map(|inv| Box::new(inv.clone()) as _);
	      future::ok(boxed_maybe)
	  },
          Err(_) => future::err(LightningError),
      }.await

  }

}

impl CoreLightningClient {

  pub async fn new(
    rpc_path: String,
    #[cfg(test)] lnd_test_context: LndTestContext,
  ) -> CoreLightningClient {

    let inner = rpc_path;

    CoreLightningClient {
      inner,
      #[cfg(test)]
      _lnd_test_context: Arc::new(lnd_test_context),
    }
  }

  pub async fn get_client(
    &self,
  ) -> LightningRPC {
    LightningRPC::new(&self.inner)
  }

}
