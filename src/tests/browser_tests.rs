use super::*;
use crate::test_utils::test_with_lnd;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::browser::PermissionDescriptor;
use chromiumoxide::cdp::browser_protocol::browser::PermissionSetting;
use chromiumoxide::cdp::browser_protocol::browser::PermissionType;
use chromiumoxide::cdp::browser_protocol::browser::SetPermissionParams;
use futures::StreamExt;
use lnd_test_context::LndTestContext;
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
    let handle = task::spawn(async move {
      loop {
        let _ = handler.next().await.unwrap();
      }
    });
    let page = browser
      .new_page(context.files_url().join("foo").unwrap().as_ref())
      .await
      .unwrap();

    page
      .find_element(".payment-request")
      .await
      .unwrap()
      .click()
      .await
      .unwrap();

    let clipboard_content = page.evaluate("navigator.clipboard.readText()").await;
    dbg!(clipboard_content);

    // let copy_button = tab
    // .wait_for_element(".payment-request")
    // // .wait_for_element(".payment-request > .copy-button")
    // .unwrap();
    // copy_button.click();
    // let clipboard_content = copy_button.call_js_fn(
    // "function foo() { return navigator.clipboard.readText(); }",
    // true,
    // );
    // dbg!(clipboard_content);

    todo!();
    handle.abort();
    // let redirect_url = redirect_url(&context, context.base_url()).await;
    // assert_eq!(&redirect_url, context.files_url());

    // /// Navigate to wikipedia

    // /// Wait for network/javascript/dom to make the search-box available
    // /// and click it.

    // /// Type in a query and press `Enter`
    // tab.type_str("WebKit")?.press_key("Enter")?;

    // /// We should end up on the WebKit-page once navigated
    // tab.wait_for_element("#firstHeading")?;
    // assert!(tab.get_url().ends_with("WebKit"));

    // /// Take a screenshot of the entire browser window
    // let _jpeg_data = tab.capture_screenshot(ScreenshotFormat::JPEG(Some(75)), None, true)?;

    // /// Take a screenshot of just the WebKit-Infobox
    // let _png_data = tab
    // .wait_for_element("#mw-content-text > div > table.infobox.vevent")?
    // .capture_screenshot(ScreenshotFormat::PNG)?;
    // Ok(())
  });
}
