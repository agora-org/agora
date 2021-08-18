use crate::{
  environment::Environment,
  error::Error,
  server::Server,
  test_utils::{
    assert_contains, assert_not_contains, test, test_with_arguments, test_with_environment,
    TestContext,
  },
};
use guard::guard_unwrap;
use hyper::{header, StatusCode};
use lexiclean::Lexiclean;
use pretty_assertions::assert_eq;
use reqwest::{redirect::Policy, Client, Url};
use scraper::{ElementRef, Html, Selector};
use std::{fs, net::TcpListener, path::Path, path::MAIN_SEPARATOR, str};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpStream,
};

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

#[test]
fn configure_port() {
  let free_port = {
    TcpListener::bind("127.0.0.1:0")
      .unwrap()
      .local_addr()
      .unwrap()
      .port()
  };

  let args = &["--port", &free_port.to_string()];

  test_with_arguments(args, |_| async move {
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
fn index_route_redirects_to_files() {
  test(|context| async move {
    let redirect_url = redirect_url(&context, context.base_url()).await;
    assert_eq!(&redirect_url, context.files_url());
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
fn server_aborts_when_directory_does_not_exist() {
  let mut environment = Environment::test(&[]);

  tokio::runtime::Builder::new_current_thread()
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
    let file = context.files_directory().join("foo");
    fs::write(&file, "").unwrap();
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
fn listing_contains_file() {
  test(|context| async move {
    fs::write(context.files_directory().join("some-test-file.txt"), "").unwrap();
    let haystack = html(context.base_url()).await.root_element().html();
    let needle = "some-test-file.txt";
    assert_contains(&haystack, needle);
  });
}

#[test]
fn listing_contains_multiple_files() {
  test(|context| async move {
    fs::write(context.files_directory().join("a.txt"), "").unwrap();
    fs::write(context.files_directory().join("b.txt"), "").unwrap();
    let haystack = html(context.base_url()).await.root_element().html();
    assert_contains(&haystack, "a.txt");
    assert_contains(&haystack, "b.txt");
  });
}

#[test]
fn listing_is_sorted_alphabetically() {
  test(|context| async move {
    fs::write(context.files_directory().join("b"), "").unwrap();
    fs::write(context.files_directory().join("c"), "").unwrap();
    fs::write(context.files_directory().join("a"), "").unwrap();
    let html = html(context.base_url()).await;
    let haystack: Vec<&str> = css_select(&html, "a:not([download])")
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
    fs::write(
      context.files_directory().join("some-test-file.txt"),
      "contents",
    )
    .unwrap();
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, "a:not([download])").as_slice());
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
    fs::write(
      context.files_directory().join("some-test-file.txt"),
      "contents",
    )
    .unwrap();
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
    fs::write(
      context
        .files_directory()
        .join("filename with special chäracters"),
      "",
    )
    .unwrap();
    let html = html(context.base_url()).await;
    let links = css_select(&html, "a");
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
fn serves_error_pages_for_404s() {
  test(|context| async move {
    let response = reqwest::get(context.files_url().join("foo.txt").unwrap())
      .await
      .unwrap();
    assert_contains(&response.text().await.unwrap(), "404 Not Found");
  });
}

#[test]
fn configure_source_directory() {
  let mut environment = Environment::test(&["--directory", "src"]);

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
    fs::write(context.files_directory().join("foo.mp4"), "hello").unwrap();

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
    fs::write(context.files_directory().join("foo"), "hello").unwrap();

    let response = get(&context.files_url().join("foo").unwrap()).await;

    assert_eq!(response.headers().get(header::CONTENT_TYPE), None);
  });
}

#[test]
fn filenames_with_spaces() {
  test(|context| async move {
    fs::write(context.files_directory().join("foo bar"), "hello").unwrap();

    let response = text(&context.files_url().join("foo%20bar").unwrap()).await;

    assert_eq!(response, "hello");
  });
}

#[test]
fn subdirectories_appear_in_listings() {
  test(|context| async move {
    fs::create_dir(context.files_directory().join("foo")).unwrap();
    fs::write(context.files_directory().join("foo/bar.txt"), "hello").unwrap();
    let root_listing = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&root_listing, "a").as_slice());
    assert_eq!(a.inner_html(), "foo/");
    let subdir_url = context
      .files_url()
      .join(a.value().attr("href").unwrap())
      .unwrap();
    let subdir_listing = html(&subdir_url).await;
    guard_unwrap!(let &[a] = css_select(&subdir_listing, "a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "bar.txt");
    let file_url = subdir_url.join(a.value().attr("href").unwrap()).unwrap();
    assert_eq!(text(&file_url).await, "hello");
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
fn redirects_correctly_for_two_layers_of_subdirectories() {
  test(|context| async move {
    fs::create_dir_all(context.files_directory().join("foo/bar")).unwrap();
    fs::write(context.files_directory().join("foo/bar/baz.txt"), "").unwrap();
    let listing = html(&context.files_url().join("foo/bar").unwrap()).await;
    guard_unwrap!(let &[a] = css_select(&listing, "a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "baz.txt")
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
fn requesting_files_with_trailing_slash_redirects() {
  test(|context| async move {
    fs::write(context.files_directory().join("foo"), "").unwrap();
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
    fs::write(context.files_directory().join("foo"), "bar").unwrap();
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
    fs::write(&context.files_directory().join("file"), "contents").unwrap();
    symlink("file", context.files_directory().join("link"));
    let response = reqwest::get(context.files_url().join("link").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn disallow_file_downloads_via_escaping_symlinks() {
  let stderr = test(|context| async move {
    fs::write(&context.files_directory().join("../file"), "contents").unwrap();
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
    let file = context.files_directory().join("../file").lexiclean();
    assert!(file.is_absolute());
    fs::write(&file, "contents").unwrap();
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
fn allow_file_downloads_via_local_intermediate_symlinks() {
  test(|context| async move {
    let dir = context.files_directory().join("dir");
    fs::create_dir(&dir).unwrap();
    symlink("dir", context.files_directory().join("link"));
    fs::write(dir.join("file"), "contents").unwrap();
    let response = reqwest::get(context.files_url().join("link/file").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
  });
}

#[test]
fn disallow_file_downloads_via_escaping_intermediate_symlinks() {
  let stderr = test(|context| async move {
    let dir = context.files_directory().join("../dir");
    fs::create_dir(&dir).unwrap();
    symlink("../dir", context.files_directory().join("link"));
    fs::write(dir.join("file"), "contents").unwrap();
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
fn show_local_symlinks_in_listings() {
  test(|context| async move {
    let file = context.files_directory().join("file");
    fs::write(&file, "").unwrap();
    symlink(file, context.files_directory().join("link"));
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a, b] = css_select(&html, "a:not([download])").as_slice());
    assert_eq!(a.inner_html(), "file");
    assert_eq!(b.inner_html(), "link");
  });
}

#[test]
fn remove_escaping_symlinks_from_listings() {
  test(|context| async move {
    fs::write(&context.files_directory().join("../escaping"), "").unwrap();
    fs::write(&context.files_directory().join("local"), "").unwrap();
    symlink("../escaping", context.files_directory().join("link"));
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[a] = css_select(&html, "a:not([download])").as_slice());
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
    fs::write(context.files_directory().join(".some-test-file.txt"), "").unwrap();
    let haystack = html(context.base_url()).await.root_element().html();
    let needle = ".some-test-file.txt";
    assert_not_contains(&haystack, needle);
  });
}

#[test]
fn return_404_for_hidden_files() {
  test(|context| async move {
    fs::write(context.files_directory().join(".foo.txt"), "").unwrap();
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
    let dir = context.files_directory().join(".dir");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("foo.txt"), "hello").unwrap();
    let response = reqwest::get(context.files_url().join(".dir/foo.txt").unwrap())
      .await
      .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
  });
}

#[test]
fn requesting_paid_file_with_no_lnd_returns_internal_error() {
  let stderr = test(|context| async move {
    fs::write(context.files_directory().join(".agora.yaml"), "paid: true").unwrap();
    fs::write(context.files_directory().join("foo"), "precious content").unwrap();
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
fn errors_contain_backtraces() {
  let stderr = test(|context| async move {
    fs::write(context.files_directory().join(".hidden"), "").unwrap();
    let status = reqwest::get(context.files_url().join(".hidden").unwrap())
      .await
      .unwrap()
      .status();
    assert_eq!(status, StatusCode::NOT_FOUND);
  });

  assert_contains(&stderr, "agora::files::Files::check_path");
}

#[test]
fn displays_index_markdown_files_as_html() {
  test(|context| async move {
    fs::write(context.files_directory().join(".index.md"), "# test header").unwrap();
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
    fs::write(
      context.temp_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("foo"), "foo").unwrap();
    let body = text(&context.files_url().join("foo").unwrap()).await;
    assert_eq!(body, "foo");
  });
}

#[test]
fn paid_files_dont_have_download_button() {
  #![allow(clippy::unused_unit)]
  test(|context| async move {
    fs::write(
      context.files_directory().join(".agora.yaml"),
      "{paid: true, base-price: 1000 sat}",
    )
    .unwrap();
    fs::write(context.files_directory().join("foo"), "foo").unwrap();
    let html = html(context.files_url()).await;
    guard_unwrap!(let &[] = css_select(&html, "a[download]").as_slice());
    guard_unwrap!(let &[link] = css_select(&html, "a:not([download])").as_slice());
    assert_eq!(link.inner_html(), "foo");
  });
}
