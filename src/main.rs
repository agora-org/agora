use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use std::{convert::Infallible, future::Future};
use std::{fmt::Display, net::SocketAddr};

#[tokio::main]
async fn main() {
    run(setup(Some(8080)).1).await
}

fn setup(port: Option<u16>) -> (u16, impl Future<Output = Result<(), impl Display>>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0)));
    let make_service = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });
    let server = Server::bind(&addr).serve(make_service);
    let port = server.local_addr().port();
    eprintln!("listening on port {}", port);
    (port, server)
}

async fn run(server: impl Future<Output = Result<(), impl Display>>) {
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

async fn handle(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new(Body::from("<h1>hello world</h1>")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test<Function, F>(test: Function)
    where
        Function: FnOnce(u16) -> F,
        F: Future<Output = ()>,
    {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (port, server) = setup(None);
                let join_handle = tokio::spawn(run(server));
                test(port).await;
                join_handle.abort();
            });
    }

    #[test]
    fn index_route_status_code_is_200() {
        test(|port| async move {
            assert_eq!(
                reqwest::get(format!("http://localhost:{}", port))
                    .await
                    .unwrap()
                    .status(),
                200
            )
        });
    }

    #[test]
    fn index_route_contains_hello_world() {
        test(|port| async move {
            let haystack = reqwest::get(format!("http://localhost:{}", port))
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let needle = "<h1>hello world</h1>";
            assert!(
                haystack.contains(needle),
                "\n{} does not contain {}\n",
                haystack,
                needle
            );
        });
    }
}
