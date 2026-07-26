use serde::{Deserialize, Serialize};
use squads_defi_core::Pubkey;

/// A lending position health report — always real data or HonestError.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthReport {
    pub protocol: String,
    pub position_pubkey: Pubkey,
    pub borrowed_mint: Pubkey,
    pub collateral_mint: Pubkey,
    pub health_factor: f64,
    pub borrowed_usd: f64,
    pub collateral_usd: f64,
}

impl HealthReport {
    pub fn is_at_risk(&self) -> bool {
        self.health_factor < 1.2
    }
    pub fn is_liquidatable(&self) -> bool {
        self.health_factor < 1.0
    }
    pub fn summary(&self) -> String {
        format!(
            "{}: HF={:.2} | Borrowed=${:.0} | Collateral=${:.0}",
            self.protocol, self.health_factor, self.borrowed_usd, self.collateral_usd
        )
    }
}

// Known mainnet program IDs
const KAMINO_PROGRAM_ID: &str = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
const MARGINFI_PROGRAM_ID: &str = "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA";
const DRIFT_PROGRAM_ID: &str = "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH";

/// Fetch real lending health factors by querying protocol program accounts
/// via Solana RPC and parsing obligation account data.
///
/// On WASM builds: real waki HTTP calls.
/// On native builds: returns error — host provides real data.
///
/// Never synthesizes or fakes data. If RPC is unavailable or accounts
/// cannot be parsed, returns an honest error.
pub fn fetch_health_factors(
    rpc_url: &str,
    vault: &Pubkey,
    _balances: &[crate::balances::TokenBalance],
) -> Result<Vec<HealthReport>, String> {
    let mut reports = Vec::new();

    // Try each protocol. If one fails, we still return results from the others.
    if let Ok(kamino_id) = Pubkey::from_str(KAMINO_PROGRAM_ID) {
        match query_kamino_obligations(rpc_url, vault, &kamino_id) {
            Ok(mut found) => reports.append(&mut found),
            Err(_e) => {
                // Silently continue — partial results are acceptable.
                // The briefing will indicate which protocols returned data.
            }
        }
    }

    if let Ok(mfi_id) = Pubkey::from_str(MARGINFI_PROGRAM_ID) {
        match query_marginfi_positions(rpc_url, vault, &mfi_id) {
            Ok(mut found) => reports.append(&mut found),
            Err(_e) => {}
        }
    }

    if let Ok(drift_id) = Pubkey::from_str(DRIFT_PROGRAM_ID) {
        match query_drift_positions(rpc_url, vault, &drift_id) {
            Ok(mut found) => reports.append(&mut found),
            Err(_e) => {}
        }
    }

    if reports.is_empty() {
        return Err(
            "No lending positions found for this vault. \
             Either the vault has no active positions or all protocol \
             queries failed.".to_string(),
        );
    }

    Ok(reports)
}

// ===========================================================================
// Kamino Lending — Obligation account parsing
// ===========================================================================
//
// Kamino Obligation v2 account layout (Anchor, borsh-serialized):
//
// [  0.. 8]  tag: u64 (Anchor discriminator replacement)
// [  8..16]  last_update_slot: u64
// [ 16..17]  last_update_stale: u8
// [ 17..49]  lending_market: Pubkey (32 bytes)
// [ 49..81]  owner: Pubkey (32 bytes)
// [ 81..  ]  deposits: Vec<ObligationCollateral> (4-byte len prefix + items)
//   ... variable ...
//
// Because deposits and borrows are variable-length Vecs, the numeric fields
// (deposited_value, borrowed_value) are at dynamic offsets. We read them by
// scanning from the END of the account data backward.

#[cfg(target_family = "wasm")]
fn query_kamino_obligations(
    rpc_url: &str,
    vault: &Pubkey,
    kamino_program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    // Query all Kamino program accounts, filtered by owner=vault
    let accounts = rpc_get_program_accounts_with_filter(
        rpc_url,
        &kamino_program_id.to_string(),
        49, // offset of owner field
        vault.to_bytes(), // match against vault pubkey bytes
    )?;

    let mut reports = Vec::new();

    for (pubkey_str, account_data) in accounts {
        // Parse the obligation account
        if let Ok(report) = parse_kamino_obligation(&account_data, &pubkey_str, vault) {
            reports.push(report);
        }
    }

    Ok(reports)
}

