use crate::{
  error::{Error, Result},
  file_stream::FileStream,
  input_path::InputPath,
  redirect::redirect,
};
use hyper::{header, Body, Request, Response, StatusCode};
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

  pub(crate) async fn serve(
    &self,
    request: &Request<Body>,
    tail: &[&str],
  ) -> Result<Response<Body>> {
    let file_path = self.base_directory.join_file_path(&tail.join(""))?;

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
      self.serve_file(&file_path).await
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
      if file_type.is_symlink() {
        continue;
      }
      entries.push((entry.file_name(), file_type));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
  }

  const ENCODE_CHARACTERS: AsciiSet = NON_ALPHANUMERIC.remove(b'/');

  async fn list(dir: &InputPath) -> Result<Response<Body>> {
    let body = html! {
      (DOCTYPE)
      html {
        head {
          meta charset="utf-8";
          title {
            "agora"
          }
          link rel="stylesheet" href="/static/index.css";
        }
        body {
          ul class="contents" {
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
          }
        }
      }
    };

    Ok(Response::new(Body::from(body.into_string())))
  }

  fn download_icon() -> Markup {
    html! {
      svg class="icon" {
        use href="/static/feather-sprite.svg#download" {}
      }
    }
  }

  async fn serve_file(&self, path: &InputPath) -> Result<Response<Body>> {
    dbg!(path);
    if let Some(lnd_client) = &self.lnd_client {
      let invoice_id = "foooooo";
      return redirect(format!("/invoices/{}", invoice_id));
    }
    let mut builder = Response::builder().status(StatusCode::OK);

    if let Some(guess) = path.mime_guess().first() {
      builder = builder.header(header::CONTENT_TYPE, guess.essence_str());
    }

    builder
      .body(Body::wrap_stream(FileStream::new(path.clone()).await?))
      .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)))
  }
}
