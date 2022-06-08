use {
  crate::millisatoshi::Millisatoshi, crate::AddLightningInvoiceResponse, crate::LightningError,
  crate::LightningInvoice, crate::LightningNodeClient, async_trait::async_trait, std::str,
};

#[cfg(unix)]
use {
  cln_rpc::primitives::{Amount, AmountOrAny},
  cln_rpc::{
    model::InvoiceRequest, model::InvoiceResponse, model::ListinvoicesInvoices,
    model::ListinvoicesInvoicesStatus, model::ListinvoicesRequest, ClnRpc, Request, Response,
  },
  std::path::Path,
};

#[cfg(test)]
use {lnd_test_context::LndTestContext, std::sync::Arc};

#[cfg(unix)]
impl From<ListinvoicesInvoices> for LightningInvoice {
  fn from(item: ListinvoicesInvoices) -> Self {
    LightningInvoice {
      value_msat: Millisatoshi::new(item.amount_msat.unwrap().msat()),
      is_settled: matches!(item.status, ListinvoicesInvoicesStatus::PAID),
      memo: item.label,
      payment_hash: item.payment_hash.to_vec(),
      payment_request: item.bolt11.unwrap(),
    }
  }
}

#[cfg(unix)]
impl From<InvoiceResponse> for AddLightningInvoiceResponse {
  fn from(item: InvoiceResponse) -> Self {
    AddLightningInvoiceResponse {
      payment_hash: item.payment_hash.to_vec(),
    }
  }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CoreLightningClient {
  inner: String,
  #[cfg(test)]
  _lnd_test_context: Arc<LndTestContext>,
}

#[async_trait]
impl LightningNodeClient for CoreLightningClient {
  #[cfg(unix)]
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

  #[cfg(not(unix))]
  async fn ping(&self) -> Result<(), LightningError> {
    Err(LightningError)
  }

  #[cfg(unix)]
  async fn add_invoice(
    &self,
    memo: &str,
    value_msat: Millisatoshi,
  ) -> Result<AddLightningInvoiceResponse, LightningError> {
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
        let cln_inv: AddLightningInvoiceResponse = r.into();
        Ok(cln_inv as _)
      }
      _ => Err(LightningError),
    }
  }

  #[cfg(not(unix))]
  async fn add_invoice(
    &self,
    _memo: &str,
    _value_msat: Millisatoshi,
  ) -> Result<AddLightningInvoiceResponse, LightningError> {
    Err(LightningError)
  }

  #[cfg(unix)]
  async fn lookup_invoice(
    &self,
    r_hash: [u8; 32],
  ) -> Result<Option<LightningInvoice>, LightningError> {
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
          let cln_inv: LightningInvoice = inv.clone().into();
          cln_inv as _
        });
        Ok(maybe_cln_inv)
      }
      _ => Err(LightningError),
    }
  }

  #[cfg(not(unix))]
  async fn lookup_invoice(
    &self,
    _r_hash: [u8; 32],
  ) -> Result<Option<LightningInvoice>, LightningError> {
    Err(LightningError)
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
