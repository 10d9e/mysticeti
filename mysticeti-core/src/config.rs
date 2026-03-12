//! Configuration: parameters, per-validator identifiers, private config.

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;

/// Network and metrics addresses for one validator (transport-agnostic in core).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identifier {
    pub public_key: PublicKey,
    pub network_address: String,
    pub metrics_address: String,
}

/// Consensus and system parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Parameters {
    pub identifiers: Vec<Identifier>,
    pub wave_length: u64,
    pub leader_timeout_ms: u64,
    pub rounds_in_epoch: u64,
    pub shutdown_grace_period_ms: u64,
    pub number_of_leaders: u32,
    pub enable_pipelining: bool,
    pub store_retain_rounds: u64,
    pub enable_cleanup: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            identifiers: vec![],
            wave_length: 3,
            leader_timeout_ms: 5_000,
            rounds_in_epoch: 100,
            shutdown_grace_period_ms: 30_000,
            number_of_leaders: 1,
            enable_pipelining: false,
            store_retain_rounds: 1000,
            enable_cleanup: true,
        }
    }
}

/// Private config for this node (not shared).
#[derive(Clone, Debug)]
pub struct PrivateConfig {
    pub authority_index: crate::types::AuthorityIndex,
    pub storage_dir: std::path::PathBuf,
}
