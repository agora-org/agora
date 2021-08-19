use crate::{
  environment::Environment,
  error::{self, Error, Result},
  files::Files,
  html,
  input_path::InputPath,
  redirect::redirect,
  static_assets::StaticAssets,
  stderr::Stderr,
};
use futures::{future::BoxFuture, FutureExt};
use hyper::{
  header::{self, HeaderValue},
  service::Service,
  Body, Request, Response,
};
use maud::html;
use snafu::ResultExt;
use std::{
  collections::BTreeMap,
  convert::Infallible,
  io::Write,
  path::Path,
  task::{self, Poll},
};

#[derive(Clone)]
pub(crate) struct RequestHandler {
  pub(crate) stderr: Stderr,
  pub(crate) files: Files,
}

impl RequestHandler {
  pub(crate) fn new(
    environment: &Environment,
    base_directory: &Path,
    lnd_client: Option<agora_lnd_client::Client>,
  ) -> Self {
    Self {
      stderr: environment.stderr.clone(),
      files: Files::new(InputPath::new(environment, base_directory), lnd_client),
    }
  }

  async fn response(self, request: Request<Body>) -> Response<Body> {
    let mut stderr = self.stderr.clone();

    match self.response_result(request).await {
      Ok(response) => response,
      Err(error) => {
        error.print_backtrace(&mut stderr);
        writeln!(stderr, "{}", error).ok();
        let mut response = html::wrap_body(html! {
          h1 {
            (error.status())
          }
        });
        *response.status_mut() = error.status();
        response
      }
    }
  }

  async fn response_result(mut self, request: Request<Body>) -> Result<Response<Body>> {
    tokio::spawn(async move { self.dispatch(request).await.map(Self::add_global_headers) })
      .await
      .context(error::RequestHandlerPanic)?
  }

  fn add_global_headers(mut response: Response<Body>) -> Response<Body> {
    response.headers_mut().insert(
      header::CACHE_CONTROL,
      HeaderValue::from_static("no-store, max-age=0"),
    );
    response
  }

  fn decode_invoice_id(invoice_id_hex: &str) -> Result<[u8; 32]> {
    let mut invoice_id = [0; 32];
    hex::decode_to_slice(invoice_id_hex, &mut invoice_id).context(error::InvoiceId)?;
    Ok(invoice_id)
  }

  async fn dispatch(&mut self, request: Request<Body>) -> Result<Response<Body>> {
    let components = request
      .uri()
      .path()
      .split_inclusive('/')
      .collect::<Vec<&str>>();

    let query = if let Some(query) = request.uri().query() {
      form_urlencoded::parse(query.as_bytes()).collect::<BTreeMap<_, _>>()
    } else {
      BTreeMap::new()
    };

    match components.as_slice() {
      ["/"] => redirect(String::from(request.uri().path()) + "files/"),
      ["/", "static/", tail @ ..] => StaticAssets::serve(tail),
      ["/", "files"] => redirect(String::from(request.uri().path()) + "/"),
      ["/", "files/", tail @ ..] if query.contains_key("invoice") => {
        let invoice_id = &query["invoice"];
        let invoice_id = Self::decode_invoice_id(invoice_id)?;
        self.files.serve_invoice(&request, tail, invoice_id).await
      }
      ["/", "files/", tail @ ..] => self.files.serve(&request, tail).await,
      ["/", "invoice/", file_name] if file_name.ends_with(".svg") => {
        let invoice_id = Self::decode_invoice_id(
          file_name
            .strip_suffix(".svg")
            .expect("file_name ends with `.svg`"),
        )?;
        self.files.serve_invoice_qr_code(&request, invoice_id).await
      }
      _ => Err(Error::RouteNotFound {
        uri_path: request.uri().path().to_owned(),
      }),
    }
  }
}

impl Service<Request<Body>> for RequestHandler {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    self.clone().response(request).map(Ok).boxed()
  }
}
