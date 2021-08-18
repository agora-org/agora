use hyper::{Body, Response};
use maud::{html, Markup, DOCTYPE};

pub(crate) fn wrap_body(body: Markup) -> Response<Body> {
  let html = html! {
    (DOCTYPE)
    html lang="en" {
      head {
        meta charset="utf-8";
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
  Response::new(Body::from(html.into_string()))
}
