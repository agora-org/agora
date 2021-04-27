use crate::{environment::Environment, request_handler::Server};
use std::{ffi::OsString, future::Future, path::PathBuf};

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
      let server = Server::setup(&environment).unwrap();
      let port = server.port();
      let join_handle = tokio::spawn(async { server.run().await.unwrap() });
      f(port, environment.tempdir.path().to_owned()).await;
      join_handle.abort();
      environment.stderr.contents()
    })
}
