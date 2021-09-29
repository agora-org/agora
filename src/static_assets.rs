use crate::common::*;

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/"]
pub(crate) struct StaticAssets;

impl StaticAssets {
  pub(crate) fn serve(tail: &[&str]) -> Result<Response<Body>> {
    let path = tail.join("");
    match StaticAssets::get(&path) {
      Some(bytes) => {
        let mut builder = Response::builder();
        if let Some(guess) = mime_guess::from_path(path).first() {
          builder = builder.header(header::CONTENT_TYPE, guess.essence_str());
        }
        builder
          .body(bytes.into())
          .map_err(|error| Error::internal(format!("Failed to construct response: {}", error)))
      }
      None => Err(error::StaticAssetNotFound { uri_path: path }.build()),
    }
  }
}
