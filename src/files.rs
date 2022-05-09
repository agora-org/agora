use {
  crate::{common::*, file_stream::FileStream, vfs::Vfs},
  // agora_lnd_client::lnrpc::invoice::InvoiceState,
  agora_lnd_client::LightningInvoice,
  maud::html,
  percent_encoding::{AsciiSet, NON_ALPHANUMERIC},
};

#[derive(Clone, Debug)]
pub(crate) struct Files {
  vfs: Vfs,
  lnd_client: Option<agora_lnd_client::Client>,
}

impl Files {
  pub(crate) fn new(
    base_directory: InputPath,
    lnd_client: Option<agora_lnd_client::Client>,
  ) -> Self {
    Self {
      vfs: Vfs::new(base_directory),
      lnd_client,
    }
  }

  pub(crate) async fn serve(
    &mut self,
    request: &Request<Body>,
    tail: &[&str],
  ) -> Result<Response<Body>> {
    let file_path = self.vfs.file_path(&tail.join(""))?;
    let file_type = self.vfs.file_type(tail)?;

    if !file_type.is_dir() {
      if let Some(stripped) = request.uri().path().strip_suffix('/') {
        return redirect(stripped.to_owned());
      }
    }

    if file_type.is_dir() && !request.uri().path().ends_with('/') {
      return redirect(String::from(request.uri().path()) + "/");
    }

    if file_type.is_dir() {
      self.serve_dir(tail, &file_path).await
    } else {
      self.access_file(request, tail, &file_path).await
    }
  }

