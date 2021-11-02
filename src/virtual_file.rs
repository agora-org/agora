use crate::common::*;
use cradle::run;

pub(crate) async fn serve(
  stderr: &mut Stderr,
  file_name: &str,
  virtual_file: &VirtualFile,
) -> Response<Body> {
  let VirtualFile::Script { source } = virtual_file;
  let tempdir = tempfile::Builder::new()
    .prefix("agora-script")
    .tempdir()
    .expect("fixme");
  let script_file = tempdir.path().join(file_name);
  tokio::fs::write(&script_file, source).await.expect("fixme");
  run!(%"chmod +x", &script_file);
  let output = tokio::process::Command::new(script_file)
    .output()
    .await
    .expect("fixme");
  if !output.stderr.is_empty() {
    write!(
      stderr,
      "script `{}` stderr: {}",
      file_name,
      String::from_utf8_lossy(&output.stderr)
    )
    .ok();
  }
  if !output.status.success() {
    write!(stderr, "script `{}` failed: {}", file_name, output.status).ok();
  }
  Response::builder()
    .body(output.stdout.into())
    .expect("builder arguments are valid")
}
