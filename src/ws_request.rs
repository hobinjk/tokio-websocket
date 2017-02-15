use std::io;

use byteorder::{BigEndian, ByteOrder};

use tokio_core::io::EasyBuf;

use ws_frame::{Frame, Header, u8_to_opcode};

#[cfg(test)]
mod tests {
    extern crate tokio_core;

    use tokio_core::io::EasyBuf;
    use ws_frame::Opcode;
    use super::*;


    #[test]
    fn fin_bin_unmasked_empty() {
        let data = vec![
            0x80u8 + 0x02u8, // fin bin
            0x00u8 + 0x00u8, // unmasked empty
        ];
        let mut buf = EasyBuf::from(data);
        let req = match decode(&mut buf) {
            Ok(Some(Request::Frame(req))) => req,
            _ => panic!("decode failed"),
        };

        assert!(req.header.is_final);
        assert_eq!(req.header.opcode, Opcode::Binary);
        assert!(!req.header.is_masked);
        assert_eq!(req.header.payload_len, 0);
    }

    #[test]
    fn fin_bin_unmasked_small_payload() {
        let data = vec![0x80u8 + 0x02u8, // fin bin
                        0x00u8 + 0x05u8, // unmasked 5 long
                        1u8,
                        2u8,
                        3u8,
                        4u8,
                        5u8];
        let mut buf = EasyBuf::from(data);
        let req = match decode(&mut buf) {
            Ok(Some(Request::Frame(req))) => req,
            _ => panic!("decode failed"),
        };
        assert!(req.header.is_final);
        assert_eq!(req.header.opcode, Opcode::Binary);
        assert!(!req.header.is_masked);
        assert_eq!(req.header.payload_len, 5);
        assert_eq!(req.payload, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn con_con_masked_medium_payload() {
        let mut data = vec![0x00u8 + 0x00u8, // continuation continuation
                            0x80u8 + 0x7eu8, // masked 16 bit length
                            0x01, // payload length = 0x01 00
                            0x00,
                            0x11, // maskingKey = 0x11121314
                            0x12,
                            0x13,
                            0x14];
        let payload = [5u8; 256]; // length 0x100
        data.extend(payload.iter());
        let mut buf = EasyBuf::from(data);
        let req = match decode(&mut buf) {
            Ok(Some(Request::Frame(req))) => req,
            e => panic!("decode failed: {:?}", e),
        };
        assert!(!req.header.is_final);
        assert_eq!(req.header.opcode, Opcode::Continuation);
        assert!(req.header.is_masked);
        assert_eq!(req.header.payload_len, 256);
        assert_eq!(req.payload.len(), 256);
        for i in 0..256 {
            assert_eq!(payload[i], req.payload[i]);
        }
    }

    #[test]
    fn con_con_masked_large_payload() {
        let mut data = vec![0x00u8 + 0x00u8, // continuation continuation
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
        let payload = [5u8; 65536]; // length 0x10000
        data.extend(payload.iter());
        let mut buf = EasyBuf::from(data);
        let req = match decode(&mut buf) {
            Ok(Some(Request::Frame(req))) => req,
            e => panic!("decode failed: {:?}", e),
        };
        assert!(!req.header.is_final);
        assert_eq!(req.header.opcode, Opcode::Continuation);
        assert!(req.header.is_masked);
        assert_eq!(req.header.payload_len, 65536);
        assert_eq!(req.payload.len(), 65536);
        for i in 0..65536 {
            assert_eq!(payload[i], req.payload[i]);
        }
    }
}

#[derive(Debug)]
pub enum Request {
    Open(),
    Frame(Frame),
}

enum ParseResult<T> {
    Complete(T, usize),
    Partial,
}

fn parse_header(easy_buf: &mut EasyBuf) -> io::Result<ParseResult<Header>> {
    if easy_buf.len() < 2 {
        return Ok(ParseResult::Partial);
    }
    let buf = easy_buf.as_slice();
    let is_final = buf[0] & 0x80 > 0;
    let opcode = match u8_to_opcode(buf[0] & 0x0f) {
        Some(op) => op,
        None => return Err(io::Error::new(io::ErrorKind::Other, "invalid opcode")),
    };
    let is_masked = buf[1] & 0x80 > 0;
    let (payload_len, buf_offset) = match buf[1] & 0x7f {
        126 => {
            let len = BigEndian::read_u16(&buf[2..]) as usize;
            (len, 4)
        }
        127 => {
            let len = BigEndian::read_u64(&buf[2..]) as usize;
            (len, 10)
        }
        x => (x as usize, 2),
    };

    let (masking_key, buf_offset) = if is_masked {
        (BigEndian::read_u32(&buf[buf_offset..]), buf_offset + 4)
    } else {
        (0, buf_offset)
    };

    Ok(ParseResult::Complete(Header {
                                 is_final: is_final,
                                 opcode: opcode,
                                 is_masked: is_masked,
                                 payload_len: payload_len,
                                 masking_key: masking_key,
                             },
                             buf_offset))
}

pub fn decode(buf: &mut EasyBuf) -> io::Result<Option<Request>> {
    // This is after the successful upgrade
    // Parse header
    let (header, offset) = match try!(parse_header(buf)) {
        ParseResult::Complete(h, offset) => (h, offset),
        ParseResult::Partial => return Ok(None),
    };
    println!("header: {:?}", header);
    if header.payload_len + offset > buf.len() {
        return Ok(None);
    }
    // Discard header data
    buf.drain_to(offset);
    let payload = buf.drain_to(header.payload_len).as_slice().to_vec();

    Ok(Some(Request::Frame(Frame {
        header: header,
        payload: payload,
    })))
}
