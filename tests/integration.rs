use {
  agora_test_context::AgoraTestContext,
  executable_path::executable_path,
  guard::guard_unwrap,
  hyper::{header, StatusCode},
  lexiclean::Lexiclean,
  scraper::{ElementRef, Html, Selector},
  std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, MAIN_SEPARATOR},
    process::Command,
    str, thread,
    time::Duration,
  },
};

fn css_select<'a>(html: &'a Html, selector: &'a str) -> Vec<ElementRef<'a>> {
  let selector = Selector::parse(selector).unwrap();
  html.select(&selector).collect::<Vec<_>>()
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

#[test]
fn server_listens_on_all_ip_addresses_http() {
  let context = AgoraTestContext::builder().address(None).build();
  let port = context.port();
  assert_eq!(context.status(""), StatusCode::OK);
  let stderr = context.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTP connections on `0.0.0.0:{}`",
    port
  )));
}

#[test]
fn server_listens_on_all_ip_addresses_https() {
  let context = AgoraTestContext::builder()
    .http_port(None)
    .address(None)
    .args(&[
      "--https-port=0",
      "--acme-cache-directory=cache",
      "--acme-domain=foo",
    ])
    .build();
  let port = context.port();
  let stderr = context.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTPS connections on `0.0.0.0:{}`",
    port
  )));
}

#[test]
fn index_route_status_code_is_200() {
  let context = AgoraTestContext::builder().build();
  assert_eq!(context.status(""), 200);
}

#[test]
fn index_route_redirects_to_files() {
  let context = AgoraTestContext::builder().build();
  let redirect_url = context.redirect_url("");
  assert_eq!(&redirect_url, context.files_url());
}

#[test]
fn no_trailing_slash_redirects_to_trailing_slash() {
  let context = AgoraTestContext::builder().build();
  fs::create_dir(context.files_directory().join("foo")).unwrap();
  let redirect_url = context.redirect_url("files/foo");
  assert_eq!(redirect_url, context.files_url().join("foo/").unwrap());
}

#[test]
fn files_route_without_trailing_slash_redirects_to_files() {
  let context = AgoraTestContext::builder().build();
  let redirect_url = context.redirect_url("files");
  assert_eq!(&redirect_url, context.files_url());
}

#[test]
fn unknown_route_status_code_is_404() {
  let context = AgoraTestContext::builder().build();
  assert_eq!(
    reqwest::blocking::get(context.base_url().join("huhu").unwrap())
      .unwrap()
      .status(),
    404
  )
}

#[test]
fn index_route_contains_title() {
  let context = AgoraTestContext::builder().build();
  let haystack = context.text("");
  let needle = "<title>/ · Agora</title>";
  assert_contains(&haystack, needle);
}

#[test]
fn directory_route_title_contains_directory_name() {
  let context = AgoraTestContext::builder().build();
  context.create_dir_all("some-directory");
  let haystack = context.text("files/some-directory");
  let needle = "<title>/some-directory/ · Agora</title>";
  assert_contains(&haystack, needle);
}

#[test]
fn error_page_title_contains_error_text() {
  let context = AgoraTestContext::builder().build();
  let url = context.base_url().join("nonexistent-file.txt").unwrap();
  let response = reqwest::blocking::get(url).unwrap();
  let haystack = response.text().unwrap();
  let needle = "<title>Not Found · Agora</title>";
  assert_contains(&haystack, needle);
}

#[test]
fn listing_contains_file() {
  let context = AgoraTestContext::builder().build();
  context.write("some-test-file.txt", "");
  let haystack = context.html("").root_element().html();
  let needle = "some-test-file.txt";
  assert_contains(&haystack, needle);
}

#[test]
fn listing_contains_multiple_files() {
  let context = AgoraTestContext::builder().build();
  context.write("a.txt", "");
  context.write("b.txt", "");
  let haystack = context.html("").root_element().html();
  assert_contains(&haystack, "a.txt");
  assert_contains(&haystack, "b.txt");
}

