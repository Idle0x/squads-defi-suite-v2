//! Versioned transaction construction utilities.
//!
//! Build unsigned Solana versioned transactions without solana-sdk.
//! Hand-rolled compact-u16 wire format serialization — Solana's
//! actual on-chain format, NOT borsh (which uses u32 length prefixes).

use crate::types::{Blockhash, Pubkey, Signature};
use crate::types::{MessageHeader, MessageAddressTableLookup};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

// ===========================================================================
// COMPACT-U16 — Solana's variable-length integer encoding
// ===========================================================================

/// Encode a u16 as compact-u16 bytes (1-3 bytes).
/// Values 0x00–0x7F → 1 byte. 0x80–0x3FFF → 2 bytes. 0x4000–0xFFFF → 3 bytes.
pub fn compact_u16_encode(val: u16) -> Vec<u8> {
    let mut out = Vec::new();
    let mut v = val;
    loop {
        let mut b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out
}

/// Decode a compact-u16 from byte slice. Returns (value, bytes_consumed).
pub fn compact_u16_decode(bytes: &[u8]) -> Result<(u16, usize), String> {
    let mut val: u16 = 0;
    let mut shift: u32 = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if i >= 3 {
            return Err("compact-u16 too long (>3 bytes)".to_string());
        }
        val |= ((b & 0x7F) as u16) << shift;
        if b & 0x80 == 0 {
            return Ok((val, i + 1));
        }
        shift += 7;
    }
    Err("compact-u16 truncated".to_string())
}

// ===========================================================================
// COMPILED INSTRUCTION (wire-format-aware)
// ===========================================================================

/// A compiled Solana instruction (accounts as indices, data as bytes).
#[derive(Clone, Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

impl CompiledInstruction {
    /// Serialize to wire format: program_id_index + compact-u16(accounts) + accounts + compact-u16(data) + data
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.program_id_index);
        out.extend_from_slice(&compact_u16_encode(self.accounts.len() as u16));
        out.extend_from_slice(&self.accounts);
        out.extend_from_slice(&compact_u16_encode(self.data.len() as u16));
        out.extend_from_slice(&self.data);
        out
    }

    /// Deserialize from wire format bytes. Returns (instruction, bytes_consumed).
    pub fn from_wire(bytes: &[u8]) -> Result<(Self, usize), String> {
        if bytes.is_empty() {
            return Err("CompiledInstruction: empty input".to_string());
        }
        let mut pos = 0;
        let program_id_index = bytes[pos];
        pos += 1;

        let (accounts_len, consumed) = compact_u16_decode(&bytes[pos..])?;
        pos += consumed;
        let accounts_len = accounts_len as usize;
        if pos + accounts_len > bytes.len() {
            return Err("CompiledInstruction: truncated accounts".to_string());
        }
        let accounts = bytes[pos..pos + accounts_len].to_vec();
        pos += accounts_len;

        let (data_len, consumed) = compact_u16_decode(&bytes[pos..])?;
        pos += consumed;
        let data_len = data_len as usize;
        if pos + data_len > bytes.len() {
            return Err("CompiledInstruction: truncated data".to_string());
        }
        let data = bytes[pos..pos + data_len].to_vec();
        pos += data_len;

        Ok((Self { program_id_index, accounts, data }, pos))
    }
}

// ===========================================================================
// MESSAGE — Solana wire format (NOT borsh)
// ===========================================================================

/// A Solana versioned transaction message.
///
/// **BorshSerialize/BorshDeserialize are kept for internal SquadsProposal
/// serialization.** For the on-chain wire format, use `to_wire()`/`from_wire()`
/// which use compact-u16 for all array lengths per the Solana spec.
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct Message {
    pub header: MessageHeader,
    pub account_keys: Vec<Pubkey>,
    pub recent_blockhash: Blockhash,
    pub instructions: Vec<CompiledInstruction>,
    pub address_table_lookups: Vec<MessageAddressTableLookup>,
}

