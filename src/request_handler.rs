use crate::{environment::Environment, stderr::Stderr};
use anyhow::{Context, Result};
use futures::{future::BoxFuture, FutureExt};
use hyper::{
  server::conn::AddrIncoming, service::Service, Body, Request, Response, Server, StatusCode,
};
use maud::{html, DOCTYPE};
use std::{
  convert::Infallible,
  ffi::OsString,
  fmt::Debug,
  fs,
  io::Write,
  net::SocketAddr,
  path::PathBuf,
  task::{self, Poll},
};
use structopt::StructOpt;
use tower::make::Shared;

pub(crate) type RequestHandlerServer = Server<AddrIncoming, Shared<RequestHandler>>;

pub(crate) async fn run_server(server: RequestHandlerServer) -> Result<()> {
  Ok(server.await?)
}

#[derive(StructOpt)]
struct Arguments {
  #[structopt(long)]
  port: Option<u16>,
}

#[derive(Clone, Debug)]
pub(crate) struct RequestHandler {
  stderr: Stderr,
  pub(crate) working_directory: PathBuf,
}

impl RequestHandler {
  pub(crate) fn bind(environment: &Environment) -> Result<RequestHandlerServer> {
    let arguments = Arguments::from_iter_safe(&environment.arguments)?;

    fs::read_dir(environment.working_directory.join("www")).context("cannot access `www`")?;
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], arguments.port.unwrap_or(0)));
    let server = Server::bind(&socket_addr).serve(Shared::new(RequestHandler {
      stderr: environment.stderr.clone(),
      working_directory: environment.working_directory.to_owned(),
    }));
    eprintln!("Listening on port {}", server.local_addr().port());
    Ok(server)
  }

  async fn response(mut self) -> Response<Body> {
    match self.list_www().await.context("cannot access `www`") {
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
            (entry.file_name().to_string_lossy())
            br;
          }
        }
      }
    };

    Ok(Response::new(Body::from(body.into_string())))
  }
}

impl Service<Request<Body>> for RequestHandler {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: Request<Body>) -> Self::Future {
    self.clone().response().map(Ok).boxed()
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use super::*;
  use futures::future::Future;
  use std::path::PathBuf;

  pub(crate) fn test<Function, F>(test: Function) -> String
  where
    Function: FnOnce(u16, PathBuf) -> F,
    F: Future<Output = ()>,
  {
    test_with_arguments(&[], test)
  }

  pub(crate) fn test_with_arguments<Function, F>(args: &[&str], f: Function) -> String
  where
    Function: FnOnce(u16, PathBuf) -> F,
    F: Future<Output = ()>,
  {
    let mut environment = Environment::test();
    environment
      .arguments
      .extend(args.iter().cloned().map(OsString::from));

    let www = environment.tempdir.path().join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = RequestHandler::bind(&environment).unwrap();
        let port = server.local_addr().port();
        let join_handle = tokio::spawn(async { run_server(server).await.unwrap() });
        f(port, environment.tempdir.path().to_owned()).await;
        join_handle.abort();
        environment.stderr.contents()
      })
  }

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
    test(|port, _dir| async move {
      assert_eq!(
        reqwest::get(format!("http://localhost:{}", port))
          .await
          .unwrap()
          .status(),
        200
      )
    });
  }

  #[test]
  fn index_route_contains_title() {
    test(|port, _dir| async move {
      let haystack = reqwest::get(format!("http://localhost:{}", port))
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
  fn listing_contains_file() {
    test(|port, dir| async move {
      std::fs::write(dir.join("www").join("some-test-file.txt"), "").unwrap();
      let haystack = reqwest::get(format!("http://localhost:{}", port))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
      let needle = "some-test-file.txt";
      assert_contains(&haystack, needle);
    });
  }

  #[test]
  fn listing_contains_multiple_files() {
    test(|port, dir| async move {
      let www = dir.join("www");
      std::fs::write(www.join("a.txt"), "").unwrap();
      std::fs::write(www.join("b.txt"), "").unwrap();
      let haystack = reqwest::get(format!("http://localhost:{}", port))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
      assert_contains(&haystack, "a.txt");
      assert_contains(&haystack, "b.txt");
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
        let error = format!("{:?}", RequestHandler::bind(&environment).unwrap_err());
        assert_contains(&error, "cannot access `www`");
        assert_contains(&error, "Caused by:");
      });
  }

  #[test]
  fn errors_in_request_handling_cause_500_status_codes() {
    test(|port, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      let status = reqwest::get(format!("http://localhost:{}", port))
        .await
        .unwrap()
        .status();
      assert_eq!(status, 500);
    });
  }

  #[test]
  fn errors_in_request_handling_are_printed_to_stderr() {
    let stderr = test(|port, dir| async move {
      let www = dir.join("www");
      std::fs::remove_dir(www).unwrap();
      reqwest::get(format!("http://localhost:{}", port))
        .await
        .unwrap();
    });
    assert_contains(&stderr, "cannot access `www`");
    assert_contains(&stderr, "Caused by:");
  }
}
