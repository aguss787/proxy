use std::net::{SocketAddr, ToSocketAddrs};

use crate::bucket;
use futures_util::future::try_join;
use hyper::upgrade::Upgraded;
use hyper::{http, Body, Client, Method, Request, Response};
use tokio::net::TcpStream;

type HttpClient = Client<hyper::client::HttpConnector>;

pub async fn handle(
    client: HttpClient,
    limiter: bucket::Bucket,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    let mut limiter = limiter;
    limiter.request().await;

    println!("req: {:?}", req);

    if Method::CONNECT == req.method() {
        // Received an HTTP request like:
        // ```
        // CONNECT www.domain.com:443 HTTP/1.1
        // Host: www.domain.com:443
        // Proxy-Connection: Keep-Alive
        // ```
        //
        // When HTTP method is CONNECT we should return an empty body
        // then we can eventually upgrade the connection and talk a new protocol.
        //
        // Note: only after client received an empty body with STATUS_OK can the
        // connection be upgraded, so we can't return a response inside
        // `on_upgrade` future.
        if let Some(addr) = host_addr(req.uri()) {
            tokio::task::spawn(async move {
                match req.into_body().on_upgrade().await {
                    Ok(upgraded) => {
                        if let Err(e) = tunnel(limiter, upgraded, addr).await {
                            eprintln!("server io error: {}", e);
                        };
                    }
                    Err(e) => eprintln!("upgrade error: {}", e),
                }
            });

            Ok(Response::new(Body::empty()))
        } else {
            eprintln!("CONNECT host is not socket addr: {:?}", req.uri());
            let mut resp = Response::new(Body::from("CONNECT must be to a socket address"));
            *resp.status_mut() = http::StatusCode::BAD_REQUEST;

            Ok(resp)
        }
    } else {
        client.request(req).await
    }
}

fn host_addr(uri: &http::Uri) -> Option<SocketAddr> {
    uri.authority()
        .and_then(|auth| auth.as_str().to_socket_addrs().ok())
        .and_then(|mut socket_addrs| socket_addrs.next())
}

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
async fn tunnel(
    limiter: bucket::Bucket,
    upgraded: Upgraded,
    addr: SocketAddr,
) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = TcpStream::connect(addr).await?;

    // Proxying data
    let amounts = {
        let (mut server_rd, mut server_wr) = server.split();
        let (mut client_rd, mut client_wr) = tokio::io::split(upgraded);

        let client_to_server = tokio::io::copy(&mut client_rd, &mut server_wr);
        let server_to_client = tokio::io::copy(&mut server_rd, &mut client_wr);

        try_join(client_to_server, server_to_client).await
    };

    // Print message when done
    match amounts {
        Ok((from_client, from_server)) => {
            println!(
                "client wrote {} bytes and received {} bytes [{:?}]",
                from_client, from_server, limiter.id
            );
        }
        Err(e) => {
            println!("tunnel error: {}", e);
        }
    };
    Ok(())
}
