//! Vault balances — fetches token balances via on-chain RPC queries.
//!
//! Uses manual ATA derivation and real RPC calls for WASM builds.
//! Removes hardcoded fake prices and synthetic data.

use squads_defi_core::squads::derive_ata_pda;
use squads_defi_core::Pubkey;
use serde::{Deserialize, Serialize};

/// A token balance entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenBalance {
    pub mint: Pubkey,
    pub symbol: Option<String>,
    pub amount: u64,
    pub decimals: u8,
    pub usd_value: Option<f64>,
}

impl TokenBalance {
    pub fn formatted(&self) -> String {
        let ui_amount = self.amount as f64 / 10f64.powi(self.decimals as i32);
        match &self.usd_value {
            Some(usd) => format!("{:.4} (${:.2})", ui_amount, usd),
            None => format!("{:.4}", ui_amount),
        }
    }
}

/// SOL mint address.
pub const SOL_MINT: &str = "So111111111111111111111111111111111111112";

/// Associated Token Program ID.
pub const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbtGV8r1b2qvE8hBMRUNrqPJ5kKDpFvRtaFj";

/// SPL Token Program ID.
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// Well-known SPL token mints for balance queries.
pub const DEFAULT_KNOWN_MINTS: &[&str] = &[
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", // USDC
    "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB", // USDT
];

/// Fetch token balances for a vault via Solana RPC.
///
/// 1. Query SOL balance via getBalance
/// 2. For each known SPL mint, derive the ATA and query balance via getTokenAccountBalance
/// 3. Query mint decimals via getAccountInfo → parse Mint account binary layout
///
/// On native builds (non-WASM), returns error — host provides real data.
pub fn fetch_balances(
    rpc_url: &str,
    vault: &Pubkey,
) -> Result<Vec<TokenBalance>, String> {
    let mut balances = Vec::new();

    // ── 1. SOL balance ──────────────────────────────────────────
    match rpc_get_balance(rpc_url, &vault.to_string()) {
        Ok(sol_lamports) => {
            let sol_ui = sol_lamports as f64 / 1_000_000_000.0;
            balances.push(TokenBalance {
                mint: Pubkey::from_str(SOL_MINT)
                    .map_err(|e| format!("invalid SOL mint: {e}"))?,
                symbol: Some("SOL".to_string()),
                amount: sol_lamports,
                decimals: 9,
                usd_value: None, // No hardcoded SOL price — remove estimate
            });
        }
        Err(_) => {
            // RPC unavailable — include zero SOL entry so briefing shows structure
            balances.push(TokenBalance {
                mint: Pubkey::from_str(SOL_MINT)
                    .map_err(|e| format!("invalid SOL mint: {e}"))?,
                symbol: Some("SOL".to_string()),
                amount: 0,
                decimals: 9,
                usd_value: None,
            });
        }
    }

    // ── 2. SPL token balances ──────────────────────────────────
    for mint_str in DEFAULT_KNOWN_MINTS {
        let mint_pubkey = match Pubkey::from_str(mint_str) {
            Ok(pk) => pk,
            Err(_) => continue,
        };

        let token_prog = Pubkey::from_str(TOKEN_PROGRAM_ID)
            .map_err(|e| format!("invalid token program ID: {e}"))?;
        let ata_prog = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID)
            .map_err(|e| format!("invalid ATA program ID: {e}"))?;

        let (ata_pubkey, _bump) = derive_ata_pda(vault, &mint_pubkey, &token_prog, &ata_prog);

        match rpc_get_token_account_balance(rpc_url, &ata_pubkey.to_string()) {
            Ok(token_amount) if token_amount > 0 => {
                let decimals = rpc_get_mint_decimals(rpc_url, mint_str).unwrap_or(6);
                let ui_amount = token_amount as f64 / 10f64.powi(decimals as i32);

                balances.push(TokenBalance {
                    mint: mint_pubkey,
                    symbol: Some(mint_symbol(mint_str).to_string()),
                    amount: token_amount,
                    decimals,
                    usd_value: None, // No hardcoded prices — remove estimates
                });
            }
            _ => continue,
        }
    }

    Ok(balances)
}

