//! Sync state machine: subscribe from round, request missing blocks, respond to requests.

use mysticeti_core::{BlockReference, BlockStore, NetworkMessage, RoundNumber};

/// Sync driver: given a block store and a sender to the network, decides when to send
/// SubscribeOwnFrom, RequestBlocks, and responds with Blocks / RequestBlocksResponse / BlockNotFound.
pub struct Syncer {
    /// Round from which we last sent SubscribeOwnFrom (per peer).
    subscribe_from: RoundNumber,
}

impl Syncer {
    pub fn new() -> Self {
        Self {
            subscribe_from: 0,
        }
    }

    /// Build a SubscribeOwnFrom message for the next round (call when connecting or periodically).
    pub fn subscribe_message(&mut self, from_round: RoundNumber) -> NetworkMessage {
        self.subscribe_from = from_round;
        NetworkMessage::SubscribeOwnFrom(from_round)
    }

    /// Handle RequestBlocks: look up in store, return Blocks or BlockNotFound.
    pub fn handle_request_blocks(
        &self,
        store: &BlockStore,
        refs: &[BlockReference],
    ) -> NetworkMessage {
        let mut blocks = Vec::new();
        let mut not_found = Vec::new();
        for r in refs {
            if let Some(b) = store.get(r) {
                blocks.push(b);
            } else {
                not_found.push(r.clone());
            }
        }
        if not_found.is_empty() {
            NetworkMessage::RequestBlocksResponse(blocks)
        } else if blocks.is_empty() {
            NetworkMessage::BlockNotFound(not_found)
        } else {
            NetworkMessage::RequestBlocksResponse(blocks)
        }
    }
}

impl Default for Syncer {
    fn default() -> Self {
        Self::new()
    }
}
