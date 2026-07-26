//! Mockable RPC client trait for Solana JSON-RPC.
//!
//! Core uses this abstraction; waki-based impl lives under `#[cfg(target_family = "wasm")]`.
//!
//! All methods are synchronous — WASM has no async runtime. The host provides
//! request/response via the synchronous waki blocking HTTP client.

use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("JSON-RPC error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Trait for making Solana JSON-RPC requests.
/// Mockable so pure-core tests never hit the network.
pub trait RpcClient: Send + Sync {
    fn request(&self, method: &str, params: Value) -> Result<Value, RpcError>;
}

// ---------------------------------------------------------------------------
// Mock RPC client for host tests
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;

/// A programmable mock RPC client for tests.
/// Register expected responses with `set_response` before calling `request`.
pub struct MockRpcClient {
    responses: Mutex<HashMap<String, Value>>,
}

impl MockRpcClient {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(HashMap::new()),
        }
    }

    /// Set a canned JSON response for a given RPC method.
    pub fn set_response(&self, method: &str, response: Value) {
        self.responses
            .lock()
            .unwrap()
            .insert(method.to_string(), response);
    }
}

impl Default for MockRpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcClient for MockRpcClient {
    fn request(&self, method: &str, _params: Value) -> Result<Value, RpcError> {
        let guard = self.responses.lock().unwrap();
        match guard.get(method) {
            Some(v) => Ok(v.clone()),
            None => Err(RpcError::Rpc {
                code: -32601,
                message: format!("unexpected method: {method}"),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience RPC methods (work with any RpcClient impl)
// ---------------------------------------------------------------------------

use crate::types::{Blockhash, Pubkey};

/// Fetch the latest blockhash from the cluster.
pub fn get_latest_blockhash(client: &dyn RpcClient) -> Result<Blockhash, RpcError> {
    let response = client
        .request("getLatestBlockhash", serde_json::json!([{"commitment": "finalized"}]))?;
    let blockhash_str = response["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| RpcError::Parse("missing blockhash in response".into()))?;
    Blockhash::from_str(blockhash_str).map_err(|e| RpcError::Parse(e))
}

/// Fetch account info for a given pubkey.
pub fn get_account_info(
    client: &dyn RpcClient,
    pubkey: &Pubkey,
) -> Result<Option<serde_json::Value>, RpcError> {
    let response = client
        .request(
            "getAccountInfo",
            serde_json::json!([pubkey.to_string(), {"encoding": "jsonParsed"}]),
        )?;
    let value = response["result"]["value"].clone();
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Submit a signed transaction and return the signature.
pub fn send_transaction(
    client: &dyn RpcClient,
    signed_tx_base64: &str,
) -> Result<String, RpcError> {
    let response = client
        .request(
            "sendTransaction",
            serde_json::json!([signed_tx_base64, {"encoding": "base64"}]),
        )?;
    response["result"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| RpcError::Parse("missing tx signature in response".into()))
}

// ---------------------------------------------------------------------------
// Waki-based RPC client (wasm32-wasip2 only)
// ---------------------------------------------------------------------------

/// An RPC client that uses `waki` (wasi:http) for HTTP requests.
/// Only available when compiling for wasm32-wasip2.
#[cfg(target_family = "wasm")]
pub struct WakiRpcClient {
    url: String,
}

#[cfg(target_family = "wasm")]
impl WakiRpcClient {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

#[cfg(target_family = "wasm")]
impl RpcClient for WakiRpcClient {
    fn request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        })
        .to_string();

        let response = waki::Client::new()
            .post(&self.url)
            .header("Content-Type", "application/json")
            .body(body.as_str())
            .send()
            .map_err(|e| RpcError::Http(format!("waki request failed: {e}")))?;

        let body_bytes = response
            .body()
            .map_err(|e| RpcError::Http(format!("waki body read failed: {e}")))?;

        let body_str =
            String::from_utf8(body_bytes).map_err(|e| RpcError::Parse(format!("utf-8: {e}")))?;

        let value: Value = serde_json::from_str(&body_str)
            .map_err(|e| RpcError::Parse(format!("json: {e}")))?;

        // Check for JSON-RPC error
        if let Some(err) = value.get("error") {
            return Err(RpcError::Rpc {
                code: err["code"].as_i64().unwrap_or(-1),
                message: err["message"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
            });
        }

        Ok(value)
    }
}