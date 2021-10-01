use crate::{
  common::*,
  test_utils::{set_up_test_certificate, test_with_arguments, test_with_lnd},
};
use chromiumoxide::{
  browser::BrowserConfig,
  cdp::browser_protocol::browser::{PermissionDescriptor, PermissionSetting, SetPermissionParams},
  Page,
};
use lnd_test_context::LndTestContext;
use pretty_assertions::assert_eq;

struct Browser {
  inner: chromiumoxide::Browser,
  handle: task::JoinHandle<()>,
}

impl Browser {
  async fn new() -> Self {
    let (inner, mut handler) = chromiumoxide::Browser::launch(
      BrowserConfig::builder()
        .arg("--allow-insecure-localhost")
        .build()
        .unwrap(),
    )
    .await
    .unwrap();

    let handle = task::spawn(async move {
      loop {
        let _ = handler.next().await.unwrap();
      }
    });

    eprintln!("Setting permissions…");
    inner
      .execute(SetPermissionParams::new(
        PermissionDescriptor::new("clipboard-read"),
        PermissionSetting::Granted,
      ))
      .await
      .unwrap();
    inner
      .execute(SetPermissionParams::new(
        PermissionDescriptor::new("clipboard-write"),
        PermissionSetting::Granted,
      ))
      .await
      .unwrap();

    Browser { inner, handle }
  }
}

impl Drop for Browser {
  fn drop(&mut self) {
    self.handle.abort();
  }
}

async fn get_clipboard_copy_display_property(page: &Page) -> String {
  page
    .evaluate(
      "window.getComputedStyle(document.getElementsByClassName('clipboard-copy')[0]).display",
    )
    .await
    .unwrap()
    .into_value()
    .unwrap()
}

#[test]
fn copy_payment_request_to_clipboard() {
  let (certificate_cache, _) = set_up_test_certificate();

  let lnd_test_context = LndTestContext::new_blocking();

  test_with_arguments(
    &[
      "--lnd-rpc-authority",
      &lnd_test_context.lnd_rpc_authority(),
      "--lnd-rpc-cert-path",
      lnd_test_context.cert_path().to_str().unwrap(),
      "--lnd-rpc-macaroon-path",
      lnd_test_context.invoice_macaroon_path().to_str().unwrap(),
      "--acme-cache-directory",
      certificate_cache.path().to_str().unwrap(),
      "--https-port=0",
      "--acme-domain=localhost",
    ],
    |context| async move {
      context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
      context.write("foo", "precious content");

      let browser = Browser::new().await;

      eprintln!("Browsing to new page…");
      let page = browser
        .inner
        .new_page(context.https_files_url().join("foo").unwrap().as_ref())
        .await
        .unwrap();

      eprintln!("Clearing clipboard…");
      page
        .evaluate("navigator.clipboard.writeText('placeholder text')")
        .await
        .unwrap();

      assert_eq!(get_clipboard_copy_display_property(&page).await, "none");

      eprintln!("Clicking clipboard copy button…");
      page
        .find_element(".payment-request")
        .await
        .unwrap()
        .hover()
        .await
        .unwrap();

      assert_eq!(get_clipboard_copy_display_property(&page).await, "block");

      page
        .find_element(".clipboard-copy")
        .await
        .unwrap()
        .click()
        .await
        .unwrap();

      let clipboard_contents = page
        .evaluate("navigator.clipboard.readText()")
        .await
        .unwrap()
        .into_value::<String>()
        .unwrap();

      let payment_request = page
        .find_element(".payment-request")
        .await
        .unwrap()
        .inner_text()
        .await
        .unwrap()
        .unwrap();

      assert_eq!(clipboard_contents, payment_request);
    },
  );
}

#[test]
fn clipboard_copy_button_does_not_appear_over_http() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
    context.write("foo", "precious content");

    let browser = Browser::new().await;

    eprintln!("Browsing to new page…");
    let page = browser
      .inner
      .new_page(context.files_url().join("foo").unwrap().as_ref())
      .await
      .unwrap();

    assert_eq!(get_clipboard_copy_display_property(&page).await, "none");

    eprintln!("Hover over payment request…");
    page
      .find_element(".payment-request")
      .await
      .unwrap()
      .hover()
      .await
      .unwrap();

    assert_eq!(get_clipboard_copy_display_property(&page).await, "none");
  });
}
