#![feature(test)]

extern crate bytes;
extern crate test;
extern crate websocket;

#[bench]
fn bench_broadcast_encode(b: &mut test::Bencher) {
    b.iter(|| {
        let frame = websocket::new_text_frame("{\"type\":\"broadcast\",\"payload\":{\"foo\": \"bar\"}}", Some(0x11223344));
        let mut buf = bytes::BytesMut::with_capacity(0);
        websocket::encode(frame, &mut buf)
    });
}

