use super::*;
use crate::test_utils::{assert_contains, test_with_arguments, test_with_lnd};
use cradle::*;
use guard::guard_unwrap;
use hyper::{header, StatusCode};
use lnd_test_context::LndTestContext;
use pretty_assertions::assert_eq;
use regex::Regex;
use scraper::Html;
use std::{fs, path::MAIN_SEPARATOR};

#[test]
fn serves_files_for_free_by_default() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(context.files_directory().join("foo"), "contents").unwrap();
    let body = text(&context.files_url().join("foo").unwrap()).await;
    assert_eq!(body, "contents",);
  });
}

#[test]
fn redirects_to_invoice_url() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::create_dir(context.files_directory().join("foo")).unwrap();
    fs::write(
      context.files_directory().join("foo/.agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("foo/bar"), "").unwrap();
    let response = reqwest::get(context.files_url().join("foo/bar").unwrap())
      .await
      .unwrap();
    let regex = Regex::new("^/invoice/[a-f0-9]{64}/foo/bar$").unwrap();
    assert!(
      regex.is_match(response.url().path()),
      "Response URL path was not invoice path: {}",
      response.url().path(),
    );
  });
}

#[test]
fn non_existant_files_dont_redirect_to_invoice() {
  let stderr = test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    assert_eq!(
      reqwest::get(context.files_url().join("foo.txt").unwrap())
        .await
        .unwrap()
        .status(),
      StatusCode::NOT_FOUND
    )
  });
  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `www{}foo.txt`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn invoice_url_serves_bech32_encoded_invoice() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(context.files_directory().join("foo"), "").unwrap();
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    let html = html(&context.files_url().join("foo").unwrap()).await;
    guard_unwrap!(let &[payment_request] = css_select(&html, ".invoice").as_slice());
    assert_contains(&payment_request.inner_html(), "lnbcrt1");
  });
}

#[test]
fn invoice_url_contains_filename() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("test-filename"), "").unwrap();
    let html = html(&context.files_url().join("test-filename").unwrap()).await;
    guard_unwrap!(let &[payment_request] = css_select(&html, ".invoice").as_slice());
    assert_contains(&payment_request.inner_html(), "test-filename");
  });
}

fn decode_qr_code_from_svg(svg: &str) -> String {
  let options = usvg::Options::default();
  let svg = usvg::Tree::from_data(svg.as_bytes(), &options).unwrap();
  let svg_size = svg.svg_node().size.to_screen_size();
  let (png_width, png_height) = (svg_size.width() * 10, svg_size.height() * 10);
  let mut pixmap = tiny_skia::Pixmap::new(png_width, png_height).unwrap();
  resvg::render(
    &svg,
    usvg::FitTo::Size(png_width, png_height),
    pixmap.as_mut(),
  )
  .unwrap();
  let png_bytes = pixmap.encode_png().unwrap();
  let img = image::load_from_memory(&png_bytes).unwrap();
  let decoder = bardecoder::default_decoder();
  let mut decoded = decoder
    .decode(&img)
    .into_iter()
    .collect::<Result<Vec<String>, _>>()
    .unwrap();
  assert_eq!(decoded.len(), 1);
  decoded.pop().unwrap()
}

#[test]
fn invoice_url_links_to_qr_code() {
  let receiver = LndTestContext::new_blocking();
  test_with_lnd(&receiver.clone(), |context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(
      context.files_directory().join("test-filename"),
      "precious content",
    )
    .unwrap();
    let response = get(&context.files_url().join("test-filename").unwrap()).await;
    let invoice_url = response.url().clone();
    let html = Html::parse_document(&response.text().await.unwrap());
    guard_unwrap!(let &[qr_code] = css_select(&html, "img.qr-code").as_slice());
    let qr_code_url = qr_code.value().attr("src").unwrap();
    assert!(
      Regex::new("^/invoice/[a-f0-9]{64}.svg$")
        .unwrap()
        .is_match(qr_code_url),
      "qr code URL is not a qr code url: {}",
      qr_code_url,
    );
    let qr_code_url = invoice_url.join(qr_code_url).unwrap();
    let response = get(&qr_code_url).await;
    assert_eq!(
      response.headers().get(header::CONTENT_TYPE).unwrap(),
      "image/svg+xml"
    );
    let qr_code_svg = response.text().await.unwrap();
    let payment_request = decode_qr_code_from_svg(&qr_code_svg);

    let sender = LndTestContext::new().await;
    sender.connect(&receiver).await;
    sender.generate_lnd_btc().await;
    sender.open_channel_to(&receiver, 1_000_000).await;
    let StdoutUntrimmed(_) =
      cmd!(sender.lncli_command().await, %"payinvoice --force", &payment_request);
    assert_eq!(text(&invoice_url).await, "precious content");
  });
}

