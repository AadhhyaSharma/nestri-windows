/// p2p/p2p_protocol_stream.rs — Nestri stream protocol handler
/// Identical to original — fully cross-platform.

use crate::p2p::p2p::NestriConnection;
use crate::p2p::p2p_safestream::SafeStream;
use anyhow::Result;
use dashmap::DashMap;
use libp2p::StreamProtocol;
use prost::Message;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type CallbackInner = dyn Fn(crate::proto::proto::ProtoMessage) -> Result<()> + Send + Sync + 'static;

pub struct Callback(Arc<CallbackInner>);
impl Callback {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(crate::proto::proto::ProtoMessage) -> Result<()> + Send + Sync + 'static,
    {
        Callback(Arc::new(f))
    }
    pub fn call(&self, data: crate::proto::proto::ProtoMessage) -> Result<()> { self.0(data) }
}
impl Clone for Callback {
    fn clone(&self) -> Self { Callback(Arc::clone(&self.0)) }
}

pub struct NestriStreamProtocol {
    tx:           Option<mpsc::Sender<Vec<u8>>>,
    safe_stream:  Arc<SafeStream>,
    callbacks:    Arc<DashMap<String, Callback>>,
    read_handle:  Option<tokio::task::JoinHandle<()>>,
    write_handle: Option<tokio::task::JoinHandle<()>>,
}

impl NestriStreamProtocol {
    const NESTRI_PROTOCOL_STREAM_PUSH: StreamProtocol =
        StreamProtocol::new("/nestri-relay/stream-push/1.0.0");

    pub async fn new(nestri_connection: NestriConnection) -> Result<Self> {
        let mut nestri_connection = nestri_connection.clone();
        let push_stream = nestri_connection
            .control
            .open_stream(nestri_connection.peer_id, Self::NESTRI_PROTOCOL_STREAM_PUSH)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open push stream: {}", e))?;

        let mut sp = NestriStreamProtocol {
            tx:           None,
            safe_stream:  Arc::new(SafeStream::new(push_stream)),
            callbacks:    Arc::new(DashMap::new()),
            read_handle:  None,
            write_handle: None,
        };
        sp.restart()?;
        Ok(sp)
    }

    pub fn restart(&mut self) -> Result<()> {
        if self.tx.is_some() && self.read_handle.is_some() && self.write_handle.is_some() {
            tracing::warn!("NestriStreamProtocol already running, restart skipped");
            return Ok(());
        }
        let (tx, rx) = mpsc::channel(1000);
        self.tx           = Some(tx);
        self.read_handle  = Some(self.spawn_read_loop());
        self.write_handle = Some(self.spawn_write_loop(rx));
        Ok(())
    }

    fn spawn_read_loop(&self) -> tokio::task::JoinHandle<()> {
        let safe_stream = self.safe_stream.clone();
        let callbacks   = self.callbacks.clone();
        tokio::spawn(async move {
            loop {
                let data = match safe_stream.receive_raw().await {
                    Ok(d)  => d,
                    Err(e) => { tracing::error!("Error receiving data: {}", e); break; }
                };
                match crate::proto::proto::ProtoMessage::decode(data.as_slice()) {
                    Ok(message) => {
                        if let Some(base) = &message.message_base {
                            let key = base.payload_type.clone();
                            if let Some(cb) = callbacks.get(&key) {
                                if let Err(e) = cb.call(message) {
                                    tracing::error!("Callback for '{}' errored: {:?}", key, e);
                                }
                            } else {
                                tracing::warn!("No callback for payload type: {}", key);
                            }
                        } else {
                            tracing::error!("No base message in decoded protobuf");
                        }
                    }
                    Err(e) => tracing::error!("Failed to decode message: {}", e),
                }
            }
        })
    }

    fn spawn_write_loop(&self, mut rx: mpsc::Receiver<Vec<u8>>) -> tokio::task::JoinHandle<()> {
        let safe_stream = self.safe_stream.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(data) => {
                        if let Err(e) = safe_stream.send_raw(&data).await {
                            tracing::error!("Error sending data: {:?}", e);
                        }
                    }
                    None => { tracing::info!("Write channel closed"); break; }
                }
            }
        })
    }

    pub fn send_message(&self, message: &crate::proto::proto::ProtoMessage) -> Result<()> {
        let mut buf = Vec::new();
        message.encode(&mut buf)?;
        let Some(tx) = &self.tx else {
            return Err(anyhow::Error::msg("NestriStreamProtocol not initialized"));
        };
        tx.try_send(buf)?;
        Ok(())
    }

    pub fn register_callback<F>(&self, response_type: &str, callback: F)
    where
        F: Fn(crate::proto::proto::ProtoMessage) -> Result<()> + Send + Sync + 'static,
    {
        self.callbacks.insert(response_type.to_string(), Callback::new(callback));
    }
}
