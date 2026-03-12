//! Mysticeti protocol handler: accept connections, parse messages, dispatch to core.

use std::collections::HashMap;
use std::sync::Arc;

use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};
use parking_lot::RwLock;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tracing::warn;

use mysticeti_core::NetworkMessage;

use crate::codec::{decode, encode};
use crate::connection::PeerIdentity;

/// ALPN for Mysticeti protocol.
pub const MYSTICETI_ALPN: &[u8] = b"mysticeti/0";

/// Protocol handler: identifies peer, runs message loop on one bi stream per connection.
#[derive(Clone)]
pub struct MysticetiProtocol {
    peer_identity: PeerIdentity,
    /// Incoming messages (msg, authority_index) for the core.
    inbox_tx: mpsc::UnboundedSender<(NetworkMessage, mysticeti_core::AuthorityIndex)>,
    /// Outgoing: authority -> sender for that connection (so sync can send responses).
    connections: Arc<RwLock<HashMap<mysticeti_core::AuthorityIndex, mpsc::UnboundedSender<NetworkMessage>>>>,
}

impl std::fmt::Debug for MysticetiProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MysticetiProtocol").finish_non_exhaustive()
    }
}

impl MysticetiProtocol {
    pub fn new(
        peer_identity: PeerIdentity,
        inbox_tx: mpsc::UnboundedSender<(NetworkMessage, mysticeti_core::AuthorityIndex)>,
    ) -> Self {
        Self {
            peer_identity,
            inbox_tx,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Send a message to a peer (if we have an active connection to that authority).
    pub fn send(&self, authority: mysticeti_core::AuthorityIndex, msg: NetworkMessage) {
        if let Some(tx) = self.connections.read().get(&authority) {
            let _ = tx.send(msg);
        }
    }
}

impl ProtocolHandler for MysticetiProtocol {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl std::future::Future<Output = Result<(), AcceptError>> + Send {
        let peer_identity = self.peer_identity.clone();
        let inbox_tx = self.inbox_tx.clone();
        let connections = self.connections.clone();

        async move {
            let remote_id = connection.remote_id();
            let authority = match peer_identity.authority_index(&remote_id) {
                Some(a) => a,
                None => {
                    warn!(?remote_id, "rejecting connection from unknown peer");
                    connection.close(1u32.into(), b"unknown peer");
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "unknown peer",
                    )
                    .into());
                }
            };
            let (mut send_stream, mut recv_stream) = match connection.accept_bi().await {
                Ok(pair) => pair,
                Err(e) => {
                    warn!(?e, "accept_bi failed");
                    return Err(AcceptError::from(e));
                }
            };
            let (out_tx, mut out_rx) = mpsc::unbounded_channel::<NetworkMessage>();
            connections.write().insert(authority, out_tx);

            let run = async move {
                let mut buf = Vec::new();
                loop {
                    tokio::select! {
                        biased;
                        Some(msg) = out_rx.recv() => {
                            if let Ok(bytes) = encode(&msg) {
                                if send_stream.write_all(&bytes).await.is_err() {
                                    break;
                                }
                            }
                        }
                        n = recv_stream.read_buf(&mut buf) => {
                            if n.unwrap_or(0) == 0 {
                                break;
                            }
                            while let Ok(Some((decoded, consumed))) = decode(&buf) {
                                let _ = inbox_tx.send((decoded, authority));
                                buf.drain(..consumed);
                            }
                        }
                    }
                }
                connections.write().remove(&authority);
            };
            run.await;
            Ok(())
        }
    }
}
