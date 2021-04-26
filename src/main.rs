use futures::future::{self, Ready};
use hyper::{server::conn::AddrIncoming, service::Service, Body, Request, Response, Server};
use maud::{html, DOCTYPE};
use std::{
  convert::Infallible,
  env, fs, io,
  net::SocketAddr,
  path::{Path, PathBuf},
  task::{Context, Poll},
};

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

type ConnectionHandlerServer = Server<AddrIncoming, ConnectionHandler>;

struct ConnectionHandler {
  working_directory: PathBuf,
}

impl ConnectionHandler {
  fn bind(working_directory: &Path, port: Option<u16>) -> ConnectionHandlerServer {
    let socket_addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));

    let connection_handler = Self {
      working_directory: working_directory.to_owned(),
    };

    let server = Server::bind(&socket_addr).serve(connection_handler);

    let port = server.local_addr().port();

    eprintln!("Listening on port {}", port);

    server
  }
}

impl<T> Service<T> for ConnectionHandler {
  type Response = RequestHandler;
  type Error = Infallible;
  type Future = Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: T) -> Self::Future {
    future::ok(RequestHandler {
      working_directory: self.working_directory.clone(),
    })
  }
}

struct RequestHandler {
  working_directory: PathBuf,
}

impl RequestHandler {
  fn response(&self) -> io::Result<Response<Body>> {
    let body = html! {
      (DOCTYPE)
      html {
        head {
          title {
            "foo"
          }
        }
        body {
          @for result in fs::read_dir(self.working_directory.join("www"))? {
            (result?.file_name().to_string_lossy())
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
  type Error = io::Error;
  type Future = Ready<Result<Self::Response, Self::Error>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Ok(()).into()
  }

  fn call(&mut self, _: Request<Body>) -> Self::Future {
    future::ready(self.response())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use future::Future;

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
