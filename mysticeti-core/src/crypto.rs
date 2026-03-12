//! Signing and verification of blocks; block digest; public key type.

use ed25519_dalek::{Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};

use crate::types::{BlockDigest, BlockSignature, StatementBlock};

/// Public key for an authority (32 bytes).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(pub [u8; 32]);

/// Secret key for signing (wraps ed25519_dalek::SecretKey for Serde / API stability).
#[derive(Clone)]
pub struct SecretKey(ed25519_dalek::SigningKey);

impl SecretKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ed25519_dalek::SignatureError> {
        let key = ed25519_dalek::SigningKey::from_bytes(
            bytes.try_into().map_err(|_| ed25519_dalek::SignatureError::new())?,
        );
        Ok(Self(key))
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.0.verifying_key().to_bytes())
    }

    /// Sign the canonical representation of a block (for digest we use hash of that representation).
    pub fn sign_block(&self, block: &StatementBlock) -> (BlockDigest, BlockSignature) {
        let message = bincode::serialize(block).expect("block serialization");
        let digest = BlockDigest(
            blake3::hash(&message).as_bytes()[..32]
                .try_into()
                .expect("32 bytes"),
        );
        let sig = self.0.sign(&message);
        let signature = BlockSignature(sig.to_bytes().to_vec());
        (digest, signature)
    }
}

/// Verify a block's signature using the creator's public key.
pub fn verify_block(
    block: &StatementBlock,
    public_key: &PublicKey,
) -> Result<(), ed25519_dalek::SignatureError> {
    let message = bincode::serialize(block).expect("block serialization");
    let sig_bytes: [u8; 64] = block
        .signature
        .0
        .as_slice()
        .try_into()
        .map_err(|_| ed25519_dalek::SignatureError::new())?;
    let signature = Signature::from_bytes(&sig_bytes);
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&public_key.0)
        .map_err(|_| ed25519_dalek::SignatureError::new())?;
    verifying_key.verify(&message, &signature)
}

/// Compute digest of a block (without signing).
pub fn block_digest(block: &StatementBlock) -> BlockDigest {
    let message = bincode::serialize(block).expect("block serialization");
    BlockDigest(
        blake3::hash(&message).as_bytes()[..32]
            .try_into()
            .expect("32 bytes"),
    )
}

// BlockSignature is defined in types.rs and used by StatementBlock.
