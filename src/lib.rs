extern crate base64;
extern crate bytes;
extern crate ring;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_minihttp;
extern crate tokio_proto;
extern crate tokio_service;

use std::io;
use bytes::BytesMut;
use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_proto::pipeline::ServerProto;
use tokio_minihttp::HttpCodec;

mod ws_frame;
mod ws_request;
mod ws_response;

pub use ws_request::{Request, decode};
pub use ws_response::{Response, encode};
pub use ws_frame::{new_text_frame, Opcode, Frame};

pub struct WebSocket;

#[derive(Debug)]
enum WebSocketState {
    Http(),
    Upgrade(String),
    Connected(),
}

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for WebSocket {
    type Request = Request;
    type Response = Response;
    type Transport = Framed<T, WebSocketCodec>;
    type BindTransport = io::Result<Framed<T, WebSocketCodec>>;

    fn bind_transport(&self, io: T) -> io::Result<Framed<T, WebSocketCodec>> {
        Ok(io.framed(WebSocketCodec::new()))
    }
}

pub struct WebSocketCodec {
    state: WebSocketState,
    http_codec: HttpCodec,
}

impl WebSocketCodec {
    pub fn new() -> WebSocketCodec {
        WebSocketCodec {
            state: WebSocketState::Http(),
            http_codec: HttpCodec,
        }
    }
}

impl Decoder for WebSocketCodec {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Request>> {
        match self.state {
            WebSocketState::Http() => {
                let req = self.http_codec.decode(buf);
                match req {
                    Ok(Some(req)) => {
                        for (header, value) in req.headers() {
                            if header == "Sec-WebSocket-Key" {
                                let value_str = String::from_utf8(value.to_vec()).unwrap();
                                self.state = WebSocketState::Upgrade(value_str);
                                let req = Request::Open();
                                return Ok(Some(req));
                            }
                        }
                        Ok(None)
                    }
                    _ => Ok(None),
                }
            }
            _ => ws_request::decode(buf),
        }
    }
}

impl Encoder for WebSocketCodec {
    type Item = Response;
    type Error = io::Error;

    fn encode(&mut self, msg: Response, buf: &mut BytesMut) -> io::Result<()> {
        self.state = match self.state {
            WebSocketState::Http() => {
                return Err(io::Error::new(io::ErrorKind::Other, "pls no"));
            }
            WebSocketState::Upgrade(ref key) => {
                try!(self.http_codec.encode(ws_response::make_accept(&key), buf));
                WebSocketState::Connected()
            }
            WebSocketState::Connected() => {
                ws_response::encode(msg, buf);
                WebSocketState::Connected()
            }
        };
        Ok(())
    }
}
