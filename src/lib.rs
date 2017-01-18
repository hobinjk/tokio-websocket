extern crate tokio_core;
extern crate byteorder;


mod request;

pub use request::{Request, decode};
