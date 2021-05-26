use crate::{
  environment::Environment,
  error::{self, Error, Result},
  file_stream::FileStream,
  input_path::InputPath,
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

pub(crate) fn redirect(location: String) -> Result<Response<Body>> {
  Response::builder()
    .status(StatusCode::FOUND)
    .header(header::LOCATION, location)
    .body(Body::empty())
    .map_err(|error| Error::internal(format!("Failed to construct redirect response: {}", error)))
}
