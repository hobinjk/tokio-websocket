use base64;
use bytes::{BytesMut, BufMut, BigEndian};
use tokio_minihttp;
use ring::digest;
use ws_frame::{Frame, opcode_to_u8};

#[cfg(test)]
mod tests {
    use ws_frame::{Opcode, Header, new_text_frame};

    use super::*;

    #[test]
    fn test_hash_key_rfc_example() {
        assert_eq!(hash_key("dGhlIHNhbXBsZSBub25jZQ=="),
                   "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn fin_bin_unmasked_empty() {
        let frame = Frame {
            header: Header {
                is_final: true,
                opcode: Opcode::Binary,
                is_masked: false,
                payload_len: 0,
                masking_key: 0,
            },
            payload: Vec::new(),
        };

        let expected_data = vec![
            0x80u8 + 0x02u8, // fin bin
            0x00u8 + 0x00u8, // unmasked empty
        ];
        let mut buf = BytesMut::with_capacity(0);
        encode(frame, &mut buf);

        assert_eq!(buf, expected_data);
    }

    #[test]
    fn fin_bin_unmasked_small_payload() {
        let frame = Frame {
            header: Header {
                is_final: true,
                opcode: Opcode::Binary,
                is_masked: false,
                payload_len: 5,
                masking_key: 0,
            },
            payload: vec![1, 2, 3, 4, 5],
        };

        let expected_data = vec![0x80u8 + 0x02u8, // fin bin
                                 0x00u8 + 0x05u8, // unmasked 5 long
                                 1u8,
                                 2u8,
                                 3u8,
                                 4u8,
                                 5u8];
        let mut buf = BytesMut::with_capacity(0);
        encode(frame, &mut buf);

        assert_eq!(buf, expected_data);
    }

    #[test]
    fn con_con_masked_medium_payload() {
        let payload = vec![5u8; 256]; // length 0x100
        let frame = Frame {
            header: Header {
                is_final: false,
                opcode: Opcode::Continuation,
                is_masked: true,
                payload_len: 256,
                masking_key: 0x11121314,
            },
            payload: payload.clone(),
        };

        let mut expected_data = vec![0x00u8 + 0x00u8, // continuation continuation
                                     0x80u8 + 0x7eu8, // masked 16 bit length
                                     0x01, // payload length = 0x01 00
                                     0x00,
                                     0x11, // maskingKey = 0x11121314
                                     0x12,
                                     0x13,
                                     0x14];
        expected_data.extend(payload.iter());

        let mut buf = BytesMut::with_capacity(0);
        encode(frame, &mut buf);

        assert_eq!(buf, expected_data);
    }

    #[test]
    fn con_con_masked_large_payload() {
        let payload = vec![5u8; 65536]; // length 0x10000
        let frame = Frame {
            header: Header {
                is_final: false,
                opcode: Opcode::Continuation,
                is_masked: true,
                payload_len: 65536,
                masking_key: 0x11121314,
            },
            payload: payload.clone(),
        };


        let mut expected_data = vec![0x00u8 + 0x00u8, // continuation continuation
                                     0x80u8 + 0x7fu8, // masked 64 bit length
                                     0x00, // payload length = 0x00 00 00 00 00 01 00 00
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x00,
                                     0x01,
                                     0x00,
                                     0x00,
                                     0x11, // maskingKey = 0x11121314
                                     0x12,
                                     0x13,
                                     0x14];
        expected_data.extend(payload.iter());

        let mut buf = BytesMut::with_capacity(0);
        encode(frame, &mut buf);

        assert_eq!(buf, expected_data);
    }

    #[test]
    fn tiny_text_frame() {
        let text = "blub";
        let mut expected_data = vec![
            0x81u8,
            0x04u8
        ];
        expected_data.extend(text.as_bytes());
        let ttf = new_text_frame(text, None);
        let mut buf = BytesMut::with_capacity(0);
        encode(ttf, &mut buf);
        assert_eq!(buf, expected_data);
    }

    #[test]
    fn tiny_text_frame_masked() {
        let text = "blub";
        let expected_start = [
            0x81u8,
            0x84u8
        ];
        let ttf = new_text_frame(text, Some(0x11121314));
        assert_eq!(ttf.clone().payload_string().unwrap(), text);
        let mut buf = BytesMut::with_capacity(128);
        encode(ttf, &mut buf);

        assert_eq!(buf[0], expected_start[0]);
        assert_eq!(buf[1], expected_start[1]);
    }
}

pub type Response = Frame;

fn response_len(msg: &Response) -> usize {
    let mut len = 2 + msg.header.payload_len;
    if msg.header.payload_len >= 126 && msg.header.payload_len < 65536 {
        len += 2;
    } else if msg.header.payload_len >= 65536 {
        len += 8;
    }

    if msg.header.is_masked {
        len += 4;
    }
    len
}
pub fn encode(msg: Response, buf: &mut BytesMut) {
    buf.reserve(response_len(&msg));
    buf.put(0u8);
    buf.put(0u8);
    if msg.header.is_final {
        buf[0] |= 0x80;
    }
    let op_u8 = opcode_to_u8(msg.header.opcode);
    buf[0] |= op_u8;
    if msg.header.is_masked {
        buf[1] |= 0x80;
    }
    if msg.header.payload_len < 126 {
        buf[1] |= msg.header.payload_len as u8;
    } else if msg.header.payload_len < 65536 {
        buf[1] |= 0x7e;
        buf.put_u16::<BigEndian>(msg.header.payload_len as u16);
    } else {
        buf[1] |= 0x7f;
        buf.put_u64::<BigEndian>(msg.header.payload_len as u64);
    }
    if msg.header.is_masked {
        buf.put_u32::<BigEndian>(msg.header.masking_key);
    }
    buf.put_slice(msg.payload.as_slice());
}

fn hash_sha1(input: &str) -> digest::Digest {
    let mut ctx = digest::Context::new(&digest::SHA1);
    ctx.update(input.as_bytes());
    ctx.finish()
}

fn hash_key(b64_key: &str) -> String {
    let mut input = b64_key.to_string();
    input.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let sha_input = hash_sha1(&input);
    base64::encode(sha_input.as_ref())
}

pub fn make_accept(b64_key: &str) -> tokio_minihttp::Response {
    let mut res = tokio_minihttp::Response::new();
    // HTTP/1.1 101 Switching Protocols
    // Upgrade: websocket
    // Connection: Upgrade
    // Sec-WebSocket-Accept: key thing
    res.status_code(101, "Switching Protocols");
    res.header("Upgrade", "websocket");
    res.header("Connection", "Upgrade");
    res.header("Sec-WebSocket-Accept", &hash_key(&b64_key));
    res
}