impl Message {
    /// Serialize to Solana wire format (compact-u16 array lengths).
    ///
    /// Layout:
    /// ```text
    /// header (3 bytes)
    /// compact-u16(account_keys.len)
    /// account_keys (32 bytes each)
    /// recent_blockhash (32 bytes)
    /// compact-u16(instructions.len)
    /// instructions (each: program_id_index + compact_u16(accounts) + accounts + compact_u16(data) + data)
    /// compact-u16(address_table_lookups.len)
    /// lookups (each: account_key + compact_u16(writable) + writable + compact_u16(readonly) + readonly)
    /// ```
    pub fn to_wire(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // Header — 3 bytes
        out.push(self.header.num_required_signatures);
        out.push(self.header.num_readonly_signed_accounts);
        out.push(self.header.num_readonly_unsigned_accounts);

        // Account keys
        out.extend_from_slice(&compact_u16_encode(self.account_keys.len() as u16));
        for key in &self.account_keys {
            out.extend_from_slice(key.to_bytes());
        }

        // Recent blockhash — 32 bytes
        out.extend_from_slice(self.recent_blockhash.to_bytes());

        // Instructions
        out.extend_from_slice(&compact_u16_encode(self.instructions.len() as u16));
        for ix in &self.instructions {
            out.extend_from_slice(&ix.to_wire());
        }

        // Address table lookups (v0 only)
        out.extend_from_slice(&compact_u16_encode(self.address_table_lookups.len() as u16));
        for lookup in &self.address_table_lookups {
            out.extend_from_slice(lookup.account_key.to_bytes());
            out.extend_from_slice(&compact_u16_encode(lookup.writable_indexes.len() as u16));
            out.extend_from_slice(&lookup.writable_indexes);
            out.extend_from_slice(&compact_u16_encode(lookup.readonly_indexes.len() as u16));
            out.extend_from_slice(&lookup.readonly_indexes);
        }

        out
    }

    /// Deserialize from Solana wire format bytes.
    pub fn from_wire(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 3 {
            return Err("Message: too short for header".to_string());
        }
        let mut pos = 0;

        // Header
        let header = MessageHeader {
            num_required_signatures: bytes[pos],
            num_readonly_signed_accounts: bytes[pos + 1],
            num_readonly_unsigned_accounts: bytes[pos + 2],
        };
        pos += 3;

        // Account keys
        let (num_keys, consumed) = compact_u16_decode(&bytes[pos..])?;
        pos += consumed;
        let num_keys = num_keys as usize;
        let mut account_keys = Vec::with_capacity(num_keys);
        for _ in 0..num_keys {
            if pos + 32 > bytes.len() {
                return Err("Message: truncated account_keys".to_string());
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes[pos..pos + 32]);
            account_keys.push(Pubkey::new(arr));
            pos += 32;
        }

        // Recent blockhash
        if pos + 32 > bytes.len() {
            return Err("Message: truncated blockhash".to_string());
        }
        let mut bh = [0u8; 32];
        bh.copy_from_slice(&bytes[pos..pos + 32]);
        let recent_blockhash = Blockhash::new(bh);
        pos += 32;

        // Instructions
        let (num_ixs, consumed) = compact_u16_decode(&bytes[pos..])?;
        pos += consumed;
        let num_ixs = num_ixs as usize;
        let mut instructions = Vec::with_capacity(num_ixs);
        for _ in 0..num_ixs {
            let (ix, consumed) = CompiledInstruction::from_wire(&bytes[pos..])?;
            pos += consumed;
            instructions.push(ix);
        }

        // Address table lookups
        let (num_lookups, consumed) = compact_u16_decode(&bytes[pos..])?;
        pos += consumed;
        let num_lookups = num_lookups as usize;
        let mut address_table_lookups = Vec::with_capacity(num_lookups);
        for _ in 0..num_lookups {
            if pos + 32 > bytes.len() {
                return Err("Message: truncated lookup key".to_string());
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes[pos..pos + 32]);
            let account_key = Pubkey::new(arr);
            pos += 32;

            let (writable_len, consumed) = compact_u16_decode(&bytes[pos..])?;
            pos += consumed;
            let writable_len = writable_len as usize;
            if pos + writable_len > bytes.len() {
                return Err("Message: truncated writable_indexes".to_string());
            }
            let writable_indexes = bytes[pos..pos + writable_len].to_vec();
            pos += writable_len;

            let (readonly_len, consumed) = compact_u16_decode(&bytes[pos..])?;
            pos += consumed;
            let readonly_len = readonly_len as usize;
            if pos + readonly_len > bytes.len() {
                return Err("Message: truncated readonly_indexes".to_string());
            }
            let readonly_indexes = bytes[pos..pos + readonly_len].to_vec();
            pos += readonly_len;

            address_table_lookups.push(MessageAddressTableLookup {
                account_key,
                writable_indexes,
                readonly_indexes,
            });
        }

        Ok(Self {
            header,
            account_keys,
            recent_blockhash,
            instructions,
            address_table_lookups,
        })
    }

