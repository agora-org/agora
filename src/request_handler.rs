use crate::common::*;

use crate::{error_page, files::Files, static_assets::StaticAssets};

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

  async fn response(mut self, request: Request<Body>) -> Result<Response<Body>> {
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
    let path = percent_encoding::percent_decode_str(request.uri().path())
      .decode_utf8()
      .context(error::InvalidUriPath {
        uri_path: request.uri().path(),
      })?;
    let components = path.split_inclusive('/').collect::<Vec<&str>>();

    let invoice_parameter = request.uri().query().and_then(|query| {
      form_urlencoded::parse(query.as_bytes())
        .filter(|(key, _value)| key == "invoice")
        .last()
        .map(|(_key, value)| value.into_owned())
    });

    match components.as_slice() {
      ["/"] => redirect(String::from(request.uri().path()) + "files/"),
      ["/", "static/", tail @ ..] => StaticAssets::serve(tail),
      ["/", "files"] => redirect(String::from(request.uri().path()) + "/"),
      ["/", "files/", tail @ ..] if invoice_parameter.is_some() => {
        let invoice_id = invoice_parameter.expect("invoice_parameter is some");
        let invoice_id = Self::decode_invoice_id(&invoice_id)?;
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

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, request: Request<Body>) -> Self::Future {
    let stderr = self.stderr.clone();
    self
      .clone()
      .response(request)
      .map(move |result| error_page::map_error(stderr, result))
      .boxed()
  }
}
