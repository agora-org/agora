use crate::{
  environment::Environment,
  error::{self, Error, Result},
  files::Files,
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
use snafu::ResultExt;
use std::{
  convert::Infallible,
  fmt::Debug,
  io::Write,
  path::Path,
  task::{self, Poll},
};

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  pub(crate) stderr: Stderr,
  pub(crate) files: Files,
}

impl RequestHandler {
  pub(crate) fn new(
    environment: &Environment,
    base_directory: &Path,
    lnd_client: Option<lnd_client::Client>,
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
        writeln!(stderr, "{}", error).unwrap();
        Response::builder()
          .status(error.status())
          .body(Body::empty())
          .unwrap()
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

  async fn dispatch(&mut self, request: Request<Body>) -> Result<Response<Body>> {
    let components = request
      .uri()
      .path()
      .split_inclusive('/')
      .collect::<Vec<&str>>();
    match components.as_slice() {
      ["/"] => redirect(String::from(request.uri().path()) + "files/"),
      ["/", "static/", tail @ ..] => StaticAssets::serve(tail),
      ["/", "files/", tail @ ..] => self.files.serve(&request, tail).await,
      ["/", "invoices/", invoice_index] => match invoice_index.parse() {
        Ok(invoice_index) => self.files.serve_invoice(&request, invoice_index).await,
        Err(_) => Err(Error::not_found(&request)),
      },
      _ => Err(Error::not_found(&request)),
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

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use crate::{
    error::Error,
    server::Server,
    test_utils::{assert_contains, test, test_with_environment, test_with_lnd, TestContext},
  };
  use cradle::*;
  use guard::guard_unwrap;
  use hyper::StatusCode;
  use lnd_test_context::LndTestContext;
  use pretty_assertions::assert_eq;
  use regex::Regex;
  use reqwest::{redirect::Policy, Client, Url};
  use scraper::{ElementRef, Html, Selector};
  use std::{fs, path::MAIN_SEPARATOR, str};
  use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
  };

  async fn get(url: &Url) -> reqwest::Response {
    let response = reqwest::get(url.clone()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
  }

  async fn text(url: &Url) -> String {
    get(url).await.text().await.unwrap()
  }

  async fn html(url: &Url) -> Html {
    Html::parse_document(&text(url).await)
  }

  fn css_select<'a>(html: &'a Html, selector: &'a str) -> Vec<ElementRef<'a>> {
    let selector = Selector::parse(selector).unwrap();
    html.select(&selector).collect::<Vec<_>>()
  }

  #[test]
  fn index_route_redirects_to_files() {
    test(|context| async move {
      let client = Client::builder().redirect(Policy::none()).build().unwrap();
      let request = client.get(context.base_url().clone()).build().unwrap();
      let response = client.execute(request).await.unwrap();
      assert_eq!(response.status(), StatusCode::FOUND);
      assert_eq!(
        &context
          .base_url()
          .join(
            response
              .headers()
              .get(header::LOCATION)
              .unwrap()
              .to_str()
              .unwrap()
          )
          .unwrap(),
        context.files_url()
      );
    });
  }

  #[test]
  fn index_route_status_code_is_200() {
    test(|context| async move {
      assert_eq!(
        reqwest::get(context.base_url().clone())
          .await
          .unwrap()
          .status(),
        200
      )
    });
  }

  #[test]
  fn unknown_route_status_code_is_404() {
    test(|context| async move {
      assert_eq!(
        reqwest::get(context.base_url().join("huhu").unwrap())
          .await
          .unwrap()
          .status(),
        404
      )
    });
  }

  #[test]
  fn index_route_contains_title() {
    test(|context| async move {
      let haystack = text(context.base_url()).await;
      let needle = "<title>agora</title>";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn server_aborts_when_directory_does_not_exist() {
    let mut environment = Environment::test(&[]);

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&mut environment).await.unwrap_err();
        guard_unwrap!(let Error::FilesystemIo { .. } = error);
      });
  }

  #[test]
  #[cfg(unix)]
  fn errors_in_request_handling_cause_500_status_codes() {
    use std::os::unix::fs::PermissionsExt;

    let stderr = test(|context| async move {
      let file = context.files_directory().join("foo");
      fs::write(&file, "").unwrap();
      let mut permissions = file.metadata().unwrap().permissions();
      permissions.set_mode(0o000);
      fs::set_permissions(file, permissions).unwrap();
      let status = reqwest::get(context.files_url().join("foo").unwrap())
        .await
        .unwrap()
        .status();
      assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    });

    assert_contains(
      &stderr,
      "IO error accessing filesystem at `www/foo`: Permission denied (os error 13)",
    );
  }

  #[test]
  fn listing_contains_file() {
    test(|context| async move {
      std::fs::write(context.files_directory().join("some-test-file.txt"), "").unwrap();
      let haystack = html(context.base_url()).await.root_element().html();
      let needle = "some-test-file.txt";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn listing_contains_multiple_files() {
    test(|context| async move {
      std::fs::write(context.files_directory().join("a.txt"), "").unwrap();
      std::fs::write(context.files_directory().join("b.txt"), "").unwrap();
      let haystack = html(context.base_url()).await.root_element().html();
      assert_contains(&haystack, "a.txt");
      assert_contains(&haystack, "b.txt");
    });
  }

  #[test]
  fn listing_is_sorted_alphabetically() {
    test(|context| async move {
      std::fs::write(context.files_directory().join("b"), "").unwrap();
      std::fs::write(context.files_directory().join("c"), "").unwrap();
      std::fs::write(context.files_directory().join("a"), "").unwrap();
      let html = html(context.base_url()).await;
      let haystack: Vec<&str> = css_select(&html, "a:not([download])")
        .into_iter()
        .map(|x| x.text())
        .flatten()
        .collect();
      assert_eq!(haystack, vec!["a", "b", "c"]);
    });
  }

  #[test]
  fn listed_files_can_be_played_in_browser() {
    test(|context| async move {
      std::fs::write(
        context.files_directory().join("some-test-file.txt"),
        "contents",
      )
      .unwrap();
      let html = html(context.files_url()).await;
      guard_unwrap!(let &[a] = css_select(&html, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "some-test-file.txt");
      let file_url = a.value().attr("href").unwrap();
      let file_url = context.files_url().join(file_url).unwrap();
      let file_contents = text(&file_url).await;
      assert_eq!(file_contents, "contents");
    });
  }

  #[test]
  fn listed_files_have_download_links() {
    test(|context| async move {
      std::fs::write(
        context.files_directory().join("some-test-file.txt"),
        "contents",
      )
      .unwrap();
      let html = html(&context.files_url()).await;
      guard_unwrap!(let &[a] = css_select(&html, "a[download]").as_slice());
      assert_contains(&a.inner_html(), "download");
      let file_url = a.value().attr("href").unwrap();
      let file_url = context.files_url().join(file_url).unwrap();
      let file_contents = text(&file_url).await;
      assert_eq!(file_contents, "contents");
    });
  }

  #[test]
  fn listed_files_have_percent_encoded_hrefs() {
    test(|context| async move {
      std::fs::write(
        context
          .files_directory()
          .join("filename with special chäracters"),
        "",
      )
      .unwrap();
      let html = html(context.base_url()).await;
      let links = css_select(&html, "a");
      assert_eq!(links.len(), 2);
      for link in links {
        assert_eq!(
          link.value().attr("href").unwrap(),
          "filename%20with%20special%20ch%C3%A4racters"
        );
      }
    });
  }

  #[test]
  fn disallow_parent_path_component() {
    let stderr = test(|context| async move {
      let mut stream =
        TcpStream::connect(format!("localhost:{}", context.base_url().port().unwrap()))
          .await
          .unwrap();
      stream
        .write_all(b"GET /files/foo/../bar.txt HTTP/1.1\n\n")
        .await
        .unwrap();
      let response = &mut [0; 1024];
      let bytes = stream.read(response).await.unwrap();
      let response = str::from_utf8(&response[..bytes]).unwrap();
      assert_contains(&response, "HTTP/1.1 400 Bad Request");
    });
    assert_contains(&stderr, &"Invalid URI file path: foo/../bar.txt");
  }

  #[test]
  fn disallow_empty_path_component() {
    let stderr = test(|context| async move {
      assert_eq!(
        reqwest::get(format!("{}foo//bar.txt", context.files_url()))
          .await
          .unwrap()
          .status(),
        StatusCode::BAD_REQUEST
      )
    });
    assert_contains(&stderr, &"Invalid URI file path: foo//bar.txt");
  }

  #[test]
  fn disallow_absolute_path() {
    let stderr = test(|context| async move {
      assert_eq!(
        reqwest::get(format!("{}/foo.txt", context.files_url()))
          .await
          .unwrap()
          .status(),
        StatusCode::BAD_REQUEST
      )
    });
    assert_contains(&stderr, &"Invalid URI file path: /foo.txt");
  }

  #[test]
  fn return_404_for_missing_files() {
    let stderr = test(|context| async move {
      assert_eq!(
        reqwest::get(context.files_url().join("foo.txt").unwrap())
          .await
          .unwrap()
          .status(),
        StatusCode::NOT_FOUND
      )
    });
    assert_contains(
      &stderr,
      &format!(
        "IO error accessing filesystem at `www{}foo.txt`",
        MAIN_SEPARATOR
      ),
    );
  }

  #[test]
  fn configure_source_directory() {
    let mut environment = Environment::test(&["--directory", "src"]);

    let src = environment.working_directory.join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("foo.txt"), "hello").unwrap();

    test_with_environment(&mut environment, |context| async move {
      assert_contains(&text(context.files_url()).await, "foo.txt");

      let file_contents = text(&context.files_url().join("foo.txt").unwrap()).await;
      assert_eq!(file_contents, "hello");
    });
  }

  #[test]
  #[cfg(unix)]
  fn downloaded_files_are_streamed() {
    use futures::StreamExt;
    use tokio::{fs::OpenOptions, sync::oneshot};

    test(|context| async move {
      let fifo_path = context.files_directory().join("fifo");

      nix::unistd::mkfifo(&fifo_path, nix::sys::stat::Mode::S_IRWXU).unwrap();

      let (sender, receiver) = oneshot::channel();

      let writer = tokio::spawn(async move {
        let mut fifo = OpenOptions::new()
          .write(true)
          .open(&fifo_path)
          .await
          .unwrap();
        fifo.write_all(b"hello").await.unwrap();
        receiver.await.unwrap();
      });

      let mut stream = get(&context.files_url().join("fifo").unwrap())
        .await
        .bytes_stream();

      assert_eq!(stream.next().await.unwrap().unwrap(), "hello");

      sender.send(()).unwrap();

      writer.await.unwrap();
    });
  }

  #[test]
  fn downloaded_files_have_correct_content_type() {
    test(|context| async move {
      fs::write(context.files_directory().join("foo.mp4"), "hello").unwrap();

      let response = get(&context.files_url().join("foo.mp4").unwrap()).await;

      assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "video/mp4"
      );
    });
  }

  #[test]
  fn unknown_files_have_no_content_type() {
    test(|context| async move {
      fs::write(context.files_directory().join("foo"), "hello").unwrap();

      let response = get(&context.files_url().join("foo").unwrap()).await;

      assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
    });
  }

  #[test]
  fn filenames_with_spaces() {
    test(|context| async move {
      fs::write(context.files_directory().join("foo bar"), "hello").unwrap();

      let response = text(&context.files_url().join("foo%20bar").unwrap()).await;

      assert_eq!(response, "hello");
    });
  }

  #[test]
  fn subdirectories_appear_in_listings() {
    test(|context| async move {
      std::fs::create_dir(context.files_directory().join("foo")).unwrap();
      std::fs::write(context.files_directory().join("foo/bar.txt"), "hello").unwrap();
      let root_listing = html(context.files_url()).await;
      guard_unwrap!(let &[a] = css_select(&root_listing, "a").as_slice());
      assert_eq!(a.inner_html(), "foo/");
      let subdir_url = context
        .files_url()
        .join(a.value().attr("href").unwrap())
        .unwrap();
      let subdir_listing = html(&subdir_url).await;
      guard_unwrap!(let &[a] = css_select(&subdir_listing, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "bar.txt");
      let file_url = subdir_url.join(a.value().attr("href").unwrap()).unwrap();
      assert_eq!(text(&file_url).await, "hello");
    });
  }

  #[test]
  fn no_trailing_slash_redirects_to_trailing_slash() {
    test(|context| async move {
      std::fs::create_dir(context.files_directory().join("foo")).unwrap();
      let client = Client::builder().redirect(Policy::none()).build().unwrap();
      let request = client
        .get(context.files_url().join("foo").unwrap())
        .build()
        .unwrap();
      let response = client.execute(request).await.unwrap();
      assert_eq!(response.status(), StatusCode::FOUND);
      assert_eq!(
        context
          .files_url()
          .join("foo")
          .unwrap()
          .join(
            response
              .headers()
              .get(header::LOCATION)
              .unwrap()
              .to_str()
              .unwrap()
          )
          .unwrap(),
        context.files_url().join("foo/").unwrap()
      );
    });
  }

  #[test]
  fn redirects_correctly_for_two_layers_of_subdirectories() {
    test(|context| async move {
      std::fs::create_dir_all(context.files_directory().join("foo/bar")).unwrap();
      std::fs::write(context.files_directory().join("foo/bar/baz.txt"), "").unwrap();
      let listing = html(&context.files_url().join("foo/bar").unwrap()).await;
      guard_unwrap!(let &[a] = css_select(&listing, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "baz.txt")
    });
  }

  #[test]
  fn file_errors_are_associated_with_file_path() {
    let stderr = test(|context| async move {
      std::fs::create_dir(context.files_directory().join("foo")).unwrap();
      assert_eq!(
        reqwest::get(context.files_url().join("foo/bar.txt").unwrap())
          .await
          .unwrap()
          .status(),
        StatusCode::NOT_FOUND
      )
    });
    assert_contains(
      &stderr,
      &format!(
        "IO error accessing filesystem at `www{}foo{}bar.txt`",
        MAIN_SEPARATOR, MAIN_SEPARATOR,
      ),
    );
  }

  #[test]
  fn requesting_files_with_trailing_slash_redirects() {
    test(|context| async move {
      std::fs::write(context.files_directory().join("foo"), "").unwrap();
      let response = reqwest::get(context.files_url().join("foo/").unwrap())
        .await
        .unwrap();
      assert!(
        response.url().as_str().ends_with("/files/foo"),
        "{} didn't end with /files/foo",
        response.url()
      );
    });
  }

  #[test]
  fn listings_are_not_cached() {
    test(|context| async move {
      let response = reqwest::get(context.files_url().clone()).await.unwrap();
      assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0",
      );
    });
  }

  #[test]
  fn files_are_not_cached() {
    test(|context| async move {
      std::fs::write(context.files_directory().join("foo"), "bar").unwrap();
      let response = reqwest::get(context.files_url().join("foo").unwrap())
        .await
        .unwrap();
      assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store, max-age=0",
      );
      assert_eq!(response.text().await.unwrap(), "bar");
    });
  }

  fn symlink(original: impl AsRef<Path>, link: impl AsRef<Path>) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(original, link).unwrap();
    #[cfg(windows)]
    if original.as_ref().is_dir() {
      std::os::windows::fs::symlink_dir(original, link).unwrap();
    } else {
      std::os::windows::fs::symlink_file(original, link).unwrap();
    }
  }

  #[test]
  fn disallow_file_downloads_via_symlinks() {
    test(|context| async move {
      let file = context.files_directory().join("file");
      std::fs::write(&file, "contents").unwrap();
      symlink(file, context.files_directory().join("link"));
      let response = reqwest::get(context.files_url().join("link").unwrap())
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
  }

  #[test]
  fn disallow_file_downloads_via_intermediate_symlinks() {
    test(|context| async move {
      let dir = context.files_directory().join("dir");
      std::fs::create_dir(&dir).unwrap();
      symlink(&dir, context.files_directory().join("link"));
      std::fs::write(dir.join("file"), "contents").unwrap();
      let response = reqwest::get(context.files_url().join("link/file").unwrap())
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
  }

  #[test]
  fn disallow_listing_directories_via_symlinks() {
    test(|context| async move {
      let dir = context.files_directory().join("dir");
      std::fs::create_dir(&dir).unwrap();
      symlink(dir, context.files_directory().join("link"));
      let response = reqwest::get(context.files_url().join("link").unwrap())
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
  }

  #[test]
  fn disallow_listing_directories_via_intermediate_symlinks() {
    test(|context| async move {
      let dir = context.files_directory().join("dir");
      std::fs::create_dir(&dir).unwrap();
      symlink(&dir, context.files_directory().join("link"));
      std::fs::create_dir(dir.join("subdir")).unwrap();
      let response = reqwest::get(context.files_url().join("link/subdir").unwrap())
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
  }

  #[test]
  fn remove_symlinks_from_listings() {
    test(|context| async move {
      let file = context.files_directory().join("file");
      std::fs::write(&file, "").unwrap();
      symlink(file, context.files_directory().join("link"));
      let html = html(context.files_url()).await;
      guard_unwrap!(let &[a] = css_select(&html, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "file");
    });
  }

  #[test]
  fn serves_static_assets() {
    test(|context| async move {
      let response = text(&context.base_url().join("static/index.css").unwrap()).await;
      let expected = std::fs::read_to_string("static/index.css").unwrap();
      assert_eq!(response, expected);
    });
  }

  #[test]
  fn sets_mime_types_for_static_assets() {
    test(|context| async move {
      let response = get(&context.base_url().join("static/index.css").unwrap()).await;
      assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/css"
      );
    });
  }

  #[test]
  fn missing_asset_not_found() {
    test(|context| async move {
      let response = reqwest::get(context.base_url().join("static/does-not-exist").unwrap())
        .await
        .unwrap();
      assert_eq!(response.status(), StatusCode::NOT_FOUND);
    });
  }

  #[test]
  fn redirects_to_invoice_url() {
    test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
      std::fs::write(context.files_directory().join("foo"), "").unwrap();
      let response = reqwest::get(context.files_url().join("foo").unwrap())
        .await
        .unwrap();
      let regex = Regex::new("^/invoices/[0-9]+$").unwrap();
      assert!(
        regex.is_match(response.url().path()),
        "Response URL path was not invoice path: {}",
        response.url().path(),
      );
    });
  }

  #[test]
  fn non_existant_files_dont_redirect_to_invoice() {
    let stderr = test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
      assert_eq!(
        reqwest::get(context.files_url().join("foo.txt").unwrap())
          .await
          .unwrap()
          .status(),
        StatusCode::NOT_FOUND
      )
    });
    assert_contains(
      &stderr,
      &format!(
        "IO error accessing filesystem at `www{}foo.txt`",
        MAIN_SEPARATOR
      ),
    );
  }

  #[test]
  fn invoice_url_serves_bech32_encoded_invoice() {
    test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
      std::fs::write(context.files_directory().join("foo"), "").unwrap();
      let html = html(&context.files_url().join("foo").unwrap()).await;
      guard_unwrap!(let &[payment_request] = css_select(&html, ".payment-request").as_slice());
      assert_contains(&payment_request.inner_html(), "lnbcrt1");
    });
  }

  #[test]
  fn paying_invoice_allows_downloading_file() {
    let receiver = LndTestContext::new_blocking();
    test_with_lnd(&receiver.clone(), |context: TestContext| async move {
      std::fs::write(context.files_directory().join("foo"), "precious content").unwrap();
      let response = get(&context.files_url().join("foo").unwrap()).await;
      let invoice_url = response.url().clone();
      let html = Html::parse_document(&response.text().await.unwrap());
      guard_unwrap!(let &[payment_request] = css_select(&html, ".payment-request").as_slice());
      let payment_request = payment_request.inner_html();
      dbg!(&payment_request);
      let sender = LndTestContext::new().await;
      sender.connect(&receiver).await;
      sender.generate_money_into_lnd().await;
      sender.open_channel_to(&receiver, 1_000_000).await;
      cmd_unit!(sender.lncli_command().await, %"walletbalance");
      cmd_unit!(sender.lncli_command().await, %"payinvoice --amt 1000 --force", &payment_request);
      assert_eq!(text(&invoice_url).await, "precious content");
      // fixme: fix non-deterministic tests
    });
  }
}
