use crate::{
  error::{self, Error, Result},
  file_stream::FileStream,
  input_path::InputPath,
  redirect::redirect,
};
use hyper::{header, Body, Request, Response, StatusCode};
use lnd_client::lnrpc::invoice::InvoiceState;
use maud::{html, Markup, DOCTYPE};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use snafu::ResultExt;
use std::{ffi::OsString, fmt::Debug, fs::FileType, path::Path};

#[derive(Clone, Debug)]
pub(crate) struct Files {
  base_directory: InputPath,
  lnd_client: Option<lnd_client::Client>,
}

impl Files {
  pub(crate) fn new(base_directory: InputPath, lnd_client: Option<lnd_client::Client>) -> Self {
    Self {
      base_directory,
      lnd_client,
    }
  }

  fn tail_to_path(&self, tail: &[&str]) -> Result<InputPath> {
    self.base_directory.join_file_path(&tail.join(""))
  }

  pub(crate) async fn serve(
    &mut self,
    request: &Request<Body>,
    tail: &[&str],
  ) -> Result<Response<Body>> {
    let file_path = self.tail_to_path(tail)?;

    for result in self.base_directory.iter_prefixes(tail) {
      let prefix = result?;
      let file_type = prefix
        .as_ref()
        .symlink_metadata()
        .with_context(|| Error::filesystem_io(&prefix))?
        .file_type();

      if file_type.is_symlink() {
        return Err(Error::SymlinkAccess {
          path: prefix.as_ref().to_owned(),
        });
      }

      if prefix
        .as_ref()
        .file_name()
        .map(|file_name| file_name.to_string_lossy().starts_with("."))
        .unwrap_or(false)
      {
        return Err(Error::HiddenFileAccess {
          path: file_path.as_ref().to_owned(),
        });
      }
    }

    let file_type = file_path
      .as_ref()
      .metadata()
      .with_context(|| Error::filesystem_io(&file_path))?
      .file_type();

    if !file_type.is_dir() {
      if let Some(stripped) = request.uri().path().strip_suffix("/") {
        return redirect(stripped.to_owned());
      }
    }

    if file_type.is_dir() && !request.uri().path().ends_with('/') {
      return redirect(String::from(request.uri().path()) + "/");
    }

    if file_type.is_dir() {
      Self::list(&file_path).await
    } else {
      self.access_file(tail, &file_path).await
    }
  }

  async fn read_dir(path: &InputPath) -> Result<Vec<(OsString, FileType)>> {
    let mut read_dir = tokio::fs::read_dir(path)
      .await
      .with_context(|| Error::filesystem_io(path))?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir
      .next_entry()
      .await
      .with_context(|| Error::filesystem_io(path))?
    {
      let file_type = entry.file_type().await.map_err(|source| {
        match path.join_relative(Path::new(&entry.file_name())) {
          Err(error) => error,
          Ok(entry_path) => Error::FilesystemIo {
            path: entry_path.display_path().to_owned(),
            source,
          },
        }
      })?;
      let file_name = entry.file_name();
      if file_type.is_symlink() || file_name.to_string_lossy().starts_with(".") {
        continue;
      }
      entries.push((file_name, file_type));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
  }

  fn serve_html(contents: Markup) -> Response<Body> {
    let html = html! {
      html {
        (DOCTYPE)
        head {
          meta charset="utf-8";
          title {
            "agora"
          }
          link rel="stylesheet" href="/static/index.css";
        }
        body {
          ul class="contents" {
            (contents)
          }
        }
      }
    };
    Response::new(Body::from(html.into_string()))
  }

  const ENCODE_CHARACTERS: AsciiSet = NON_ALPHANUMERIC.remove(b'/');

  async fn list(dir: &InputPath) -> Result<Response<Body>> {
    let contents = html! {
      @for (file_name, file_type) in Self::read_dir(dir).await? {
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
          @if file_type.is_file() {
            a download href=(encoded) {
              (Files::download_icon())
            }
          }
        }
      }
    };
    Ok(Files::serve_html(contents))
  }

  fn download_icon() -> Markup {
    html! {
      svg class="icon" {
        use href="/static/feather-sprite.svg#download" {}
      }
    }
  }

  async fn access_file(&mut self, tail: &[&str], path: &InputPath) -> Result<Response<Body>> {
    match &mut self.lnd_client {
      Some(lnd_client) => {
        let file_path = tail.join("");
        let invoice = lnd_client
          .add_invoice(&file_path, 1000)
          .await
          .context(error::LndRpcStatus)?;
        redirect(format!(
          "/invoice/{}/{}",
          hex::encode(invoice.r_hash),
          file_path,
        ))
      }
      None => Self::serve_file(path).await,
    }
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
    r_hash: [u8; 32],
  ) -> Result<Response<Body>> {
    let lnd_client = self
      .lnd_client
      .as_mut()
      .ok_or_else(|| Error::LndNotConfigured {
        uri_path: request.uri().path().to_owned(),
      })?;
    let invoice = lnd_client
      .lookup_invoice(r_hash)
      .await
      .context(error::LndRpcStatus)?
      .ok_or(Error::InvoiceNotFound { r_hash })?;
    match invoice.state() {
      InvoiceState::Settled => {
        let tail_from_invoice = invoice.memo.split_inclusive('/').collect::<Vec<&str>>();
        let path = self.tail_to_path(&tail_from_invoice)?;
        Self::serve_file(&path).await
      }
      _ => Ok(Files::serve_html(html! {
        div class="invoice" {
          div class="label" {
            "Lightning Payment Request to access "
            span class="filename" {
                (invoice.memo)
            }
            ":"
          }
          div class="payment-request" {
            (invoice.payment_request)
          }
        }
      })),
    }
  }
}
