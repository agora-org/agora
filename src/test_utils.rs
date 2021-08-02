use crate::{environment::Environment, server::Server};
#[cfg(feature = "slow-tests")]
use lnd_test_context::LndTestContext;
use reqwest::Url;
use std::{
  ffi::OsString,
  future::Future,
  panic,
  path::{Path, PathBuf},
};
use tokio::task;

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

#[track_caller]
pub(crate) fn assert_not_contains(haystack: &str, needle: &str) {
  assert!(
    !haystack.contains(needle),
    "\n{:?} contains {:?}\n",
    haystack,
    needle
  );
}

pub(crate) fn test<Function, F>(f: Function) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
{
  test_with_arguments(&[], f)
}

#[cfg(feature = "slow-tests")]
pub(crate) fn test_with_lnd<Function, Fut>(lnd_test_context: &LndTestContext, f: Function) -> String
where
  Function: FnOnce(TestContext) -> Fut,
  Fut: Future<Output = ()>,
{
  test_with_arguments(
    &[
      "--lnd-rpc-authority",
      &lnd_test_context.lnd_rpc_authority(),
      "--lnd-rpc-cert-path",
      lnd_test_context.cert_path().to_str().unwrap(),
      "--lnd-rpc-macaroon-path",
      lnd_test_context.invoice_macaroon_path().to_str().unwrap(),
    ],
    f,
  )
}

pub(crate) fn test_with_arguments<Function, F>(args: &[&str], f: Function) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
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
  F: Future<Output = ()> + 'static,
{
  tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      let server = Server::setup(environment).await.unwrap();
      let files_directory = server.directory().to_owned();
      let port = server.port();
      let server_join_handle = tokio::spawn(async { server.run().await.unwrap() });
      let url = Url::parse(&format!("http://localhost:{}", port)).unwrap();
      let test_result = task::LocalSet::new()
        .run_until(async move {
          task::spawn_local(f(TestContext {
            base_url: url.clone(),
            files_url: url.join("files/").unwrap(),
            files_directory,
          }))
          .await
        })
        .await;
      if let Err(test_join_error) = test_result {
        eprintln!("stderr from server: {}", environment.stderr.contents());
        if test_join_error.is_panic() {
          panic::resume_unwind(test_join_error.into_panic());
        } else {
          panic!("test shouldn't be cancelled: {}", test_join_error);
        }
      }
      server_join_handle.abort();
      match server_join_handle.await {
        Err(server_join_error) if server_join_error.is_cancelled() => {}
        Err(server_join_error) => {
          panic::resume_unwind(server_join_error.into_panic());
        }
        Ok(()) => panic!("server terminated"),
      }
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
