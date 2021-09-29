pub(crate) use std::{
  convert::Infallible,
  env,
  ffi::OsString,
  fs::{self, FileType},
  future,
  future::Future,
  io::{self, Write},
  mem::MaybeUninit,
  net::{SocketAddr, ToSocketAddrs},
  path::{Path, PathBuf},
  pin::Pin,
  str,
  sync::Arc,
  task::{Context, Poll},
  time::Duration,
};

pub(crate) use ::{
  agora_lnd_client::Millisatoshi,
  futures::{
    future::{BoxFuture, OptionFuture},
    FutureExt, Stream, StreamExt,
  },
  http::uri::Authority,
  hyper::{
    header::{self, HeaderValue},
    server::conn::AddrIncoming,
    service::Service,
    Body, Request, Response, StatusCode,
  },
  lexiclean::Lexiclean,
  maud::Markup,
  serde::Deserialize,
  snafu::{IntoError, ResultExt},
  structopt::StructOpt,
  tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    task,
  },
};

pub(crate) use crate::{
  arguments::Arguments,
  config::Config,
  environment::Environment,
  error::{self, Error, Result},
  error_page, html,
  https_redirect_service::HttpsRedirectService,
  https_request_handler::HttpsRequestHandler,
  input_path::InputPath,
  redirect::redirect,
  request_handler::RequestHandler,
  server::Server,
  stderr::Stderr,
};

#[cfg(test)]
mod test {
  pub(crate) use tempfile::TempDir;
}

#[cfg(test)]
pub(crate) use test::*;
