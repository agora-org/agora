use {
  crate::test_utils::{https_client, set_up_test_certificate, test_with_arguments},
  pretty_assertions::assert_eq,
};

#[cfg(feature = "slow-tests")]
mod browser_tests;
#[cfg(feature = "slow-tests")]
mod slow_tests;

#[cfg(feature = "slow-tests")]
async fn get(url: &reqwest::Url) -> reqwest::Response {
  let response = reqwest::get(url.clone()).await.unwrap();
  assert_eq!(response.status(), reqwest::StatusCode::OK);
  response
}

#[cfg(feature = "slow-tests")]
async fn text(url: &reqwest::Url) -> String {
  get(url).await.text().await.unwrap()
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
