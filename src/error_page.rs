use crate::{
  error::{Error, Result},
  html,
  stderr::Stderr,
};
use hyper::{Body, Response};
use maud::html;
use std::{convert::Infallible, io::Write};

pub(crate) fn map_error(
  mut stderr: Stderr,
  result: Result<Response<Body>, Error>,
) -> Result<Response<Body>, Infallible> {
  Ok(result.unwrap_or_else(|error| {
    error.print_backtrace(&mut stderr);
    writeln!(stderr, "{}", error).ok();
    let mut response = html::wrap_body(html! {
      h1 {
        (error.status())
      }
    });
    *response.status_mut() = error.status();
    response
  }))
}
