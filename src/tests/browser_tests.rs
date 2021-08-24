use super::*;
use crate::test_utils::test_with_lnd;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::browser::PermissionDescriptor;
use chromiumoxide::cdp::browser_protocol::browser::PermissionSetting;
use chromiumoxide::cdp::browser_protocol::browser::PermissionType;
use chromiumoxide::cdp::browser_protocol::browser::SetPermissionParams;
use futures::StreamExt;
use lnd_test_context::LndTestContext;
use pretty_assertions::assert_eq;
use regex::Regex;
use tokio::task;

#[test]
fn copy_payment_request_to_clipboard() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
    context.write("foo", "precious content");

    let (browser, mut handler) = Browser::launch(BrowserConfig::builder().build().unwrap())
      .await
      .unwrap();

    let handle = task::spawn(async move {
      loop {
        let _ = handler.next().await.unwrap();
      }
    });

    dbg!("Setting permissions…");
    browser
      .execute(SetPermissionParams::new(
        PermissionDescriptor::new("clipboard-read"),
        PermissionSetting::Granted,
      ))
      .await
      .unwrap();
    browser
      .execute(SetPermissionParams::new(
        PermissionDescriptor::new("clipboard-write"),
        PermissionSetting::Granted,
      ))
      .await
      .unwrap();

    dbg!("Browsing to new page…");
    let page = browser
      .new_page(context.files_url().join("foo").unwrap().as_ref())
      .await
      .unwrap();

    dbg!("Clearing clipboard…");
    page
      .evaluate("navigator.clipboard.writeText('placeholder text')")
      .await
      .unwrap();

    dbg!("Clicking payment request…");
    let payment_request = page
      .find_element(".payment-request")
      .await
      .unwrap()
      .inner_text()
      .await
      .unwrap()
      .unwrap();

    page
      .find_element(".payment-request")
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

    assert_eq!(clipboard_contents, payment_request);

    handle.abort();
  });
}