#[test]
fn paying_invoice_allows_downloading_file() {
  let receiver = LndTestContext::new_blocking();
  test_with_lnd(&receiver.clone(), |context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("foo"), "precious content").unwrap();
    let response = get(&context.files_url().join("foo").unwrap()).await;
    let invoice_url = response.url().clone();
    let html = Html::parse_document(&response.text().await.unwrap());
    guard_unwrap!(let &[payment_request] = css_select(&html, ".payment-request").as_slice());
    let payment_request = payment_request.inner_html();
    let sender = LndTestContext::new().await;
    sender.connect(&receiver).await;
    sender.generate_lnd_btc().await;
    sender.open_channel_to(&receiver, 1_000_000).await;
    let StdoutUntrimmed(_) =
      cmd!(sender.lncli_command().await, %"payinvoice --force", &payment_request);
    assert_eq!(text(&invoice_url).await, "precious content");
  });
}

#[test]
fn allows_configuring_invoice_amount() {
  test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    use lightning_invoice::Invoice;
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1234 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("foo"), "precious content").unwrap();
    let response = get(&context.files_url().join("foo").unwrap()).await;
    let html = Html::parse_document(&response.text().await.unwrap());
    guard_unwrap!(let &[invoice_element] = css_select(&html, ".invoice").as_slice());
    assert_contains(&invoice_element.inner_html(), "1,234 satoshis");
    guard_unwrap!(let &[payment_request] = css_select(&html, ".payment-request").as_slice());
    let payment_request = payment_request.inner_html();
    let invoice = payment_request.parse::<Invoice>().unwrap();
    assert_eq!(invoice.amount_pico_btc().unwrap(), 1234 * 1000 * 10);
  });
}

#[test]
fn configuring_paid_without_base_price_returns_error() {
  let stderr = test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(context.files_directory().join(".agora.yaml"), "paid: true").unwrap();
    fs::write(context.files_directory().join("foo"), "precious content").unwrap();
    let response = reqwest::get(context.files_url().join("foo").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
  });
  assert_contains(
    &stderr,
    &format!(
      "Missing base price for paid file `www{}foo`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn returns_404_for_made_up_invoice() {
  let stderr = test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("test-filename"), "").unwrap();
    assert_eq!(
      reqwest::get(
        context
          .base_url()
          .join(&format!("invoice/{}/test-filename", "a".repeat(64)))
          .unwrap()
      )
      .await
      .unwrap()
      .status(),
      StatusCode::NOT_FOUND
    );
  });

  assert_contains(&stderr, &format!("Invoice not found: {}", "a".repeat(64)));
}

#[test]
fn returns_404_for_made_up_invoice_qr_code() {
  let stderr = test_with_lnd(&LndTestContext::new_blocking(), |context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("test-filename"), "").unwrap();
    assert_eq!(
      reqwest::get(
        context
          .base_url()
          .join(&format!("invoice/{}.svg", "a".repeat(64)))
          .unwrap()
      )
      .await
      .unwrap()
      .status(),
      StatusCode::NOT_FOUND
    );
  });

  assert_contains(&stderr, &format!("Invoice not found: {}", "a".repeat(64)));
}

#[test]
fn warns_when_lnd_is_unreachable_at_startup() {
  let context = LndTestContext::new_blocking();
  let stderr = test_with_arguments(
    &[
      "--lnd-rpc-authority",
      "127.0.0.1:12345",
      "--lnd-rpc-cert-path",
      context.cert_path().to_str().unwrap(),
      "--lnd-rpc-macaroon-path",
      context.invoice_macaroon_path().to_str().unwrap(),
    ],
    |_context| async move {},
  );
  assert_contains(
    &stderr,
    "warning: Cannot connect to LND gRPC server at `127.0.0.1:12345`: LND RPC call failed: ",
  );
}
