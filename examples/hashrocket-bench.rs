extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate websocket;
extern crate serde_json;

use serde_json::Value;

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::{Error, ErrorKind};

use tokio_core::net::TcpListener;
use tokio_core::reactor::Core;
use tokio_io::AsyncRead;

use futures::{Future, Stream, Sink};
use futures::sync::mpsc;

use websocket::{Request, WebSocketCodec, new_text_frame, Opcode, Frame};

const NULL_PAYLOAD: &'static Value = &Value::Null;

enum Message {
    Echo(Frame),
    Broadcast(Frame, Frame),
    None(),
}

fn process_frame(frame: Frame) -> Message {
    if frame.header.opcode == Opcode::Close {
        return Message::Echo(frame);
    }
    if frame.header.opcode != Opcode::Text {
        return Message::None();
    }
    // TODO send back pongs

    let payload = frame.payload_string().unwrap();
    if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(&payload) {
        if let Some(&Value::String(ref s)) = obj.get("type") {
            if s == "echo" {
                return Message::Echo(frame);
            }
            if s == "broadcast" {
                let msg = format!(r#"{{"type":"broadcastResult","payload":{}}}"#, obj.get("payload").unwrap_or(NULL_PAYLOAD));
                return Message::Broadcast(frame, new_text_frame(&msg, None));
            }
        }
    }
    Message::None()
}

fn main() {
    // Set up using skeleton of chat example, use encode and decode directly
    let addr = "0.0.0.0:8084".parse().unwrap();

    let mut core = Core::new().unwrap();

    let handle = core.handle();
    let socket = TcpListener::bind(&addr, &handle).unwrap();

    let connections = Rc::new(RefCell::new(HashMap::new()));

    let srv = socket.incoming().for_each(move |(conn, addr)| {
        let (sink, stream) = conn.framed(WebSocketCodec::new()).split();
        let (tx, rx) = mpsc::unbounded();

        connections.borrow_mut().insert(addr, tx);

        let connections_inner = connections.clone();
        let reader = stream.for_each(move |req| {
            let mut conns = connections_inner.borrow_mut();
            match req {
                Request::Frame(frame) => {
                    match process_frame(frame) {
                        Message::None() => {},
                        Message::Echo(frame) => {
                            if frame.header.opcode == Opcode::Close {
                                conns.remove(&addr);
                                return Err(Error::new(ErrorKind::Other, "close requested"))
                            }
                            let tx = conns.get_mut(&addr).unwrap();
                            let masked_frame = new_text_frame(&frame.payload_string().unwrap(), None);
                            mpsc::UnboundedSender::send(&mut std::borrow::BorrowMut::borrow_mut(tx), masked_frame).unwrap();
                        },
                        Message::Broadcast(broadcast_frame, echo_frame) => {
                            let masked_frame = new_text_frame(&broadcast_frame.payload_string().unwrap(), None);
                            for (&t_addr, tx) in conns.iter_mut() {
                                mpsc::UnboundedSender::send(&mut std::borrow::BorrowMut::borrow_mut(tx), masked_frame.clone()).unwrap();
                                if addr == t_addr {
                                    mpsc::UnboundedSender::send(&mut std::borrow::BorrowMut::borrow_mut(tx), echo_frame.clone()).unwrap();
                                }
                            }
                        },
                    }
                },
                Request::Open() => {
                    let tx = conns.get_mut(&addr).unwrap();
                    mpsc::UnboundedSender::send(&mut std::borrow::BorrowMut::borrow_mut(tx), new_text_frame("this message is dropped", None)).unwrap();
                }
            }
            Ok(())
        });
        let connections = connections.clone();
        let writer = rx.map_err(|_| Error::new(ErrorKind::Other, "receiver error")).fold(sink, |sink, msg| {
            sink.send(msg)
        });
        let reader = reader.map_err(|_| Error::new(ErrorKind::Other, "transmitter error"));
        let conn = reader.map(|_| ()).select(writer.map(|_| ()));
        handle.spawn(conn.then(move |_| {
            connections.borrow_mut().remove(&addr);
            Ok(())
        }));
        Ok(())
    });

    core.run(srv).unwrap();
}
