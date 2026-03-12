//! Connection management: map EndpointId to authority index, connect to peers.

use std::collections::HashMap;
use std::sync::Arc;

use iroh::EndpointId;
use parking_lot::RwLock;

use mysticeti_core::{AuthorityIndex, Committee};

/// Maps Iroh EndpointId (PublicKey) to committee AuthorityIndex.
/// Used to identify which validator an incoming connection belongs to.
#[derive(Clone)]
pub struct PeerIdentity {
    #[allow(dead_code)]
    committee: Committee,
    /// EndpointId (bytes) -> AuthorityIndex. Populated from config (e.g. same order as committee).
    endpoint_to_authority: Arc<RwLock<HashMap<[u8; 32], AuthorityIndex>>>,
}

impl PeerIdentity {
    pub fn new(committee: Committee) -> Self {
        let endpoint_to_authority = Arc::new(RwLock::new(HashMap::new()));
        Self {
            committee,
            endpoint_to_authority,
        }
    }

    /// Register a peer's EndpointId as the given authority (e.g. from config).
    pub fn register(&self, endpoint_id: &EndpointId, authority: AuthorityIndex) {
        let key = *endpoint_id.as_bytes();
        self.endpoint_to_authority.write().insert(key, authority);
    }

    /// Resolve connection remote id to AuthorityIndex. Returns None if unknown (reject in handler).
    pub fn authority_index(&self, endpoint_id: &EndpointId) -> Option<AuthorityIndex> {
        let key = *endpoint_id.as_bytes();
        self.endpoint_to_authority.read().get(&key).copied()
    }
}
