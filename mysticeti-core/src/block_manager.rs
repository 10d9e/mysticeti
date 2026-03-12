//! Block manager: causal ordering, inclusion invariants, and block creation support.

use crate::block_store::BlockStore;
use crate::block_validator::BlockValidator;
use crate::committee::Committee;
use crate::data::Data;
use crate::types::{AuthorityIndex, BlockReference, RoundNumber, StatementBlock};

/// Manages the DAG: ensures blocks respect round ordering and inclusion rules.
pub struct BlockManager {
    store: BlockStore,
    committee: Committee,
    validator: BlockValidator,
}

impl BlockManager {
    pub fn new(committee: Committee) -> Self {
        Self {
            store: BlockStore::new(),
            validator: BlockValidator::new(committee.clone()),
            committee,
        }
    }

    pub fn store(&self) -> &BlockStore {
        &self.store
    }

    pub fn committee(&self) -> &Committee {
        &self.committee
    }

    /// Add blocks from the network; returns only those that were newly accepted.
    pub fn add_blocks(&self, blocks: Vec<Data<StatementBlock>>) -> Vec<Data<StatementBlock>> {
        let mut accepted = Vec::new();
        for block in blocks {
            if self.validator.validate(block.inner(), self.store()) {
                let reference = block.reference();
                if !self.store.contains(&reference) {
                    self.store.insert(block.clone());
                    accepted.push(block);
                }
            }
        }
        accepted
    }

    /// Get the highest round we have for an authority.
    pub fn highest_round(&self, authority: AuthorityIndex) -> Option<RoundNumber> {
        self.store.max_round_for_authority(authority)
    }

    /// Get references to blocks that must be included at the next round for an authority
    /// (e.g. one from each of a quorum of authorities at round - 1). Simplified: return all refs from round r-1.
    pub fn required_includes_for_round(
        &self,
        _authority: AuthorityIndex,
        round: RoundNumber,
    ) -> Vec<BlockReference> {
        if round == 0 {
            return vec![];
        }
        let prev = round - 1;
        self.store.references_at_round(prev)
    }
}

