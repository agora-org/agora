use crate::{
  environment::Environment,
  error::{Error, Result},
  file_path::FilePath,
  file_stream::FileStream,
  stderr::Stderr,
};
use futures::{future::BoxFuture, FutureExt};
use hyper::{header, service::Service, Body, Request, Response, StatusCode};
use maud::{html, DOCTYPE};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};
use snafu::ResultExt;
use std::{
  convert::Infallible,
  ffi::OsString,
  fmt::Debug,
  fs::FileType,
  io::{self, Write},
  path::{Path, PathBuf},
  task::{self, Poll},
};

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  pub(crate) stderr: Stderr,
  pub(crate) directory: PathBuf,
  base_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn new(environment: &Environment, base_directory: &Path) -> Self {
    Self {
      stderr: environment.stderr.clone(),
      directory: environment.working_directory.join(base_directory),
      base_directory: base_directory.to_owned(),
    }
  }

  async fn response(mut self, request: Request<Body>) -> Response<Body> {
    match self.dispatch(request).await {
      Ok(response) => response,
      Err(error) => {
        writeln!(self.stderr, "{}", error).unwrap();
        Response::builder()
          .status(error.status())
          .body(Body::empty())
          .unwrap()
      }
    }
  }

  async fn dispatch(&self, request: Request<Body>) -> Result<Response<Body>> {
    let file_path = &FilePath::new(&self.base_directory, &self.directory, request.uri())?;
    if file_path.as_ref().is_dir() {
      if !request.uri().path().ends_with('/') {
        return Response::builder()
          .status(StatusCode::FOUND)
          .header(header::LOCATION, String::from(request.uri().path()) + "/")
          .body(Body::empty())
          .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)));
      }

      self
        .list(file_path.as_ref())
        .await
        .context(Error::filesystem_io(file_path))
    } else {
      self.serve_file(file_path).await
    }
  }

  const SET: AsciiSet = NON_ALPHANUMERIC.remove(b'/');

  async fn list(&self, dir: &Path) -> io::Result<Response<Body>> {
    let body = html! {
      (DOCTYPE)
      html {
        head {
          meta charset="utf-8";
          title {
            "foo"
          }
        }
        body {
          ul {
            @for (file_name, file_type) in Self::read_dir(dir).await? {
              @let file_name = {
                let mut file_name = file_name.to_string_lossy().into_owned();
                if file_type.is_dir() {
                  file_name.push('/');
                }
                file_name
              };
              @let encoded = percent_encoding::utf8_percent_encode(&file_name, &Self::SET);
              li {
                a href=(encoded) {
                  (file_name)
                }
                @if file_type.is_file() {
                  " - "
                  a download href=(encoded) {
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

  async fn read_dir(path: &Path) -> io::Result<Vec<(OsString, FileType)>> {
    let mut read_dir = tokio::fs::read_dir(path).await?;
    let mut entries = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
      entries.push((entry.file_name(), entry.file_type().await?));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
  }

  async fn serve_file(&self, path: &FilePath) -> Result<Response<Body>> {
    let mut builder = Response::builder().status(StatusCode::OK);

    if let Some(guess) = path.mime_guess().first() {
      builder = builder.header(header::CONTENT_TYPE, guess.essence_str());
    }

    builder
      .body(Body::wrap_stream(FileStream::new(path.clone()).await?))
      .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)))
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
    test_utils::{test, test_with_environment},
  };
  use guard::guard_unwrap;
  use hyper::StatusCode;
  use pretty_assertions::assert_eq;
  use reqwest::{redirect::Policy, Client, IntoUrl, Url};
  use scraper::{ElementRef, Html, Selector};
  use std::{fs, str};
  use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
  };

  #[track_caller]
  fn assert_contains(haystack: &str, needle: &str) {
    assert!(
      haystack.contains(needle),
      "\n{:?} does not contain {:?}\n",
      haystack,
      needle
    );
  }

  async fn get(url: impl IntoUrl) -> reqwest::Response {
    let response = reqwest::get(url).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
  }

  async fn text(url: impl IntoUrl) -> String {
    get(url).await.text().await.unwrap()
  }

  async fn html(url: &Url) -> Html {
    Html::parse_document(&text(url.clone()).await)
  }

  fn css_select<'a>(html: &'a Html, selector: &'a str) -> Vec<ElementRef<'a>> {
    let selector = Selector::parse(selector).unwrap();
    html.select(&selector).collect::<Vec<_>>()
  }

  #[test]
  fn index_route_status_code_is_200() {
    test(|url, _dir| async move { assert_eq!(reqwest::get(url).await.unwrap().status(), 200) });
  }

  #[test]
  fn index_route_contains_title() {
    test(|url, _dir| async move {
      let haystack = text(url).await;
      let needle = "<title>foo</title>";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn server_aborts_when_directory_does_not_exist() {
    let environment = Environment::test(&[]);

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&environment).unwrap_err();
        guard_unwrap!(let Error::FilesystemIo { .. } = error);
      });
  }

  #[test]
  #[cfg(unix)]
  fn errors_in_request_handling_cause_500_status_codes() {
    use std::os::unix::fs::PermissionsExt;

    test(|url, dir| async move {
      let file = dir.join("www/foo");
      fs::write(&file, "").unwrap();
      let mut permissions = file.metadata().unwrap().permissions();
      permissions.set_mode(0o000);
      fs::set_permissions(file, permissions).unwrap();
      let status = reqwest::get(url.join("foo").unwrap())
        .await
        .unwrap()
        .status();
      assert_eq!(status, 500);
    });
  }

  #[test]
  fn errors_in_request_handling_are_printed_to_stderr() {
    let stderr = test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      reqwest::get(url).await.unwrap();
    });

    assert_contains(&stderr, "IO error accessing filesystem at `www/`: ");

    assert_contains(
      &stderr,
      if cfg!(target_os = "windows") {
        "The system cannot find the path specified."
      } else {
        "No such file or directory"
      },
    );
  }

  #[test]
  fn listing_contains_file() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "").unwrap();
      let haystack = html(&url).await.root_element().html();
      let needle = "some-test-file.txt";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn listing_contains_multiple_files() {
    test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::write(www.join("a.txt"), "").unwrap();
      std::fs::write(www.join("b.txt"), "").unwrap();
      let haystack = html(&url).await.root_element().html();
      assert_contains(&haystack, "a.txt");
      assert_contains(&haystack, "b.txt");
    });
  }

  #[test]
  fn listing_is_sorted_alphabetically() {
    test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::write(www.join("b"), "").unwrap();
      std::fs::write(www.join("c"), "").unwrap();
      std::fs::write(www.join("a"), "").unwrap();
      let html = html(&url).await;
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
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "contents").unwrap();
      let html = html(&url).await;
      guard_unwrap!(let &[a] = css_select(&html, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "some-test-file.txt");
      let file_url = a.value().attr("href").unwrap();
      let file_url = url.join(file_url).unwrap();
      let file_contents = text(file_url).await;
      assert_eq!(file_contents, "contents");
    });
  }

  #[test]
  fn listed_files_have_download_links() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "contents").unwrap();
      let html = html(&url).await;
      guard_unwrap!(let &[a] = css_select(&html, "a[download]").as_slice());
      assert_eq!(a.inner_html(), "download");
      let file_url = a.value().attr("href").unwrap();
      let file_url = url.join(file_url).unwrap();
      let file_contents = text(file_url).await;
      assert_eq!(file_contents, "contents");
    });
  }

  #[test]
  fn listed_files_have_percent_encoded_hrefs() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("filename with special chäracters"), "").unwrap();
      let html = html(&url).await;
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
    let stderr = test(|url, _dir| async move {
      let mut stream = TcpStream::connect(format!("localhost:{}", url.port().unwrap()))
        .await
        .unwrap();
      stream
        .write_all(b"GET /foo/../bar.txt HTTP/1.1\n\n")
        .await
        .unwrap();
      let response = &mut [0; 1024];
      let bytes = stream.read(response).await.unwrap();
      let response = str::from_utf8(&response[..bytes]).unwrap();
      assert_contains(&response, "HTTP/1.1 400 Bad Request");
    });
    assert_contains(&stderr, &"Invalid URL file path: /foo/../bar.txt");
  }

  #[test]
  fn disallow_empty_path_component() {
    let stderr = test(|url, _dir| async move {
      assert_eq!(
        reqwest::get(format!("{}foo//bar.txt", url))
          .await
          .unwrap()
          .status(),
        StatusCode::BAD_REQUEST
      )
    });
    assert_contains(&stderr, &"Invalid URL file path: /foo//bar.txt");
  }

  #[test]
  fn disallow_absolute_path() {
    let stderr = test(|url, _dir| async move {
      assert_eq!(
        reqwest::get(format!("{}/foo.txt", url))
          .await
          .unwrap()
          .status(),
        StatusCode::BAD_REQUEST
      )
    });
    assert_contains(&stderr, &"Invalid URL file path: //foo.txt");
  }

  #[test]
  fn return_404_for_missing_files() {
    let stderr = test(|url, _dir| async move {
      assert_eq!(
        reqwest::get(url.join("foo.txt").unwrap())
          .await
          .unwrap()
          .status(),
        StatusCode::NOT_FOUND
      )
    });
    assert_contains(&stderr, "IO error accessing filesystem at `www/foo.txt`");
  }

  #[test]
  fn configure_source_directory() {
    let environment = Environment::test(&["--directory", "src"]);

    let src = environment.working_directory.join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("foo.txt"), "hello").unwrap();

    test_with_environment(&environment, |url, _dir| async move {
      assert_contains(&text(url.clone()).await, "foo.txt");

      let file_contents = text(url.join("foo.txt").unwrap()).await;
      assert_eq!(file_contents, "hello");
    });
  }

  #[test]
  #[cfg(unix)]
  fn downloaded_files_are_streamed() {
    use futures::StreamExt;
    use tokio::{fs::OpenOptions, sync::oneshot};

    test(|url, dir| async move {
      let fifo_path = dir.join("www").join("fifo");

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

      let mut stream = get(url.join("fifo").unwrap()).await.bytes_stream();

      assert_eq!(stream.next().await.unwrap().unwrap(), "hello");

      sender.send(()).unwrap();

      writer.await.unwrap();
    });
  }

  #[test]
  fn downloaded_files_have_correct_content_type() {
    test(|url, dir| async move {
      fs::write(dir.join("www/foo.mp4"), "hello").unwrap();

      let response = get(url.join("foo.mp4").unwrap()).await;

      assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "video/mp4"
      );
    });
  }

  #[test]
  fn unknown_files_have_no_content_type() {
    test(|url, dir| async move {
      fs::write(dir.join("www/foo"), "hello").unwrap();

      let response = get(url.join("foo").unwrap()).await;

      assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
    });
  }

  #[test]
  fn filenames_with_spaces() {
    test(|url, dir| async move {
      fs::write(dir.join("www/foo bar"), "hello").unwrap();

      let response = text(url.join("foo%20bar").unwrap()).await;

      assert_eq!(response, "hello");
    });
  }

  #[test]
  fn subdirectories_appear_in_listings() {
    test(|url, dir| async move {
      std::fs::create_dir(dir.join("www/foo")).unwrap();
      std::fs::write(dir.join("www/foo/bar.txt"), "hello").unwrap();
      let root_listing = html(&url).await;
      guard_unwrap!(let &[a] = css_select(&root_listing, "a").as_slice());
      assert_eq!(a.inner_html(), "foo/");
      let subdir_url = url.join(a.value().attr("href").unwrap()).unwrap();
      let subdir_listing = html(&subdir_url).await;
      guard_unwrap!(let &[a] = css_select(&subdir_listing, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "bar.txt");
      let file_url = subdir_url.join(a.value().attr("href").unwrap()).unwrap();
      assert_eq!(text(file_url).await, "hello");
    });
  }

  #[test]
  fn no_trailing_slash_redirects_to_trailing_slash() {
    test(|url, dir| async move {
      std::fs::create_dir(dir.join("www/foo")).unwrap();
      let client = Client::builder().redirect(Policy::none()).build().unwrap();
      let request = client.get(url.join("foo").unwrap()).build().unwrap();
      let response = client.execute(request).await.unwrap();
      assert_eq!(response.status(), StatusCode::FOUND);
      assert_eq!(
        url
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
        url.join("foo/").unwrap()
      );
    });
  }

  #[test]
  fn redirects_correctly_for_two_layers_of_subdirectories() {
    test(|url, dir| async move {
      std::fs::create_dir_all(dir.join("www/foo/bar")).unwrap();
      std::fs::write(dir.join("www/foo/bar/baz.txt"), "").unwrap();
      let listing = html(&url.join("foo/bar").unwrap()).await;
      guard_unwrap!(let &[a] = css_select(&listing, "a:not([download])").as_slice());
      assert_eq!(a.inner_html(), "baz.txt")
    });
  }

  #[test]
  fn file_errors_are_associated_with_file_path() {
    let stderr = test(|url, dir| async move {
      std::fs::create_dir(dir.join("www/foo")).unwrap();
      assert_eq!(
        reqwest::get(url.join("foo/bar.txt").unwrap())
          .await
          .unwrap()
          .status(),
        StatusCode::NOT_FOUND
      )
    });
    assert_contains(
      &stderr,
      "IO error accessing filesystem at `www/foo/bar.txt`",
    );
  }
}
