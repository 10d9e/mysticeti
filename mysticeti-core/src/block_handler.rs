//! Block handler: produce new blocks and statements (transactions + votes) for the core.

use crate::data::Data;
use crate::types::{AuthorityIndex, BaseStatement, RoundNumber, StatementBlock};

/// Handles block proposals and produces statements (tx shares, votes) for new blocks.
pub trait BlockHandler: Send + Sync {
    /// Called when the core is about to create a new block. Return statements to include.
    fn statements_for_block(
        &self,
        _authority: AuthorityIndex,
        _round: RoundNumber,
        _includes: &[crate::types::BlockReference],
    ) -> Vec<BaseStatement> {
        vec![]
    }

    /// Called when blocks are received (e.g. for fast path or vote generation). Default: no-op.
    fn on_blocks(&self, _blocks: &[Data<StatementBlock>]) {}
}

/// No-op handler for testing or minimal nodes.
#[derive(Clone, Default)]
pub struct NoOpBlockHandler;

impl BlockHandler for NoOpBlockHandler {}
