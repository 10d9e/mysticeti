//! Base committer: wave-based direct and indirect commit decisions.

use crate::committee::Committee;
use crate::data::Data;
use crate::types::{AuthorityIndex, BlockReference, RoundNumber, StatementBlock};

/// Status of a leader in a wave: committed block, skipped, or undecided.
#[derive(Clone, Debug)]
pub enum LeaderStatus {
    Commit(Data<StatementBlock>),
    Skip(AuthorityIndex, RoundNumber),
    Undecided(AuthorityIndex, RoundNumber),
}

/// Wave-based committer: one leader per wave, direct commit if quorum in decision round, else indirect via anchor.
pub struct BaseCommitter {
    committee: Committee,
    wave_length: u64,
    leader_offset: u32,
}

impl BaseCommitter {
    pub fn new(committee: Committee, wave_length: u64, leader_offset: u32) -> Self {
        Self {
            committee,
            wave_length,
            leader_offset,
        }
    }

    fn leader_round(&self, wave: u64) -> RoundNumber {
        wave * self.wave_length
    }

    fn decision_round(&self, wave: u64) -> RoundNumber {
        wave * self.wave_length + self.wave_length - 1
    }

    /// Try to decide the leader for the given wave. Returns LeaderStatus.
    /// Direct: leader committed if there are quorum certificates in the decision round that include the leader's block.
    /// Indirect: if a later wave committed, we can decide this wave's leader by following the anchor.
    pub fn try_decide(
        &self,
        wave: u64,
        get_block: &impl Fn(&BlockReference) -> Option<Data<StatementBlock>>,
        get_round_blocks: &impl Fn(RoundNumber) -> Vec<BlockReference>,
    ) -> Option<LeaderStatus> {
        let leader_round = self.leader_round(wave);
        let decision_round = self.decision_round(wave);
        let leader_authority = self.committee.elect_leader(leader_round, self.leader_offset);
        let leader_round_refs = get_round_blocks(leader_round);
        let leader_block_ref = leader_round_refs
            .iter()
            .find(|r| r.authority == leader_authority)
            .cloned();
        let leader_block = leader_block_ref.as_ref().and_then(|r| get_block(r));
        let decision_refs = get_round_blocks(decision_round);
        let mut certificates = 0u64;
        for r in &decision_refs {
            if let Some(b) = get_block(r) {
                let includes_leader = leader_block_ref
                    .as_ref()
                    .map_or(false, |lr| b.includes.contains(lr));
                if includes_leader {
                    certificates += self
                        .committee
                        .authority(r.authority)
                        .map(|a| a.stake)
                        .unwrap_or(0);
                }
            }
        }
        if certificates >= self.committee.quorum_threshold {
            return leader_block.map(LeaderStatus::Commit);
        }
        let blame_stake: u64 = decision_refs
            .iter()
            .filter_map(|r| self.committee.authority(r.authority))
            .map(|a| a.stake)
            .sum();
        if blame_stake >= self.committee.quorum_threshold {
            return Some(LeaderStatus::Skip(leader_authority, leader_round));
        }
        Some(LeaderStatus::Undecided(leader_authority, leader_round))
    }
}
