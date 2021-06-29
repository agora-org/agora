use crate::{environment::Environment, server::Server};
use reqwest::Url;
use std::{
  ffi::OsString,
  future::Future,
  path::{Path, PathBuf},
};

macro_rules! assert_matches {
  ($expression:expr, $( $pattern:pat )|+ $( if $guard:expr )?) => {
    match $expression {
      $( $pattern )|+ $( if $guard )? => {}
      left => panic!(
        "assertion failed: (left ~= right)\n  left: `{:?}`\n right: `{}`",
        left,
        stringify!($($pattern)|+ $(if $guard)?)
      ),
    }
  }
}

#[track_caller]
pub(crate) fn assert_contains(haystack: &str, needle: &str) {
  assert!(
    haystack.contains(needle),
    "\n{:?} does not contain {:?}\n",
    haystack,
    needle
  );
}

pub(crate) fn test<Function, F>(f: Function) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()>,
{
  test_with_arguments(&[], f)
}

pub(crate) fn test_with_arguments<Function, F>(args: &[&str], f: Function) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()>,
{
  let mut environment = Environment::test(&[]);
  environment
    .arguments
    .extend(args.iter().cloned().map(OsString::from));

  let www = environment.working_directory.join("www");
  std::fs::create_dir(&www).unwrap();

  test_with_environment(&mut environment, f)
}

pub(crate) fn test_with_environment<Function, F>(
  environment: &mut Environment,
  f: Function,
) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()>,
{
  tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      let server = Server::setup(environment).await.unwrap();
      let files_directory = server.directory().to_owned();
      let port = server.port();
      let join_handle = tokio::spawn(async { server.run().await.unwrap() });
      let url = Url::parse(&format!("http://localhost:{}", port)).unwrap();
      f(TestContext {
        base_url: url.clone(),
        files_url: url.join("files/").unwrap(),
        files_directory,
      })
      .await;
      join_handle.abort();
      environment.stderr.contents()
    })
}

pub(crate) struct TestContext {
  base_url: Url,
  files_directory: PathBuf,
  files_url: Url,
}

impl TestContext {
  pub(crate) fn files_url(&self) -> &Url {
    &self.files_url
  }

  pub(crate) fn files_directory(&self) -> &Path {
    &self.files_directory
  }

  pub(crate) fn base_url(&self) -> &Url {
    &self.base_url
  }
}
