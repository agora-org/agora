use crate::common::*;
use maud::html;

pub(crate) fn map_error(
  mut stderr: Stderr,
  result: Result<Response<Body>, Error>,
) -> Response<Body> {
  result.unwrap_or_else(|error| {
    error.print_backtrace(&mut stderr);
    writeln!(stderr, "{}", error).ok();
    let title = error.status().to_string();
    let mut response = html::wrap_body(
      title.as_str(),
      html! {
        h1 {
          (error.status())
        }
      },
    );
    *response.status_mut() = error.status();
    response
  })
}
