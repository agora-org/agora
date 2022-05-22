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


#[derive(Debug, Clone)]
pub struct CoreLightningInvoice {
    pub value_msat: Millisatoshi,
    pub is_settled: bool,
    pub memo: std::string::String,
    pub payment_hash: Vec<u8>,
    pub payment_request: std::string::String,
}


#[derive(Debug, Clone)]
pub struct CoreLightningAddInvoiceResult {
    pub payment_hash: Vec<u8>,
}


impl LightningInvoice for CoreLightningInvoice {
    fn value_msat(&self) -> Millisatoshi {
	self.value_msat
    }

    fn is_settled(&self) -> bool {
	self.is_settled
    }

    fn memo(&self) -> &std::string::String {
	&self.memo
    }

    fn payment_hash(&self) -> &Vec<u8> {
        &self.payment_hash
    }

    fn payment_request(&self) -> &std::string::String {
	&self.payment_request
    }

}

impl From<ListInvoice> for CoreLightningInvoice {
    fn from(item: ListInvoice) -> Self {
        CoreLightningInvoice {
	    value_msat: Millisatoshi::new(item.amount_msat.unwrap().0),
	    is_settled: item.status == "paid",
	    memo: item.label,
	    payment_hash: item.payment_hash.as_bytes().to_vec(),
	    payment_request: item.bolt11,
	}
    }
}


impl AddLightningInvoiceResponse for CoreLightningAddInvoiceResult {

    fn payment_hash(&self) -> &Vec<u8> {
        &self.payment_hash
    }

}

impl From<Invoice> for CoreLightningAddInvoiceResult {
    fn from(item: Invoice) -> Self {
        CoreLightningAddInvoiceResult {
	    payment_hash: item.payment_hash.as_bytes().to_vec(),
	}
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
	      let cln_added_invoice: CoreLightningAddInvoiceResult = invoice_resp.into();
	      let boxed_invoice = Box::new(cln_added_invoice) as _;
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
	      let boxed_maybe = maybe_invoice.map(|inv| {
		  let cln_invoice: CoreLightningInvoice = inv.clone().into();
		  Box::new(cln_invoice) as _
	      });
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