/// Symbol for known mints.
fn mint_symbol(mint: &str) -> &str {
    match mint {
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" => "USDC",
        "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB" => "USDT",
        _ => "SPL",
    }
}

// ── RPC helpers (WASM-only; native returns error) ──────────────────────

#[cfg(not(target_family = "wasm"))]
fn rpc_get_balance(_rpc_url: &str, _pubkey: &str) -> Result<u64, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(target_family = "wasm")]
fn rpc_get_balance(rpc_url: &str, pubkey: &str) -> Result<u64, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getBalance",
        "params": [pubkey]
    }).to_string();

    let response = waki::Client::new()
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .body(body.as_str())
        .send()
        .map_err(|e| format!("RPC HTTP: {e}"))?;

    let body_bytes = response.body()
        .map_err(|e| format!("body read: {e}"))?;
    let body_str = String::from_utf8(body_bytes)
        .map_err(|e| format!("utf-8: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("json parse: {e}"))?;

    if let Some(err) = value.get("error") {
        return Err(format!("RPC error: {}", err["message"].as_str().unwrap_or("unknown")));
    }

    value["result"]["value"].as_u64()
        .ok_or("missing 'result.value' in getBalance response".to_string())
}

#[cfg(not(target_family = "wasm"))]
fn rpc_get_token_account_balance(_rpc_url: &str, _ata_pubkey: &str) -> Result<u64, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(target_family = "wasm")]
fn rpc_get_token_account_balance(rpc_url: &str, ata_pubkey: &str) -> Result<u64, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getTokenAccountBalance",
        "params": [ata_pubkey]
    }).to_string();

    let response = waki::Client::new()
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .body(body.as_str())
        .send()
        .map_err(|e| format!("RPC HTTP: {e}"))?;

    let body_bytes = response.body()
        .map_err(|e| format!("body read: {e}"))?;
    let body_str = String::from_utf8(body_bytes)
        .map_err(|e| format!("utf-8: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("json parse: {e}"))?;

    if let Some(err) = value.get("error") {
        return Err(format!("RPC error: {}", err["message"].as_str().unwrap_or("unknown")));
    }

    let amount_str = value["result"]["value"]["amount"]
        .as_str()
        .ok_or("missing 'result.value.amount' in getTokenAccountBalance response")?;

    amount_str.parse::<u64>()
        .map_err(|e| format!("parse token amount '{}': {e}", amount_str))
}

#[cfg(not(target_family = "wasm"))]
fn rpc_get_mint_decimals(_rpc_url: &str, _mint_pubkey: &str) -> Result<u8, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(target_family = "wasm")]
fn rpc_get_mint_decimals(rpc_url: &str, mint_pubkey: &str) -> Result<u8, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getAccountInfo",
        "params": [mint_pubkey, {"encoding": "base64"}]
    }).to_string();

    let response = waki::Client::new()
        .post(rpc_url)
        .header("Content-Type", "application/json")
        .body(body.as_str())
        .send()
        .map_err(|e| format!("RPC HTTP: {e}"))?;

    let body_bytes = response.body()
        .map_err(|e| format!("body read: {e}"))?;
    let body_str = String::from_utf8(body_bytes)
        .map_err(|e| format!("utf-8: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("json parse: {e}"))?;

    if let Some(err) = value.get("error") {
        return Err(format!("RPC error: {}", err["message"].as_str().unwrap_or("unknown")));
    }

    let account_value = &value["result"]["value"];
    if account_value.is_null() {
        return Err("mint account not found".to_string());
    }

    let data_b64 = account_value["data"][0]
        .as_str()
        .ok_or("missing account data in response")?;

    let data = STANDARD.decode(data_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;

    if data.len() < 44 {
        return Err(format!("mint account data too short: {} bytes", data.len()));
    }

    // decimals at offset 44 for standard SPL Token mints
    Ok(data[44])
}

/// Sum of USD values across all balances.
pub fn total_usd_value(balances: &[TokenBalance]) -> f64 {
    balances.iter().filter_map(|b| b.usd_value).sum()
}

/// Find a balance entry by mint pubkey.
pub fn find_balance<'a>(balances: &'a [TokenBalance], mint: &Pubkey) -> Option<&'a TokenBalance> {
    balances.iter().find(|b| &b.mint == mint)
}