//! Solana Pay URL builder — pure core, no WASM dependencies.
//!
//! Implements the Solana Pay specification:
//! https://docs.solanapay.com/spec
//!
//! URL format:
//! solana:<recipient>?amount=<amount>&spl-token=<mint>&reference=<ref>&label=<label>&message=<message>&memo=<memo>

use squads_defi_core::Pubkey;

/// Build a Solana Pay transfer request URL.
pub fn build_pay_url(
    recipient: &str,
    amount: Option<&str>,
    spl_token: Option<&str>,
    label: Option<&str>,
    message: Option<&str>,
    memo: Option<&str>,
) -> Result<String, String> {
    // Validate recipient
    Pubkey::from_str(recipient)
        .map_err(|e| format!("Invalid recipient address: {e}"))?;

    let mut url = format!("solana:{recipient}");
    let mut first = true;

    let mut add_param = |url: &mut String, key: &str, value: &str| {
        url.push(if first { first = false; '?' } else { '&' });
        let encoded = value
            .replace('%', "%25")
            .replace(' ', "%20")
            .replace('&', "%26")
            .replace('=', "%3D")
            .replace('#', "%23");
        url.push_str(&format!("{key}={encoded}"));
    };

    if let Some(amt) = amount {
        add_param(&mut url, "amount", amt);
    }
    if let Some(token) = spl_token {
        Pubkey::from_str(token)
            .map_err(|e| format!("Invalid SPL token mint: {e}"))?;
        add_param(&mut url, "spl-token", token);
    }
    if let Some(lbl) = label {
        add_param(&mut url, "label", lbl);
    }
    if let Some(msg) = message {
        add_param(&mut url, "message", msg);
    }
    if let Some(m) = memo {
        add_param(&mut url, "memo", m);
    }

    Ok(url)
}
