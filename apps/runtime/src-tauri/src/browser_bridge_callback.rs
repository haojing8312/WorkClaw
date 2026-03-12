use crate::commands::browser_bridge_install::BrowserBridgeInstallStore;
use crate::commands::feishu_browser_setup::FeishuBrowserSetupStore;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct BrowserBridgeCallbackServer {
    pool: SqlitePool,
    store: FeishuBrowserSetupStore,
    install_store: BrowserBridgeInstallStore,
    task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl BrowserBridgeCallbackServer {
    pub fn new(
        pool: SqlitePool,
        store: FeishuBrowserSetupStore,
        install_store: BrowserBridgeInstallStore,
    ) -> Self {
        Self {
            pool,
            store,
            install_store,
            task: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(&self) -> Result<String, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("bind browser bridge callback listener failed: {}", e))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("read browser bridge callback listener addr failed: {}", e))?;
        let pool = self.pool.clone();
        let store = self.store.clone();
        let install_store = self.install_store.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                let pool = pool.clone();
                let store = store.clone();
                let install_store = install_store.clone();
                tokio::spawn(async move {
                    let response = match read_http_body(&mut socket).await {
                        Ok(body) => match handle_browser_bridge_payload(
                            &pool,
                            &store,
                            &install_store,
                            &body,
                        )
                        .await
                        {
                            Ok(payload) => http_json_response(200, &payload),
                            Err(error) => http_json_response(400, &json!({ "error": error })),
                        },
                        Err(error) => http_json_response(400, &json!({ "error": error })),
                    };
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        *self.task.lock().unwrap() = Some(task);
        Ok(format!("http://{}", addr))
    }

    pub fn stop(&self) {
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
    }
}

impl Drop for BrowserBridgeCallbackServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Deserialize)]
struct BrowserBridgeEnvelope {
    version: u8,
    #[serde(rename = "sessionId")]
    session_id: String,
    kind: String,
    payload: BrowserBridgePayload,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum BrowserBridgePayload {
    #[serde(rename = "credentials.report")]
    CredentialsReport {
        #[serde(rename = "appId")]
        app_id: String,
        #[serde(rename = "appSecret")]
        app_secret: String,
    },
    #[serde(rename = "session.start")]
    SessionStart { provider: String },
    #[serde(rename = "session.resume")]
    SessionResume {
        #[serde(rename = "sessionId")]
        _session_id: String,
    },
    #[serde(rename = "page.report")]
    PageReport {
        #[serde(rename = "page")]
        _page: String,
    },
    #[serde(rename = "bridge.hello")]
    BridgeHello,
}

async fn handle_browser_bridge_payload(
    pool: &SqlitePool,
    store: &FeishuBrowserSetupStore,
    install_store: &BrowserBridgeInstallStore,
    body: &str,
) -> Result<serde_json::Value, String> {
    let envelope: BrowserBridgeEnvelope = serde_json::from_str(body)
        .map_err(|e| format!("invalid browser bridge envelope: {}", e))?;

    if envelope.version != 1 {
        return Err("unsupported browser bridge version".to_string());
    }
    if envelope.kind != "request" {
        return Err("browser bridge callback only accepts request envelopes".to_string());
    }

    match envelope.payload {
        BrowserBridgePayload::CredentialsReport { app_id, app_secret } => {
            store
                .report_credentials_and_bind(pool, envelope.session_id.clone(), app_id, app_secret)
                .await?;
            Ok(json!({
                "version": 1,
                "sessionId": envelope.session_id,
                "kind": "response",
                "payload": {
                    "type": "action.pause",
                    "reason": "browser bridge credentials bound locally",
                    "step": "ENABLE_LONG_CONNECTION",
                    "title": "本地绑定已完成",
                    "instruction": "请前往事件与回调，开启长连接接受事件。",
                    "ctaLabel": "继续到事件与回调"
                }
            }))
        }
        BrowserBridgePayload::SessionStart { provider } => Ok(json!({
            "version": 1,
            "sessionId": envelope.session_id,
            "kind": "response",
            "payload": {
                "type": "action.open",
                "url": format!("https://open.feishu.cn/?provider={}", provider)
            }
        })),
        BrowserBridgePayload::BridgeHello => {
            install_store.mark_connected_now();
            Ok(json!({
                "version": 1,
                "sessionId": envelope.session_id,
                "kind": "response",
                "payload": {
                    "type": "action.detect_step"
                }
            }))
        }
        BrowserBridgePayload::SessionResume { .. } | BrowserBridgePayload::PageReport { .. } => {
            Ok(json!({
                "version": 1,
                "sessionId": envelope.session_id,
                "kind": "response",
                "payload": {
                    "type": "action.detect_step"
                }
            }))
        }
    }
}

async fn read_http_body(socket: &mut tokio::net::TcpStream) -> Result<String, String> {
    let mut buffer = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 4096];
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        let n = socket
            .read(&mut chunk)
            .await
            .map_err(|e| format!("read browser bridge callback request failed: {}", e))?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);

        if header_end.is_none() {
            header_end = find_header_end(&buffer);
            if let Some(end) = header_end {
                let header = String::from_utf8_lossy(&buffer[..end]).to_string();
                content_length = parse_content_length(&header)?;
            }
        }

        if let Some(end) = header_end {
            let body_len = buffer.len().saturating_sub(end + 4);
            if body_len >= content_length {
                let body = &buffer[end + 4..end + 4 + content_length];
                return String::from_utf8(body.to_vec())
                    .map_err(|e| format!("decode browser bridge callback body failed: {}", e));
            }
        }
    }

    Err("browser bridge callback request body incomplete".to_string())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(header: &str) -> Result<usize, String> {
    for line in header.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| format!("invalid content-length: {}", e));
            }
        }
    }
    Ok(0)
}

fn http_json_response(status: u16, payload: &serde_json::Value) -> String {
    let status_text = if status == 200 { "OK" } else { "Bad Request" };
    let body = payload.to_string();
    format!(
        "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    )
}