  // Percent encode all unicode codepoints, even though
  // they are allowed by the spec:
  // https://url.spec.whatwg.org/#url-code-points
  const ENCODE_CHARACTERS: AsciiSet = NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b'-')
    .remove(b'.')
    .remove(b'/')
    .remove(b':')
    .remove(b';')
    .remove(b'=')
    .remove(b'?')
    .remove(b'@')
    .remove(b'_')
    .remove(b'~');

  fn render_index(&self, dir: &InputPath) -> Result<Option<Markup>> {
    use pulldown_cmark::{html, Options, Parser};

    let markdown = match self.vfs.index_file_markdown(dir)? {
      None => return Ok(None),
      Some(markdown) => markdown,
    };

    let options = Options::ENABLE_FOOTNOTES
      | Options::ENABLE_STRIKETHROUGH
      | Options::ENABLE_TABLES
      | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(&markdown, options);
    let mut html = String::new();
    html::push_html(&mut html, parser);
    Ok(Some(maud::PreEscaped(html)))
  }

  async fn serve_dir(&self, tail: &[&str], dir: &InputPath) -> Result<Response<Body>> {
    let body = html! {
      ul class="listing" {
        @for entry in self.vfs.read_dir(dir).await? {

          @let file_name = {
            let mut file_name = entry.file_name.to_string_lossy().into_owned();
            if entry.file_type.is_dir() {
              file_name.push('/');
            }
            file_name
          };
          @let encoded = percent_encoding::utf8_percent_encode(&file_name, &Self::ENCODE_CHARACTERS);
          li {
            a href=(encoded) class="view" {
              (file_name)
            }

            @if let Some(file_size) = entry.file_size {
              span class="filesize" {
                (file_size.display_size())
              }
            }
            @if entry.file_type.is_file() && !entry.paid {
              a download href=(encoded) {
                (Files::icon("download"))
              }
            }
          }
        }
      }
      @if let Some(index) = self.render_index(dir)? {
        div {
          (index)
        }
      }
    };
    Ok(html::wrap_body(&format!("/{}", tail.join("")), body))
  }

  fn icon(name: &str) -> Markup {
    html! {
      svg class="icon" {
        use href=(format!("/static/feather-sprite.svg#{}",name)) {}
      }
    }
  }

  async fn access_file(
    &mut self,
    request: &Request<Body>,
    tail: &[&str],
    path: &InputPath,
  ) -> Result<Response<Body>> {
    if !self.vfs.paid(path)? {
      return Self::serve_file(path).await;
    }

    let lnd_client = self.lnd_client.as_mut().ok_or_else(|| {
      error::LndNotConfiguredPaidFileRequest {
        path: path.display_path().to_owned(),
      }
      .build()
    })?;

    let file_path = tail.join("");
    let base_price = self.vfs.base_price(path)?.ok_or_else(|| {
      error::ConfigMissingBasePrice {
        path: path.display_path(),
      }
      .build()
    })?;
    let invoice = lnd_client
      .add_invoice(&file_path, base_price)
      .await
      .context(error::LndRpcStatus)?;
    redirect(format!(
      "{}?invoice={}",
      request.uri().path(),
      hex::encode(invoice.r_hash),
    ))
  }

  async fn serve_file(path: &InputPath) -> Result<Response<Body>> {
    let mut builder = Response::builder().status(StatusCode::OK);
    if let Some(guess) = path.mime_guess().first() {
      builder = builder.header(header::CONTENT_TYPE, guess.essence_str());
    }
    builder
      .body(Body::wrap_stream(FileStream::new(path.clone()).await?))
      .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)))
  }

  pub(crate) async fn serve_invoice(
    &mut self,
    request: &Request<Body>,
    request_tail: &[&str],
    r_hash: [u8; 32],
  ) -> Result<Response<Body>> {
    let lnd_client = self.lnd_client.as_mut().ok_or_else(|| {
      error::LndNotConfiguredInvoiceRequest {
        uri_path: request.uri().path().to_owned(),
      }
      .build()
    })?;
    let invoice = lnd_client
      .lookup_invoice(r_hash)
      .await
      .context(error::LndRpcStatus)?
      .ok_or_else(|| error::InvoiceNotFound { r_hash }.build())?;

    let request_tail = request_tail.join("");
    if request_tail != invoice.memo {
      return Err(
        error::InvoicePathMismatch {
          invoice_tail: invoice.memo,
          request_tail,
          r_hash,
        }
        .build(),
      );
    }

    let value = invoice.value_msat();
    // TODO: Use custom trait for Invoice with settled function.
    if invoice.is_settled() {
        let path = self.vfs.file_path(&invoice.memo)?;
        Self::serve_file(&path).await
    } else {
        let qr_code_url = format!("/invoice/{}.svg", hex::encode(invoice.r_hash));
        let filename = invoice.memo;
        Ok(html::wrap_body(
          &format!("Invoice for {}", filename),
          html! {
            div class="invoice" {
              div class="label" {
                "Lightning Payment Request for " (value) " to access "
                span class="filename" {
                    (filename)
                }

                ":"
              }
              div class="payment-request"{
                button class="clipboard-copy" onclick=(
                  format!("navigator.clipboard.writeText(\"{}\")", invoice.payment_request)
                ) {
                  (Files::icon("clipboard"))
                }
                (invoice.payment_request)
              }

              div class="links" {
                a class="payment-link" href={"lightning:" (invoice.payment_request)} {
                  "Open invoice in wallet"
                }
                a class="reload-link" href=(request.uri()) {
                  "Access file"
                }
              }
              img
                class="qr-code"
                alt="Lightning Network Invoice QR Code"
                src=(qr_code_url)
                width="400"
                height="400";
            }
            div class="instructions" {
              "To access "
              span class="filename" {
                  (filename)
              }
              ":"
              ol {
                li {
                  "Pay the invoice for " (value) " above "
                  "with your Lightning Network wallet by "
                  "scanning the QR code, "
                  "copying the payment request string, or "
                  "clicking the \"Open invoice in wallet\" link."
                }
                li {
                  "Click the \"Access file\" link or reload the page."
                }
              }
            }
          },
        ))
      }
    }

  pub(crate) async fn serve_invoice_qr_code(
    &mut self,
    request: &Request<Body>,
    r_hash: [u8; 32],
  ) -> Result<Response<Body>> {
    use qrcodegen::{QrCode, QrCodeEcc};

    let lnd_client = self.lnd_client.as_mut().ok_or_else(|| {
      error::LndNotConfiguredInvoiceRequest {
        uri_path: request.uri().path().to_owned(),
      }
      .build()
    })?;
    let invoice = lnd_client
      .lookup_invoice(r_hash)
      .await
      .context(error::LndRpcStatus)?
      .ok_or_else(|| error::InvoiceNotFound { r_hash }.build())?;
    let payment_request = invoice.payment_request.to_uppercase();
    let qr_code = QrCode::encode_text(&payment_request, QrCodeEcc::Medium)
      .context(error::PaymentRequestTooLongForQrCode { payment_request })?;
    Ok(
      Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml")
        .body(Body::from(qr_code.to_svg_string(4)))
        .expect("All arguments to response builder are valid"),
    )
  }
}
