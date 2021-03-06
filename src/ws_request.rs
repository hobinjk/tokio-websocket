use std::io;

use bytes::{BytesMut, BigEndian, ByteOrder};

use ws_frame::{Frame, Header, u8_to_opcode};

#[cfg(test)]
mod tests {
    extern crate tokio_core;

    use ws_frame::Opcode;
    use super::*;


    #[test]
    fn fin_bin_unmasked_empty() {
        let data = vec![
            0x80u8 + 0x02u8, // fin bin
            0x00u8 + 0x00u8, // unmasked empty
        ];
        let mut buf = BytesMut::from(data);
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
        let mut buf = BytesMut::from(data);
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
        let mut buf = BytesMut::from(data);
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
        let mut buf = BytesMut::from(data);
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

    #[test]
    fn afl_crash_0() {
        let data = vec![0x12, 0xff, 0xff, 0xff, 0x7f, 0x01, 0x06, 0xff, 0x7f, 0x00];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_1() {
        let data = vec![0x81, 0xb1];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_2() {
        let data = vec![0x40, 0x91];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_3() {
        let data = vec![0x2a, 0xec, 0x2a, 0x2a, 0xa9];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_4() {
        let data = vec![0x80, 0xff, 0xf7];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_5() {
        let data = vec![0x59, 0xe3];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_6() {
        let data = vec![0x98, 0x98, 0x98, 0x98, 0xbd];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_7() {
        let data = vec![0x8a, 0x7e, 0x62];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
    }

    #[test]
    fn afl_crash_8() {
        let data = vec![0xf1, 0xfe, 0xd5, 0xd5, 0xfe, 0x81];
        let mut buf = BytesMut::from(data);
        let _ = decode(&mut buf);
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

fn parse_header(buf: &mut BytesMut) -> io::Result<ParseResult<Header>> {
    if buf.len() < 2 {
        return Ok(ParseResult::Partial);
    }
    let is_final = buf[0] & 0x80 > 0;
    let opcode = match u8_to_opcode(buf[0] & 0x0f) {
        Some(op) => op,
        None => return Err(io::Error::new(io::ErrorKind::Other, "invalid opcode")),
    };
    let is_masked = buf[1] & 0x80 > 0;
    let (payload_len, buf_offset) = match buf[1] & 0x7f {
        126 => {
            if buf.len() < 4 {
                return Err(io::Error::new(io::ErrorKind::Other, "not enough bytes"));
            }
            let len = BigEndian::read_u16(&buf[2..]) as usize;
            (len, 4)
        }
        127 => {
            if buf.len() < 6 {
                return Err(io::Error::new(io::ErrorKind::Other, "not enough bytes"));
            }
            let len = BigEndian::read_u64(&buf[2..]) as usize;
            (len, 10)
        }
        x => (x as usize, 2),
    };

    let (masking_key, buf_offset) = if is_masked {
        if buf.len() < buf_offset + 4 {
            return Err(io::Error::new(io::ErrorKind::Other, "not enough bytes"));
        }
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

pub fn decode(buf: &mut BytesMut) -> io::Result<Option<Request>> {
    // This is after the successful upgrade
    // Parse header
    let (header, offset) = match try!(parse_header(buf)) {
        ParseResult::Complete(h, offset) => (h, offset),
        ParseResult::Partial => return Ok(None),
    };
    if header.payload_len + offset > buf.len() {
        return Ok(None);
    }
    // Discard header data
    buf.split_to(offset);
    let payload = buf.split_to(header.payload_len).to_vec();

    Ok(Some(Request::Frame(Frame {
        header: header,
        payload: payload,
    })))
}
