//! Data wrapper with cached serialized bytes to avoid re-serialization when sending to multiple peers.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::types::StatementBlock;

/// Arc-wrapped value with optional cached bincode bytes for efficient multi-send.
#[derive(Clone)]
pub struct Data<T> {
    inner: Arc<T>,
    /// Cached serialized form; filled on first serialization.
    cached: Arc<RwLock<Option<Vec<u8>>>>,
}

impl<T> Data<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(value),
            cached: Arc::new(RwLock::new(None)),
        }
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> std::ops::Deref for Data<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T: Serialize> Data<T> {
    /// Serialize to bytes, using cache if available.
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        {
            let guard = self.cached.read();
            if let Some(ref bytes) = *guard {
                return Ok(bytes.clone());
            }
        }
        let bytes = bincode::serialize(self.inner.as_ref())?;
        self.cached.write().replace(bytes.clone());
        Ok(bytes)
    }
}

impl Data<StatementBlock> {
    /// Convenience constructor for block data.
    pub fn block(block: StatementBlock) -> Self {
        Self::new(block)
    }
}

impl<T: Serialize + for<'de> Deserialize<'de>> Data<T> {
    /// Deserialize from bytes (no cache on the receiving side until first to_bytes).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        let value: T = bincode::deserialize(bytes)?;
        Ok(Self::new(value))
    }
}

impl<T: Serialize> Serialize for Data<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Data<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = T::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Data<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
