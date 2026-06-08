/// p2p/p2p.rs — libp2p peer-to-peer connection manager
/// Identical to the original — libp2p is fully cross-platform (Windows/Linux/macOS).
/// No changes needed whatsoever.

use anyhow::Result;
use libp2p::futures::StreamExt;
use libp2p::multiaddr::Protocol;
use libp2p::{
    Multiaddr, PeerId, Swarm, identity,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use libp2p_autonat as autonat;
use libp2p_identify as identify;
use libp2p_noise as noise;
use libp2p_ping as ping;
use libp2p_stream as stream;
use libp2p_tcp as tcp;
use libp2p_yamux as yamux;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct NestriConnection {
    pub peer_id: PeerId,
    pub control: stream::Control,
}

#[derive(NetworkBehaviour)]
struct NestriBehaviour {
    identify:  identify::Behaviour,
    ping:      ping::Behaviour,
    stream:    stream::Behaviour,
    autonatv2: autonat::v2::client::Behaviour,
}

impl NestriBehaviour {
    fn new(key: identity::PublicKey) -> Self {
        Self {
            identify: identify::Behaviour::new(identify::Config::new(
                "/ipfs/id/1.0.0".to_string(),
                key,
            )),
            ping:      ping::Behaviour::default(),
            stream:    stream::Behaviour::default(),
            autonatv2: autonat::v2::client::Behaviour::default(),
        }
    }
}

pub struct NestriP2P {
    swarm: Arc<Mutex<Swarm<NestriBehaviour>>>,
}

impl NestriP2P {
    pub async fn new() -> Result<Self> {
        let swarm = Arc::new(Mutex::new(
            libp2p::SwarmBuilder::with_new_identity()
                .with_tokio()
                .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
                .with_quic()
                .with_dns()?
                .with_behaviour(|key| NestriBehaviour::new(key.public()))?
                .build(),
        ));

        let swarm_clone = swarm.clone();
        tokio::spawn(swarm_loop(swarm_clone));

        Ok(NestriP2P { swarm })
    }

    pub async fn connect(&self, conn_url: &str) -> Result<NestriConnection> {
        let conn_addr: Multiaddr = conn_url.parse()?;
        let mut swarm_lock = self.swarm.lock().await;
        swarm_lock.dial(conn_addr.clone())?;

        let Some(Protocol::P2p(peer_id)) = conn_addr.iter().last() else {
            return Err(anyhow::Error::msg("Invalid multiaddr: missing /p2p/<peer_id>"));
        };

        Ok(NestriConnection {
            peer_id,
            control: swarm_lock.behaviour().stream.new_control(),
        })
    }
}

async fn swarm_loop(swarm: Arc<Mutex<Swarm<NestriBehaviour>>>) {
    loop {
        let event = swarm.lock().await.select_next_some().await;
        match event {
            SwarmEvent::Behaviour(NestriBehaviourEvent::Ping(ping::Event { peer, connection, result })) => {
                match result {
                    Ok(latency) => tracing::debug!("Ping {} conn {:?} latency {}us", peer, connection, latency.as_micros()),
                    Err(e)      => tracing::warn!("Ping error {} conn {:?}: {:?}", peer, connection, e),
                }
            }
            SwarmEvent::Behaviour(NestriBehaviourEvent::Autonatv2(
                autonat::v2::client::Event { server, tested_addr, bytes_sent, result }
            )) => {
                match result {
                    Ok(())  => tracing::debug!("AutoNAT v2 verified '{}' via '{}' ({} bytes)", tested_addr, server, bytes_sent),
                    Err(e)  => tracing::warn!("AutoNAT v2 failed '{}' via '{}': {:?}", tested_addr, server, e),
                }
            }
            SwarmEvent::NewListenAddr { address, .. }        => tracing::info!("Listening on: '{}'", address),
            SwarmEvent::ConnectionEstablished { peer_id, .. } => tracing::info!("Connected: {}", peer_id),
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                if let Some(e) = cause { tracing::error!("Connection to {} closed: {}", peer_id, e); }
                else                    { tracing::info!("Connection to {} closed", peer_id); }
            }
            SwarmEvent::IncomingConnection { local_addr, send_back_addr, .. } =>
                tracing::info!("Incoming from {} → {}", send_back_addr, local_addr),
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(pid) = peer_id { tracing::error!("Failed to connect to {}: {}", pid, error); }
                else                       { tracing::error!("Outgoing connection error: {}", error); }
            }
            SwarmEvent::ExternalAddrConfirmed { address } =>
                tracing::info!("External address confirmed: {}", address),
            _ => {}
        }
    }
}
