use std::fmt::Display;

use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarInt {
    pub value: i32,
}

impl Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl From<i32> for VarInt {
    fn from(value: i32) -> Self {
        Self { value }
    }
}

impl From<VarInt> for i32 {
    fn from(varint: VarInt) -> i32 {
        varint.value
    }
}

impl VarInt {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn into_inner(self) -> i32 {
        self.value
    }

    pub async fn read(reader: &mut (impl AsyncRead + std::marker::Unpin)) -> Result<Self> {
        let mut value = 0;
        let mut position = 0;

        loop {
            let byte = reader.read_u8().await? as i32;
            value |= ((byte & 0x7F) as i32) << position;
            if (byte & 0x80) == 0 {
                break;
            }
            position += 7;
            if position >= 32 {
                return Err(anyhow::anyhow!("VarInt is too big"));
            }
        }

        Ok(Self::new(value))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut value = self.value;
        let mut bytes = Vec::new();

        loop {
            if (value & !0x7F) == 0 {
                bytes.push(value as u8);
                break;
            } else {
                bytes.push((value & 0x7F | 0x80) as u8);
                value >>= 7;
            }
        }

        bytes
    }

    pub async fn write(&self, writer: &mut (impl AsyncWrite + std::marker::Unpin)) -> Result<()> {
        let mut value = self.value;

        loop {
            if (value & !0x7F) == 0 {
                writer.write_u8(value as u8).await?;
                break;
            } else {
                writer.write_u8((value & 0x7F | 0x80) as u8).await?;
                value >>= 7;
            }
        }

        Ok(())
    }

    pub fn length(&self) -> usize {
        let mut value = self.value;
        let mut length = 0;

        loop {
            length += 1;
            if (value & !0x7F) == 0 {
                break;
            } else {
                value >>= 7;
            }
        }

        length
    }
}