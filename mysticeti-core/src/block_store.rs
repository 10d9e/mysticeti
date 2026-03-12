//! Block storage: store and retrieve blocks by reference.

use std::collections::HashMap;

use parking_lot::RwLock;

use crate::data::Data;
use crate::types::{BlockReference, RoundNumber, StatementBlock};

/// In-memory block store keyed by BlockReference.
#[derive(Default)]
pub struct BlockStore {
    blocks: RwLock<HashMap<BlockReference, Data<StatementBlock>>>,
}

impl BlockStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, reference: &BlockReference) -> Option<Data<StatementBlock>> {
        self.blocks.read().get(reference).cloned()
    }

    pub fn contains(&self, reference: &BlockReference) -> bool {
        self.blocks.read().contains_key(reference)
    }

    pub fn insert(&self, block: Data<StatementBlock>) {
        let reference = block.reference();
        self.blocks.write().insert(reference, block);
    }

    pub fn insert_many(&self, blocks: Vec<Data<StatementBlock>>) {
        let mut guard = self.blocks.write();
        for block in blocks {
            let reference = block.reference();
            guard.insert(reference, block);
        }
    }

    /// Remove blocks with round < (current_round - retain_rounds). Call periodically if cleanup enabled.
    pub fn cleanup_old_rounds(&self, current_round: crate::types::RoundNumber, retain_rounds: u64) {
        let cutoff = current_round.saturating_sub(retain_rounds);
        self.blocks.write().retain(|_, b| b.round >= cutoff);
    }

    /// Iterate over all block references (e.g. for required_includes).
    pub fn references_at_round(&self, round: RoundNumber) -> Vec<BlockReference> {
        self.blocks
            .read()
            .iter()
            .filter(|(r, _)| r.round == round)
            .map(|(r, _)| r.clone())
            .collect()
    }

    /// Highest round we have for the given authority.
    pub fn max_round_for_authority(
        &self,
        authority: crate::types::AuthorityIndex,
    ) -> Option<RoundNumber> {
        self.blocks
            .read()
            .iter()
            .filter(|(r, _)| r.authority == authority)
            .map(|(r, _)| r.round)
            .max()
    }
}