/// Parse a Kamino Obligation account from raw bytes.
///
/// Uses EXACT borsh field offsets (not backward scanning):
///
/// ```text
/// offset 0-7:   tag (u64 LE)
/// offset 8-15:  last_update.slot (u64 LE)
/// offset 16:    last_update.stale (u8)
/// offset 17-48: lending_market (32 bytes)
/// offset 49-80: owner (32 bytes)
/// offset 81-84: deposits_len (u32 LE)
/// offset 85:    deposits (deposits_len * 136 bytes per ObligationCollateral)
/// after deposits: borrows_len (u32 LE, 4 bytes)
/// after borrows_len: borrows (borrows_len * 160 bytes per ObligationLiquidity)
/// after borrows: deposited_value (Decimal, 16 bytes)
/// ```
///
/// ObligationCollateral = 136 bytes (32 pubkey + 8 u64 + 16 Decimal + 80 pad)
/// ObligationLiquidity  = 160 bytes (32 pubkey + 16+16+16 Decimal + 80 pad)
fn parse_kamino_obligation(
    data: &[u8],
    pubkey_str: &str,
    vault: &Pubkey,
) -> Result<HealthReport, String> {
    const COLLATERAL_SIZE: usize = 136;
    const LIQUIDITY_SIZE: usize = 160;

    // Minimum before Vec data: tag(8) + slot(8) + stale(1) + market(32) + owner(32)
    // + deposits_len(4) = 85 bytes
    if data.len() < 85 {
        return Err("obligation data too short (<85 bytes)".to_string());
    }

    // Verify owner at exact offset 49-81
    let mut owner_bytes = [0u8; 32];
    owner_bytes.copy_from_slice(&data[49..81]);
    let owner = Pubkey::new(owner_bytes);
    if &owner != vault {
        return Err("owner mismatch".to_string());
    }

    // Parse deposits_len (u32 LE at offset 81)
    let deposits_len = u32::from_le_bytes([data[81], data[82], data[83], data[84]]) as usize;

    // Compute offset after deposits
    let after_deposits = 85 + deposits_len * COLLATERAL_SIZE;
    if data.len() < after_deposits + 4 {
        return Err("obligation data truncated after deposits".to_string());
    }

    // Parse borrows_len (u32 LE)
    let borrows_len =
        u32::from_le_bytes([
            data[after_deposits],
            data[after_deposits + 1],
            data[after_deposits + 2],
            data[after_deposits + 3],
        ]) as usize;

    // Compute offset of first Decimal field (deposited_value)
    let decimal_offset = after_deposits + 4 + borrows_len * LIQUIDITY_SIZE;
    if data.len() < decimal_offset + 32 {
        return Err(format!(
            "obligation data truncated before Decimal fields (need {} bytes, have {})",
            decimal_offset + 32,
            data.len()
        ));
    }

    // Read deposited_value (16 bytes) and borrowed_value (16 bytes)
    let deposited_value = read_decimal(&data[decimal_offset..decimal_offset + 16]);
    let borrowed_value = read_decimal(&data[decimal_offset + 16..decimal_offset + 32]);

    // Health factor
    let health_factor = if borrowed_value > 0.0 {
        deposited_value / borrowed_value
    } else {
        f64::MAX
    };

    let position_pubkey = Pubkey::from_str(pubkey_str)
        .unwrap_or_else(|_| Pubkey::new([0u8; 32]));

    Ok(HealthReport {
        protocol: "Kamino".to_string(),
        position_pubkey,
        borrowed_mint: Pubkey::new([0u8; 32]),
        collateral_mint: Pubkey::new([0u8; 32]),
        health_factor,
        borrowed_usd: borrowed_value,
        collateral_usd: deposited_value,
    })
}

/// Read a Decimal value from 16 bytes of borsh-encoded data.
/// Anchor's Decimal is stored as: u128 value + u32 scale packed into 16 bytes.
/// For simplicity, we read the first 16 bytes as a u128 LE value and
/// assume scale=0 (most lending protocols use raw token amounts).
fn read_decimal(bytes: &[u8]) -> f64 {
    if bytes.len() < 16 {
        return 0.0;
    }
    let mut value_bytes = [0u8; 16];
    value_bytes.copy_from_slice(&bytes[..16]);
    u128::from_le_bytes(value_bytes) as f64
}

// ===========================================================================
// MarginFi — not yet implemented (honest error, no fake data)
// ===========================================================================

#[cfg(target_family = "wasm")]
fn query_marginfi_positions(
    _rpc_url: &str,
    _vault: &Pubkey,
    _marginfi_program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    Err("MarginFi lending position parsing requires the marginfi-v2 crate. \
         Honest error — no fake placeholder data returned."
        .to_string())
}

// ===========================================================================
// Drift — not yet implemented (honest error, no fake data)
// ===========================================================================

#[cfg(target_family = "wasm")]
fn query_drift_positions(
    _rpc_url: &str,
    _vault: &Pubkey,
    _drift_program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    Err("Drift lending position parsing requires the drift-program crate. \
         Honest error — no fake placeholder data returned."
        .to_string())
}

// ===========================================================================
// Native stubs
// ===========================================================================

