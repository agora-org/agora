use crate::{
  environment::Environment,
  error::{self, Error, Result},
  file_stream::FileStream,
  input_path::InputPath,
  redirect::redirect,
  request_handler::RequestHandler,
  static_assets::StaticAssets,
  stderr::Stderr,
};
use futures::{future::BoxFuture, FutureExt};
use hyper::{
  header::{self, HeaderValue},
  service::Service,
  Body, Request, Response, StatusCode,
};
use maud::{html, DOCTYPE};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use snafu::ResultExt;
use std::{
  convert::Infallible,
  ffi::OsString,
  fmt::Debug,
  fs::FileType,
  io::Write,
  path::Path,
  task::{self, Poll},
};

pub(crate) async fn serve(
  base_directory: &InputPath,
  request: &Request<Body>,
  tail: &[&str],
) -> Result<Response<Body>> {
  let file_path = base_directory.join_file_path(&tail.join(""))?;

  for result in base_directory.iter_prefixes(tail) {
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
    crate::files::list(&file_path).await
  } else {
    crate::files::serve_file(&file_path).await
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
          "foo"
        }
        link rel="stylesheet" href="/static/index.css";
      }
      body {
        ul {
          @for (file_name, file_type) in crate::files::read_dir(dir).await? {
            @let file_name = {
              let mut file_name = file_name.to_string_lossy().into_owned();
              if file_type.is_dir() {
                file_name.push('/');
              }
              file_name
            };
            @let encoded = percent_encoding::utf8_percent_encode(&file_name, &ENCODE_CHARACTERS);
            li {
              a href=(encoded) {
                (file_name)
              }
              @if file_type.is_file() {
                " - "
                a download href=(encoded) class=("download") {
                  "download"
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

async fn serve_file(path: &InputPath) -> Result<Response<Body>> {
  let mut builder = Response::builder().status(StatusCode::OK);

  if let Some(guess) = path.mime_guess().first() {
    builder = builder.header(header::CONTENT_TYPE, guess.essence_str());
  }

  builder
    .body(Body::wrap_stream(FileStream::new(path.clone()).await?))
    .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)))
}
