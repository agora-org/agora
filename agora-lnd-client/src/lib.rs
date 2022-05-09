
// impl Client {
//   pub async fn new(
//     authority: Authority,
//     certificate: Option<X509>,
//     macaroon: Option<Vec<u8>>,
//     #[cfg(test)] lnd_test_context: LndTestContext,
//   ) -> Result<Client, openssl::error::ErrorStack> {
//     let grpc_service = HttpsService::new(authority, certificate)?;

//     let macaroon = macaroon.map(|macaroon| {
//       hex::encode_upper(macaroon)
//         .parse::<AsciiMetadataValue>()
//         .expect("Client::new: hex characters are valid metadata values")
//     });

//     let inner = LightningClient::with_interceptor(grpc_service, MacaroonInterceptor { macaroon });

//     Ok(Client {
//       inner,
//       #[cfg(test)]
//       _lnd_test_context: Arc::new(lnd_test_context),
//     })
//   }

//   pub async fn ping(&mut self) -> Result<(), Status> {
//     let request = tonic::Request::new(ListInvoiceRequest {
//       index_offset: 0,
//       num_max_invoices: 0,
//       pending_only: false,
//       reversed: false,
//     });

//     self.inner.list_invoices(request).await?;

//     Ok(())
//   }

//   pub async fn add_invoice(
//     &mut self,
//     memo: &str,
//     value_msat: Millisatoshi,
//   ) -> Result<AddInvoiceResponse, Status> {
//     let request = tonic::Request::new(Invoice {
//       memo: memo.to_owned(),
//       value_msat: value_msat.value().try_into().map_err(|source| {
//         Status::new(
//           Code::InvalidArgument,
//           format!("invalid value for `value_msat`: {}", source),
//         )
//       })?,
//       ..Invoice::default()
//     });
//     Ok(self.inner.add_invoice(request).await?.into_inner())
//   }

//   pub async fn lookup_invoice(&mut self, r_hash: [u8; 32]) -> Result<Option<Invoice>, Status> {
//     let request = tonic::Request::new(PaymentHash {
//       r_hash: r_hash.to_vec(),
//       ..PaymentHash::default()
//     });
//     match self.inner.lookup_invoice(request).await {
//       Ok(response) => Ok(Some(response.into_inner())),
//       Err(status) => {
//         if status.code() == Code::Unknown
//           && (status.message() == "there are no existing invoices"
//             || status.message() == "unable to locate invoice")
//         {
//           Ok(None)
//         } else {
//           Err(status)
//         }
//       }
//     }
//   }

//   #[cfg(test)]
//   async fn with_cert(lnd_test_context: LndTestContext, cert: &str) -> Self {
//     Self::new(
//       format!("localhost:{}", lnd_test_context.lnd_rpc_port)
//         .parse()
//         .unwrap(),
//       Some(X509::from_pem(cert.as_bytes()).unwrap()),
//       Some(
//         tokio::fs::read(lnd_test_context.invoice_macaroon_path())
//           .await
//           .unwrap(),
//       ),
//       lnd_test_context,
//     )
//     .await
//     .unwrap()
//   }

//   #[cfg(test)]
//   async fn with_test_context(lnd_test_context: LndTestContext) -> Self {
//     let cert = std::fs::read_to_string(lnd_test_context.cert_path()).unwrap();
//     Self::with_cert(lnd_test_context, &cert).await
//   }
// }


// mod https_service;
mod https_service;
mod millisatoshi;
mod lnd;


pub use millisatoshi::Millisatoshi;

pub use lnd::Client;
