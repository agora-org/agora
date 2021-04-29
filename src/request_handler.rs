use crate::{
  environment::Environment,
  error::{self, Error, Result},
  stderr::Stderr,
};
use futures::{future::BoxFuture, FutureExt};
use hyper::{service::Service, Body, Request, Response, Uri};
use maud::{html, DOCTYPE};
use snafu::ResultExt;
use std::{
  convert::Infallible,
  fmt::Debug,
  io::{self, Write},
  path::{Component, Path, PathBuf},
  task::{self, Poll},
};

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  pub(crate) stderr: Stderr,
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn new(environment: &Environment) -> Self {
    Self {
      stderr: environment.stderr.clone(),
      working_directory: environment.working_directory.to_owned(),
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
    request.uri().path();
    match request.uri().path() {
      "/" => self.list_www().await.context(error::WwwIo),
      _ => self.serve_file(&FilePath::from_uri(request.uri())?).await,
    }
  }

  async fn list_www(&self) -> io::Result<Response<Body>> {
    let mut read_dir = tokio::fs::read_dir(self.working_directory.join("www")).await?;
    let body = html! {
      (DOCTYPE)
      html {
        head {
          title {
            "foo"
          }
        }
        body {
          @while let Some(entry) = read_dir.next_entry().await? {
            a download href=(format!("./{}", entry.file_name().to_string_lossy())) {
              (entry.file_name().to_string_lossy())
            }
            br;
          }
        }
      }
    };

    Ok(Response::new(Body::from(body.into_string())))
  }

  async fn serve_file(&self, file_path: &FilePath) -> Result<Response<Body>> {
    let file_contents = tokio::fs::read(self.working_directory.join("www").join(file_path))
      .await
      .context(error::FileIo)?;
    Ok(Response::new(Body::from(file_contents)))
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

#[derive(Debug)]
struct FilePath {
  inner: String,
}

impl FilePath {
  fn from_uri(uri: &Uri) -> Result<Self> {
    let path = uri
      .path()
      .strip_prefix('/')
      .ok_or_else(|| Error::InvalidPath { uri: uri.clone() })?;

    for component in Path::new(path).components() {
      match component {
        Component::Normal(_) => {}
        _ => return Err(Error::InvalidPath { uri: uri.clone() }),
      }
    }

    for component in path.split('/') {
      if component.is_empty() {
        return Err(Error::InvalidPath { uri: uri.clone() });
      }
    }

    Ok(Self {
      inner: path.to_owned(),
    })
  }
}

impl AsRef<Path> for FilePath {
  fn as_ref(&self) -> &Path {
    self.inner.as_ref()
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use crate::{error::Error, server::Server, test_utils::test};
  use guard::guard_unwrap;
  use hyper::StatusCode;
  use pretty_assertions::assert_eq;
  use reqwest::Url;
  use scraper::{ElementRef, Html, Selector};
  use std::str;
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

  #[test]
  fn index_route_status_code_is_200() {
    test(|url, _dir| async move { assert_eq!(reqwest::get(url).await.unwrap().status(), 200) });
  }

  #[test]
  fn index_route_contains_title() {
    test(|url, _dir| async move {
      let haystack = reqwest::get(url).await.unwrap().text().await.unwrap();
      let needle = "<title>foo</title>";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn server_aborts_when_directory_does_not_exist() {
    let environment = Environment::test();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = Server::setup(&environment).unwrap_err();
        guard_unwrap!(let Error::WwwIo { .. } = error);
      });
  }

  #[test]
  fn errors_in_request_handling_cause_500_status_codes() {
    test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      let status = reqwest::get(url).await.unwrap().status();
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
    assert_contains(&stderr, "IO error accessing `www`: ");
    assert_contains(&stderr, "No such file or directory");
  }

  async fn get_html(url: &Url) -> Html {
    Html::parse_document(
      &reqwest::get(url.clone())
        .await
        .unwrap()
        .text()
        .await
        .unwrap(),
    )
  }

  fn css_select<'a>(html: &'a Html, selector: &'a str) -> Vec<ElementRef<'a>> {
    let selector = Selector::parse(selector).unwrap();
    html.select(&selector).collect::<Vec<_>>()
  }

  #[test]
  fn listing_contains_file() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "").unwrap();
      let haystack = get_html(&url).await.root_element().html();
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
      let haystack = get_html(&url).await.root_element().html();
      assert_contains(&haystack, "a.txt");
      assert_contains(&haystack, "b.txt");
    });
  }

  #[test]
  fn listed_files_are_download_links() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "").unwrap();
      let html = get_html(&url).await;
      guard_unwrap!(let &[a] = css_select(&html, "a").as_slice());
      assert_eq!(a.inner_html(), "some-test-file.txt");
      assert_eq!(a.value().attr("download"), Some(""));
    });
  }

  #[test]
  fn listed_files_can_be_downloaded() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "contents").unwrap();
      let html = get_html(&url).await;
      guard_unwrap!(let &[a] = css_select(&html, "a").as_slice());
      let file_url = a.value().attr("href").unwrap();
      let file_url = url.join(file_url).unwrap();
      let file_contents = reqwest::get(file_url).await.unwrap().text().await.unwrap();
      assert_eq!(file_contents, "contents");
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
    assert_contains(&stderr, &format!("Invalid URL file path: /foo/../bar.txt"));
  }

  #[test]
  fn disallow_empty_path_component() {
    test(|url, _dir| async move {
      assert_eq!(
        reqwest::get(format!("{}foo//bar.txt", url))
          .await
          .unwrap()
          .status(),
        StatusCode::BAD_REQUEST
      )
    });
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
    assert_contains(&stderr, &format!("Invalid URL file path: //foo.txt"));
  }
}