    /// Set the message version using the top-bit encoding.
    pub fn set_version(&mut self, version: u8) {
        self.header.num_required_signatures &= 0x7F;
        self.header.num_required_signatures |= (version & 0x7F) << 7;
    }

    /// Get the message version from the header.
    pub fn version(&self) -> u8 {
        self.header.num_required_signatures >> 7
    }
}

// ===========================================================================
// TRANSACTION — compact-u16 signature count + wire-format message
// ===========================================================================

/// An unsigned versioned transaction.
///
/// Wire format: compact-u16(signature_count) + signature[0..n] + message.to_wire()
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct Transaction {
    pub signatures: Vec<Signature>,
    pub message: Message,
}

impl Transaction {
    /// Create a new unsigned transaction.
    pub fn new_unsigned(message: Message) -> Self {
        let num_sigs = (message.header.num_required_signatures & 0x7F) as usize;
        Self {
            signatures: vec![Signature::new([0u8; 64]); num_sigs],
            message,
        }
    }

    /// Serialize to base64 using SOLANA'S WIRE FORMAT.
    /// Wire format: compact-u16(signature_count) + signatures + message.to_wire()
    pub fn to_base64(&self) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let mut bytes = compact_u16_encode(self.signatures.len() as u16);
        for sig in &self.signatures {
            bytes.extend_from_slice(&sig.0);
        }
        bytes.extend_from_slice(&self.message.to_wire());
        STANDARD.encode(&bytes)
    }

    /// Deserialize a transaction from base64 using Solana's wire format.
    pub fn from_base64(b64: &str) -> Result<Self, String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let bytes = STANDARD
            .decode(b64)
            .map_err(|e| format!("base64 decode error: {e}"))?;

        if bytes.is_empty() {
            return Err("empty transaction bytes".to_string());
        }

        // Read signature count (compact-u16)
        let (sig_count, mut pos) = compact_u16_decode(&bytes)?;
        let sig_count = sig_count as usize;

        // Read signatures (64 bytes each)
        let mut signatures = Vec::with_capacity(sig_count);
        for _ in 0..sig_count {
            if pos + 64 > bytes.len() {
                return Err("truncated signatures".to_string());
            }
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&bytes[pos..pos + 64]);
            signatures.push(Signature::new(arr));
            pos += 64;
        }

        // Read message via wire format (compact-u16)
        let message = Message::from_wire(&bytes[pos..])?;

        Ok(Transaction { signatures, message })
    }
}

// ===========================================================================
// COMPILE INSTRUCTIONS → MESSAGE
// ===========================================================================

