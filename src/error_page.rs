use crate::common::*;

use maud::html;

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
