use crate::{environment::Environment, stderr::Stderr};
use anyhow::{Context, Result};
use futures::{future::BoxFuture, FutureExt};
use hyper::{service::Service, Body, Request, Response, StatusCode};
use maud::{html, DOCTYPE};
use std::{
  convert::Infallible,
  fmt::Debug,
  io::Write,
  path::PathBuf,
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

  async fn response_without_errors(mut self, request: Request<Body>) -> Response<Body> {
    match self.response(request).await {
      Ok(response) => response,
      Err(error) => {
        writeln!(self.stderr, "{:?}", error).unwrap();
        Response::builder()
          .status(StatusCode::INTERNAL_SERVER_ERROR)
          .body(Body::empty())
          .unwrap()
      }
    }
  }

  async fn response(&self, request: Request<Body>) -> Result<Response<Body>> {
    dbg!(request.uri().path());
    match request.uri().path() {
      "/" => self.list_www().await.context("cannot access `www`"),
      file_path => self.serve_file(file_path).await,
    }
  }

  async fn list_www(&self) -> Result<Response<Body>> {
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

  async fn serve_file(&self, file_path: &str) -> Result<Response<Body>> {
    // todo: stream files
    let file_contents = tokio::fs::read(dbg!(self
      .working_directory
      .join("www")
      .join(file_path.trim_start_matches('/'))))
    .await?;
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
    self
      .clone()
      .response_without_errors(request)
      .map(Ok)
      .boxed()
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use crate::{server::Server, test_utils::test};
  use guard::guard_unwrap;
  use pretty_assertions::assert_eq;
  use reqwest::Url;
  use scraper::{ElementRef, Html, Selector};

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
    test(
      |url, _dir| async move { assert_eq!(reqwest::get(url.as_str()).await.unwrap().status(), 200) },
    );
  }

  #[test]
  fn index_route_contains_title() {
    test(|url, _dir| async move {
      let haystack = reqwest::get(url.as_str())
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
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
        let error = format!("{:?}", Server::setup(&environment).unwrap_err());
        assert_contains(&error, "cannot access `www`");
        assert_contains(&error, "Caused by:");
      });
  }

  #[test]
  fn errors_in_request_handling_cause_500_status_codes() {
    test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      let status = reqwest::get(url.as_str()).await.unwrap().status();
      assert_eq!(status, 500);
    });
  }

  #[test]
  fn errors_in_request_handling_are_printed_to_stderr() {
    let stderr = test(|url, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      reqwest::get(url.as_str()).await.unwrap();
    });
    assert_contains(&stderr, "cannot access `www`");
    assert_contains(&stderr, "Caused by:");
  }

  async fn get_html(url: &Url) -> Html {
    Html::parse_document(
      &reqwest::get(url.as_str())
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
      assert_eq!(dbg!(a.value().attr("download")), Some(""));
    });
  }

  #[test]
  fn listed_files_can_be_downloaded() {
    test(|url, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "contents").unwrap();
      let html = get_html(&url).await;
      guard_unwrap!(let &[a] = css_select(&html, "a").as_slice());
      let file_url = a.value().attr("href").unwrap();
      let file_url = dbg!(url.join(file_url).unwrap());
      let file_contents = reqwest::get(file_url).await.unwrap().text().await.unwrap();
      assert_eq!(file_contents, "contents");
    });
  }

  #[test]
  fn disallow_access_outside_of_www() {}
}
