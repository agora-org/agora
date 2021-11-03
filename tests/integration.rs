use ::{
  agora_test_context::{get, AgoraInstance},
  guard::guard_unwrap,
  hyper::{header, StatusCode},
  reqwest::{redirect::Policy, Client, Url},
  scraper::{ElementRef, Html, Selector},
  std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
  },
  tokio::io::AsyncWriteExt,
};

#[test]
fn server_listens_on_all_ip_addresses_http() {
  let tempdir = tempfile::tempdir().unwrap();
  let agora = AgoraInstance::new(tempdir, vec!["--http-port=0"], false);
  let port = agora.port;
  assert_eq!(
    reqwest::blocking::get(agora.base_url()).unwrap().status(),
    StatusCode::OK
  );
  let stderr = agora.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTP connections on `0.0.0.0:{}`",
    port
  )));
}

#[test]
fn server_listens_on_all_ip_addresses_https() {
  let tempdir = tempfile::tempdir().unwrap();
  let agora = AgoraInstance::new(
    tempdir,
    vec![
      "--https-port=0",
      "--acme-cache-directory=cache",
      "--acme-domain=foo",
    ],
    false,
  );
  let port = agora.port;
  let stderr = agora.kill();
  assert!(stderr.contains(&format!(
    "Listening for HTTPS connections on `0.0.0.0:{}`",
    port
  )));
}

struct TestContext {
  base_url: reqwest::Url,
  files_url: reqwest::Url,
  files_directory: PathBuf,
}

impl TestContext {
  pub(crate) fn base_url(&self) -> &reqwest::Url {
    &self.base_url
  }

  pub(crate) fn files_url(&self) -> &reqwest::Url {
    &self.files_url
  }

  pub(crate) fn files_directory(&self) -> &std::path::Path {
    &self.files_directory
  }

  pub(crate) fn write(&self, path: &str, content: &str) -> std::path::PathBuf {
    let path = self.files_directory.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, content).unwrap();
    path
  }
}

fn test<Function, F>(f: Function)
where
  Function: FnOnce(TestContext) -> F,
  F: Future<Output = ()> + 'static,
{
  let tempdir = tempfile::tempdir().unwrap();

  let agora = AgoraInstance::new(tempdir, vec!["--address=localhost", "--http-port=0"], false);

  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap()
    .block_on(async {
      f(TestContext {
        base_url: agora.base_url().clone(),
        files_url: agora.base_url().join("files/").unwrap(),
        files_directory: agora.tempdir.path().to_owned(),
      })
      .await;
    });

  agora.kill();
}

async fn redirect_url(context: &TestContext, url: &Url) -> Url {
  let client = Client::builder().redirect(Policy::none()).build().unwrap();
  let request = client.get(url.clone()).build().unwrap();
  let response = client.execute(request).await.unwrap();
  assert_eq!(response.status(), StatusCode::FOUND);
  context
    .base_url()
    .join(
      response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap(),
    )
    .unwrap()
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
fn index_route_status_code_is_200() {
  test(|context| async move {
    assert_eq!(
      reqwest::get(context.base_url().clone())
        .await
        .unwrap()
        .status(),
      200
    )
  });
}

#[test]
fn index_route_redirects_to_files() {
  test(|context| async move {
    let redirect_url = redirect_url(&context, context.base_url()).await;
    assert_eq!(&redirect_url, context.files_url());
  });
}

#[test]
fn no_trailing_slash_redirects_to_trailing_slash() {
  test(|context| async move {
    fs::create_dir(context.files_directory().join("foo")).unwrap();
    let redirect_url = redirect_url(&context, &context.files_url().join("foo").unwrap()).await;
    assert_eq!(redirect_url, context.files_url().join("foo/").unwrap());
  });
}

#[test]
fn files_route_without_trailing_slash_redirects_to_files() {
  test(|context| async move {
    let redirect_url = redirect_url(&context, &context.base_url().join("files").unwrap()).await;
    assert_eq!(&redirect_url, context.files_url());
  });
}

#[test]
fn unknown_route_status_code_is_404() {
  test(|context| async move {
    assert_eq!(
      reqwest::get(context.base_url().join("huhu").unwrap())
        .await
        .unwrap()
        .status(),
      404
    )
  });
}

#[test]
fn index_route_contains_title() {
  test(|context| async move {
    let haystack = text(context.base_url()).await;
    let needle = "<title>agora</title>";
    assert_contains(&haystack, needle);
  });
}

#[test]
fn listing_contains_file() {
  test(|context| async move {
    context.write("some-test-file.txt", "");
    let haystack = html(context.base_url()).await.root_element().html();
    let needle = "some-test-file.txt";
    assert_contains(&haystack, needle);
  });
}

#[test]
fn listing_contains_multiple_files() {
  test(|context| async move {
    context.write("a.txt", "");
    context.write("b.txt", "");
    let haystack = html(context.base_url()).await.root_element().html();
    assert_contains(&haystack, "a.txt");
    assert_contains(&haystack, "b.txt");
  });
}

#[test]
fn listing_is_sorted_alphabetically() {
  test(|context| async move {
    context.write("b", "");
    context.write("c", "");
    context.write("a", "");
    let html = html(context.base_url()).await;
    let haystack: Vec<&str> = css_select(&html, ".listing a:not([download])")
      .into_iter()
      .map(|x| x.text())
      .flatten()
      .collect();
    assert_eq!(haystack, vec!["a", "b", "c"]);
  });
}

#[test]
fn listed_files_can_be_played_in_browser() {
  test(|context| async move {
    context.write("some-test-file.txt", "contents");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "some-test-file.txt");
    let file_url = a.value().attr("href").unwrap();
    let file_url = context.files_url().join(file_url).unwrap();
    let file_contents = text(&file_url).await;
    assert_eq!(file_contents, "contents");
  });
}

