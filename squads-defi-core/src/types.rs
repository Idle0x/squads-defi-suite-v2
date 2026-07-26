//! Hand-rolled Solana primitive types.
//!
//! No solana-sdk dependency — uses bs58 + borsh for wasm32-wasip2 compatibility.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Serialize, Deserialize};
use std::fmt;

/// A Solana public key (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, BorshSerialize, BorshDeserialize)]
pub struct Pubkey(pub [u8; 32]);

// Custom serde: Pubkey serializes as a base58 string, not a 32-element array.
impl serde::Serialize for Pubkey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Pubkey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Pubkey::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Pubkey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| format!("invalid base58: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!(
                "pubkey must be 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn to_string(&self) -> String {
        bs58::encode(self.0).into_string()
    }

    pub fn to_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pubkey({})", self.to_string())
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// A Solana ed25519 signature (64 bytes).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(pub [u8; 64]);

// Custom serde: Signature serializes as a base58 string, not a 64-element array.
impl serde::Serialize for Signature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Signature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        let bytes = bs58::decode(&s)
            .into_vec()
            .map_err(serde::de::Error::custom)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom(format!(
                "signature must be 64 bytes, got {}",
                bytes.len()
            )));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Signature(arr))
    }
}

impl Signature {
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }

    pub fn to_string(&self) -> String {
        bs58::encode(self.0).into_string()
    }
}

// Borsh impls for [u8; 64] — borsh only provides up to [u8; 32].
impl BorshSerialize for Signature {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.0)
    }
}

impl BorshDeserialize for Signature {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut arr = [0u8; 64];
        reader.read_exact(&mut arr)?;
        Ok(Signature(arr))
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Signature({})", self.to_string())
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// A Solana recent blockhash (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Blockhash(pub [u8; 32]);

// Custom serde: Blockhash serializes as a base58 string.
impl serde::Serialize for Blockhash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Blockhash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        let bytes = bs58::decode(&s)
            .into_vec()
            .map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom(format!(
                "blockhash must be 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Blockhash(arr))
    }
}

impl Blockhash {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        let bytes = bs58::decode(s)
            .into_vec()
            .map_err(|e| format!("invalid base58: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!("blockhash must be 32 bytes, got {}", bytes.len()));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn to_string(&self) -> String {
        bs58::encode(self.0).into_string()
    }

    pub fn to_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Blockhash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Blockhash({})", self.to_string())
    }
}

impl fmt::Display for Blockhash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

// ============================================================================
// Wire-format types — Solana transaction binary layout
// ============================================================================

/// Solana MessageHeader — the first 3 bytes of every versioned transaction message.
/// This is MANDATORY for wire-format compatibility.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct MessageHeader {
    /// Number of required signatures (signers that must sign the tx).
    /// The TOP BIT encodes the message version: 0xxxxxxx = legacy, 1xxxxxxx = versioned.
    pub num_required_signatures: u8,
    /// Number of read-only signer accounts.
    pub num_readonly_signed_accounts: u8,
    /// Number of read-only non-signer accounts.
    pub num_readonly_unsigned_accounts: u8,
}

/// An account in a Solana instruction BEFORE compilation.
/// This is a BUILD-TIME type only — never serialized into the transaction.
#[derive(Clone, Debug, BorshSerialize)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

/// A Solana instruction BEFORE account index resolution.
/// Resolved to CompiledInstruction during message compilation.
#[derive(Clone, Debug)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

/// A single Address Lookup Table reference in a v0 transaction.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct MessageAddressTableLookup {
    pub account_key: Pubkey,
    pub writable_indexes: Vec<u8>,
    pub readonly_indexes: Vec<u8>,
}