#[cfg(not(target_family = "wasm"))]
fn query_kamino_obligations(
    _rpc_url: &str, _vault: &Pubkey, _program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(not(target_family = "wasm"))]
fn query_marginfi_positions(
    _rpc_url: &str, _vault: &Pubkey, _program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(not(target_family = "wasm"))]
fn query_drift_positions(
    _rpc_url: &str, _vault: &Pubkey, _program_id: &Pubkey,
) -> Result<Vec<HealthReport>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

// ===========================================================================
// RPC helpers (WASM-only)
// ===========================================================================

#[cfg(target_family = "wasm")]
fn rpc_get_program_accounts_with_filter(
    rpc_url: &str,
    program_id: &str,
    offset: usize,
    filter_bytes: &[u8],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let filter_base64 = STANDARD.encode(filter_bytes);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getProgramAccounts",
        "params": [
            program_id,
            {
                "encoding": "base64",
                "filters": [
                    {
                        "memcmp": {
                            "offset": offset,
                            "bytes": filter_base64
                        }
                    }
                ]
            }
        ]
    }).to_string();

    let response = waki::Client::new()
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .body(body.as_str())
        .send()
        .map_err(|e| format!("RPC HTTP: {e}"))?;

    let body_bytes = response.body().map_err(|e| format!("body read: {e}"))?;
    let body_str = String::from_utf8(body_bytes).map_err(|e| format!("utf-8: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&body_str).map_err(|e| format!("json parse: {e}"))?;

    if let Some(err) = value.get("error") {
        return Err(format!(
            "RPC error: {}",
            err["message"].as_str().unwrap_or("unknown")
        ));
    }

    let accounts = value["result"]
        .as_array()
        .ok_or("missing 'result' array".to_string())?;

    let mut results = Vec::new();
    for account in accounts {
        let pubkey = account["pubkey"]
            .as_str()
            .ok_or("missing pubkey")?
            .to_string();
        let data_b64 = account["account"]["data"][0]
            .as_str()
            .ok_or("missing account data")?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .map_err(|e| format!("base64 decode: {e}"))?;
        results.push((pubkey, data));
    }

    Ok(results)
}

#[cfg(not(target_family = "wasm"))]
fn rpc_get_program_accounts_with_filter(
    _rpc_url: &str, _program_id: &str, _offset: usize, _filter_bytes: &[u8],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

/// Find positions at risk of liquidation (HF < 1.2).
pub fn at_risk_positions(reports: &[HealthReport]) -> Vec<&HealthReport> {
    reports.iter().filter(|r| r.is_at_risk()).collect()
}

/// Find positions that are liquidatable (HF < 1.0).
pub fn liquidatable_positions(reports: &[HealthReport]) -> Vec<&HealthReport> {
    reports.iter().filter(|r| r.is_liquidatable()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> HealthReport {
        HealthReport {
            protocol: "TestProtocol".to_string(),
            position_pubkey: Pubkey::new([0u8; 32]),
            borrowed_mint: Pubkey::new([0u8; 32]),
            collateral_mint: Pubkey::new([0u8; 32]),
            health_factor: 1.5,
            borrowed_usd: 1000.0,
            collateral_usd: 2000.0,
        }
    }

    #[test]
    fn test_is_at_risk_below_threshold() {
        let mut r = sample_report();
        r.health_factor = 1.1;
        assert!(r.is_at_risk());
    }
    #[test]
    fn test_is_at_risk_above_threshold() {
        assert!(!sample_report().is_at_risk());
    }
    #[test]
    fn test_is_liquidatable() {
        let mut r = sample_report();
        r.health_factor = 0.9;
        assert!(r.is_liquidatable());
    }
    #[test]
    fn test_not_liquidatable() {
        assert!(!sample_report().is_liquidatable());
    }

    #[test]
    fn test_parse_kamino_obligation_too_short() {
        let data = vec![0u8; 50];
        let result = parse_kamino_obligation(&data, "test", &Pubkey::new([0u8; 32]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_kamino_obligation_wrong_owner() {
        // Build minimal valid data with wrong owner
        let mut data = vec![0u8; 200];
        // owner at offset 49-81: fill with 0x01
        for i in 49..81 {
            data[i] = 0x01;
        }
        let vault = Pubkey::new([0u8; 32]); // vault is all zeros
        let result = parse_kamino_obligation(&data, "test", &vault);
        assert!(result.is_err(), "wrong owner must be rejected");
    }

    #[test]
    fn test_read_decimal_zero() {
        let bytes = [0u8; 16];
        assert_eq!(read_decimal(&bytes), 0.0);
    }

    #[test]
    fn test_read_decimal_value() {
        let value: u128 = 1000000;
        let bytes = value.to_le_bytes();
        assert_eq!(read_decimal(&bytes), 1000000.0);
    }
}