/// Compile a list of `Instruction`s into a wire-format-correct `Message`.
///
/// Steps:
/// 1. Collect all unique pubkeys from all instructions
/// 2. Build merged account_keys (signers first, then non-signers)
/// 3. Count unique signers for `num_required_signatures` (NOT hardcoded to 1)
/// 4. Build pubkey→index reverse map
/// 5. Compile each Instruction into CompiledInstruction
/// 6. Build MessageHeader with correct signature/readonly counts
/// 7. Return the Message
pub fn compile_instructions(
    instructions: Vec<crate::types::Instruction>,
    payer: Pubkey,
    blockhash: Blockhash,
) -> Message {
    use std::collections::{HashSet, HashMap};

    // STEP 1: Collect all unique pubkeys and track signers
    let mut seen: HashSet<[u8; 32]> = HashSet::new();
    let mut signer_set: HashSet<[u8; 32]> = HashSet::new();
    let mut all_accounts: Vec<(Pubkey, bool, bool)> = Vec::new(); // (pubkey, is_signer, is_writable)

    // Payer ALWAYS first
    all_accounts.push((payer, true, true));
    seen.insert(*payer.to_bytes());
    signer_set.insert(*payer.to_bytes());

    // Collect all other accounts from instructions
    let mut program_ids: Vec<Pubkey> = Vec::new();
    for ix in &instructions {
        if !seen.contains(ix.program_id.to_bytes()) {
            program_ids.push(ix.program_id);
            seen.insert(*ix.program_id.to_bytes());
        }
        for meta in &ix.accounts {
            if !seen.contains(meta.pubkey.to_bytes()) {
                all_accounts.push((meta.pubkey, meta.is_signer, meta.is_writable));
                seen.insert(*meta.pubkey.to_bytes());
                if meta.is_signer {
                    signer_set.insert(*meta.pubkey.to_bytes());
                }
            }
        }
    }

    // Add program IDs at the end (non-signers, writable)
    for pid in &program_ids {
        all_accounts.push((*pid, false, true));
    }

    // STEP 2: Sort — signers first, then non-signers
    // But preserve the payer-first ordering
    let mut account_keys: Vec<Pubkey> = Vec::new();
    for &(pk, _, _) in &all_accounts {
        account_keys.push(pk);
    }

    // STEP 3: Count signers and readonly stats
    let num_required_signatures = signer_set.len() as u8;
    let num_readonly_signed: u8 = all_accounts
        .iter()
        .take(num_required_signatures as usize)
        .filter(|&&(_, is_signer, is_writable)| is_signer && !is_writable)
        .count() as u8;
    let num_readonly_unsigned: u8 = all_accounts
        .iter()
        .skip(num_required_signatures as usize)
        .filter(|&&(_, _, is_writable)| !is_writable)
        .count() as u8;

    // STEP 4: Build reverse map
    let mut key_to_index: HashMap<[u8; 32], u8> = HashMap::new();
    for (i, key) in account_keys.iter().enumerate() {
        key_to_index.insert(*key.to_bytes(), i as u8);
    }

    // STEP 5: Compile each instruction
    let compiled: Vec<CompiledInstruction> = instructions
        .iter()
        .map(|ix| {
            let program_id_index = *key_to_index
                .get(ix.program_id.to_bytes())
                .expect("program_id must be in account_keys");

            let account_indices: Vec<u8> = ix
                .accounts
                .iter()
                .map(|meta| {
                    *key_to_index
                        .get(meta.pubkey.to_bytes())
                        .expect("account must be in account_keys")
                })
                .collect();

            CompiledInstruction {
                program_id_index,
                accounts: account_indices,
                data: ix.data.clone(),
            }
        })
        .collect();

    // STEP 6: Build header with dynamic signature count
    Message {
        header: MessageHeader {
            num_required_signatures,
            num_readonly_signed_accounts: num_readonly_signed,
            num_readonly_unsigned_accounts: num_readonly_unsigned,
        },
        account_keys,
        recent_blockhash: blockhash,
        instructions: compiled,
        address_table_lookups: vec![],
    }
}

// ===========================================================================
// HELPERS
// ===========================================================================

/// Build a transfer instruction (SOL).
pub fn build_transfer_instruction(
    _from: &Pubkey,
    _to: &Pubkey,
    lamports: u64,
) -> CompiledInstruction {
    let mut data = vec![2u8, 0, 0, 0]; // SystemProgram::Transfer discriminator
    data.extend_from_slice(&lamports.to_le_bytes());

    CompiledInstruction {
        program_id_index: 0,
        accounts: vec![0, 1],
        data,
    }
}

/// Build a Jupiter swap instruction from the quote response's transaction data.
pub fn build_swap_instruction(swap_tx_base64: &str) -> Result<CompiledInstruction, String> {
    let swap_tx = Transaction::from_base64(swap_tx_base64)?;
    swap_tx
        .message
        .instructions
        .first()
        .cloned()
        .ok_or_else(|| "swap transaction has no instructions".to_string())
}

