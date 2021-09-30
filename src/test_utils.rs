use crate::common::*;
use crate::server::TestContext;
#[cfg(feature = "slow-tests")]
use lnd_test_context::LndTestContext;
use reqwest::{Certificate, Client, ClientBuilder};
use std::panic;

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
    "assert_contains:\n---\n{}\n---\ndoes not contain:\n---\n{:?}\n---\n",
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

pub(crate) fn set_up_test_certificate() -> (TempDir, Certificate) {
  use rcgen::{
    BasicConstraints, Certificate, CertificateParams, IsCa, KeyPair, SanType,
    PKCS_ECDSA_P256_SHA256,
  };

  let root_certificate = {
    let mut params: CertificateParams = Default::default();
    params.key_pair = Some(KeyPair::generate(&PKCS_ECDSA_P256_SHA256).unwrap());
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    Certificate::from_params(params).unwrap()
  };

  let certificate_keys = KeyPair::generate(&PKCS_ECDSA_P256_SHA256).unwrap();
  let certificate_keys_pem = certificate_keys.serialize_pem();
  let certificate = {
    let mut params = CertificateParams::from_ca_cert_pem(
      &root_certificate.serialize_pem().unwrap(),
      certificate_keys,
    )
    .unwrap();
    params
      .subject_alt_names
      .push(SanType::DnsName("localhost".to_string()));
    Certificate::from_params(params).unwrap()
  };
  let certificate_file = vec![
    certificate_keys_pem,
    certificate
      .serialize_pem_with_signer(&root_certificate)
      .unwrap(),
    root_certificate.serialize_pem().unwrap(),
  ]
  .join("\r\n");
  let tempdir = TempDir::new().unwrap();
  fs::write(
    tempdir
      .path()
      .join("cached_cert_83kei_h4oopqh8sXFFlhGeQJIS_pkJJv-y5XDpnLtyw"),
    certificate_file,
  )
  .unwrap();
  (
    tempdir,
    reqwest::Certificate::from_pem(root_certificate.serialize_pem().unwrap().as_bytes()).unwrap(),
  )
}

pub(crate) async fn https_client(context: &TestContext, root_certificate: Certificate) -> Client {
  let client = ClientBuilder::new()
    .add_root_certificate(root_certificate)
    .build()
    .unwrap();

  let mut error = None;
  for _ in 0..10 {
    match client.get(context.https_files_url().clone()).send().await {
      Ok(_) => return client,
      Err(err) => {
        error = Some(err);
      }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  panic!(
    "HTTPS server not ready after one second:\n{}",
    error.unwrap()
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
  Fut: Future<Output = ()> + 'static,
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
  let mut environment = Environment::test();
  environment
    .arguments
    .extend(args.iter().cloned().map(OsString::from));

  let www = environment.working_directory.join("www");
  std::fs::create_dir(&www).unwrap();

  test_with_environment(&mut environment, f)
}

pub(crate) fn test_with_environment<Function, F>(
  environment: &mut Environment,
  test_function: Function,
) -> String
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
{
  env_logger::builder().is_test(true).try_init().ok();
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      let server = Server::setup(environment).await.unwrap();
      let test_context = server.test_context(environment);
      let server_join_handle = tokio::spawn(async { server.run().await.unwrap() });
      let test_result = tokio::task::LocalSet::new()
        .run_until(async move { tokio::task::spawn_local(test_function(test_context)).await })
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
