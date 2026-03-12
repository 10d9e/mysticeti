//! Linearizer: turn committed leaders into an ordered CommittedSubDag of blocks.

use crate::data::Data;
use crate::types::StatementBlock;

/// Ordered sequence of committed blocks for execution.
#[derive(Clone, Debug)]
pub struct CommittedSubDag {
    pub blocks: Vec<Data<StatementBlock>>,
}

/// Converts committed leader blocks (from committer) into a causal order for execution.
pub struct Linearizer;

impl Linearizer {
    pub fn new() -> Self {
        Self
    }

    /// Order the committed leader blocks: respect DAG causality (includes) so each block's
    /// dependencies appear before it. Use topological sort.
    pub fn linearize(&self, committed_leaders: Vec<Data<StatementBlock>>) -> CommittedSubDag {
        if committed_leaders.is_empty() {
            return CommittedSubDag { blocks: vec![] };
        }
        let refs: std::collections::HashMap<_, _> = committed_leaders
            .iter()
            .map(|b| (b.reference(), b.clone()))
            .collect();
        let mut in_degree: std::collections::HashMap<crate::types::BlockReference, usize> =
            committed_leaders.iter().map(|b| (b.reference(), 0)).collect();
        for b in &committed_leaders {
            let b_ref = b.reference();
            for inc in &b.includes {
                if refs.contains_key(inc) {
                    *in_degree.get_mut(&b_ref).unwrap() += 1;
                }
            }
        }
        let mut queue: std::collections::VecDeque<_> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(r, _)| refs.get(r).cloned().unwrap())
            .collect();
        let mut result = Vec::new();
        while let Some(b) = queue.pop_front() {
            let b_ref = b.reference();
            result.push(b.clone());
            for other in &committed_leaders {
                let o_ref = other.reference();
                if o_ref != b_ref && other.includes.iter().any(|i| i == &b_ref) {
                    if let Some(d) = in_degree.get_mut(&o_ref) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(other.clone());
                        }
                    }
                }
            }
        }
        CommittedSubDag { blocks: result }
    }
}

impl Default for Linearizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Data;
    use crate::types::{AuthorityIndex, BlockDigest, BlockSignature, EpochNumber, StatementBlock};

    #[test]
    fn linearize_empty() {
        let l = Linearizer::new();
        let subdag = l.linearize(vec![]);
        assert!(subdag.blocks.is_empty());
    }

    #[test]
    fn linearize_single() {
        let block = StatementBlock::new(
            AuthorityIndex(0),
            0,
            BlockDigest([0u8; 32]),
            vec![],
            vec![],
            EpochNumber(0),
            BlockSignature(vec![]),
        );
        let l = Linearizer::new();
        let subdag = l.linearize(vec![Data::new(block)]);
        assert_eq!(subdag.blocks.len(), 1);
    }
}

