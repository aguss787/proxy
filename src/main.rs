use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};

type HttpClient = Client<hyper::client::HttpConnector>;

mod bucket;
mod proxy;

async fn handle(
    client: HttpClient,
    limiter: bucket::Bucket,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    proxy::handle(client, limiter, req).await
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3128));
    let client = HttpClient::new();
    let default_bucket = bucket::new();

    let make_service = make_service_fn(move |_| {
        let client = client.clone();
        let default_bucket = default_bucket.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                handle(client.clone(), default_bucket.clone(), req)
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }

    println!("Done")
}
