use crate::{
  common::*,
  test_utils::{
    assert_contains, https_client, set_up_test_certificate, test, test_with_arguments,
    test_with_environment,
  },
};
use guard::guard_unwrap;
use pretty_assertions::assert_eq;
use reqwest::Url;
use scraper::{ElementRef, Html, Selector};
use std::{net::TcpListener, path::MAIN_SEPARATOR};
use tokio::net::TcpStream;

#[cfg(feature = "slow-tests")]
mod browser_tests;
#[cfg(feature = "slow-tests")]
mod slow_tests;

async fn get(url: &Url) -> reqwest::Response {
  let response = reqwest::get(url.clone()).await.unwrap();
  assert_eq!(response.status(), StatusCode::OK);
  response
}

async fn text(url: &Url) -> String {
  get(url).await.text().await.unwrap()
}

async fn html(url: &Url) -> Html {
  Html::parse_document(&text(url).await)
}

fn css_select<'a>(html: &'a Html, selector: &'a str) -> Vec<ElementRef<'a>> {
  let selector = Selector::parse(selector).unwrap();
  html.select(&selector).collect::<Vec<_>>()
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
fn configure_port() {
  let free_port = {
    TcpListener::bind("127.0.0.1:0")
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  };

  let mut environment = Environment::test();
  environment.arguments = vec![
    "agora".into(),
    "--address=localhost".into(),
    "--directory=www".into(),
    "--http-port".into(),
    free_port.to_string().into(),
  ];
  let www = environment.working_directory.join("www");
  std::fs::create_dir(&www).unwrap();

  test_with_environment(&mut environment, |_| async move {
    assert_eq!(
      reqwest::get(format!("http://localhost:{}", free_port))
        .await
        .unwrap()
        .status(),
      200
    )
  });
}

#[test]
fn server_aborts_when_directory_does_not_exist() {
  let mut environment = Environment::test();

  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(
      async {
        #![allow(clippy::unused_unit)]
        let error = Server::setup(&mut environment).await.err().unwrap();
        guard_unwrap!(let Error::FilesystemIo { .. } = error);
      },
    );
}

#[test]
#[cfg(unix)]
fn errors_in_request_handling_cause_500_status_codes() {
  use std::os::unix::fs::PermissionsExt;

  let stderr = test(|context| async move {
    let file = context.write("foo", "");
    let mut permissions = file.metadata().unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(file, permissions).unwrap();
    let status = reqwest::get(context.files_url().join("foo").unwrap())
      .await
      .unwrap()
      .status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
  });

  assert_contains(
    &stderr,
    "IO error accessing filesystem at `www/foo`: Permission denied (os error 13)",
  );
}

#[test]
fn disallow_parent_path_component() {
  let stderr = test(|context| async move {
    let mut stream =
      TcpStream::connect(format!("localhost:{}", context.base_url().port().unwrap()))
        .await
        .unwrap();
    stream
      .write_all(b"GET /files/foo/../bar.txt HTTP/1.1\n\n")
      .await
      .unwrap();
    let response = &mut [0; 1024];
    let bytes = stream.read(response).await.unwrap();
    let response = str::from_utf8(&response[..bytes]).unwrap();
    assert_contains(response, "HTTP/1.1 400 Bad Request");
  });
  assert_contains(&stderr, "Invalid URI file path: foo/../bar.txt");
}

#[test]
fn disallow_empty_path_component() {
  let stderr = test(|context| async move {
    assert_eq!(
      reqwest::get(format!("{}foo//bar.txt", context.files_url()))
        .await
        .unwrap()
        .status(),
      StatusCode::BAD_REQUEST
    )
  });
  assert_contains(&stderr, "Invalid URI file path: foo//bar.txt");
}

#[test]
fn disallow_absolute_path() {
  let stderr = test(|context| async move {
    assert_eq!(
      reqwest::get(format!("{}/foo.txt", context.files_url()))
        .await
        .unwrap()
        .status(),
      StatusCode::BAD_REQUEST
    )
  });
  assert_contains(&stderr, "Invalid URI file path: /foo.txt");
}

#[test]
fn return_404_for_missing_files() {
  let stderr = test(|context| async move {
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
fn configure_source_directory() {
  let mut environment = Environment::test();
  environment.arguments = vec![
    "agora".into(),
    "--address=localhost".into(),
    "--http-port=0".into(),
    "--directory=src".into(),
  ];

  let src = environment.working_directory.join("src");
  fs::create_dir(&src).unwrap();
  fs::write(src.join("foo.txt"), "hello").unwrap();

  test_with_environment(&mut environment, |context| async move {
    assert_contains(&text(context.files_url()).await, "foo.txt");

    let file_contents = text(&context.files_url().join("foo.txt").unwrap()).await;
    assert_eq!(file_contents, "hello");
  });
}

#[test]
fn file_errors_are_associated_with_file_path() {
  let stderr = test(|context| async move {
    fs::create_dir(context.files_directory().join("foo")).unwrap();
    assert_eq!(
      reqwest::get(context.files_url().join("foo/bar.txt").unwrap())
        .await
        .unwrap()
        .status(),
      StatusCode::NOT_FOUND
    )
  });
  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `www{}foo{}bar.txt`",
      MAIN_SEPARATOR, MAIN_SEPARATOR,
    ),
  );
}

#[test]
fn disallow_file_downloads_via_escaping_symlinks() {
  let stderr = test(|context| async move {
    context.write("../file", "contents");
    symlink("../file", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `www{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_file_downloads_via_absolute_escaping_symlinks() {
  let stderr = test(|context| async move {
    let file = context.write("../file", "contents");
    let file = file.lexiclean();
    assert!(file.is_absolute());
    symlink(file, context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `www{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_file_downloads_via_escaping_intermediate_symlinks() {
  let stderr = test(|context| async move {
    context.write("../dir/file", "contents");
    symlink("../dir", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link/file").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `www{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_listing_directories_via_escaping_symlinks() {
  let stderr = test(|context| async move {
    let dir = context.files_directory().join("../dir");
    fs::create_dir(&dir).unwrap();
    symlink("../dir", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `www{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_listing_directories_via_intermediate_escaping_symlinks() {
  let stderr = test(|context| async move {
    let dir = context.files_directory().join("../dir");
    fs::create_dir(&dir).unwrap();
    symlink("../dir", context.files_directory().join("link"));
    fs::create_dir(dir.join("subdir")).unwrap();
    let response = reqwest::get(context.files_url().join("link/subdir").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `www{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn requesting_paid_file_with_no_lnd_returns_internal_error() {
  let stderr = test(|context| async move {
    context.write(".agora.yaml", "paid: true");
    context.write("foo", "precious content");
    let status = reqwest::get(context.files_url().join("foo").unwrap())
      .await
      .unwrap()
      .status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
  });

  assert_contains(
    &stderr,
    &format!(
      "Paid file request requires LND client configuration: `www{}foo`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn displays_index_markdown_files_as_html() {
  test(|context| async move {
    context.write(".index.md", "# test header");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[index_header] = css_select(&html, "h1").as_slice());
    assert_eq!(index_header.inner_html(), "test header");
  });
}

#[test]
fn returns_error_if_index_is_unusable() {
  let stderr = test(|context| async move {
    fs::create_dir(context.files_directory().join(".index.md")).unwrap();
    let status = reqwest::get(context.files_url().clone())
      .await
      .unwrap()
      .status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
  });

  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `www{}.index.md`: ",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn ignores_access_config_outside_of_base_directory() {
  test(|context| async move {
    context.write("../.agora.yaml", "{paid: true, base-price: 1000 sat}");
    context.write("foo", "foo");
    let body = text(&context.files_url().join("foo").unwrap()).await;
    assert_eq!(body, "foo");
  });
}

#[test]
fn paid_files_dont_have_download_button() {
  #![allow(clippy::unused_unit)]
  test(|context| async move {
    context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
    context.write("foo", "foo");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[] = css_select(&html, ".listing a[download]").as_slice());
    guard_unwrap!(let &[link] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(link.inner_html(), "foo");
  });
}

#[test]
fn filenames_with_percent_encoded_characters() {
  test(|context| async move {
    context.write("=", "contents");
    let contents = text(&context.files_url().join("%3D").unwrap()).await;
    assert_eq!(contents, "contents");
    let contents = text(&context.files_url().join("=").unwrap()).await;
    assert_eq!(contents, "contents");
  });
}

#[test]
fn filenames_with_percent_encoding() {
  test(|context| async move {
    context.write("foo%20bar", "contents");
    let contents = text(&context.files_url().join("foo%2520bar").unwrap()).await;
    assert_eq!(contents, "contents");
  });
}

#[test]
fn filenames_with_invalid_percent_encoding() {
  test(|context| async move {
    context.write("%80", "contents");
    let contents = text(&context.files_url().join("%2580").unwrap()).await;
    assert_eq!(contents, "contents");
  });
}

#[test]
fn space_is_percent_encoded() {
  test(|context| async move {
    context.write("foo bar", "contents");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.value().attr("href").unwrap(), "foo%20bar");
  });
}

#[test]
fn doesnt_percent_encode_allowed_ascii_characters() {
  test(|context| async move {
    let allowed_ascii_characters = if cfg!(windows) {
      "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!$&'()+,-.;=@_~"
    } else {
      "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!$&'()*+,-.:;=?@_~"
    };
    context.write(allowed_ascii_characters, "contents");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.value().attr("href").unwrap(), allowed_ascii_characters);
  });
}

#[test]
fn percent_encodes_unicode() {
  test(|context| async move {
    context.write("Å", "contents");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.value().attr("href").unwrap(), "%C3%85");
  });
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
fn creates_cert_cache_directory_if_it_doesnt_exist() {
  test_with_arguments(
    &[
      "--acme-cache-directory",
      "cache-directory",
      "--https-port=0",
      "--acme-domain=localhost",
    ],
    |context| async move {
      let cache_directory = context.working_directory().join("cache-directory");
      for _ in 0..100 {
        if cache_directory.is_dir() {
          return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
      panic!("Cache directory not created after ten seconds");
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