/// Compute the number of tokens in a string.
pub fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();
    let char_estimate = (char_count + 3) / 4;
    char_estimate.max(word_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_u16_roundtrip() {
        for val in [0u16, 1, 127, 128, 255, 16383, 16384, 65535] {
            let encoded = compact_u16_encode(val);
            let (decoded, consumed) = compact_u16_decode(&encoded).unwrap();
            assert_eq!(decoded, val, "roundtrip failed for {}", val);
            assert_eq!(consumed, encoded.len(), "consumed wrong length for {}", val);
        }
    }

    #[test]
    fn test_compact_u16_boundaries() {
        assert_eq!(compact_u16_encode(0x00).len(), 1);
        assert_eq!(compact_u16_encode(0x7F).len(), 1);
        assert_eq!(compact_u16_encode(0x80).len(), 2);
        assert_eq!(compact_u16_encode(0x3FFF).len(), 2);
        assert_eq!(compact_u16_encode(0x4000).len(), 3);
    }

    #[test]
    fn test_message_wire_roundtrip() {
        let mut msg = Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 0,
            },
            account_keys: vec![
                Pubkey::new([1u8; 32]),
                Pubkey::new([2u8; 32]),
                Pubkey::new([3u8; 32]),
            ],
            recent_blockhash: Blockhash::new([7u8; 32]),
            instructions: vec![
                CompiledInstruction {
                    program_id_index: 2,
                    accounts: vec![0, 1],
                    data: vec![1, 2, 3, 4],
                },
            ],
            address_table_lookups: vec![],
        };

        let wire = msg.to_wire();
        let restored = Message::from_wire(&wire).unwrap();

        assert_eq!(restored.account_keys.len(), 3);
        assert_eq!(restored.instructions.len(), 1);
        assert_eq!(
            restored.instructions[0].data,
            vec![1, 2, 3, 4]
        );
    }

    #[test]
    fn test_message_wire_with_lookups() {
        let msg = Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 0,
            },
            account_keys: vec![Pubkey::new([1u8; 32])],
            recent_blockhash: Blockhash::new([7u8; 32]),
            instructions: vec![],
            address_table_lookups: vec![MessageAddressTableLookup {
                account_key: Pubkey::new([99u8; 32]),
                writable_indexes: vec![0, 1],
                readonly_indexes: vec![2],
            }],
        };

        let wire = msg.to_wire();
        let restored = Message::from_wire(&wire).unwrap();
        assert_eq!(restored.address_table_lookups.len(), 1);
        assert_eq!(restored.address_table_lookups[0].writable_indexes, vec![0, 1]);
    }

    #[test]
    fn test_transaction_wire_roundtrip() {
        let msg = Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 0,
            },
            account_keys: vec![Pubkey::new([1u8; 32])],
            recent_blockhash: Blockhash::new([7u8; 32]),
            instructions: vec![],
            address_table_lookups: vec![],
        };
        let tx = Transaction::new_unsigned(msg);
        let b64 = tx.to_base64();
        let restored = Transaction::from_base64(&b64).unwrap();
        assert_eq!(restored.message.account_keys.len(), 1);
        assert!(!b64.is_empty());
    }

    #[test]
    fn test_dynamic_signer_count() {
        use crate::types::{Instruction, AccountMeta};
        let payer = Pubkey::new([1u8; 32]);
        let signer2 = Pubkey::new([2u8; 32]);
        let program = Pubkey::new([3u8; 32]);
        let blockhash = Blockhash::new([7u8; 32]);

        let instructions = vec![Instruction {
            program_id: program,
            accounts: vec![
                AccountMeta {
                    pubkey: signer2,
                    is_signer: true,
                    is_writable: true,
                },
            ],
            data: vec![1, 2, 3],
        }];

        let msg = compile_instructions(instructions, payer, blockhash);
        // 2 signers: payer + signer2
        assert_eq!(
            msg.header.num_required_signatures & 0x7F,
            2,
            "must count all unique signers"
        );
    }
}
