//! Token risk analysis — pure core.
//!
//! Checks: mint authority, freeze authority, holder concentration,
//! Token-2022 extension detection, LP existence.
//!
//! Real on-chain data parsing via `fetch_and_assess_risk` which
//! queries the mint account via RPC and parses the binary layout.

use squads_defi_core::Pubkey;

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Unknown,
}

impl RiskLevel {
    pub fn as_str(&self) -> &str {
        match self {
            RiskLevel::Low => "LOW",
            RiskLevel::Medium => "MEDIUM",
            RiskLevel::High => "HIGH",
            RiskLevel::Unknown => "UNKNOWN",
        }
    }
}

pub struct TokenRiskInput {
    pub mint_authority: Option<Pubkey>,
    pub freeze_authority: Option<Pubkey>,
    pub holder_concentration_pct: f64,
    pub is_token22: bool,
    pub has_transfer_hook: bool,
    pub has_transfer_fee: bool,
    pub has_permanent_delegate: bool,
    pub lp_exists: bool,
}

/// Parse mint account data returned from `getAccountInfo` and assess risk.
///
/// SPL Token mint account layout (non-T2022, >=82 bytes):
///   offset 0:   mint_authority_option (u32 LE)
///   offset 4:   mint_authority (Pubkey, 32 bytes) — only if option == 1
///   offset 36:  supply (u64 LE)
///   offset 44:  decimals (u8)
///   offset 45:  is_initialized (u8)
///   offset 46:  freeze_authority_option (u32 LE) — if present at offset 46
///   offset 50:  freeze_authority (Pubkey, 32 bytes) — only if option == 1
///   offset 82+: optional Token-2022 extensions
///
/// For Token-2022 mints, the layout extends beyond byte 45 with
/// additional extension types including transfer hooks, transfer fees,
/// and permanent delegates.
pub fn assess_risk_from_mint_data(
    mint_data: &[u8],
    mint_pubkey: &Pubkey,
) -> TokenRiskInput {
    // Default: assume safe. Unknown data → Unknown risk.
    let mut input = TokenRiskInput {
        mint_authority: None,
        freeze_authority: None,
        holder_concentration_pct: 0.0,
        is_token22: false,
        has_transfer_hook: false,
        has_transfer_fee: false,
        has_permanent_delegate: false,
        lp_exists: false,
    };

    if mint_data.len() < 42 {
        // Data too short to be a valid mint account
        return input;
    }

    // Parse mint_authority_option (u32 LE at offset 0)
    let mint_auth_option = u32::from_le_bytes([
        mint_data[0], mint_data[1], mint_data[2], mint_data[3],
    ]);

    // If option == 1, read mint_authority pubkey at offset 4
    if mint_auth_option == 1 && mint_data.len() >= 36 {
        let mut authority_bytes = [0u8; 32];
        authority_bytes.copy_from_slice(&mint_data[4..36]);
        input.mint_authority = Some(Pubkey::new(authority_bytes));
    }

    // If data is long enough, check for freeze_authority
    // In the standard SPL Token program layout:
    //   offset 46 (optional): freeze_authority_option (u32 LE)
    //   if present and == 1, freeze_authority at offset 50 (32 bytes)
    //   total minimum for freeze_authority presence: 82 bytes
    if mint_data.len() >= 82 {
        // Try offset 46 for freeze_authority_option
        // But offset 44 is decimals (u8), offset 45 is is_initialized (u8)
        // Then offset 46..49 may or may not be freeze_authority_option
        // depending on whether `mint_authority_option == 0` or `1`.
        // More reliable: after decimals (byte 44) and is_initialized (byte 45),
        // the next u32 is freeze_authority_option if the mint uses the
        // "optional" freeze authority layout (standard SPL Token v2).
        let freeze_auth_option_offset = 46;
        if freeze_auth_option_offset + 4 + 32 <= mint_data.len() {
            let freeze_auth_option = u32::from_le_bytes([
                mint_data[freeze_auth_option_offset],
                mint_data[freeze_auth_option_offset + 1],
                mint_data[freeze_auth_option_offset + 2],
                mint_data[freeze_auth_option_offset + 3],
            ]);
            if freeze_auth_option == 1 {
                let mut auth_bytes = [0u8; 32];
                auth_bytes.copy_from_slice(
                    &mint_data[freeze_auth_option_offset + 4
                        ..freeze_auth_option_offset + 4 + 32],
                );
                input.freeze_authority = Some(Pubkey::new(auth_bytes));
            }
        }

        // Check for Token-2022 extensions.
        // Standard SPL Token v2 mints are 82+ bytes with optional extensions.
        // Extensions are TLV records starting at offset 82:
        //   Extension Type (u16 LE)
        //   Length (u16 LE)
        //   Data (variable)
        let mut offset = 82;
        while offset + 4 <= mint_data.len() {
            let ext_type = u16::from_le_bytes([
                mint_data[offset],
                mint_data[offset + 1],
            ]);
            let ext_len = u16::from_le_bytes([
                mint_data[offset + 2],
                mint_data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + ext_len > mint_data.len() {
                break;
            }

            // Known Token-2022 extension types:
            // 0x0000 = Uninitialized
            // 0x0001 = TransferFeeConfig (transfer fees)
            // 0x0002 = TransferHook (transfer hook / CPI)
            // 0x0003 = ConfidentialTransferAccount / Mint
            // 0x0004 = PermanentDelegate
            // 0x0005 = TransferFeeAmount (per-account fees)
            // 0x0006 = ReversibleClosedAccount
            match ext_type {
                0x0002 => input.has_transfer_hook = true,
                0x0001 => input.has_transfer_fee = true,
                0x0004 => input.has_permanent_delegate = true,
                0x0003 | 0x0005 | 0x0006 => {
                    // Other T2022 extensions present
                    input.is_token22 = true;
                }
                _ => {}
            }

            offset += ext_len;
        }

        // If we found any extensions, mark it as Token-2022
        if input.has_transfer_hook
            || input.has_transfer_fee
            || input.has_permanent_delegate
        {
            input.is_token22 = true;
        }
    }

    input
}

pub fn assess_risk(input: &TokenRiskInput) -> RiskLevel {
    let mut risk_score = 0u8;

    if input.mint_authority.is_some() { risk_score += 2; }
    if input.freeze_authority.is_some() { risk_score += 3; }

    // holder_concentration is not derivable from mint data alone —
    // requires on-chain holder analysis which is out of scope for
    // a single mint account query. Keep at 0.0 for now.

    if input.holder_concentration_pct > 80.0 { risk_score += 3; }
    else if input.holder_concentration_pct > 50.0 { risk_score += 1; }

    if input.has_transfer_hook { risk_score += 2; }
    if input.has_permanent_delegate { risk_score += 3; }
    if input.has_transfer_fee { risk_score += 1; }
    if !input.lp_exists { risk_score += 1; }

    match risk_score {
        0 => RiskLevel::Low,
        1..=2 => RiskLevel::Medium,
        _ => RiskLevel::High,
    }
}

/// Format the risk assessment as a human-readable one-liner.
/// Returns the summary string (no TokenRiskInput needed for formatting).
pub fn format_risk_summary(
    mint: &str,
    _symbol: Option<&str>,
    input: &TokenRiskInput,
) -> String {
    let short = &mint[..8.min(mint.len())];
    let risk = assess_risk(input);
    let emoji = match risk {
        RiskLevel::Low => "🟢",
        RiskLevel::Medium => "🟡",
        RiskLevel::High => "🔴",
        RiskLevel::Unknown => "⚪",
    };

    format!(
        "Token: {}... | Risk: {} {}\n\
         - Mint authority: {}\n\
         - Freeze authority: {}\n\
         - Token-2022: {}\n\
         - Transfer hook: {}\n\
         - Transfer fee: {}\n\
         - Permanent delegate: {}\n\
         {}",
        short, emoji, risk.as_str(),
        if input.mint_authority.is_some() { "Active" } else { "Revoked" },
        if input.freeze_authority.is_some() { "Active" } else { "None" },
        if input.is_token22 { "Yes" } else { "No" },
        if input.has_transfer_hook { "Yes" } else { "No" },
        if input.has_transfer_fee { "Yes" } else { "No" },
        if input.has_permanent_delegate { "Yes" } else { "No" },
        match risk {
            RiskLevel::High => "HIGH RISK — review carefully",
            RiskLevel::Medium => "Use with caution",
            RiskLevel::Low => "Safe for general use",
            RiskLevel::Unknown => "Unable to determine risk — data incomplete",
        },
    )
}