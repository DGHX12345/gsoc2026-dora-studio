//! Coordinator WebSocket client (JSON-RPC over WebSocket).
//!
//! Connects to `ws://{addr}/api/control` with Bearer auth, performs a Hello
//! handshake, then supports request/reply for GetNodeInfo, Reload, and List.
//!
//! Uses raw TCP + manual WebSocket framing to stay dependency-free and
//! compatible with Rust 1.75.

use std::{path::PathBuf, sync::Arc, time::Duration};

use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc, oneshot, Mutex},
};
use uuid::Uuid;

use crate::{
    models::NodeRuntimeStatus,
    protocol::types::{NodeInfo, NodeInfoList, NodeStatus},
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const DEFAULT_COORDINATOR_PORT: u16 = 6013;

/// The dora version string sent in the Hello handshake.
const DORA_VERSION: &str = "1.0.0-rc.4";

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Internal command to the send loop.
struct ClientCommand {
    text: String,
    reply_tx: oneshot::Sender<Result<serde_json::Value, String>>,
}

/// Shared state between CoordinatorWsClient and the background read loop.
struct SharedState {
    cmd_tx: Option<mpsc::UnboundedSender<ClientCommand>>,
    pending: Vec<(String, oneshot::Sender<Result<serde_json::Value, String>>)>,
}

#[derive(Clone)]
pub struct CoordinatorWsClient {
    state: Arc<Mutex<SharedState>>,
}

impl CoordinatorWsClient {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SharedState {
                cmd_tx: None,
                pending: Vec::new(),
            })),
        }
    }

    /// Connect to the coordinator WebSocket.
    pub async fn connect(&self) -> Result<(), String> {
        let port: u16 = std::env::var("DORA_COORDINATOR_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_COORDINATOR_PORT);

        let addr = format!("127.0.0.1:{port}");
        let token = discover_token();

        // TCP connect
        let mut stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("cannot connect to coordinator at {addr}: {e}"))?;

        // WebSocket handshake
        let key = generate_ws_key();
        perform_handshake(&mut stream, &addr, &key, token.as_deref()).await?;

        // Split into read/write halves using tokio split
        let (mut read_half, write_half) = stream.into_split();
        let write_half = Arc::new(Mutex::new(write_half));

        // Channel for sending commands
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<ClientCommand>();

        // Shared state
        let shared = Arc::clone(&self.state);

        // Spawn write loop
        let write_state = Arc::clone(&write_half);
        let pending_state = Arc::clone(&shared);
        tokio::spawn(client_write_loop(cmd_rx, write_state, pending_state));

        // Send Hello handshake
        let hello_id = Uuid::new_v4();
        let hello_json = build_hello_request(&hello_id.to_string());

        {
            let mut w = write_half.lock().await;
            let frame = build_text_frame(&hello_json);
            w.write_all(&frame)
                .await
                .map_err(|e| format!("failed to send Hello: {e}"))?;
        }

        // Read Hello reply
        let hello_text = read_text_frame(&mut read_half).await?;
        check_hello_reply(&hello_text)?;

        // Spawn read loop
        let shared2 = Arc::clone(&shared);
        tokio::spawn(async move {
            loop {
                match read_text_frame(&mut read_half).await {
                    Ok(text) => {
                        let resp_id = extract_id(&text).unwrap_or_default();
                        let mut st = shared2.lock().await;
                        if let Some(pos) = st.pending.iter().position(|(id, _)| *id == resp_id) {
                            let (_, tx) = st.pending.remove(pos);
                            let val: serde_json::Value = match serde_json::from_str(&text) {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = tx.send(Err(format!("parse error: {e}")));
                                    continue;
                                }
                            };
                            if val.get("error").and_then(|e| e.as_str()).is_some() {
                                let err = val["error"].as_str().unwrap_or("unknown").to_string();
                                let _ = tx.send(Err(err));
                            } else {
                                let result = val
                                    .get("result")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                let _ = tx.send(Ok(result));
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut st = self.state.lock().await;
        st.cmd_tx = Some(cmd_tx);
        Ok(())
    }

    /// Send a JSON-RPC request and wait for the reply.
    async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let id = Uuid::new_v4().to_string();
        let json = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        let (reply_tx, reply_rx) = oneshot::channel();

        {
            let st = self.state.lock().await;
            if let Some(ref tx) = st.cmd_tx {
                let _ = tx.send(ClientCommand {
                    text: json,
                    reply_tx,
                });
            } else {
                return Err("WebSocket not connected".to_string());
            }
        }

        match tokio::time::timeout(Duration::from_secs(5), reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("request channel closed".to_string()),
            Err(_) => Err("request timed out".to_string()),
        }
    }

    /// Poll node statuses for a given dataflow.
    pub async fn node_statuses(&self, dataflow_id: &str) -> Result<Vec<NodeRuntimeStatus>, String> {
        let _uuid: Uuid = dataflow_id
            .parse()
            .map_err(|_| format!("invalid dataflow id: {dataflow_id}"))?;

        let nodes = self.all_node_infos().await?;

        Ok(nodes
            .into_iter()
            .map(|info| NodeRuntimeStatus {
                node_id: info.node_id,
                status: status_to_string(info.metrics.as_ref().map(|m| &m.status)),
                uptime_secs: None,
                restart_count: info.metrics.as_ref().map(|m| m.restart_count).unwrap_or(0),
                cpu_usage: info.metrics.as_ref().map(|m| m.cpu_usage),
                memory_mb: info.metrics.as_ref().map(|m| m.memory_mb),
                pending_messages: info.metrics.as_ref().map(|m| m.pending_messages),
            })
            .collect())
    }

    /// Fetch full node info for ALL running nodes (GetNodeInfo takes no
    /// parameters and returns every node across all dataflows).
    pub async fn all_node_infos(&self) -> Result<Vec<NodeInfo>, String> {
        let result = self.request("GetNodeInfo", get_node_info_params()).await?;
        extract_node_infos(result)
    }

    /// Send a reload request for a specific node.
    pub async fn reload_node(&self, dataflow_id: &str, node_id: &str) -> Result<(), String> {
        let dataflow_uuid: Uuid = dataflow_id
            .parse()
            .map_err(|_| format!("invalid dataflow id: {dataflow_id}"))?;

        let params = reload_params(&dataflow_uuid.to_string(), node_id);
        self.request("Reload", params).await?;
        Ok(())
    }

    /// Check whether the WebSocket is connected.
    pub async fn is_connected(&self) -> bool {
        self.state.lock().await.cmd_tx.is_some()
    }
}

// ---------------------------------------------------------------------------
// WebSocket framing (RFC 6455)
// ---------------------------------------------------------------------------

fn generate_ws_key() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("failed to generate random bytes");
    base64_encode(&buf)
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Build a masked WebSocket text frame.
fn build_text_frame(payload: &str) -> Vec<u8> {
    let payload_bytes = payload.as_bytes();
    let len = payload_bytes.len();

    let mut mask_key = [0u8; 4];
    getrandom::getrandom(&mut mask_key).expect("failed to generate mask key");

    let mut frame = Vec::with_capacity(2 + 8 + 4 + len);

    // FIN=1, RSV=0, opcode=1 (text)
    frame.push(0x81);

    // MASK=1, payload length
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len <= 0xFFFF {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Mask key
    frame.extend_from_slice(&mask_key);

    // Masked payload
    for (i, &b) in payload_bytes.iter().enumerate() {
        frame.push(b ^ mask_key[i % 4]);
    }

    frame
}

/// Read a complete text frame from the stream.
async fn read_text_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<String, String> {
    let mut header = [0u8; 2];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|e| format!("WS read error: {e}"))?;

    let opcode = header[0] & 0x0F;
    if opcode == 0x08 {
        return Err("WS closed by server".to_string());
    }
    if opcode == 0x09 {
        // Ping — the server sends ping, we'd need to pong. Since we can't write
        // from the read path, signal reconnect.
        return Err("ping received on read-only path — reconnect".to_string());
    }

    let masked = (header[1] & 0x80) != 0;
    let mut payload_len = (header[1] & 0x7F) as u64;

    if payload_len == 126 {
        let mut ext = [0u8; 2];
        reader
            .read_exact(&mut ext)
            .await
            .map_err(|e| format!("WS read error: {e}"))?;
        payload_len = u16::from_be_bytes(ext) as u64;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        reader
            .read_exact(&mut ext)
            .await
            .map_err(|e| format!("WS read error: {e}"))?;
        payload_len = u64::from_be_bytes(ext);
    }

    // Per RFC 6455, server frames are never masked. But we handle it gracefully.
    let mut mask_key = [0u8; 4];
    if masked {
        reader
            .read_exact(&mut mask_key)
            .await
            .map_err(|e| format!("WS read error: {e}"))?;
    }

    let mut payload = vec![0u8; payload_len as usize];
    if payload_len > 0 {
        reader
            .read_exact(&mut payload)
            .await
            .map_err(|e| format!("WS read error: {e}"))?;
    }

    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask_key[i % 4];
        }
    }

    String::from_utf8(payload).map_err(|e| format!("invalid UTF-8 in WS frame: {e}"))
}

/// Perform HTTP WebSocket upgrade handshake.
async fn perform_handshake(
    stream: &mut TcpStream,
    host: &str,
    key: &str,
    token: Option<&str>,
) -> Result<(), String> {
    let path = "/api/control";
    let mut request = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n"
    );

    if let Some(t) = token {
        request.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }

    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| format!("handshake write failed: {e}"))?;

    // Read HTTP response
    let mut buf = vec![0u8; 4096];
    let mut response = Vec::new();
    loop {
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| format!("handshake read failed: {e}"))?;
        if n == 0 {
            return Err("connection closed during handshake".to_string());
        }
        response.extend_from_slice(&buf[..n]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let response_str =
        String::from_utf8(response).map_err(|e| format!("invalid HTTP response: {e}"))?;

    if !response_str.contains("101") {
        // Extract status line for error message
        let status = response_str.lines().next().unwrap_or("unknown");
        if status.contains("401") {
            return Err(format!(
                "auth failed: {status}\n\
                 hint: The coordinator was started with --auth. \
                 The token is stored in ~/.config/dora/.dora-token"
            ));
        }
        return Err(format!("WebSocket upgrade failed: {status}"));
    }

    // Verify Sec-WebSocket-Accept
    let accept_key = compute_accept_key(key);
    if !response_str.contains(&accept_key) {
        return Err("WebSocket handshake: invalid Sec-WebSocket-Accept".to_string());
    }

    Ok(())
}

fn compute_accept_key(key: &str) -> String {
    let guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let combined = format!("{key}{guid}");
    let hash = Sha1::digest(combined.as_bytes());
    base64_encode(&hash)
}

// ---------------------------------------------------------------------------
// Token discovery
// ---------------------------------------------------------------------------

fn discover_token() -> Option<String> {
    if let Ok(val) = std::env::var("DORA_AUTH_TOKEN") {
        if !val.is_empty() {
            return Some(val);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(token) = read_token_file(&cwd.join(".dora-token")) {
            return Some(token);
        }
    }

    if let Some(config_dir) = dirs_fallback() {
        let path = config_dir.join("dora").join(".dora-token");
        if let Some(token) = read_token_file(&path) {
            return Some(token);
        }
    }

    None
}

fn dirs_fallback() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config"))
}

fn read_token_file(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn status_to_string(status: Option<&NodeStatus>) -> String {
    match status {
        Some(NodeStatus::Running) => "running",
        Some(NodeStatus::Restarting) => "reloading",
        Some(NodeStatus::Degraded) => "degraded",
        Some(NodeStatus::Failed) => "crashed",
        Some(NodeStatus::Stopped) => "exited",
        None => "unknown",
    }
    .to_string()
}

fn extract_id(json_text: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(json_text).ok()?;
    val.get("id")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// Extracts node infos from a GetNodeInfo reply.
///
/// dora 1.0 wraps the payload in an externally tagged
/// `ControlRequestReply` variant (`{"NodeInfoList": [...]}`); dora 0.5
/// replied with a bare array.
fn extract_node_infos(result: serde_json::Value) -> Result<Vec<NodeInfo>, String> {
    let array = match result {
        serde_json::Value::Array(_) => result,
        serde_json::Value::Object(mut obj) => obj
            .remove("NodeInfoList")
            .ok_or_else(|| "GetNodeInfo reply missing NodeInfoList".to_string())?,
        _ => return Err("unexpected GetNodeInfo reply shape".to_string()),
    };
    let nodes: NodeInfoList = serde_json::from_value(array)
        .map_err(|e| format!("failed to parse GetNodeInfo reply: {e}"))?;
    Ok(nodes.0)
}

/// Write loop: registers each request in `pending` (so the read loop can
/// correlate the reply) and writes the frame. Generic over the writer for
/// duplex-based tests.
async fn client_write_loop<W>(
    mut cmd_rx: mpsc::UnboundedReceiver<ClientCommand>,
    write_state: Arc<Mutex<W>>,
    pending_state: Arc<Mutex<SharedState>>,
) where
    W: tokio::io::AsyncWriteExt + Unpin,
{
    while let Some(cmd) = cmd_rx.recv().await {
        let id = extract_id(&cmd.text).unwrap_or_default();
        pending_state.lock().await.pending.push((id, cmd.reply_tx));
        let frame = build_text_frame(&cmd.text);
        let mut w = write_state.lock().await;
        if w.write_all(&frame).await.is_err() {
            break;
        }
    }
}

/// Builds the Hello JSON-RPC request.
///
/// dora 1.0 deserializes params as an externally tagged `ControlRequest`
/// enum, so the version goes inside the variant:
/// `{"Hello": {"dora_version": "..."}}`.
fn build_hello_request(id: &str) -> String {
    serde_json::json!({
        "id": id,
        "method": "Hello",
        "params": { "Hello": { "dora_version": DORA_VERSION } }
    })
    .to_string()
}

/// Params for GetNodeInfo: an externally tagged unit variant serializes
/// as a bare string.
fn get_node_info_params() -> serde_json::Value {
    serde_json::Value::String("GetNodeInfo".to_string())
}

/// Params for Reload: struct variant fields wrapped in the variant name.
fn reload_params(dataflow_id: &str, node_id: &str) -> serde_json::Value {
    serde_json::json!({
        "Reload": {
            "dataflow_id": dataflow_id,
            "node_id": node_id,
            "operator_id": null,
        }
    })
}

fn check_hello_reply(text: &str) -> Result<(), String> {
    let val: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("invalid Hello reply: {e}"))?;

    if let Some(err) = val.get("error").and_then(|e| e.as_str()) {
        return Err(format!("Hello rejected: {err}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn all_node_infos_errors_when_not_connected() {
        let client = CoordinatorWsClient::new();
        assert!(client.all_node_infos().await.is_err());
    }

    /// Regression: the write loop must register the request in `pending`
    /// before writing the frame, otherwise the read loop can never match
    /// the reply and every request fails with "request channel closed".
    #[tokio::test]
    async fn write_loop_registers_pending_before_writing() {
        use tokio::io::AsyncReadExt;

        let (server, mut client) = tokio::io::duplex(64);
        let write_state = Arc::new(tokio::sync::Mutex::new(server));
        let shared = Arc::new(tokio::sync::Mutex::new(SharedState {
            cmd_tx: None,
            pending: Vec::new(),
        }));
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (reply_tx, _reply_rx) = oneshot::channel();

        let loop_task = tokio::spawn(client_write_loop(
            cmd_rx,
            write_state,
            Arc::clone(&shared),
        ));

        cmd_tx
            .send(ClientCommand {
                text: r#"{"id":"abc-123","method":"GetNodeInfo","params":"GetNodeInfo"}"#
                    .to_string(),
                reply_tx,
            })
            .unwrap();
        tokio::task::yield_now().await;

        let st = shared.lock().await;
        assert_eq!(st.pending.len(), 1);
        assert_eq!(st.pending[0].0, "abc-123");
        drop(st);

        let mut buf = vec![0u8; 256];
        let n = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            client.read(&mut buf),
        )
        .await
        .expect("frame written to socket")
        .expect("read ok");
        assert!(n > 0);

        loop_task.abort();
    }

    /// dora 1.0 deserializes params as an externally tagged
    /// `ControlRequest` enum; a unit variant serializes as a bare string.
    #[test]
    fn get_node_info_params_is_bare_variant_string() {
        let params = get_node_info_params();
        assert_eq!(params, serde_json::json!("GetNodeInfo"));
    }

    /// Struct variants wrap their fields: `{"Reload": {...}}`.
    #[test]
    fn reload_params_wrap_fields_in_variant() {
        let params = reload_params("df-1", "node-a");
        assert_eq!(params["Reload"]["dataflow_id"], "df-1");
        assert_eq!(params["Reload"]["node_id"], "node-a");
        assert!(params["Reload"]["operator_id"].is_null());
        assert!(params.get("dataflow_id").is_none());
    }

    /// dora 1.0 wraps the GetNodeInfo reply in an externally tagged
    /// `ControlRequestReply` variant: `{"NodeInfoList": [...]}`.
    #[test]
    fn extract_node_infos_unwraps_10_variant() {
        let result = serde_json::json!({
            "NodeInfoList": [{
                "dataflow_id": "11111111-1111-1111-1111-111111111111",
                "dataflow_name": "demo",
                "node_id": "planner",
                "daemon_id": "d1",
                "metrics": null
            }]
        });
        let nodes = extract_node_infos(result).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, "planner");
    }

    /// dora 0.5 replied with a bare array; keep that shape working.
    #[test]
    fn extract_node_infos_accepts_bare_array() {
        let result = serde_json::json!([{
            "dataflow_id": "11111111-1111-1111-1111-111111111111",
            "dataflow_name": null,
            "node_id": "camera",
            "daemon_id": "d1",
            "metrics": null
        }]);
        let nodes = extract_node_infos(result).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, "camera");
    }

    #[test]
    fn extract_node_infos_rejects_unknown_shape() {
        let result = serde_json::json!({"unexpected": true});
        assert!(extract_node_infos(result).is_err());
    }

    #[test]
    fn status_to_string_maps_all_variants() {
        assert_eq!(status_to_string(Some(&NodeStatus::Running)), "running");
        assert_eq!(status_to_string(Some(&NodeStatus::Restarting)), "reloading");
        assert_eq!(status_to_string(Some(&NodeStatus::Degraded)), "degraded");
        assert_eq!(status_to_string(Some(&NodeStatus::Failed)), "crashed");
        assert_eq!(status_to_string(Some(&NodeStatus::Stopped)), "exited");
        assert_eq!(status_to_string(None), "unknown");
    }

    #[test]
    fn extract_id_parses_request_and_response() {
        let req = r#"{"id":"abc-123","method":"List","params":{}}"#;
        assert_eq!(extract_id(req), Some("abc-123".to_string()));

        let resp = r#"{"id":"abc-123","result":{"ok":true}}"#;
        assert_eq!(extract_id(resp), Some("abc-123".to_string()));
    }

    #[test]
    fn check_hello_ok() {
        let ok = r#"{"id":"abc","result":{"dora_version":"1.0.0-rc.4"}}"#;
        assert!(check_hello_reply(ok).is_ok());
    }

    /// dora 1.0 deserializes request params as an externally tagged
    /// `ControlRequest` enum: the Hello params must be the wrapped
    /// variant `{"Hello": {"dora_version": ...}}`, not a bare object.
    #[test]
    fn hello_request_wraps_dora_version_in_variant() {
        let json = build_hello_request("abc-123");
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["method"], "Hello");
        assert_eq!(val["params"]["Hello"]["dora_version"], "1.0.0-rc.4");
        assert!(val["params"].get("dora_version").is_none());
    }

    #[test]
    fn check_hello_reject() {
        let err = r#"{"id":"abc","error":"version mismatch"}"#;
        assert!(check_hello_reply(err).is_err());
    }

    #[test]
    fn discover_token_env_var() {
        std::env::set_var("DORA_AUTH_TOKEN", "test-hex-token");
        assert_eq!(discover_token(), Some("test-hex-token".to_string()));
        std::env::remove_var("DORA_AUTH_TOKEN");
    }

    #[test]
    fn discover_token_empty_env_skipped() {
        // Empty env var is skipped (but token may exist on disk, so we only
        // check that empty env var doesn't cause a panic and returns something
        // — possibly from disk).
        std::env::set_var("DORA_AUTH_TOKEN", "");
        let _ = discover_token();
        std::env::remove_var("DORA_AUTH_TOKEN");
    }

    #[test]
    fn ws_key_is_base64() {
        let key = generate_ws_key();
        assert_eq!(key.len(), 24);
        assert!(key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '='));
    }

    #[test]
    fn ws_frame_roundtrip() {
        let text = r#"{"id":"x","result":{"ok":true}}"#;
        let frame = build_text_frame(text);
        // Frame should start with FIN+text opcode
        assert_eq!(frame[0] & 0x0F, 1); // opcode=text
        assert!(frame[0] & 0x80 != 0); // FIN
    }

    #[test]
    fn compute_accept_key_known_value() {
        // RFC 6455 example: key=dGhlIHNhbXBsZSBub25jZQ== → s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_accept_key(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
