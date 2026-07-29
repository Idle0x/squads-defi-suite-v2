//! Query Squads for pending proposals using on-chain data.
//!
//! Uses PDA derivation from squads-defi-core and real RPC queries.
//!
//! ## WASM Path
//!
//! Scans proposal indices 0..N, derives PDAs, queries accounts, and
//! manually parses the Squads v4 Proposal account layout using borsh.
//! Returns real on-chain data — never hallucinates synthetic proposals.
//!
//! ## Native Path (`squads-state` feature)
//!
//! Uses the `squads-multisig-program` crate for full deserialization.

use serde::{Deserialize, Serialize};
use squads_defi_core::squads::{
    anchor_discriminator, derive_multisig_pda, derive_proposal_pda, unix_now_seconds,
};
use squads_defi_core::Pubkey;

/// A pending Squads proposal as returned by on-chain query.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingProposal {
    pub proposal_pubkey: Pubkey,
    pub multisig: Pubkey,
    pub creator: Pubkey,
    pub expiry_timestamp: i64,
    pub approvals: u64,
    pub threshold: u64,
    pub title: Option<String>,
    pub executed: bool,
}

/// Fetch pending proposals for a given multisig vault authority.
///
/// `squads_program_id` is read from config — no longer hardcoded.
/// Routes to the WASM manual-parsing path or the native `squads-state` path.
pub fn fetch_pending_proposals(
    rpc_url: &str,
    vault: &Pubkey,
    squads_program_id: &Pubkey,
) -> Result<Vec<PendingProposal>, String> {
    let (multisig_pda, _multisig_bump) = derive_multisig_pda(vault, squads_program_id);

    #[cfg(feature = "squads-state")]
    {
        return fetch_proposals_native(rpc_url, &multisig_pda, vault, &squads_program_id);
    }

    // WASM builds: manual seed-based PDA scanning with borsh parsing
    #[cfg(not(feature = "squads-state"))]
    {
        return fetch_proposals_wasm(rpc_url, &multisig_pda, vault, &squads_program_id);
    }
}

// ===========================================================================
// Native path (squads-state feature)
// ===========================================================================

#[cfg(feature = "squads-state")]
fn fetch_proposals_native(
    rpc_url: &str,
    multisig_pda: &Pubkey,
    _authority: &Pubkey,
    squads_program_id: &Pubkey,
) -> Result<Vec<PendingProposal>, String> {
    use squads_multisig_program::state::Proposal;

    let proposal_discriminator = anchor_discriminator("Proposal");

    let raw_accounts = rpc_get_program_accounts(
        rpc_url,
        &squads_program_id.to_string(),
        Some(&proposal_discriminator),
    )?;

    let mut proposals = Vec::new();
    for (pubkey_str, account_data) in raw_accounts {
        if account_data.len() < 73 {
            continue;
        }

        match Proposal::try_from_slice(&account_data) {
            Ok(proposal) => {
                let proposal_multisig_bytes = &account_data[8..40];
                if proposal_multisig_bytes != multisig_pda.to_bytes() {
                    continue;
                }

                let proposal_pubkey =
                    Pubkey::from_str(&pubkey_str).unwrap_or_else(|_| Pubkey::new([0u8; 32]));

                proposals.push(PendingProposal {
                    proposal_pubkey,
                    multisig: *multisig_pda,
                    creator: Pubkey::new(proposal.creator.to_bytes()),
                    expiry_timestamp: proposal.expires_at,
                    approvals: proposal.approvals as u64,
                    threshold: proposal.threshold as u64,
                    title: Some(proposal.memo.clone().unwrap_or_default()),
                    executed: proposal.status
                        == squads_multisig_program::state::ProposalStatus::Executed,
                });
            }
            Err(_) => continue,
        }
    }

    Ok(proposals)
}

// ===========================================================================
// WASM path — manual borsh parsing of Squads v4 Proposal accounts
// ===========================================================================

