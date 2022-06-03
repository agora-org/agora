#![cfg(unix)]

use {
  crate::millisatoshi::Millisatoshi,
  crate::AddLightningInvoiceResponse,
  crate::LightningError,
  crate::LightningInvoice,
  crate::LightningNodeClient,
  async_trait::async_trait,
  cln_rpc::primitives::{Amount, AmountOrAny},
  cln_rpc::{
    model::InvoiceRequest, model::InvoiceResponse, model::ListinvoicesInvoices,
    model::ListinvoicesInvoicesStatus, model::ListinvoicesRequest, ClnRpc, Request, Response,
  },
  std::path::Path,
  std::str,
};

#[cfg(test)]
use {lnd_test_context::LndTestContext, std::sync::Arc};

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

impl From<ListinvoicesInvoices> for CoreLightningInvoice {
  fn from(item: ListinvoicesInvoices) -> Self {
    CoreLightningInvoice {
      value_msat: Millisatoshi::new(item.amount_msat.unwrap().msat()),
      is_settled: matches!(item.status, ListinvoicesInvoicesStatus::PAID),
      memo: item.label,
      payment_hash: item.payment_hash.to_vec(),
      payment_request: item.bolt11.unwrap(),
    }
  }
}

impl AddLightningInvoiceResponse for CoreLightningAddInvoiceResult {
  fn payment_hash(&self) -> &Vec<u8> {
    &self.payment_hash
  }
}

impl From<InvoiceResponse> for CoreLightningAddInvoiceResult {
  fn from(item: InvoiceResponse) -> Self {
    CoreLightningAddInvoiceResult {
      payment_hash: item.payment_hash.to_vec(),
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
    let p = Path::new(&self.inner);
    let mut rpc = ClnRpc::new(p).await.map_err(|_| LightningError)?;

    rpc
      .call(Request::ListInvoices(ListinvoicesRequest {
        payment_hash: None,
        label: None,
        invstring: None,
        offer_id: None,
      }))
      .await
      .map_err(|_| LightningError)?;

    Ok(())
  }

  async fn add_invoice(
    &self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<Box<dyn AddLightningInvoiceResponse + Send>, LightningError> {
    let value_msat_num = value_msat.value();

    let p = Path::new(&self.inner);
    let mut rpc = ClnRpc::new(p).await.map_err(|_| LightningError)?;

    let response = rpc
      .call(Request::Invoice(InvoiceRequest {
        msatoshi: AmountOrAny::Amount(Amount::from_msat(value_msat_num)),
        label: memo.to_owned(),
        description: "".to_owned(),
        cltv: None,
        deschashonly: None,
        expiry: None,
        exposeprivatechannels: None,
        fallbacks: None,
        preimage: None,
      }))
      .await
      .map_err(|_| LightningError)?;

    match response {
      Response::Invoice(r) => {
        let cln_inv: CoreLightningAddInvoiceResult = r.into();
        Ok(Box::new(cln_inv) as _)
      }
      _ => Err(LightningError),
    }
  }

  async fn lookup_invoice(
    &self,
    r_hash: [u8; 32],
  ) -> Result<Option<Box<dyn LightningInvoice + Send>>, LightningError> {
    let payment_hash_hex = hex::encode(&r_hash);

    let p = Path::new(&self.inner);
    let mut rpc = ClnRpc::new(p).await.map_err(|_| LightningError)?;

    let response = rpc
      .call(Request::ListInvoices(ListinvoicesRequest {
        payment_hash: Some(payment_hash_hex),
        label: None,
        invstring: None,
        offer_id: None,
      }))
      .await
      .map_err(|_| LightningError)?;

    match response {
      Response::ListInvoices(r) => {
        let maybe_invoice = r.invoices.get(0);
        let maybe_cln_inv = maybe_invoice.map(|inv| {
          let cln_inv: CoreLightningInvoice = inv.clone().into();
          Box::new(cln_inv) as _
        });
        Ok(maybe_cln_inv)
      }
      _ => Err(LightningError),
    }
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
}
