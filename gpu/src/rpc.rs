use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};

pub struct BitcoinRpcClient {
    url: String,
    auth_header: String,
    agent: ureq::Agent,
}

#[derive(Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: &'a str,
    method: &'a str,
    params: Value,
}

impl BitcoinRpcClient {
    pub fn new(url: &str, user: &str, password: &str) -> Self {
        let credentials = format!("{user}:{password}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
        Self {
            url: url.trim_end_matches('/').to_string(),
            auth_header: format!("Basic {encoded}"),
            agent: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .build(),
        }
    }

    fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = RpcRequest {
            jsonrpc: "1.0",
            id: "miner",
            method,
            params,
        };

        let response: Value = self
            .agent
            .post(&self.url)
            .set("Authorization", &self.auth_header)
            .set("Content-Type", "application/json")
            .send_json(body)
            .with_context(|| format!("RPC call '{method}' failed"))?
            .into_json()
            .context("RPC response was not valid JSON")?;

        if let Some(err) = response.get("error").filter(|v| !v.is_null()) {
            return Err(anyhow!("RPC error from '{method}': {err}"));
        }

        response
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow!("RPC response missing 'result' field"))
    }

    pub fn get_block_template(&self) -> Result<Value> {
        let params = json!([{ "rules": ["segwit"] }]);
        self.call("getblocktemplate", params)
    }

    pub fn submit_block(&self, hex_block: &str) -> Result<Option<String>> {
        let result = self.call("submitblock", json!([hex_block]))?;
        // submitblock returns null on success, or a string error reason.
        match result {
            Value::Null => Ok(None),
            Value::String(s) if s.is_empty() => Ok(None),
            Value::String(s) => Ok(Some(s)),
            other => Ok(Some(other.to_string())),
        }
    }

    pub fn get_blockchain_info(&self) -> Result<Value> {
        self.call("getblockchaininfo", json!([]))
    }
}
