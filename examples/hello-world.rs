extern crate futures;
extern crate tokio_proto;
extern crate tokio_service;
extern crate websocket;

use std::io;

use futures::future;
use websocket::{Request, Response, WebSocket, new_text_frame};
use tokio_proto::TcpServer;
use tokio_service::Service;

struct HelloWorld;

impl Service for HelloWorld {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Future = future::Ok<Response, io::Error>;
    fn call(&self, request: Request) -> Self::Future {
        println!("{:?}", request);
        match request {
            Request::Open() => {
                // This gets dropped, should signal that
                let res = new_text_frame("Hello world!", None);
                future::ok(res)
            },
            Request::Frame(_) => {
                let res = new_text_frame("Hello world!", None);
                future::ok(res)
            },
        }

    }
}

fn main() {
    let addr = "0.0.0.0:8084".parse().unwrap();
    TcpServer::new(WebSocket, addr)
        .serve(|| Ok(HelloWorld));
}