#[test]
fn listed_files_have_download_links() {
  test(|context| async move {
    context.write("some-test-file.txt", "contents");
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, "a[download]").as_slice());
    assert_contains(&a.inner_html(), "download");
    let file_url = a.value().attr("href").unwrap();
    let file_url = context.files_url().join(file_url).unwrap();
    let file_contents = text(&file_url).await;
    assert_eq!(file_contents, "contents");
  });
}

#[test]
fn listed_files_have_percent_encoded_hrefs() {
  test(|context| async move {
    context.write("filename with special chäracters", "");
    let html = html(context.base_url()).await;
    let links = css_select(&html, ".listing a");
    assert_eq!(links.len(), 2);
    for link in links {
      assert_eq!(
        link.value().attr("href").unwrap(),
        "filename%20with%20special%20ch%C3%A4racters"
      );
    }
  });
}

#[test]
fn serves_error_pages() {
  test(|context| async move {
    let response = reqwest::get(context.files_url().join("foo.txt").unwrap())
      .await
      .unwrap();
    assert_contains(&response.text().await.unwrap(), "404 Not Found");
  });
}

#[test]
#[cfg(unix)]
fn downloaded_files_are_streamed() {
  use futures::StreamExt;
  use tokio::{fs::OpenOptions, sync::oneshot};

  test(|context| async move {
    let fifo_path = context.files_directory().join("fifo");

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

    let mut stream = get(&context.files_url().join("fifo").unwrap())
      .await
      .bytes_stream();

    assert_eq!(stream.next().await.unwrap().unwrap(), "hello");

    sender.send(()).unwrap();

    writer.await.unwrap();
  });
}

#[test]
fn downloaded_files_have_correct_content_type() {
  test(|context| async move {
    context.write("foo.mp4", "hello");

    let response = get(&context.files_url().join("foo.mp4").unwrap()).await;

    assert_eq!(
      response.headers().get(header::CONTENT_TYPE).unwrap(),
      "video/mp4"
    );
  });
}

#[test]
fn unknown_files_have_no_content_type() {
  test(|context| async move {
    context.write("foo", "hello");

    let response = get(&context.files_url().join("foo").unwrap()).await;

    assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
  });
}

#[test]
fn filenames_with_spaces() {
  test(|context| async move {
    context.write("foo bar", "hello");

    let response = text(&context.files_url().join("foo%20bar").unwrap()).await;

    assert_eq!(response, "hello");
  });
}

#[test]
fn subdirectories_appear_in_listings() {
  test(|context| async move {
    context.write("foo/bar.txt", "hello");
    let root_listing = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&root_listing, ".listing a").as_slice());
    assert_eq!(a.inner_html(), "foo/");
    let subdir_url = context
      .files_url()
      .join(a.value().attr("href").unwrap())
      .unwrap();
    let subdir_listing = html(&subdir_url).await;
    guard_unwrap!(let &[a] = css_select(&subdir_listing, ".listing a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "bar.txt");
    let file_url = subdir_url.join(a.value().attr("href").unwrap()).unwrap();
    assert_eq!(text(&file_url).await, "hello");
  });
}