#[test]
fn listing_is_sorted_alphabetically() {
  let context = AgoraTestContext::builder().build();
  context.write("b", "");
  context.write("c", "");
  context.write("a", "");
  let html = context.html("");
  let haystack: Vec<&str> = css_select(&html, ".listing a:not([download])")
    .into_iter()
    .flat_map(|x| x.text())
    .collect();
  assert_eq!(haystack, vec!["a", "b", "c"]);
}

#[test]
fn listed_files_can_be_played_in_browser() {
  let context = AgoraTestContext::builder().build();
  context.write("some-test-file.txt", "contents");
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.inner_html(), "some-test-file.txt");
  let file_url = a.value().attr("href").unwrap();
  let file_contents = context.text(&format!("files/{}", file_url));
  assert_eq!(file_contents, "contents");
}

#[test]
fn listed_files_have_download_links() {
  let context = AgoraTestContext::builder().build();
  context.write("some-test-file.txt", "contents");
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, "a[download]").as_slice());
  assert_contains(&a.inner_html(), "download");
  let file_url = a.value().attr("href").unwrap();
  let file_contents = context.text(&format!("files/{}", file_url));
  assert_eq!(file_contents, "contents");
}

#[test]
fn listed_files_have_percent_encoded_hrefs() {
  let context = AgoraTestContext::builder().build();
  context.write("filename with special chäracters", "");
  let html = context.html("");
  let links = css_select(&html, ".listing a");
  assert_eq!(links.len(), 2);
  for link in links {
    assert_eq!(
      link.value().attr("href").unwrap(),
      "filename%20with%20special%20ch%C3%A4racters"
    );
  }
}

#[test]
fn serves_error_pages() {
  let context = AgoraTestContext::builder().build();
  let response = reqwest::blocking::get(context.files_url().join("foo.txt").unwrap()).unwrap();
  assert_contains(&response.text().unwrap(), "404 Not Found");
}

#[test]
#[cfg(unix)]
fn downloaded_files_are_streamed() {
  use {
    futures::StreamExt,
    reqwest::Url,
    tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::oneshot},
  };

  let context = AgoraTestContext::builder().build();

  async fn get(url: &Url) -> reqwest::Response {
    let response = reqwest::get(url.clone()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
  }

  let files_url = context.files_url();
  let files_directory = context.files_directory();

  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async move {
      let fifo_path = files_directory.join("fifo");

      nix::unistd::mkfifo(&fifo_path, nix::sys::stat::Mode::S_IRWXU).unwrap();

      let (sender, receiver) = oneshot::channel();

      let writer = tokio::spawn(async move {
        let mut fifo = OpenOptions::new()
          .write(true)
          .open(&fifo_path)
          .await
          .unwrap();
        fifo.write_all(b"hello").await.unwrap();
        receiver.await.unwrap();
      });

      let mut stream = get(&files_url.join("fifo").unwrap()).await.bytes_stream();

      assert_eq!(stream.next().await.unwrap().unwrap(), "hello");

      sender.send(()).unwrap();

      writer.await.unwrap();
    });

  context.kill();
}

#[test]
fn downloaded_files_have_correct_content_type() {
  let context = AgoraTestContext::builder().build();
  context.write("foo.mp4", "hello");

  let response = context.get("files/foo.mp4");

  assert_eq!(
    response.headers().get(header::CONTENT_TYPE).unwrap(),
    "video/mp4"
  );
}

#[test]
fn unknown_files_have_no_content_type() {
  let context = AgoraTestContext::builder().build();
  context.write("foo", "hello");

  let response = context.get("files/foo");

  assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
}

#[test]
fn filenames_with_spaces() {
  let context = AgoraTestContext::builder().build();
  context.write("foo bar", "hello");
  assert_eq!(context.text("files/foo%20bar"), "hello");
}

#[test]
fn subdirectories_appear_in_listings() {
  let context = AgoraTestContext::builder().build();
  context.write("foo/bar.txt", "hello");
  let root_listing = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&root_listing, ".listing a").as_slice());
  assert_eq!(a.inner_html(), "foo/");
  let subdir_url = context
    .files_url()
    .join(a.value().attr("href").unwrap())
    .unwrap();
  let subdir_listing = context.html(&subdir_url);
  guard_unwrap!(let &[a] = css_select(&subdir_listing, ".listing a:not([download])").as_slice());
  assert_eq!(a.inner_html(), "bar.txt");
  let file_url = subdir_url.join(a.value().attr("href").unwrap()).unwrap();
  assert_eq!(context.text(&file_url), "hello");
}

