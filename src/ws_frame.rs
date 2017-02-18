
#[derive(Debug, Clone)]
pub struct Frame {
    pub header: Header,
    pub payload: Vec<u8>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Opcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub is_final: bool,
    pub opcode: Opcode,
    pub is_masked: bool,
    pub payload_len: usize,
    pub masking_key: u32,
}

pub fn opcode_to_u8(opcode: Opcode) -> u8 {
    match opcode {
        Opcode::Continuation => 0,
        Opcode::Text => 1,
        Opcode::Binary => 2,
        Opcode::Close => 8,
        Opcode::Ping => 9,
        Opcode::Pong => 10,
    }
}

pub fn u8_to_opcode(bits: u8) -> Option<Opcode> {
    match bits {
        0 => Some(Opcode::Continuation),
        1 => Some(Opcode::Text),
        2 => Some(Opcode::Binary),
        8 => Some(Opcode::Close),
        9 => Some(Opcode::Ping),
        10 => Some(Opcode::Pong),
        _ => None,
    }
}

pub fn new_text_frame(text: &str) -> Frame {
    Frame {
        header: Header {
            is_final: true,
            opcode: Opcode::Text,
            is_masked: false,
            payload_len: text.len(),
            masking_key: 0,
        },
        payload: text.as_bytes().to_vec(),
    }
}
