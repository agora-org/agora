use crate::connection_handler::ConnectionHandler;
use connection_handler::ConnectionHandlerServer;
use std::env;

mod connection_handler;
mod request_handler;

#[tokio::main]
async fn main() {
  let server = ConnectionHandler::bind(&env::current_dir().unwrap(), Some(8080));
  run(server).await
}

async fn run(server: ConnectionHandlerServer) {
  if let Err(e) = server.await {
    eprintln!("server error: {}", e);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::future::Future;
  use std::path::PathBuf;

  fn test<Function, F>(test: Function)
  where
    Function: FnOnce(u16, PathBuf) -> F,
    F: Future<Output = ()>,
  {
    let tempdir = tempfile::tempdir().unwrap();

    tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap()
      .block_on(async {
        let server = ConnectionHandler::bind(&tempdir.path(), None);
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
    test(|port, dir| async move {
      let www = dir.join("www");
      std::fs::create_dir(&www).unwrap();
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
    test(|port, dir| async move {
      let www = dir.join("www");
      std::fs::create_dir(&www).unwrap();
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
      let www = dir.join("www");
      std::fs::create_dir(&www).unwrap();
      std::fs::write(www.join("some-test-file.txt"), "").unwrap();
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
      std::fs::create_dir(&www).unwrap();
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
}