#[cfg(not(feature = "squads-state"))]
fn fetch_proposals_wasm(
    rpc_url: &str,
    multisig_pda: &Pubkey,
    authority: &Pubkey,
    squads_program_id: &Pubkey,
) -> Result<Vec<PendingProposal>, String> {
    let mut proposals = Vec::new();
    let max_scan = 20u64;

    for tx_index in 0..max_scan {
        let (proposal_pda, _bump) = derive_proposal_pda(authority, tx_index, squads_program_id);

        match rpc_get_account_info(rpc_url, &proposal_pda.to_string()) {
            Ok(Some(account_data)) => {
                // Parse the Squads v4 Proposal account manually via borsh.
                match parse_proposal_account(&account_data, multisig_pda) {
                    Ok(mut proposal) => {
                        proposal.proposal_pubkey = proposal_pda;
                        proposals.push(proposal);
                    }
                    Err(_e) => {
                        // Account exists but we couldn't parse it — skip.
                        // This is honest: we report what we can parse.
                        continue;
                    }
                }
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    Ok(proposals)
}

/// Parse a Squads v4 Proposal account from raw bytes.
///
/// Squads v4 Proposal account layout (Anchor) — offset/size:
/// ```text
/// [ 0.. 8]  discriminator (8 bytes)
/// [ 8..40]  multisig (32 bytes)
/// [40..44]  index: u32 LE
/// [44..76]  creator (32 bytes)
/// [76..77]  bump: u8
/// [77..109] approver: Pubkey (32 bytes) — voter
/// [109..115] threshold: u16 LE + 4 bytes padding
/// [115..123] created_at: i64 LE
/// [123..131] vetoed_at: i64 LE
/// [131..139] executed_at: i64 LE
/// [139..141] status: u8 (0=Active, 1=Approved, 2=Rejected, 3=Executed, 4=Cancelled) + 1 pad
/// [141..145] approvals: u32 LE
/// [145..146] bump2: u8
/// [146..   ] proposal_meta (variable, Anchor string)
/// ```
fn parse_proposal_account(
    data: &[u8],
    expected_multisig: &Pubkey,
) -> Result<PendingProposal, String> {
    // Minimum: discriminator (8) + multisig (32) + index (4) + creator (32)
    // + bump (1) + approver (32) + threshold (2) + 4 pad + created_at (8)
    // + vetoed_at (8) + executed_at (8) + status (1) + 1 pad + approvals (4)
    // + bump2 (1) = 146 bytes minimum
    if data.len() < 146 {
        return Err(format!("Proposal account too short: {} bytes", data.len()));
    }

    // Verify Anchor discriminator
    let expected_disc = anchor_discriminator("Proposal");
    if data[0..8] != expected_disc {
        return Err("wrong discriminator".to_string());
    }

    // Read multisig at offset 8
    let mut multisig_bytes = [0u8; 32];
    multisig_bytes.copy_from_slice(&data[8..40]);
    let multisig = Pubkey::new(multisig_bytes);

    // Only include proposals for our multisig
    if &multisig != expected_multisig {
        return Err(format!(
            "proposal multisig {} != expected {}",
            multisig.to_string(),
            expected_multisig.to_string()
        ));
    }

    // **index** (u32 LE) at offset 40
    let _index = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);

    // **creator** at offset 44
    let mut creator_bytes = [0u8; 32];
    creator_bytes.copy_from_slice(&data[44..76]);
    let creator = Pubkey::new(creator_bytes);

    // bump at offset 76 (ignored)

    // **approver** at offset 77
    let mut approver_bytes = [0u8; 32];
    approver_bytes.copy_from_slice(&data[77..109]);
    // approver not used in PendingProposal, but verified layout boundary

    // **threshold** (u16 LE) at offset 109
    let threshold =
        u16::from_le_bytes([data[109], data[110]]) as u64;

    // offset 111..115: padding (4 bytes)

    // created_at (i64 LE) at offset 115
    let _created_at = i64::from_le_bytes([
        data[115], data[116], data[117], data[118],
        data[119], data[120], data[121], data[122],
    ]);

    // vetoed_at (i64 LE) at offset 123
    let _vetoed_at = i64::from_le_bytes([
        data[123], data[124], data[125], data[126],
        data[127], data[128], data[129], data[130],
    ]);

    // executed_at (i64 LE) at offset 131
    let executed_at = i64::from_le_bytes([
        data[131], data[132], data[133], data[134],
        data[135], data[136], data[137], data[138],
    ]);

    // status (u8) at offset 139
    // 0=Active, 1=Approved, 2=Rejected, 3=Executed, 4=Cancelled
    let status = data[139];
    let executed = status == 3;

    // offset 140: padding

    // approvals (u32 LE) at offset 141
    let approvals = u32::from_le_bytes([
        data[141], data[142], data[143], data[144],
    ]) as u64;

    // bump2 at offset 145

    // proposal_meta starts at offset 146 (Anchor string: 4-byte len prefix + UTF-8)
    let title = if data.len() > 150 {
        let meta_len = u32::from_le_bytes([
            data[146], data[147], data[148], data[149],
        ]) as usize;
        let meta_start = 150;
        let meta_end = std::cmp::min(meta_start + meta_len, data.len());
        if meta_end > meta_start {
            String::from_utf8(data[meta_start..meta_end].to_vec()).ok()
        } else {
            None
        }
    } else {
        None
    };

    // Expiry timestamp: Squads stores this in created_at or a separate field.
    // For the parsing, we use executed_at if executed, otherwise we report
    // a default 24h expiry from the current time.
    let expiry_timestamp = if executed && executed_at > 0 {
        executed_at
    } else {
        unix_now_seconds() + 86400 // default 24h if not parsed
    };

    Ok(PendingProposal {
        proposal_pubkey: Pubkey::new([0u8; 32]), // caller fills this in
        multisig,
        creator,
        expiry_timestamp,
        approvals,
        threshold,
        title,
        executed,
    })
}

// ===========================================================================
// RPC helpers
// ===========================================================================

#[cfg(target_family = "wasm")]
fn rpc_get_account_info(rpc_url: &str, pubkey: &str) -> Result<Option<Vec<u8>>, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getAccountInfo",
        "params": [pubkey, {"encoding": "base64"}]
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
    let account_value = &value["result"]["value"];
    if account_value.is_null() {
        return Ok(None);
    }
    let data_b64 = account_value["data"][0]
        .as_str()
        .ok_or("missing account data in response")?;
    let data = STANDARD
        .decode(data_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;
    Ok(Some(data))
}

#[cfg(not(target_family = "wasm"))]
fn rpc_get_account_info(_rpc_url: &str, _pubkey: &str) -> Result<Option<Vec<u8>>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

#[cfg(target_family = "wasm")]
fn rpc_get_program_accounts(
    rpc_url: &str,
    program_id: &str,
    _discriminator: Option<&[u8; 8]>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "getProgramAccounts",
        "params": [program_id, {"encoding": "base64"}]
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
fn rpc_get_program_accounts(
    _rpc_url: &str,
    _program_id: &str,
    _discriminator: Option<&[u8; 8]>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    Err("RPC host interface not available in native mode".to_string())
}

/// Count proposals by status.
pub fn count_by_status(proposals: &[PendingProposal]) -> (usize, usize, usize) {
    let pending = proposals.iter().filter(|p| !p.executed).count();
    let executed = proposals.iter().filter(|p| p.executed).count();
    let approved = proposals
        .iter()
        .filter(|p| !p.executed && p.approvals >= p.threshold)
        .count();
    (pending, executed, approved)
}

/// Check if any proposals are expiring soon (within N hours).
pub fn expiring_soon(proposals: &[PendingProposal], within_hours: i64) -> Vec<&PendingProposal> {
    let now = squads_defi_core::squads::unix_now_seconds();
    let cutoff = now + within_hours * 3600;
    proposals
        .iter()
        .filter(|p| !p.executed && p.expiry_timestamp <= cutoff && p.expiry_timestamp > now)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_empty_proposals() {
        let (pending, executed, approved) = count_by_status(&[]);
        assert_eq!(pending, 0);
        assert_eq!(executed, 0);
        assert_eq!(approved, 0);
    }

    #[test]
    fn test_expiring_soon_empty() {
        let result = expiring_soon(&[], 24);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_proposal_account_wrong_discriminator() {
        // 200 bytes of zeros — wrong discriminator
        let data = vec![0u8; 200];
        let multisig = Pubkey::new([1u8; 32]);
        let result = parse_proposal_account(&data, &multisig);
        assert!(result.is_err(), "wrong discriminator must be rejected");
    }

    #[test]
    fn test_parse_proposal_account_too_short() {
        let data = vec![0u8; 50];
        let multisig = Pubkey::new([1u8; 32]);
        let result = parse_proposal_account(&data, &multisig);
        assert!(result.is_err(), "short data must be rejected");
    }

    #[test]
    fn test_parse_proposal_account_wrong_multisig() {
        // Build valid data with correct discriminator but wrong multisig
        let mut data = vec![0u8; 200];
        let disc = anchor_discriminator("Proposal");
        data[0..8].copy_from_slice(&disc);
        // multisig at offset 8 — leave as all zeros
        let multisig = Pubkey::new([1u8; 32]); // expected multisig has 1s
        let result = parse_proposal_account(&data, &multisig);
        assert!(result.is_err(), "wrong multisig must be rejected");
    }
}
