extern crate base64;
extern crate byteorder;
extern crate ring;
extern crate tokio_core;
extern crate tokio_minihttp;
extern crate tokio_proto;
extern crate tokio_service;

use std::io;
use tokio_core::io::{Codec, EasyBuf, Framed, Io};
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

impl<T: Io + 'static> ServerProto<T> for WebSocket {
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

impl Codec for WebSocketCodec {
    type In = Request;
    type Out = Response;

    fn decode(&mut self, buf: &mut EasyBuf) -> io::Result<Option<Request>> {
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

    fn encode(&mut self, msg: Response, buf: &mut Vec<u8>) -> io::Result<()> {
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
