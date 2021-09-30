use crate::common::*;
use maud::{html, DOCTYPE};

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
        script type="module" src="/static/index.js" {}
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
