use std::string;

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

impl Frame {
    pub fn payload_string(&self) -> Result<String, string::FromUtf8Error> {
        if !self.header.is_masked {
            return String::from_utf8(self.payload.clone())
        }
        String::from_utf8(mask_bytes(self.header.masking_key, &self.payload))
    }
}

fn mask_bytes(masking_key: u32, bytes: &[u8]) -> Vec<u8> {
    let mut i = 0;
    let masking_keys = [
        ((masking_key & 0xff000000) >> 24) as u8,
        ((masking_key & 0x00ff0000) >> 16) as u8,
        ((masking_key & 0x0000ff00) >> 8) as u8,
        (masking_key & 0x000000ff) as u8,
    ];
    let mut masked = Vec::new();
    masked.reserve(bytes.len());
    for b in bytes.iter() {
        masked.push(b ^ masking_keys[i]);
        i = (i + 1) % 4;
    }
    masked
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

pub fn new_text_frame(text: &str, masking_key: Option<u32>) -> Frame {
    let masked_text = match masking_key {
        Some(masking_key) => mask_bytes(masking_key, text.as_bytes()),
        None => text.as_bytes().to_vec(),
    };

    Frame {
        header: Header {
            is_final: true,
            opcode: Opcode::Text,
            is_masked: masking_key.is_some(),
            payload_len: text.len(),
            masking_key: masking_key.unwrap_or(0),
        },
        payload: masked_text,
    }
}