#[test]
fn redirects_correctly_for_two_layers_of_subdirectories() {
  test(|context| async move {
    context.write("foo/bar/baz.txt", "");
    let listing = html(&context.files_url().join("foo/bar").unwrap()).await;
    guard_unwrap!(let &[a] = css_select(&listing, ".listing a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "baz.txt")
  });
}

#[test]
fn requesting_files_with_trailing_slash_redirects() {
  test(|context| async move {
    context.write("foo", "");
    let response = reqwest::get(context.files_url().join("foo/").unwrap())
      .await
      .unwrap();
    assert!(
      response.url().as_str().ends_with("/files/foo"),
      "{} didn't end with /files/foo",
      response.url()
    );
  });
}

#[test]
fn listings_are_not_cached() {
  test(|context| async move {
    let response = reqwest::get(context.files_url().clone()).await.unwrap();
    assert_eq!(
      response.headers().get(header::CACHE_CONTROL).unwrap(),
      "no-store, max-age=0",
    );
  });
}

#[test]
fn files_are_not_cached() {
  test(|context| async move {
    context.write("foo", "bar");
    let response = reqwest::get(context.files_url().join("foo").unwrap())
      .await
      .unwrap();
    assert_eq!(
      response.headers().get(header::CACHE_CONTROL).unwrap(),
      "no-store, max-age=0",
    );
    assert_eq!(response.text().await.unwrap(), "bar");
  });
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
  test(|context| async move {
    context.write("file", "contents");
    symlink("file", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn allow_file_downloads_via_local_intermediate_symlinks() {
  test(|context| async move {
    context.write("dir/file", "contents");
    symlink("dir", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link/file").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn allow_listing_directories_via_local_symlinks() {
  test(|context| async move {
    let dir = context.files_directory().join("dir");
    fs::create_dir(&dir).unwrap();
    symlink("dir", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn allow_listing_directories_via_intermediate_local_symlinks() {
  test(|context| async move {
    let dir = context.files_directory().join("dir");
    fs::create_dir(&dir).unwrap();
    symlink("dir", context.files_directory().join("link"));
    fs::create_dir(dir.join("subdir")).unwrap();
    let response = reqwest::get(context.files_url().join("link/subdir").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn show_local_symlinks_in_listings() {
  test(|context| async move {
    context.write("file", "");
    symlink("file", context.files_directory().join("link"));
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a, b] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "file");
    assert_eq!(b.inner_html(), "link");
  });
}

#[test]
fn remove_escaping_symlinks_from_listings() {
  test(|context| async move {
    context.write("../escaping", "");
    context.write("local", "");
    symlink("../escaping", context.files_directory().join("link"));
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, ".listing a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "local");
  });
}

#[test]
fn serves_static_assets() {
  test(|context| async move {
    let response = text(&context.base_url().join("static/index.css").unwrap()).await;
    let expected = fs::read_to_string("static/index.css").unwrap();
    assert_eq!(response, expected);
  });
}

#[test]
fn sets_mime_types_for_static_assets() {
  test(|context| async move {
    let response = get(&context.base_url().join("static/index.css").unwrap()).await;
    assert_eq!(
      response.headers().get(header::CONTENT_TYPE).unwrap(),
      "text/css"
    );
  });
}

#[test]
fn missing_asset_not_found() {
  test(|context| async move {
    let response = reqwest::get(context.base_url().join("static/does-not-exist").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
}

#[test]
fn listing_does_not_contain_hidden_file() {
  test(|context| async move {
    context.write(".some-test-file.txt", "");
    let haystack = html(context.base_url()).await.root_element().html();
    let needle = ".some-test-file.txt";
    assert_not_contains(&haystack, needle);
  });
}

#[test]
fn return_404_for_hidden_files() {
  test(|context| async move {
    context.write(".foo.txt", "");
    assert_eq!(
      reqwest::get(context.files_url().join(".foo.txt").unwrap())
        .await
        .unwrap()
        .status(),
      StatusCode::NOT_FOUND
    )
  });
}

#[test]
fn return_404_for_hidden_directories() {
  test(|context| async move {
    let dir = context.files_directory().join(".dir");
    fs::create_dir(&dir).unwrap();
    let response = reqwest::get(context.files_url().join(".dir").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
}

#[test]
fn return_404_for_files_in_hidden_directories() {
  test(|context| async move {
    context.write(".dir/foo.txt", "hello");
    let response = reqwest::get(context.files_url().join(".dir/foo.txt").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
}

#[test]
fn apple_touch_icon_is_served_under_root() {
  test(|context| async move {
    let response = get(&context.base_url().join("apple-touch-icon.png").unwrap()).await;
    assert_eq!(
      response.headers().get(header::CONTENT_TYPE).unwrap(),
      "image/png"
    );
  });
}

#[test]
fn favicon_is_served_at_favicon_ico() {
  test(|context| async move {
    let response = get(&context.base_url().join("favicon.ico").unwrap()).await;
    assert_eq!(
      response.headers().get(header::CONTENT_TYPE).unwrap(),
      "image/x-icon"
    );
  });
}
