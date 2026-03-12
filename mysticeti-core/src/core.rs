//! Core loop: add blocks, try new block, try commit, drive WAL and commit observer.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::block_handler::BlockHandler;
use crate::block_manager::BlockManager;
use crate::commit_observer::CommitObserver;
use crate::committee::Committee;
use crate::config::Parameters;
use crate::consensus::{LeaderStatus, UniversalCommitter};
use crate::crypto::{block_digest, SecretKey};
use crate::data::Data;
use crate::types::{BlockDigest, BlockReference, BlockSignature, RoundNumber, StatementBlock};

/// Core state machine: ingests blocks, creates new blocks, runs consensus, notifies commit observer.
pub struct Core<H: BlockHandler> {
    params: Parameters,
    committee: Committee,
    manager: BlockManager,
    committer: UniversalCommitter,
    signer: Option<SecretKey>,
    authority_index: crate::types::AuthorityIndex,
    handler: H,
    last_committed_round: AtomicU64,
}

impl<H: BlockHandler> Core<H> {
    pub fn new(
        params: Parameters,
        committee: Committee,
        authority_index: crate::types::AuthorityIndex,
        signer: Option<SecretKey>,
        handler: H,
    ) -> Self {
        let committer = UniversalCommitter::new(
            committee.clone(),
            params.wave_length,
            params.number_of_leaders,
        );
        Self {
            params: params.clone(),
            committee: committee.clone(),
            manager: BlockManager::new(committee),
            committer,
            signer,
            authority_index,
            handler,
            last_committed_round: AtomicU64::new(u64::MAX),
        }
    }

    pub fn manager(&self) -> &BlockManager {
        &self.manager
    }

    pub fn committee(&self) -> &Committee {
        &self.committee
    }

    /// Add blocks received from the network. Returns newly accepted blocks.
    pub fn add_blocks(&self, blocks: Vec<Data<StatementBlock>>) -> Vec<Data<StatementBlock>> {
        let accepted = self.manager.add_blocks(blocks);
        if !accepted.is_empty() {
            self.handler.on_blocks(&accepted);
        }
        accepted
    }

    /// Try to create a new block for this authority at the next round.
    pub fn try_new_block(&self) -> Option<Data<StatementBlock>> {
        let store = self.manager.store();
        let my_round = self
            .manager
            .highest_round(self.authority_index)
            .map(|r| r + 1)
            .unwrap_or(0);
        let includes = self
            .manager
            .required_includes_for_round(self.authority_index, my_round);
        let statements = self.handler.statements_for_block(
            self.authority_index,
            my_round,
            &includes,
        );
        let digest_placeholder = BlockDigest([0u8; 32]);
        let sig_placeholder = BlockSignature(vec![]);
        let block_for_signing = StatementBlock::new(
            self.authority_index,
            my_round,
            digest_placeholder.clone(),
            includes.clone(),
            statements.clone(),
            self.committee.epoch,
            sig_placeholder,
        );
        let (digest, signature) = self
            .signer
            .as_ref()
            .map(|s| s.sign_block(&block_for_signing))
            .unwrap_or_else(|| (block_digest(&block_for_signing), BlockSignature(vec![])));
        let block = StatementBlock::new(
            self.authority_index,
            my_round,
            digest,
            includes,
            statements,
            self.committee.epoch,
            signature,
        );
        let block = Data::new(block);
        store.insert(block.clone());
        Some(block)
    }

