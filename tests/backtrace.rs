use {agora_test_context::AgoraTestContext, reqwest::StatusCode};

#[test]
fn errors_contain_backtraces() {
  let context = AgoraTestContext::builder().backtraces(true).build();
  assert_eq!(context.status("files/.hidden"), StatusCode::NOT_FOUND);
  let stderr = context.kill();
  assert!(stderr.contains("agora::vfs::Vfs::check_path"));
}
