use crate::common::*;

pub(crate) fn redirect(location: String) -> Result<Response<Body>> {
  Response::builder()
    .status(StatusCode::FOUND)
    .header(header::LOCATION, location)
    .body(Body::empty())
    .map_err(|error| Error::internal(format!("Failed to construct redirect response: {}", error)))
}
