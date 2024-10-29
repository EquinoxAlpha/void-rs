use crate::nbt::NamedTag;

use super::varint::VarInt;


pub struct PacketBuilder {
    pub id: i32,
    pub buffer: Vec<u8>,
}

impl PacketBuilder {
    pub fn new(id: i32) -> Self {
        PacketBuilder {
            id,
            buffer: Vec::new(),
        }
    }

    pub fn with_var_int(mut self, mut value: i32) -> Self {
        loop {
            let mut byte = (value & 0b01111111) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0b10000000;
            }
            self.buffer.push(byte);
            if value == 0 {
                break;
            }
        }
        self
    }


    pub fn with_uuid(self, value: u128) -> Self {
        self.with_raw_bytes(&value.to_be_bytes())
    }

    pub fn with_string(self, value: &str) -> Self {
        // let mut pkt = self.with_var_int(value.len() as i32);
        // pkt.buffer.extend_from_slice(value.as_bytes());
        // pkt
        self.with_var_int(value.len() as i32)
            .with_raw_bytes(value.as_bytes())
    }

    pub fn with_i64(mut self, value: i64) -> Self {
        self.buffer.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn with_i32(mut self, value: i32) -> Self {
        self.buffer.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn with_i16(mut self, value: i16) -> Self {
        self.buffer.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn with_u8(mut self, value: u8) -> Self {
        self.buffer.push(value);
        self
    }

    pub fn with_float(mut self, value: f32) -> Self {
        self.buffer.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn with_double(mut self, value: f64) -> Self {
        self.buffer.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn with_nbt(mut self, value: &NamedTag) -> Self {
        self.buffer.extend_from_slice(&value.to_bytes());
        self
    }

    pub fn with_bool(mut self, value: bool) -> Self {
        self.buffer.push(if value { 1 } else { 0 });
        self
    }

    pub fn with_raw_bytes(mut self, value: &[u8]) -> Self {
        self.buffer.extend_from_slice(value);
        self
    }

    pub fn with_position(mut self, x: i64, y: i64, z: i64) -> Self {
        let value = (x.to_be() & 0x3FFFFFF_i64.to_be()) | (z.to_be() & 0x3FFFFFF_i64.to_be()) << 26 | (y.to_be() & 0xFFF_i64.to_be()) << 52;
        self.buffer.extend_from_slice(&value.to_ne_bytes());
        self
    }

    pub fn build(self) -> Vec<u8> {
        let mut buf = Vec::new();
        let id = VarInt::from(self.id);
        let length = VarInt::from(self.buffer.len() as i32 + id.length() as i32);
        buf.extend_from_slice(&length.to_bytes());
        buf.extend_from_slice(&id.to_bytes());
        buf.extend_from_slice(&self.buffer);
        buf
    }
}

impl Into<Vec<u8>> for PacketBuilder {
    fn into(self) -> Vec<u8> {
        self.build()
    }
}