use crate::{
  config::Config,
  error::{self, Error, Result},
  file_stream::FileStream,
  html,
  input_path::InputPath,
  redirect::redirect,
};
use agora_lnd_client::lnrpc::invoice::InvoiceState;
use hyper::{header, Body, Request, Response, StatusCode};
use lexiclean::Lexiclean;
use maud::{html, Markup};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use snafu::{IntoError, ResultExt};
use std::{
  ffi::OsString,
  fmt::Debug,
  fs::{self, FileType},
  io,
  path::Path,
};

#[derive(Clone, Debug)]
pub(crate) struct Files {
  base_directory: InputPath,
  lnd_client: Option<agora_lnd_client::Client>,
}

impl Files {
  pub(crate) fn new(
    base_directory: InputPath,
    lnd_client: Option<agora_lnd_client::Client>,
  ) -> Self {
    Self {
      base_directory,
      lnd_client,
    }
  }

  fn tail_to_path(&self, tail: &[&str]) -> Result<InputPath> {
    self.base_directory.join_file_path(&tail.join(""))
  }

  fn check_path(&self, path: &InputPath) -> Result<()> {
    if path
      .as_ref()
      .symlink_metadata()
      .with_context(|| Error::filesystem_io(path))?
      .file_type()
      .is_symlink()
    {
      let link = fs::read_link(path.as_ref()).with_context(|| Error::filesystem_io(path))?;

      let destination = path
        .as_ref()
        .parent()
        .expect("Input paths are always absolute, and thus have parents or are `/`, and `/` cannot be a symlink.")
        .join(link)
        .lexiclean();

      if !destination.starts_with(&self.base_directory) {
        return Err(
          error::SymlinkAccess {
            path: path.display_path().to_owned(),
          }
          .build(),
        );
      }
    }

    if path
      .as_ref()
      .file_name()
      .map(|file_name| file_name.to_string_lossy().starts_with('.'))
      .unwrap_or(false)
    {
      return Err(
        error::HiddenFileAccess {
          path: path.as_ref().to_owned(),
        }
        .build(),
      );
    }

    Ok(())
  }

  fn config_for_dir(&self, dir: &Path) -> Result<Config> {
    Config::for_dir(self.base_directory.as_ref(), dir)
  }

  pub(crate) async fn serve(
    &mut self,
    request: &Request<Body>,
    tail: &[&str],
  ) -> Result<Response<Body>> {
    let file_path = self.tail_to_path(tail)?;

    for result in self.base_directory.iter_prefixes(tail) {
      let prefix = result?;
      self.check_path(&prefix)?;
    }

    let file_type = file_path
      .as_ref()
      .metadata()
      .with_context(|| Error::filesystem_io(&file_path))?
      .file_type();

    if !file_type.is_dir() {
      if let Some(stripped) = request.uri().path().strip_suffix('/') {
        return redirect(stripped.to_owned());
      }
    }

    if file_type.is_dir() && !request.uri().path().ends_with('/') {
      return redirect(String::from(request.uri().path()) + "/");
    }

    if file_type.is_dir() {
      self.serve_dir(&file_path).await
    } else {
      self.access_file(request, tail, &file_path).await
    }
  }

  async fn read_dir(&self, path: &InputPath) -> Result<Vec<(OsString, FileType)>> {
    let mut read_dir = tokio::fs::read_dir(path)
      .await
      .with_context(|| Error::filesystem_io(path))?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir
      .next_entry()
      .await
      .with_context(|| Error::filesystem_io(path))?
    {
      let input_path = path.join_relative(Path::new(&entry.file_name()))?;
      if self.check_path(&input_path).is_err() {
        continue;
      }
      let file_type = entry
        .file_type()
        .await
        .with_context(|| Error::filesystem_io(&input_path))?;
      entries.push((entry.file_name(), file_type));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
  }

  const ENCODE_CHARACTERS: AsciiSet = NON_ALPHANUMERIC.remove(b'/');

  fn render_index(dir: &InputPath) -> Result<Option<Markup>> {
    use pulldown_cmark::{html, Options, Parser};

    let file = dir.join_relative(".index.md".as_ref())?;

    let markdown = match fs::read_to_string(&file) {
      Ok(markdown) => markdown,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(source) => return Err(Error::filesystem_io(&file).into_error(source)),
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

  async fn serve_dir(&self, dir: &InputPath) -> Result<Response<Body>> {
    let config = self.config_for_dir(dir.as_ref())?;
    let body = html! {
      ul class="listing" {
        @for (file_name, file_type) in self.read_dir(dir).await? {
          @let file_name = {
            let mut file_name = file_name.to_string_lossy().into_owned();
            if file_type.is_dir() {
              file_name.push('/');
            }
            file_name
          };
          @let encoded = percent_encoding::utf8_percent_encode(&file_name, &Self::ENCODE_CHARACTERS);
          li {
            a href=(encoded) class="view" {
              (file_name)
            }
            @if file_type.is_file() && !config.paid() {
              a download href=(encoded) {
                (Files::download_icon())
              }
            }
          }
        }
      }
      @if let Some(index) = Self::render_index(dir)? {
        div {
          (index)
        }
      }
    };
    Ok(html::wrap_body(body))
  }

  fn download_icon() -> Markup {
    html! {
      svg class="icon" {
        use href="/static/feather-sprite.svg#download" {}
      }
    }
  }

  async fn access_file(
    &mut self,
    request: &Request<Body>,
    tail: &[&str],
    path: &InputPath,
  ) -> Result<Response<Body>> {
    let config = self.config_for_dir(
      path
        .as_ref()
        .parent()
        .ok_or_else(|| Error::internal(format!("Failed to get parent of file: {:?}", path)))?,
    )?;

    if !config.paid() {
      return Self::serve_file(path).await;
    }

    let lnd_client = self.lnd_client.as_mut().ok_or_else(|| {
      error::LndNotConfiguredPaidFileRequest {
        path: path.display_path().to_owned(),
      }
      .build()
    })?;

    let file_path = tail.join("");
    let base_price = config.base_price.ok_or_else(|| {
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
    match invoice.state() {
      InvoiceState::Settled => {
        let invoice_tail = invoice.memo.split_inclusive('/').collect::<Vec<&str>>();
        let path = self.tail_to_path(&invoice_tail)?;
        Self::serve_file(&path).await
      }
      _ => {
        let qr_code_url = format!("/invoice/{}.svg", hex::encode(invoice.r_hash));
        Ok(html::wrap_body(html! {
          div class="invoice" {
            div class="label" {
              (format!("Lightning Payment Request for {} to access ", value))
              span class="filename" {
                  (invoice.memo)
              }
              ":"
            }
            div class="payment-request" {
              (invoice.payment_request)
            }
            a class="payment-link" href={"lightning:" (invoice.payment_request)} {
              "Open in wallet"
            }
            img class="qr-code" alt="Lightning Network Invoice QR Code" src=(qr_code_url);
          }
        }))
      }
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
