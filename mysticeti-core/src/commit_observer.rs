//! Commit observer: handle committed leaders and produce CommittedSubDag for execution.

use crate::consensus::{CommittedSubDag, LeaderStatus, Linearizer};
use crate::data::Data;
use crate::types::StatementBlock;

/// Observes commit decisions and turns them into ordered subdags (e.g. for execution or WAL).
pub trait CommitObserver: Send + Sync {
    /// Handle a batch of committed leader statuses from the committer.
    fn handle_commit(&self, committed: Vec<LeaderStatus>);
}

/// Simple implementation: linearize committed blocks and send the subdag on a channel.
pub struct SimpleCommitObserver {
    linearizer: Linearizer,
    tx: tokio::sync::mpsc::UnboundedSender<CommittedSubDag>,
}

impl SimpleCommitObserver {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<CommittedSubDag>) -> Self {
        Self {
            linearizer: Linearizer::new(),
            tx,
        }
    }

    pub fn linearizer(&self) -> &Linearizer {
        &self.linearizer
    }
}

impl CommitObserver for SimpleCommitObserver {
    fn handle_commit(&self, committed: Vec<LeaderStatus>) {
        let blocks: Vec<Data<StatementBlock>> = committed
            .into_iter()
            .filter_map(|s| {
                if let LeaderStatus::Commit(b) = s {
                    Some(b)
                } else {
                    None
                }
            })
            .collect();
        if blocks.is_empty() {
            return;
        }
        let subdag = self.linearizer.linearize(blocks);
        let _ = self.tx.send(subdag);
    }
}
