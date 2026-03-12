//! Committee: validator set, stake, quorum thresholds, leader election, genesis.

use serde::{Deserialize, Serialize};

use crate::types::{AuthorityIndex, BlockDigest, EpochNumber, RoundNumber, StatementBlock};
use crate::crypto::PublicKey;
use crate::data::Data;
use crate::types::BlockSignature;

/// Single authority in the committee: stake and public key (and optional hostname for logging).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Authority {
    pub stake: u64,
    pub public_key: PublicKey,
    pub hostname: String,
}

/// Committee: list of authorities, epoch, and thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Committee {
    pub authorities: Vec<Authority>,
    pub epoch: EpochNumber,
    /// f+1 (validity)
    pub validity_threshold: u64,
    /// 2f+1 (quorum)
    pub quorum_threshold: u64,
}

impl Committee {
    pub fn size(&self) -> usize {
        self.authorities.len()
    }

    pub fn authority(&self, index: AuthorityIndex) -> Option<&Authority> {
        self.authorities.get(index.0 as usize)
    }

    pub fn get_public_key(&self, index: AuthorityIndex) -> Option<&PublicKey> {
        self.authority(index).map(|a| &a.public_key)
    }

    pub fn known_authority(&self, public_key: &PublicKey) -> Option<AuthorityIndex> {
        self.authorities
            .iter()
            .position(|a| a.public_key.0 == public_key.0)
            .map(|i| AuthorityIndex(i as u64))
    }

    /// Total stake in the committee.
    pub fn total_stake(&self) -> u64 {
        self.authorities.iter().map(|a| a.stake).sum()
    }

    /// Elect leader for a round (stake-weighted). offset allows multiple leaders per round.
    pub fn elect_leader(&self, round: RoundNumber, offset: u32) -> AuthorityIndex {
        let n = self.authorities.len() as u64;
        if n == 0 {
            return AuthorityIndex(0);
        }
        let index = ((round as u128 + offset as u128) % n as u128) as u64;
        AuthorityIndex(index)
    }

    /// Genesis blocks for an authority: empty block at round 0.
    pub fn genesis_blocks(&self, for_authority: AuthorityIndex) -> Vec<Data<StatementBlock>> {
        if self.authority(for_authority).is_none() {
            return vec![];
        }
        let digest = BlockDigest([0u8; 32]);
        let block = StatementBlock::new(
            for_authority,
            0,
            digest,
            vec![],
            vec![],
            self.epoch,
            BlockSignature(vec![]),
        );
        vec![Data::new(block)]
    }
}