    /// Try to commit: run universal committer, then notify commit observer. Returns committed leader statuses.
    pub fn try_commit<O: CommitObserver>(&self, observer: &O) -> Vec<LeaderStatus> {
        let _store = self.manager.store();
        let get_block = |r: &BlockReference| _store.get(r).clone();
        let get_round_blocks = |round: RoundNumber| _store.references_at_round(round);
        let last_committed = self.last_committed_round.load(Ordering::SeqCst);
        let last_wave = if last_committed == u64::MAX {
            u64::MAX
        } else {
            last_committed / self.params.wave_length
        };
        let results = self
            .committer
            .try_commit(last_wave, &get_block, &get_round_blocks);
        let committed_waves: u64 = results
            .iter()
            .filter_map(|s| {
                if let LeaderStatus::Commit(b) = s {
                    Some(b.round / self.params.wave_length)
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);
        let any_commit = results
            .iter()
            .any(|s| matches!(s, LeaderStatus::Commit(_)));
        let should_update = (last_wave == u64::MAX && any_commit)
            || (last_wave != u64::MAX && committed_waves > last_wave);
        if should_update {
            self.last_committed_round.store(
                committed_waves * self.params.wave_length + self.params.wave_length - 1,
                Ordering::SeqCst,
            );
        }
        observer.handle_commit(results.clone());
        if self.params.enable_cleanup {
            let current = self
                .manager
                .highest_round(self.authority_index)
                .unwrap_or(0);
            _store.cleanup_old_rounds(current, self.params.store_retain_rounds);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_handler::NoOpBlockHandler;
    use crate::commit_observer::SimpleCommitObserver;
    use crate::committee::{Authority, Committee};
    use crate::data::Data;
    use crate::types::{
        AuthorityIndex, BlockDigest, BlockSignature, EpochNumber, StatementBlock,
    };

    fn test_committee() -> Committee {
        Committee {
            authorities: (0..4u8)
                .map(|i| Authority {
                    stake: 1,
                    public_key: crate::crypto::PublicKey([i; 32]),
                    hostname: format!("v{}", i),
                })
                .collect(),
            epoch: EpochNumber(0),
            validity_threshold: 2,
            quorum_threshold: 3,
        }
    }

    #[test]
    fn try_commit_emits_commit_after_three_rounds() {
        let committee = test_committee();
        let params = Parameters {
            wave_length: 3,
            number_of_leaders: 1,
            ..Default::default()
        };
        let core = Core::new(
            params,
            committee,
            AuthorityIndex(0),
            None,
            NoOpBlockHandler,
        );
        let r0: Vec<_> = (0..4u64)
            .map(|a| {
                Data::new(StatementBlock::new(
                    AuthorityIndex(a),
                    0,
                    BlockDigest([0u8; 32]),
                    vec![],
                    vec![],
                    EpochNumber(0),
                    BlockSignature(vec![]),
                ))
            })
            .collect();
        core.add_blocks(r0.clone());
        let refs0: Vec<_> = r0.iter().map(|b| b.reference()).collect();
        let r1: Vec<_> = (0..4u64)
            .map(|a| {
                Data::new(StatementBlock::new(
                    AuthorityIndex(a),
                    1,
                    BlockDigest([0u8; 32]),
                    refs0.clone(),
                    vec![],
                    EpochNumber(0),
                    BlockSignature(vec![]),
                ))
            })
            .collect();
        core.add_blocks(r1.clone());
        let refs1: Vec<_> = r1.iter().map(|b| b.reference()).collect();
        let mut refs_01 = refs0.clone();
        refs_01.extend(refs1);
        let r2: Vec<_> = (0..4u64)
            .map(|a| {
                Data::new(StatementBlock::new(
                    AuthorityIndex(a),
                    2,
                    BlockDigest([0u8; 32]),
                    refs_01.clone(),
                    vec![],
                    EpochNumber(0),
                    BlockSignature(vec![]),
                ))
            })
            .collect();
        core.add_blocks(r2);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let observer = SimpleCommitObserver::new(tx);
        let results = core.try_commit(&observer);
        let commits: Vec<_> = results
            .iter()
            .filter_map(|s| {
                if let LeaderStatus::Commit(_) = s {
                    Some(())
                } else {
                    None
                }
            })
            .collect();
        assert!(!commits.is_empty(), "expected at least one Commit, got {:?}", results);
        let _ = rx.try_recv();
    }
}

