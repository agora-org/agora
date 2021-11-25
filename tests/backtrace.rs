use ::{agora_test_context::AgoraInstance, reqwest::StatusCode, std::fs};

#[test]
fn errors_contain_backtraces() {
  let tempdir = tempfile::tempdir().unwrap();
  fs::write(tempdir.path().join(".hidden"), "").unwrap();
  let agora = AgoraInstance::new(tempdir, vec!["--address=localhost", "--http-port=0"], true);
  let status = reqwest::blocking::get(agora.base_url().join("/files/.hidden").unwrap())
    .unwrap()
    .status();
  assert_eq!(status, StatusCode::NOT_FOUND);
  let stderr = agora.kill();
  assert!(stderr.contains("agora::files::Files::check_path"));
}
