//! DAG and statement invariants: validate blocks before adding to the store.

use crate::block_store::BlockStore;
use crate::committee::Committee;
use crate::types::{BaseStatement, StatementBlock};

pub struct BlockValidator {
    committee: Committee,
}

impl BlockValidator {
    pub fn new(committee: Committee) -> Self {
        Self { committee }
    }

    /// Validate a block: creator in committee, includes from lower rounds, quorum at r-1, statements valid.
    pub fn validate(&self, block: &StatementBlock, store: &BlockStore) -> bool {
        if block.creator.0 as usize >= self.committee.size() {
            return false;
        }
        let round = block.round;
        for inc in &block.includes {
            if inc.round >= round {
                return false;
            }
            if !store.contains(inc) {
                return false;
            }
        }
        // Simplified: require at least one include from round-1 (reference uses quorum of r-1).
        if round > 0 {
            let prev_round = round - 1;
            let included_authorities: std::collections::HashSet<u64> = block
                .includes
                .iter()
                .filter(|r| r.round == prev_round)
                .map(|r| r.authority.0)
                .collect();
            let stake_included: u64 = self
                .committee
                .authorities
                .iter()
                .enumerate()
                .filter(|(i, _)| included_authorities.contains(&(*i as u64)))
                .map(|(_, a)| a.stake)
                .sum();
            if stake_included < self.committee.quorum_threshold {
                return false;
            }
        }
        // Statements: votes must reference blocks in this block or causal history (simplified: allow).
        for stmt in &block.statements {
            if !self.validate_statement(stmt, block, store) {
                return false;
            }
        }
        true
    }

    fn validate_statement(
        &self,
        stmt: &BaseStatement,
        block: &StatementBlock,
        _store: &BlockStore,
    ) -> bool {
        match stmt {
            BaseStatement::Share(_) => true,
            BaseStatement::Vote(loc, _) => {
                // Locator's block should be in includes or be this block.
                if loc.block.authority == block.creator && loc.block.round == block.round {
                    return (loc.index as usize) < block.statements.len();
                }
                block.includes.contains(&loc.block)
            }
            BaseStatement::VoteRange(range) => {
                range.count > 0
                    && (block.includes.contains(&range.start.block)
                        || (range.start.block.authority == block.creator
                            && range.start.block.round == block.round))
            }
        }
    }
}
