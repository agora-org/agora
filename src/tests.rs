use {
  crate::{
    common::*,
    environment::Environment,
    test_utils::{
      https_client, set_up_test_certificate, test_with_arguments, test_with_environment,
    },
  },
  pretty_assertions::assert_eq,
};

#[cfg(feature = "slow-tests")]
mod browser_tests;
#[cfg(feature = "slow-tests")]
mod slow_tests;

async fn get_with_status(url: &reqwest::Url, status_code: reqwest::StatusCode) -> reqwest::Response {
  let response = reqwest::get(url.clone()).await.unwrap();
  assert_eq!(response.status(), status_code);
  response
}

async fn get(url: &reqwest::Url) -> reqwest::Response {
  get_with_status(url, reqwest::StatusCode::OK).await
}

#[cfg(feature = "slow-tests")]
async fn get_payment_required(url: &reqwest::Url) -> reqwest::Response {
  get_with_status(url, reqwest::StatusCode::PAYMENT_REQUIRED).await
}

async fn text(url: &reqwest::Url) -> String {
  get(url).await.text().await.unwrap()
}

#[cfg(feature = "slow-tests")]
async fn text_payment_required(url: &reqwest::Url) -> String {
  get_payment_required(url).await.text().await.unwrap()
}

#[test]
fn serves_https_requests_with_cert_from_cache_directory() {
  let (certificate_cache, root_certificate) = set_up_test_certificate();

  test_with_arguments(
    &[
      "--acme-cache-directory",
      certificate_cache.path().to_str().unwrap(),
      "--https-port=0",
      "--acme-domain=localhost",
    ],
    |context| async move {
      context.write("file", "encrypted content");
      let client = https_client(&context, root_certificate).await;
      let response = client
        .get(context.https_files_url().join("file").unwrap())
        .send()
        .await
        .unwrap();
      let body = response.text().await.unwrap();
      assert_eq!(body, "encrypted content");
    },
  );
}

#[test]
fn redirects_requests_from_port_80_to_443() {
  let (certificate_cache, root_certificate) = set_up_test_certificate();

  test_with_arguments(
    &[
      "--acme-cache-directory",
      certificate_cache.path().to_str().unwrap(),
      "--https-port=0",
      "--https-redirect-port=0",
      "--acme-domain=localhost",
    ],
    |context| async move {
      context.write("file", "encrypted content");
      let client = https_client(&context, root_certificate).await;
      let response = client
        .get(format!(
          "http://localhost:{}/files/file",
          context.https_redirect_port()
        ))
        .send()
        .await
        .unwrap();
      assert!(response.url().to_string().starts_with("https:"));
      let body = response.text().await.unwrap();
      assert_eq!(body, "encrypted content");
    },
  );
}

fn symlink(contents: impl AsRef<Path>, link: impl AsRef<Path>) {
  #[cfg(unix)]
  std::os::unix::fs::symlink(contents, link).unwrap();
  #[cfg(windows)]
  {
    let target = link.as_ref().parent().unwrap().join(&contents);
    if target.is_dir() {
      std::os::windows::fs::symlink_dir(contents, link).unwrap();
    } else if target.is_file() {
      std::os::windows::fs::symlink_file(contents, link).unwrap();
    } else {
      panic!(
        "unsupported file type for paths: contents: `{}`, link: `{}`",
        contents.as_ref().display(),
        link.as_ref().display(),
      );
    }
  }
}

#[test]
fn bugfix_symlink_with_relative_base_directory() {
  let mut environment = Environment::test();

  let www = environment.working_directory.join("www");
  std::fs::create_dir(&www).unwrap();

  let working_directory = environment.working_directory.join("working_directory");
  std::fs::create_dir(&working_directory).unwrap();

  environment.working_directory = working_directory;

  environment.arguments = vec![
    "agora".into(),
    "--address=localhost".into(),
    "--http-port=0".into(),
    "--directory=../www".into(),
  ];

  test_with_environment(&mut environment, |context| async move {
    context.write("file", "precious content");
    symlink("file", context.files_directory().join("link"));
    let content = text(&context.files_url().join("file").unwrap()).await;
    assert_eq!(content, "precious content");
    let link = text(&context.files_url().join("link").unwrap()).await;
    assert_eq!(link, "precious content");
  });
}
