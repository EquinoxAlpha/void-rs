use anyhow::Result;
use tokio::io::{AsyncRead, AsyncWrite};
// use tokio_byteorder::{AsyncReadBytesExt, AsyncWriteBytesExt, BigEndian};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use varint::VarInt;

pub mod varint;
pub mod packet;

pub async fn read_generic_packet(reader: &mut (impl AsyncRead + std::marker::Unpin)) -> Result<(i32, Vec<u8>)> {
    let length = VarInt::read(reader).await?.into_inner();
    let packet_id = VarInt::read(reader).await?;
    let length = length - packet_id.length() as i32;
    let mut buffer = vec![0; length as usize];
    reader.read_exact(&mut buffer).await?;
    Ok((packet_id.into_inner(), buffer))
}

pub async fn write_generic_packet(writer: &mut (impl AsyncWrite + std::marker::Unpin), packet_id: i32, buffer: &[u8]) -> Result<()> {
    let length = VarInt::new((VarInt::new(packet_id).length() + buffer.len()) as i32);
    length.write(writer).await?;
    VarInt::new(packet_id).write(writer).await?;
    writer.write_all(buffer).await?;
    Ok(())
}

pub async fn read_string(reader: &mut (impl AsyncRead + std::marker::Unpin)) -> Result<String> {
    let length = VarInt::read(reader).await?.into_inner();
    let mut buffer = vec![0; length as usize];
    reader.read_exact(&mut buffer).await?;
    Ok(String::from_utf8(buffer)?)
}

pub async fn write_string(writer: &mut (impl AsyncWrite + std::marker::Unpin), string: &str) -> Result<()> {
    let length = VarInt::new(string.len() as i32);
    length.write(writer).await?;
    writer.write_all(string.as_bytes()).await?;
    Ok(())
}