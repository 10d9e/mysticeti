//! Protocol types: blocks, references, statements, and round numbers.

use serde::{Deserialize, Serialize};

/// Maximum number of authorities in the committee.
pub const MAX_AUTHORITIES: usize = 512;

/// Round number in the DAG (monotonically increasing per authority).
pub type RoundNumber = u64;

/// Index of an authority in the committee (0..n).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct AuthorityIndex(pub u64);

impl AuthorityIndex {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

/// Digest of a block (hash or content-based identifier).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockDigest(pub [u8; 32]);

impl BlockDigest {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Reference to a block: (authority, round, digest).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockReference {
    pub authority: AuthorityIndex,
    pub round: RoundNumber,
    pub digest: BlockDigest,
}

/// Opaque transaction payload (application-defined).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction(pub Vec<u8>);

/// Locator for a transaction within a block's statements (block ref + index).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionLocator {
    pub block: BlockReference,
    pub index: u32,
}

/// Range of transaction locators (for batched votes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionLocatorRange {
    pub start: TransactionLocator,
    pub count: u32,
}

/// Vote on a transaction (Accept or Reject).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vote {
    Accept,
    Reject,
}

/// Statement in a block: either a transaction share or a vote (or batched votes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseStatement {
    /// Share a transaction (proposer includes it in their block).
    Share(Transaction),
    /// Vote on a single transaction.
    Vote(TransactionLocator, Vote),
    /// Batched accept votes for a range of transactions.
    VoteRange(TransactionLocatorRange),
}

/// Epoch number (for reconfiguration).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EpochNumber(pub u64);

/// Signature over block content (authority's signature).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockSignature(pub Vec<u8>);

/// A block in the DAG: creator, round, digest, includes (prior refs), statements, epoch, signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatementBlock {
    pub creator: AuthorityIndex,
    pub round: RoundNumber,
    pub digest: BlockDigest,
    /// References to blocks from strictly lower rounds that this block includes.
    pub includes: Vec<BlockReference>,
    /// Statements: transaction shares and votes.
    pub statements: Vec<BaseStatement>,
    pub epoch: EpochNumber,
    pub signature: BlockSignature,
}

impl StatementBlock {
    /// Create a new block (digest and signature typically set by crypto layer).
    pub fn new(
        creator: AuthorityIndex,
        round: RoundNumber,
        digest: BlockDigest,
        includes: Vec<BlockReference>,
        statements: Vec<BaseStatement>,
        epoch: EpochNumber,
        signature: BlockSignature,
    ) -> Self {
        Self {
            creator,
            round,
            digest,
            includes,
            statements,
            epoch,
            signature,
        }
    }

    pub fn reference(&self) -> BlockReference {
        BlockReference {
            authority: self.creator,
            round: self.round,
            digest: self.digest.clone(),
        }
    }
}
