/// p2p/p2p_safestream.rs — Framed stream with varint length prefix
/// Identical to original — pure async Rust, fully cross-platform.

use anyhow::Result;
use libp2p::futures::io::{ReadHalf, WriteHalf};
use libp2p::futures::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use unsigned_varint::{decode, encode};

pub struct SafeStream {
    stream_read:  Arc<Mutex<ReadHalf<libp2p::Stream>>>,
    stream_write: Arc<Mutex<WriteHalf<libp2p::Stream>>>,
}

impl SafeStream {
    pub fn new(stream: libp2p::Stream) -> Self {
        let (read, write) = stream.split();
        SafeStream {
            stream_read:  Arc::new(Mutex::new(read)),
            stream_write: Arc::new(Mutex::new(write)),
        }
    }

    pub async fn send_raw(&self, data: &[u8]) -> Result<()> {
        self.send_with_length_prefix(data).await
    }

    pub async fn receive_raw(&self) -> Result<Vec<u8>> {
        self.receive_with_length_prefix().await
    }

    async fn send_with_length_prefix(&self, data: &[u8]) -> Result<()> {
        let mut stream_write = self.stream_write.lock().await;
        let mut length_buf = encode::usize_buffer();
        let length_bytes = encode::usize(data.len(), &mut length_buf);
        stream_write.write_all(length_bytes).await?;
        stream_write.write_all(data).await?;
        stream_write.flush().await?;
        Ok(())
    }

    async fn receive_with_length_prefix(&self) -> Result<Vec<u8>> {
        let mut stream_read = self.stream_read.lock().await;
        let mut length_buf = Vec::new();
        let mut temp_byte = [0u8; 1];
        loop {
            stream_read.read_exact(&mut temp_byte).await?;
            length_buf.push(temp_byte[0]);
            if temp_byte[0] & 0x80 == 0 { break; }
            if length_buf.len() > 10 { anyhow::bail!("Invalid varint encoding"); }
        }
        let (length, _) = decode::usize(&length_buf)
            .map_err(|e| anyhow::anyhow!("Failed to decode varint: {}", e))?;
        let mut buffer = vec![0u8; length];
        stream_read.read_exact(&mut buffer).await?;
        Ok(buffer)
    }
}