#[test]
fn redirects_correctly_for_two_layers_of_subdirectories() {
  let context = AgoraTestContext::builder().build();
  context.write("foo/bar/baz.txt", "");
  let listing = context.html("files/foo/bar");
  guard_unwrap!(let &[a] = css_select(&listing, ".listing a:not([download])").as_slice());
  assert_eq!(a.inner_html(), "baz.txt")
}

#[test]
fn requesting_files_with_trailing_slash_redirects() {
  let context = AgoraTestContext::builder().build();
  context.write("foo", "");
  let response = reqwest::blocking::get(context.files_url().join("foo/").unwrap()).unwrap();
  assert!(
    response.url().as_str().ends_with("/files/foo"),
    "{} didn't end with /files/foo",
    response.url()
  );
}

#[test]
fn listings_are_not_cached() {
  let context = AgoraTestContext::builder().build();
  let response = reqwest::blocking::get(context.files_url().clone()).unwrap();
  assert_eq!(
    response.headers().get(header::CACHE_CONTROL).unwrap(),
    "no-store, max-age=0",
  );
}

#[test]
fn files_are_not_cached() {
  let context = AgoraTestContext::builder().build();
  context.write("foo", "bar");
  let response = reqwest::blocking::get(context.files_url().join("foo").unwrap()).unwrap();
  assert_eq!(
    response.headers().get(header::CACHE_CONTROL).unwrap(),
    "no-store, max-age=0",
  );
  assert_eq!(response.text().unwrap(), "bar");
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
fn allow_file_downloads_via_local_symlinks() {
  let context = AgoraTestContext::builder().build();
  context.write("file", "contents");
  symlink("file", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn allow_file_downloads_via_local_intermediate_symlinks() {
  let context = AgoraTestContext::builder().build();
  context.write("dir/file", "contents");
  symlink("dir", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link/file").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn allow_listing_directories_via_local_symlinks() {
  let context = AgoraTestContext::builder().build();
  let dir = context.files_directory().join("dir");
  fs::create_dir(&dir).unwrap();
  symlink("dir", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn allow_listing_directories_via_intermediate_local_symlinks() {
  let context = AgoraTestContext::builder().build();
  let dir = context.files_directory().join("dir");
  fs::create_dir(&dir).unwrap();
  symlink("dir", context.files_directory().join("link"));
  fs::create_dir(dir.join("subdir")).unwrap();
  let response = reqwest::blocking::get(context.files_url().join("link/subdir").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn show_local_symlinks_in_listings() {
  let context = AgoraTestContext::builder().build();
  context.write("file", "");
  symlink("file", context.files_directory().join("link"));
  let html = context.html("files/");
  guard_unwrap!(let &[a, b] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.inner_html(), "file");
  assert_eq!(b.inner_html(), "link");
}

#[test]
fn remove_escaping_symlinks_from_listings() {
  let context = AgoraTestContext::builder().build();
  context.write("../escaping", "");
  context.write("local", "");
  symlink("../escaping", context.files_directory().join("link"));
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.inner_html(), "local");
}

#[test]
fn serves_static_assets() {
  let context = AgoraTestContext::builder().build();
  let response = context.text("static/index.css");
  let expected = fs::read_to_string("static/index.css").unwrap();
  assert_eq!(response, expected);
}

#[test]
fn sets_mime_types_for_static_assets() {
  let context = AgoraTestContext::builder().build();
  let response = context.get("static/index.css");
  assert_eq!(
    response.headers().get(header::CONTENT_TYPE).unwrap(),
    "text/css"
  );
}

#[test]
fn missing_asset_not_found() {
  let context = AgoraTestContext::builder().build();
  let response =
    reqwest::blocking::get(context.base_url().join("static/does-not-exist").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn listing_does_not_contain_hidden_file() {
  let context = AgoraTestContext::builder().build();
  context.write(".some-test-file.txt", "");
  let haystack = context.html("").root_element().html();
  let needle = ".some-test-file.txt";
  assert_not_contains(&haystack, needle);
}

#[test]
fn return_404_for_hidden_files() {
  let context = AgoraTestContext::builder().build();
  context.write(".foo.txt", "");
  assert_eq!(
    reqwest::blocking::get(context.files_url().join(".foo.txt").unwrap())
      .unwrap()
      .status(),
    StatusCode::NOT_FOUND
  )
}

#[test]
fn return_404_for_hidden_directories() {
  let context = AgoraTestContext::builder().build();
  let dir = context.files_directory().join(".dir");
  fs::create_dir(&dir).unwrap();
  let response = reqwest::blocking::get(context.files_url().join(".dir").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn return_404_for_files_in_hidden_directories() {
  let context = AgoraTestContext::builder().build();
  context.write(".dir/foo.txt", "hello");
  let response = reqwest::blocking::get(context.files_url().join(".dir/foo.txt").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn apple_touch_icon_is_served_under_root() {
  let context = AgoraTestContext::builder().build();
  let response = context.get("apple-touch-icon.png");
  assert_eq!(
    response.headers().get(header::CONTENT_TYPE).unwrap(),
    "image/png"
  );
}

#[test]
fn favicon_is_served_at_favicon_ico() {
  let context = AgoraTestContext::builder().build();
  let response = context.get("favicon.ico");
  assert_eq!(
    response.headers().get(header::CONTENT_TYPE).unwrap(),
    "image/x-icon"
  );
}

#[test]
#[cfg(unix)]
fn errors_in_request_handling_cause_500_status_codes() {
  use std::os::unix::fs::PermissionsExt;

  let context = AgoraTestContext::builder().build();
  let file = context.write("foo", "");
  let mut permissions = file.metadata().unwrap().permissions();
  permissions.set_mode(0o000);
  fs::set_permissions(file, permissions).unwrap();
  let status = reqwest::blocking::get(context.files_url().join("foo").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

  let stderr = context.kill();

  assert_contains(
    &stderr,
    "IO error accessing filesystem at `files/foo`: Permission denied (os error 13)",
  );
}

#[test]
fn disallow_parent_path_component() {
  let context = AgoraTestContext::builder().build();
  let mut stream =
    std::net::TcpStream::connect(format!("localhost:{}", context.base_url().port().unwrap()))
      .unwrap();
  stream
    .write_all(b"GET /files/foo/../bar.txt HTTP/1.1\n\n")
    .unwrap();
  let response = &mut [0; 1024];
  let bytes = stream.read(response).unwrap();
  let response = str::from_utf8(&response[..bytes]).unwrap();
  assert_contains(response, "HTTP/1.1 400 Bad Request");
  let stderr = context.kill();
  assert_contains(&stderr, "Invalid URI file path: foo/../bar.txt");
}

#[test]
fn disallow_empty_path_component() {
  let context = AgoraTestContext::builder().build();
  assert_eq!(
    reqwest::blocking::get(format!("{}foo//bar.txt", context.files_url()))
      .unwrap()
      .status(),
    StatusCode::BAD_REQUEST
  );
  let stderr = context.kill();
  assert_contains(&stderr, "Invalid URI file path: foo//bar.txt");
}

#[test]
fn disallow_absolute_path() {
  let context = AgoraTestContext::builder().build();
  assert_eq!(
    reqwest::blocking::get(format!("{}/foo.txt", context.files_url()))
      .unwrap()
      .status(),
    StatusCode::BAD_REQUEST
  );
  let stderr = context.kill();
  assert_contains(&stderr, "Invalid URI file path: /foo.txt");
}

#[test]
fn return_404_for_missing_files() {
  let context = AgoraTestContext::builder().build();
  assert_eq!(
    reqwest::blocking::get(context.files_url().join("foo.txt").unwrap())
      .unwrap()
      .status(),
    StatusCode::NOT_FOUND
  );
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `files{}foo.txt`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn returns_error_if_index_is_unusable() {
  let context = AgoraTestContext::builder().build();
  fs::create_dir(context.files_directory().join(".index.md")).unwrap();
  let status = reqwest::blocking::get(context.files_url().clone())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `files{}.index.md`: ",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn ignores_access_config_outside_of_base_directory() {
  let context = AgoraTestContext::builder().build();
  context.write("../.agora.yaml", "{paid: true, base-price: 1000 sat}");
  context.write("foo", "foo");
  let body = context.text("files/foo");
  assert_eq!(body, "foo");
}

#[test]
fn paid_files_dont_have_download_button() {
  #![allow(clippy::unused_unit)]
  let context = AgoraTestContext::builder().build();
  context.write(".agora.yaml", "{paid: true, base-price: 1000 sat}");
  context.write("foo", "foo");
  let html = context.html("files/");
  guard_unwrap!(let &[] = css_select(&html, ".listing a[download]").as_slice());
  guard_unwrap!(let &[link] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(link.inner_html(), "foo");
}

#[test]
fn filenames_with_percent_encoded_characters() {
  let context = AgoraTestContext::builder().build();
  context.write("=", "contents");
  let contents = context.text("files/%3D");
  assert_eq!(contents, "contents");
  let contents = context.text("files/=");
  assert_eq!(contents, "contents");
}

#[test]
fn filenames_with_percent_encoding() {
  let context = AgoraTestContext::builder().build();
  context.write("foo%20bar", "contents");
  let contents = context.text("files/foo%2520bar");
  assert_eq!(contents, "contents");
}

#[test]
fn filenames_with_invalid_percent_encoding() {
  let context = AgoraTestContext::builder().build();
  context.write("%80", "contents");
  let contents = context.text("files/%2580");
  assert_eq!(contents, "contents");
}

#[test]
fn space_is_percent_encoded() {
  let context = AgoraTestContext::builder().build();
  context.write("foo bar", "contents");
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.value().attr("href").unwrap(), "foo%20bar");
}

#[test]
fn doesnt_percent_encode_allowed_ascii_characters() {
  let context = AgoraTestContext::builder().build();
  let allowed_ascii_characters = if cfg!(windows) {
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!$&'()+,-.;=@_~"
  } else {
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!$&'()*+,-.:;=?@_~"
  };
  context.write(allowed_ascii_characters, "contents");
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.value().attr("href").unwrap(), allowed_ascii_characters);
}

#[test]
fn percent_encodes_unicode() {
  let context = AgoraTestContext::builder().build();
  context.write("Å", "contents");
  let html = context.html("files/");
  guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
  assert_eq!(a.value().attr("href").unwrap(), "%C3%85");
}

#[test]
fn requesting_paid_file_with_no_lnd_returns_internal_error() {
  let context = AgoraTestContext::builder().build();
  context.write(".agora.yaml", "paid: true");
  context.write("foo", "precious content");
  let status = reqwest::blocking::get(context.files_url().join("foo").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Paid file request requires LND client configuration: `files{}foo`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn displays_index_markdown_files_as_html() {
  let context = AgoraTestContext::builder().build();
  context.write(".index.md", "# test header");
  let html = context.html("files/");
  guard_unwrap!(let &[index_header] = css_select(&html, "h1").as_slice());
  assert_eq!(index_header.inner_html(), "test header");
}

#[test]
fn file_errors_are_associated_with_file_path() {
  let context = AgoraTestContext::builder().build();
  fs::create_dir(context.files_directory().join("foo")).unwrap();
  assert_eq!(
    reqwest::blocking::get(context.files_url().join("foo/bar.txt").unwrap())
      .unwrap()
      .status(),
    StatusCode::NOT_FOUND
  );
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "IO error accessing filesystem at `files{}foo{}bar.txt`",
      MAIN_SEPARATOR, MAIN_SEPARATOR,
    ),
  );
}

#[test]
fn disallow_file_downloads_via_escaping_symlinks() {
  let context = AgoraTestContext::builder().build();
  context.write("../file", "contents");
  symlink("../file", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `files{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_file_downloads_via_absolute_escaping_symlinks() {
  let context = AgoraTestContext::builder().build();
  let file = context.write("../file", "contents");
  let file = file.lexiclean();
  assert!(file.is_absolute());
  symlink(file, context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `files{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_file_downloads_via_escaping_intermediate_symlinks() {
  let context = AgoraTestContext::builder().build();
  context.write("../dir/file", "contents");
  symlink("../dir", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link/file").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `files{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_listing_directories_via_escaping_symlinks() {
  let context = AgoraTestContext::builder().build();
  let dir = context.files_directory().join("../dir");
  fs::create_dir(&dir).unwrap();
  symlink("../dir", context.files_directory().join("link"));
  let response = reqwest::blocking::get(context.files_url().join("link").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `files{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn disallow_listing_directories_via_intermediate_escaping_symlinks() {
  let context = AgoraTestContext::builder().build();
  let dir = context.files_directory().join("../dir");
  fs::create_dir(&dir).unwrap();
  symlink("../dir", context.files_directory().join("link"));
  fs::create_dir(dir.join("subdir")).unwrap();
  let response = reqwest::blocking::get(context.files_url().join("link/subdir").unwrap()).unwrap();
  assert_eq!(response.status(), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert_contains(
    &stderr,
    &format!(
      "Forbidden access to escaping symlink: `files{}link`",
      MAIN_SEPARATOR
    ),
  );
}

#[test]
fn listing_renders_file_sizes() {
  let context = AgoraTestContext::builder().build();
  context.write("some-test-file.txt", "abc");
  context.write("large-file.txt", &"A".repeat(4096));
  let html = context.html("files/");
  guard_unwrap!(let &[li1, li2] =  css_select(&html, ".listing li").as_slice());
  assert_contains(&li1.inner_html(), "large-file.txt");
  assert_contains(&li1.inner_html(), "4.0 KiB");

  assert_contains(&li2.inner_html(), "some-test-file.txt");
  assert_contains(&li2.inner_html(), "3 B");
}

#[test]
fn listing_does_not_render_directory_file_sizes() {
  let context = AgoraTestContext::builder().build();
  context.create_dir_all("some-directory");
  let html = context.html("files/");
  guard_unwrap!(let &[li] =  css_select(&html, ".listing li").as_slice());
  assert_contains(&li.inner_html(), "some-directory");
  assert_not_contains(&li.inner_html(), "B");
}

#[test]
fn configure_files_directory() {
  let context = AgoraTestContext::builder()
    .write("foo/bar.txt", "hello")
    .files_directory("foo")
    .build();

  assert_contains(&context.text("files/"), "bar.txt");

  let file_contents = context.text("files/bar.txt");

  assert_eq!(file_contents, "hello");
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

  let _context = AgoraTestContext::builder()
    .http_port(Some(free_port))
    .build();

  assert_eq!(
    reqwest::blocking::get(format!("http://localhost:{}", free_port))
      .unwrap()
      .status(),
    200
  );
}

#[test]
fn creates_cert_cache_directory_if_it_doesnt_exist() {
  let context = AgoraTestContext::builder()
    .args(&[
      "--acme-cache-directory",
      "cache-directory",
      "--https-port=0",
      "--acme-domain=localhost",
    ])
    .build();

  let cache_directory = context.current_dir().join("cache-directory");

  for _ in 0..100 {
    if cache_directory.is_dir() {
      return;
    }

    thread::sleep(Duration::from_millis(100));
  }

  panic!("Cache directory not created after ten seconds");
}

#[test]
fn server_aborts_when_directory_does_not_exist() {
  let output = Command::new(executable_path("agora"))
    .arg("--directory=does/not/exist")
    .arg("--http-port=0")
    .output()
    .unwrap();

  assert!(!output.status.success());

  assert_contains(
    str::from_utf8(&output.stderr).unwrap(),
    "IO error accessing filesystem",
  );
}

#[test]
#[cfg(not(windows))]
fn errors_printed_in_red_and_bold() {
  use {executable_path::executable_path, std::process::Command};

  let output = Command::new(executable_path("agora"))
    .arg("--directory=/does/not/exist")
    .arg("--http-port=8080")
    .env("TERM", "vt100")
    .output()
    .unwrap();

  let stderr = str::from_utf8(&output.stderr).unwrap();

  assert_contains(stderr, "\u{1b}[31merror\u{1b}[0m\u{1b}[1m: ");
}
