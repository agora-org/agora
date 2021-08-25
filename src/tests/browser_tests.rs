use crate::test_utils::test_with_lnd;
use chromiumoxide::{
  browser::BrowserConfig,
  cdp::browser_protocol::browser::{PermissionDescriptor, PermissionSetting, SetPermissionParams},
};
use futures::StreamExt;
use lnd_test_context::LndTestContext;
use pretty_assertions::assert_eq;
use tokio::task;

struct Browser {
  inner: chromiumoxide::Browser,
  handle: task::JoinHandle<()>,
}

impl Browser {
  async fn new() -> Self {
    let (inner, mut handler) =
      chromiumoxide::Browser::launch(BrowserConfig::builder().build().unwrap())
        .await
        .unwrap();

    let handle = task::spawn(async move {
      loop {
        let _ = handler.next().await.unwrap();
      }
    });

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

#[test]
fn copy_payment_request_to_clipboard() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
    context.write("foo", "precious content");

    let browser = Browser::new().await;

    dbg!("Browsing to new page…");
    let page = browser
      .inner
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
      .hover()
      .await
      .unwrap()
      .inner_text()
      .await
      .unwrap()
      .unwrap();

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

    assert_eq!(clipboard_contents, payment_request);
  });
}
