//! Universal committer: composes multiple base committers (multiple leaders per round, pipelining).

use crate::committee::Committee;
use crate::consensus::base_committer::{BaseCommitter, LeaderStatus};
use crate::data::Data;
use crate::types::{BlockReference, RoundNumber, StatementBlock};

/// Composes several BaseCommitters (e.g. different leader_offset) to decide multiple leaders per wave.
pub struct UniversalCommitter {
    committers: Vec<BaseCommitter>,
}

impl UniversalCommitter {
    pub fn new(committee: Committee, wave_length: u64, number_of_leaders: u32) -> Self {
        let committers = (0..number_of_leaders)
            .map(|offset| BaseCommitter::new(committee.clone(), wave_length, offset))
            .collect();
        Self { committers }
    }

    /// Try to commit: iterate waves from last_decided+1, collect Commit/Skip until we hit Undecided.
    /// Returns the ordered list of decided leader statuses.
    pub fn try_commit(
        &self,
        last_decided_wave: u64,
        get_block: &impl Fn(&BlockReference) -> Option<Data<StatementBlock>>,
        get_round_blocks: &impl Fn(RoundNumber) -> Vec<BlockReference>,
    ) -> Vec<LeaderStatus> {
        let mut wave = last_decided_wave.wrapping_add(1);
        let mut results = Vec::new();
        loop {
            let mut any_undecided = false;
            for committer in &self.committers {
                if let Some(status) = committer.try_decide(wave, get_block, get_round_blocks) {
                    if matches!(&status, LeaderStatus::Undecided(_, _)) {
                        any_undecided = true;
                    }
                    results.push(status);
                }
            }
            if any_undecided {
                break;
            }
            wave += 1;
        }
        results
    }
}
