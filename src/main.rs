use request_handler::{RequestHandler, RequestHandlerServer};
use std::env;

mod request_handler;

#[tokio::main]
async fn main() {
  let server = RequestHandler::bind(&env::current_dir().unwrap(), Some(8080)).unwrap();
  run(server).await
}

async fn run(server: RequestHandlerServer) {
  if let Err(e) = server.await {
    eprintln!("server error: {}", e);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::future::Future;
  use std::{io, path::PathBuf};

  fn test<Function, F>(test: Function)
  where
    Function: FnOnce(u16, PathBuf) -> F,
    F: Future<Output = ()>,
  {
    let tempdir = tempfile::tempdir().unwrap();
    let www = tempdir.path().join("www");
    std::fs::create_dir(&www).unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = RequestHandler::bind(&tempdir.path(), None).unwrap();
        let port = server.local_addr().port();
        let join_handle = tokio::spawn(run(server));
        test(port, tempdir.path().to_owned()).await;
        join_handle.abort();
      });
  }

  #[track_caller]
  fn assert_contains(haystack: &str, needle: &str) {
    assert!(
      haystack.contains(needle),
      "\n{} does not contain {}\n",
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
  fn test_listing_contains_file() {
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
  fn test_listing_contains_multiple_files() {
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
  fn test_server_aborts_when_directory_does_not_exist() {
    let tempdir = tempfile::tempdir().unwrap();
    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        assert_eq!(
          RequestHandler::bind(&tempdir.path(), None)
            .unwrap_err()
            .kind(),
          io::ErrorKind::NotFound
        );
      });
  }

  #[test]
  #[ignore]
  fn errors_in_request_handling_cause_500_status_codes() {}

  #[test]
  #[ignore]
  fn errors_in_request_handling_are_printed_to_stderr() {}
}
