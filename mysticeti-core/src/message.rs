//! Wire message types for the Mysticeti protocol (transport-agnostic).

use serde::{Deserialize, Serialize};

use crate::data::Data;
use crate::types::{BlockReference, RoundNumber, StatementBlock};

/// Messages exchanged between validators (reference protocol vocabulary).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// Subscribe to peer's own blocks from the given round (excluding).
    SubscribeOwnFrom(RoundNumber),
    /// Batch of blocks.
    Blocks(Vec<Data<StatementBlock>>),
    /// Request specific blocks by reference.
    RequestBlocks(Vec<BlockReference>),
    /// Response with requested blocks.
    RequestBlocksResponse(Vec<Data<StatementBlock>>),
    /// Requested blocks were not available.
    BlockNotFound(Vec<BlockReference>),
}
