use crate::{environment::Environment, server::Server};
use reqwest::Url;
use std::{ffi::OsString, future::Future, path::PathBuf};

pub(crate) fn test<Function, F>(f: Function) -> String
where
  Function: FnOnce(Url, PathBuf) -> F,
  F: Future<Output = ()>,
{
  test_with_arguments(&[], f)
}

pub(crate) fn test_with_arguments<Function, F>(args: &[&str], f: Function) -> String
where
  Function: FnOnce(Url, PathBuf) -> F,
  F: Future<Output = ()>,
{
  let mut environment = Environment::test(&[]);
  environment
    .arguments
    .extend(args.iter().cloned().map(OsString::from));

  let www = environment.working_directory.join("www");
  std::fs::create_dir(&www).unwrap();

  test_with_environment(&environment, f)
}

pub(crate) fn test_with_environment<Function, F>(environment: &Environment, f: Function) -> String
where
  Function: FnOnce(Url, PathBuf) -> F,
  F: Future<Output = ()>,
{
  tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      let server = Server::setup(&environment).unwrap();
      let port = server.port();
      let join_handle = tokio::spawn(async { server.run().await.unwrap() });
      let url = Url::parse(&format!("http://localhost:{}", port)).unwrap();
      f(url, environment.working_directory.clone()).await;
      join_handle.abort();
      environment.stderr.contents()
    })
}
