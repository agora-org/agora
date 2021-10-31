use crate::common::*;
use cradle::prelude::*;
use tempfile::TempDir;

pub(crate) async fn serve(config: Config, file_name: &str) -> Option<Response<Body>> {
  match config.files.get(file_name) {
    None => None,
    Some(virtual_file) => {
      let VirtualFile::script { source } = virtual_file;
      let source = if dbg!(source).starts_with("#!") {
        source.to_string()
      } else {
        format!("#!/usr/bin/sh\n{}", source)
      };
      let tempdir = TempDir::new().expect("fixme");
      let script_file = tempdir.path().join("script");
      tokio::fs::write(&script_file, source).await.expect("fixme");
      run!(%"chmod +x", &script_file);
      run!(%"cat", &script_file);
      let output = tokio::process::Command::new(script_file)
        .output()
        .await
        .expect("fixme");
      Some(
        Response::builder()
          .body(output.stdout.into())
          .expect("builder arguments are valid"),
      )
    }
  }
}
