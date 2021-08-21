use hyper::{header, Body, Response};
use maud::{html, Markup, DOCTYPE};

pub(crate) fn wrap_body(body: Markup) -> Response<Body> {
  let html = html! {
    (DOCTYPE)
    html lang="en" {
      head {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1";
        title {
          "agora"
        }
        link rel="stylesheet" href="/static/index.css";
      }
      body {
        (body)
      }
    }
  };
  Response::builder()
    .header(header::CONTENT_TYPE, "text/html")
    .body(Body::from(html.into_string()))
    .expect("builder arguments are valid")
}
