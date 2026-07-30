//! Build unsigned swap transactions from Jupiter quotes.

use squads_defi_core::jupiter::{Quote, SwapInstructionsResponse, SwapInstructionData};
use squads_defi_core::tx::{Transaction, compile_instructions};
use squads_defi_core::{Blockhash, Pubkey, Instruction, AccountMeta};
use base64::{engine::general_purpose::STANDARD, Engine};

use crate::error::PluginError;

/// Build an unsigned swap transaction from a Jupiter quote (legacy path).
pub fn build_swap_transaction(
    quote: &Quote,
    user_wallet: &Pubkey,
    blockhash: Blockhash,
) -> Result<Transaction, PluginError> {
    let swap_tx_b64 = quote
        .swap_transaction
        .as_ref()
        .ok_or_else(|| PluginError::Swap("quote has no swap transaction data".to_string()))?;

    let swap_tx = Transaction::from_base64(swap_tx_b64)
        .map_err(|e| PluginError::Swap(format!("failed to decode swap tx: {e}")))?;

    if swap_tx.message.instructions.is_empty() {
        return Err(PluginError::Swap(
            "decoded swap transaction has no instructions".to_string(),
        ));
    }

    let total_keys = swap_tx.message.account_keys.len();
    let num_sigs = (swap_tx.message.header.num_required_signatures & 0x7F) as usize;
    let num_ro_signed = swap_tx.message.header.num_readonly_signed_accounts as usize;
    let num_ro_unsigned = swap_tx.message.header.num_readonly_unsigned_accounts as usize;

    let is_writable = |idx: u8| -> bool {
        let i = idx as usize;
        if i >= total_keys { return false; }
        if i >= num_sigs.saturating_sub(num_ro_signed) && i < num_sigs { return false; }
        if i >= total_keys.saturating_sub(num_ro_unsigned) { return false; }
        true
    };

    let is_signer = |idx: u8| -> bool { (idx as usize) < num_sigs };

    let mut instructions: Vec<Instruction> = Vec::new();
    for ci in &swap_tx.message.instructions {
        let accounts: Vec<AccountMeta> = ci.accounts.iter().map(|&idx| {
            let pk = swap_tx.message.account_keys.get(idx as usize)
                .copied()
                .unwrap_or_else(|| Pubkey::new([0u8; 32]));
            AccountMeta {
                pubkey: pk,
                is_signer: is_signer(idx),
                is_writable: is_writable(idx),
            }
        }).collect();

        let program_id = swap_tx.message.account_keys.get(ci.program_id_index as usize)
            .copied()
            .unwrap_or_else(|| Pubkey::new([0u8; 32]));

        instructions.push(Instruction { program_id, accounts, data: ci.data.clone() });
    }

    let message = compile_instructions(instructions, *user_wallet, blockhash);
    Ok(Transaction::new_unsigned(message))
}

/// Build a real unsigned swap transaction from Jupiter's /swap-instructions response.
///
/// The host calls Jupiter's /swap-instructions endpoint with the quote response,
/// receives the actual executable instruction data, and passes the parsed
/// SwapInstructionsResponse to the plugin.
///
/// This function:
/// 1. Converts each Jupiter SwapInstructionData into our Instruction type
/// 2. Orders them: compute_budget → setup → swap → cleanup
/// 3. Compiles them into a versioned transaction message
/// 4. Returns base64-encoded unsigned transaction
pub fn build_real_swap_tx(
    swap_instructions: &SwapInstructionsResponse,
    wallet_pubkey: &Pubkey,
    blockhash: &Blockhash,
) -> Result<String, PluginError> {
    let mut instructions: Vec<Instruction> = Vec::new();

    // Helper: convert Jupiter's SwapInstructionData → our Instruction
    let convert = |sd: &SwapInstructionData| -> Result<Instruction, PluginError> {
        let program_id = Pubkey::from_str(&sd.program_id)
            .map_err(|e| PluginError::Swap(format!(
                "invalid program_id '{}': {}", sd.program_id, e
            )))?;

        let data = STANDARD.decode(&sd.data)
            .map_err(|e| PluginError::Swap(format!(
                "base64 decode failed for program {}: {}", sd.program_id, e
            )))?;

        let accounts: Vec<AccountMeta> = sd.accounts.iter().map(|a| {
            AccountMeta {
                pubkey: Pubkey::from_str(&a.pubkey)
                    .unwrap_or_else(|_| Pubkey::new([0u8; 32])),
                is_signer: a.is_signer,
                is_writable: a.is_writable,
            }
        }).collect();

        Ok(Instruction { program_id, accounts, data })
    };

    // ORDER MATTERS:
    // 1. Compute budget instructions (set CU limit + price) — must be FIRST
    if let Some(ref cbis) = swap_instructions.compute_budget_instructions {
        for cb in cbis {
            instructions.push(convert(cb)?);
        }
    }

    // 2. Setup instructions (create ATAs, wrap SOL into wSOL, etc.)
    for si in &swap_instructions.setup_instructions {
        instructions.push(convert(si)?);
    }

    // 3. The main swap instruction (Jupiter program invocation)
    instructions.push(convert(&swap_instructions.swap_instruction)?);

    // 4. Cleanup instruction (unwrap wSOL, close ATAs) — if present
    if let Some(ref ci) = swap_instructions.cleanup_instruction {
        instructions.push(convert(ci)?);
    }

    // 5. Compile into a versioned transaction message
    let message = compile_instructions(instructions, *wallet_pubkey, *blockhash);
    let tx = Transaction::new_unsigned(message);
    Ok(tx.to_base64())
}
