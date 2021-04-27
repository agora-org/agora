use crate::{
  request_handler::{run_server, RequestHandler},
  stderr::Stderr,
};
use anyhow::Result;
use std::env;

mod request_handler;
mod stderr;

#[tokio::main]
async fn main() {
  if let Err(error) = run().await {
    eprintln!("{:?}", error);
    std::process::exit(1);
  }
}

async fn run() -> Result<()> {
  let stderr = Stderr::production();
  let server = RequestHandler::bind(&stderr, &env::current_dir()?, Some(8080))?;
  run_server(server).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::future::Future;
  use std::{
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex},
  };

  fn test<Function, F>(test: Function) -> String
  where
    Function: FnOnce(u16, PathBuf) -> F,
    F: Future<Output = ()>,
  {
    let tempdir = tempfile::tempdir().unwrap();
    let www = tempdir.path().join("www");
    std::fs::create_dir(&www).unwrap();

    let stderr = Arc::new(Mutex::new(Cursor::new(vec![])));
    let stderr_clone = stderr.clone();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server =
          RequestHandler::bind(&Stderr::Test(stderr_clone), &tempdir.path(), None).unwrap();
        let port = server.local_addr().port();
        let join_handle = tokio::spawn(async { run_server(server).await.unwrap() });
        test(port, tempdir.path().to_owned()).await;
        join_handle.abort();
        String::from_utf8(stderr.lock().unwrap().clone().into_inner()).unwrap()
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
    let tempdir = tempfile::tempdir().unwrap();
    let stderr = Stderr::Test(Arc::new(Mutex::new(Cursor::new(vec![]))));
    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let error = format!(
          "{:?}",
          RequestHandler::bind(&stderr, &tempdir.path(), None).unwrap_err()
        );
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
