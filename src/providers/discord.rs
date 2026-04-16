use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, info, warn};

use crate::config::DiscordConfig;
use crate::i18n::{Locale, Messages};
use crate::types::{Decision, FeedbackRequest, FeedbackResponse, TimeoutKind};

use super::Provider;

const API_BASE: &str = "https://discord.com/api/v10";
const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

// Gateway intents: GUILD_MESSAGES (1<<9) | DIRECT_MESSAGES (1<<12) | MESSAGE_CONTENT (1<<15)
// We do NOT need GUILDS; INTERACTION_CREATE has no intent requirement (always dispatched).
const INTENTS: u64 = (1 << 9) | (1 << 12) | (1 << 15);

// Discord component types
const COMPONENT_TYPE_ACTION_ROW: u64 = 1;
const COMPONENT_TYPE_BUTTON: u64 = 2;
const BUTTON_STYLE_SUCCESS: u64 = 3;
const BUTTON_STYLE_DANGER: u64 = 4;

// Interaction types / response types
const INTERACTION_TYPE_COMPONENT: u64 = 3;
const INTERACTION_RESPONSE_UPDATE_MESSAGE: u64 = 7;

// Custom IDs for buttons
const BTN_APPROVE: &str = "openfeedback::approve";
const BTN_REJECT: &str = "openfeedback::reject";

pub struct DiscordProvider {
    config: DiscordConfig,
    client: Client,
    messages: Messages,
}

impl DiscordProvider {
    pub fn new(config: DiscordConfig, locale: Locale) -> Self {
        Self {
            config,
            client: Client::new(),
            messages: locale.messages(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bot {}", self.config.bot_token)
    }

    fn is_trusted(&self, user_id: &str) -> bool {
        self.config.trusted_user_ids.is_empty()
            || self.config.trusted_user_ids.iter().any(|u| u == user_id)
    }

    fn build_components(&self, disabled: bool) -> Value {
        json!([
            {
                "type": COMPONENT_TYPE_ACTION_ROW,
                "components": [
                    {
                        "type": COMPONENT_TYPE_BUTTON,
                        "style": BUTTON_STYLE_SUCCESS,
                        "label": self.messages.approve_button,
                        "custom_id": BTN_APPROVE,
                        "disabled": disabled,
                    },
                    {
                        "type": COMPONENT_TYPE_BUTTON,
                        "style": BUTTON_STYLE_DANGER,
                        "label": self.messages.reject_button,
                        "custom_id": BTN_REJECT,
                        "disabled": disabled,
                    }
                ]
            }
        ])
    }

    fn build_content(&self, request: &FeedbackRequest) -> String {
        // Discord content is plain-text/markdown; keep it simple and readable.
        format!(
            "\u{1F50D} **{}**\n\n{}\n\n_{}_",
            escape_md(&request.title),
            request.body,
            self.messages.prompt_text,
        )
    }

    async fn send_message(&self, request: &FeedbackRequest) -> Result<String> {
        let body = json!({
            "content": self.build_content(request),
            "components": self.build_components(false),
            "allowed_mentions": { "parse": [] },
        });

        let resp = self
            .client
            .post(format!(
                "{API_BASE}/channels/{}/messages",
                self.config.channel_id
            ))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .context("Failed to POST Discord message")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Discord sendMessage failed ({status}): {text}");
        }

        let msg: DiscordMessage = resp.json().await.context("Failed to parse message")?;
        info!(message_id = %msg.id, "Feedback request sent to Discord");
        Ok(msg.id)
    }

    async fn edit_remove_components(&self, message_id: &str) -> Result<()> {
        let body = json!({ "components": [] });
        self.client
            .patch(format!(
                "{API_BASE}/channels/{}/messages/{message_id}",
                self.config.channel_id
            ))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .context("Failed to PATCH message (clear components)")?;
        Ok(())
    }

    async fn respond_interaction_update(
        &self,
        interaction_id: &str,
        token: &str,
        new_content: String,
    ) -> Result<()> {
        // Type 7 = UPDATE_MESSAGE: edits the original message in a single call.
        let body = json!({
            "type": INTERACTION_RESPONSE_UPDATE_MESSAGE,
            "data": {
                "content": new_content,
                "components": [],
            }
        });
        self.client
            .post(format!(
                "{API_BASE}/interactions/{interaction_id}/{token}/callback"
            ))
            .json(&body)
            .send()
            .await
            .context("Failed to POST interaction callback")?;
        Ok(())
    }

    async fn send_plain(&self, content: &str, reply_to: Option<&str>) -> Result<String> {
        let mut body = json!({
            "content": content,
            "allowed_mentions": { "parse": [] },
        });
        if let Some(msg_id) = reply_to {
            body["message_reference"] = json!({ "message_id": msg_id });
        }
        let resp = self
            .client
            .post(format!(
                "{API_BASE}/channels/{}/messages",
                self.config.channel_id
            ))
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await
            .context("Failed to POST plain message")?;
        let msg: DiscordMessage = resp.json().await.unwrap_or(DiscordMessage::default());
        Ok(msg.id)
    }

    async fn send_and_wait_impl(
        &self,
        request: &FeedbackRequest,
    ) -> Result<FeedbackResponse> {
        let message_id = self.send_message(request).await?;

        // Connect gateway + listen for events, bounded by the request timeout.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(request.timeout_secs);

        let outcome =
            tokio::time::timeout_at(deadline, self.run_gateway(&message_id, request)).await;

        match outcome {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(e)) => {
                // Gateway failed mid-run — best-effort cleanup then surface the error.
                self.edit_remove_components(&message_id).await.ok();
                Err(e)
            }
            Err(_elapsed) => {
                info!("Discord: timeout reached, no response received");
                self.edit_remove_components(&message_id).await.ok();
                let notice = match request.timeout_kind {
                    TimeoutKind::Final => self.messages.timeout_notice,
                    TimeoutKind::Escalated => self.messages.escalated_notice,
                };
                self.send_plain(notice, None).await.ok();
                let mut resp = FeedbackResponse::timeout(&request.title);
                resp.provider = Some("discord".to_string());
                Ok(resp)
            }
        }
    }

    /// Establish a Gateway session and return as soon as a decision is made.
    async fn run_gateway(
        &self,
        message_id: &str,
        request: &FeedbackRequest,
    ) -> Result<FeedbackResponse> {
        let mut ws = connect_gateway().await?;
        debug!("Discord gateway connected");

        // 1. HELLO
        let hello = read_gateway(&mut ws).await?;
        let heartbeat_ms = hello["d"]["heartbeat_interval"]
            .as_u64()
            .context("HELLO missing heartbeat_interval")?;

        // 2. IDENTIFY
        let identify = json!({
            "op": 2,
            "d": {
                "token": self.config.bot_token,
                "intents": INTENTS,
                "properties": {
                    "os": std::env::consts::OS,
                    "browser": "openfeedback",
                    "device": "openfeedback",
                }
            }
        });
        ws.send(WsMessage::Text(identify.to_string())).await?;

        // 3. Set up heartbeat task via a channel so we can multiplex cleanly.
        // We'll run the heartbeat inline by interleaving tokio::select! with the read loop.
        let mut hb_interval = tokio::time::interval(Duration::from_millis(heartbeat_ms));
        hb_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // tick() fires immediately the first time — skip that so we don't race IDENTIFY.
        hb_interval.tick().await;

        let mut pending_feedback_prompt_id: Option<String> = None;
        let mut last_seq: Option<u64> = None;

        loop {
            tokio::select! {
                _ = hb_interval.tick() => {
                    let hb = json!({ "op": 1, "d": last_seq });
                    if ws.send(WsMessage::Text(hb.to_string())).await.is_err() {
                        anyhow::bail!("Gateway heartbeat send failed");
                    }
                }

                Some(ws_msg) = ws.next() => {
                    let msg = match ws_msg {
                        Ok(m) => m,
                        Err(e) => anyhow::bail!("Gateway read error: {e}"),
                    };
                    let text = match msg {
                        WsMessage::Text(t) => t,
                        WsMessage::Close(frame) => {
                            anyhow::bail!("Gateway closed: {:?}", frame);
                        }
                        WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Binary(_) | WsMessage::Frame(_) => continue,
                    };
                    let payload: Value = serde_json::from_str(&text)
                        .context("Gateway payload not JSON")?;
                    let op = payload["op"].as_u64().unwrap_or(999);
                    if let Some(s) = payload["s"].as_u64() {
                        last_seq = Some(s);
                    }
                    match op {
                        11 => { /* HEARTBEAT_ACK */ }
                        1 => {
                            // Server requested immediate heartbeat
                            let hb = json!({ "op": 1, "d": last_seq });
                            ws.send(WsMessage::Text(hb.to_string())).await.ok();
                        }
                        7 | 9 => {
                            // Reconnect / Invalid Session — don't bother reconnecting in a
                            // CLI one-shot, surface as error so orchestrator can react.
                            anyhow::bail!("Gateway asked us to reconnect/invalid-session (op {op})");
                        }
                        0 => {
                            let event = payload["t"].as_str().unwrap_or("");
                            match event {
                                "INTERACTION_CREATE" => {
                                    if let Some(result) = self.handle_interaction(
                                        &mut ws,
                                        &payload["d"],
                                        message_id,
                                        request,
                                        &mut pending_feedback_prompt_id,
                                    ).await? {
                                        // Decision made. If approve: done. If reject: wait briefly
                                        // for follow-up text, then return.
                                        let (decision, user, user_id) = result;
                                        let feedback = if decision == Decision::Rejected
                                            && request.reject_feedback_timeout_secs > 0
                                        {
                                            // Wait for a message event up to the feedback window,
                                            // while keeping the gateway alive for heartbeats.
                                            self.wait_for_feedback(
                                                &mut ws,
                                                &mut hb_interval,
                                                &mut last_seq,
                                                message_id,
                                                pending_feedback_prompt_id.as_deref(),
                                                request.reject_feedback_timeout_secs,
                                            ).await
                                        } else {
                                            None
                                        };

                                        let _ = ws.close(None).await;

                                        return Ok(FeedbackResponse {
                                            decision,
                                            user,
                                            user_id,
                                            feedback,
                                            timestamp: Utc::now(),
                                            request_title: request.title.clone(),
                                            provider: Some("discord".to_string()),
                                            escalated_from: None,
                                        });
                                    }
                                }
                                "MESSAGE_CREATE" => {
                                    // User replied to our request message directly:
                                    // treat as "approved with feedback" (same semantics as Telegram).
                                    if let Some((user, user_id, text)) =
                                        self.match_reply_to_original(&payload["d"], message_id)
                                    {
                                        self.edit_remove_components(message_id).await.ok();
                                        let _ = ws.close(None).await;
                                        return Ok(FeedbackResponse {
                                            decision: Decision::Approved,
                                            user,
                                            user_id,
                                            feedback: Some(text),
                                            timestamp: Utc::now(),
                                            request_title: request.title.clone(),
                                            provider: Some("discord".to_string()),
                                            escalated_from: None,
                                        });
                                    }
                                }
                                "READY" => {
                                    debug!("Discord gateway READY");
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Returns `Some((decision, user_display, user_id))` if a trusted user clicked a known button.
    /// Otherwise returns `None` (ignored / untrusted / not our message).
    async fn handle_interaction(
        &self,
        ws: &mut WsStream,
        d: &Value,
        our_message_id: &str,
        request: &FeedbackRequest,
        pending_feedback_prompt_id: &mut Option<String>,
    ) -> Result<Option<(Decision, String, i64)>> {
        let itype = d["type"].as_u64().unwrap_or(0);
        if itype != INTERACTION_TYPE_COMPONENT {
            return Ok(None);
        }

        let msg_id = d["message"]["id"].as_str().unwrap_or("");
        if msg_id != our_message_id {
            return Ok(None);
        }

        let interaction_id = d["id"].as_str().unwrap_or("").to_string();
        let token = d["token"].as_str().unwrap_or("").to_string();
        let custom_id = d["data"]["custom_id"].as_str().unwrap_or("");

        // Discord puts the acting user in "member.user" (guild) or "user" (DM).
        let user_obj = if d.get("member").is_some() {
            &d["member"]["user"]
        } else {
            &d["user"]
        };
        let user_id_str = user_obj["id"].as_str().unwrap_or("").to_string();

        if !self.is_trusted(&user_id_str) {
            warn!(user_id = %user_id_str, "Untrusted Discord user attempted action");
            // Silently respond (DEFERRED_UPDATE) to avoid "didn't respond" error.
            // Interaction callbacks do not require bot auth — the token in the
            // URL is the per-interaction credential.
            let body = json!({ "type": 6 });
            let _ = self
                .client
                .post(format!(
                    "{API_BASE}/interactions/{interaction_id}/{token}/callback"
                ))
                .json(&body)
                .send()
                .await;
            return Ok(None);
        }

        let decision = match custom_id {
            BTN_APPROVE => Decision::Approved,
            BTN_REJECT => Decision::Rejected,
            _ => return Ok(None),
        };

        let user_name = display_name(user_obj);
        let user_id_i64: i64 = user_id_str.parse().unwrap_or(0);

        // Respond with UPDATE_MESSAGE: clears components and shows final state inline.
        let updated = match decision {
            Decision::Approved => format!("{}\n\n{}", self.build_content(request), self.messages.approved_callback),
            Decision::Rejected => format!("{}\n\n{}", self.build_content(request), self.messages.rejected_callback),
            _ => self.build_content(request),
        };
        self.respond_interaction_update(&interaction_id, &token, updated)
            .await
            .ok();

        info!(decision = ?decision, user = %user_name, "Discord response received");

        // If rejected, send a follow-up prompt (reply to original) so the user can type a reason.
        if decision == Decision::Rejected {
            let prompt = strip_html(self.messages.reject_feedback_prompt);
            if let Ok(id) = self.send_plain(&prompt, Some(our_message_id)).await
                && !id.is_empty()
            {
                *pending_feedback_prompt_id = Some(id);
            }
        }

        // Keep gateway alive (we may want to receive MESSAGE_CREATE next).
        let _ = ws;
        Ok(Some((decision, user_name, user_id_i64)))
    }

    async fn wait_for_feedback(
        &self,
        ws: &mut WsStream,
        hb_interval: &mut tokio::time::Interval,
        last_seq: &mut Option<u64>,
        our_message_id: &str,
        feedback_prompt_id: Option<&str>,
        wait_secs: u64,
    ) -> Option<String> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_secs);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return None,
                _ = hb_interval.tick() => {
                    let hb = json!({ "op": 1, "d": *last_seq });
                    let _ = ws.send(WsMessage::Text(hb.to_string())).await;
                }
                Some(Ok(WsMessage::Text(text))) = ws.next() => {
                    let payload: Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    if let Some(s) = payload["s"].as_u64() { *last_seq = Some(s); }
                    if payload["op"].as_u64() != Some(0) { continue; }
                    if payload["t"].as_str() != Some("MESSAGE_CREATE") { continue; }
                    let d = &payload["d"];

                    // Accept: reply to original request, reply to our "please explain" prompt,
                    // or any message from a trusted user in this channel right after reject.
                    let channel_ok = d["channel_id"].as_str() == Some(self.config.channel_id.as_str());
                    if !channel_ok { continue; }

                    let author_id = d["author"]["id"].as_str().unwrap_or("").to_string();
                    if !self.is_trusted(&author_id) { continue; }

                    let ref_id = d["message_reference"]["message_id"].as_str();
                    let is_reply_to_us = ref_id == Some(our_message_id)
                        || (feedback_prompt_id.is_some() && ref_id == feedback_prompt_id);

                    let text_content = d["content"].as_str().unwrap_or("").to_string();
                    if text_content.is_empty() { continue; }

                    if is_reply_to_us {
                        return Some(text_content);
                    }
                    // If not a reply, accept the first text message from trusted user
                    // (best-effort match, same flexibility as Telegram's wait_for_reject_feedback).
                    return Some(text_content);
                }
            }
        }
    }

    fn match_reply_to_original(
        &self,
        d: &Value,
        our_message_id: &str,
    ) -> Option<(String, i64, String)> {
        if d["channel_id"].as_str() != Some(self.config.channel_id.as_str()) {
            return None;
        }
        // Ignore bot's own messages
        if d["author"]["bot"].as_bool() == Some(true) {
            return None;
        }
        let ref_id = d["message_reference"]["message_id"].as_str()?;
        if ref_id != our_message_id {
            return None;
        }
        let user_id_str = d["author"]["id"].as_str().unwrap_or("");
        if !self.is_trusted(user_id_str) {
            return None;
        }
        let user_id: i64 = user_id_str.parse().unwrap_or(0);
        let user_name = display_name(&d["author"]);
        let content = d["content"].as_str().unwrap_or("").to_string();
        if content.is_empty() {
            return None;
        }
        Some((user_name, user_id, content))
    }
}

#[async_trait]
impl Provider for DiscordProvider {
    async fn send_and_wait(&self, request: &FeedbackRequest) -> Result<FeedbackResponse> {
        self.send_and_wait_impl(request).await
    }
}

// --- helpers & local types ---

type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

#[derive(Debug, Deserialize, Default)]
struct DiscordMessage {
    #[serde(default)]
    id: String,
}

/// Establish a Gateway WSS connection, honoring `HTTPS_PROXY` / `HTTP_PROXY`
/// (HTTP CONNECT tunneling) when set. Falls back to direct connect if the
/// proxy attempt fails.
async fn connect_gateway() -> Result<WsStream> {
    let proxy = std::env::var("HTTPS_PROXY")
        .ok()
        .or_else(|| std::env::var("https_proxy").ok())
        .or_else(|| std::env::var("HTTP_PROXY").ok())
        .or_else(|| std::env::var("http_proxy").ok());

    if let Some(proxy_url) = proxy {
        debug!(proxy = %proxy_url, "Attempting Discord gateway via HTTP CONNECT proxy");
        match connect_via_http_proxy(&proxy_url, "gateway.discord.gg", 443).await {
            Ok(stream) => {
                let (ws, _) = tokio_tungstenite::client_async_tls(GATEWAY_URL, stream)
                    .await
                    .context("WSS handshake over proxy failed")?;
                return Ok(ws);
            }
            Err(e) => {
                warn!(error = %e, "Proxy connect failed, falling back to direct");
            }
        }
    }

    let (ws, _) = tokio_tungstenite::connect_async(GATEWAY_URL)
        .await
        .context("Failed to connect Discord gateway")?;
    Ok(ws)
}

async fn connect_via_http_proxy(
    proxy_url: &str,
    target_host: &str,
    target_port: u16,
) -> Result<tokio::net::TcpStream> {
    // Parse "http://host:port" (schemes other than http:// are unsupported
    // for CONNECT; we only support plain-HTTP proxies, which is standard).
    let rest = proxy_url
        .strip_prefix("http://")
        .or_else(|| proxy_url.strip_prefix("HTTP://"))
        .unwrap_or(proxy_url);
    let rest = rest.trim_end_matches('/');
    // Strip any userinfo / path — keep host[:port] only.
    let rest = rest.split('@').next_back().unwrap_or(rest);
    let rest = rest.split('/').next().unwrap_or(rest);
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(80)),
        None => (rest.to_string(), 80),
    };

    let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
        .await
        .with_context(|| format!("Failed to connect to proxy {host}:{port}"))?;

    let req = format!(
        "CONNECT {target_host}:{target_port} HTTP/1.1\r\nHost: {target_host}:{target_port}\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;

    // Read until end of headers (\r\n\r\n).
    let mut buf = vec![0u8; 2048];
    let mut total = 0usize;
    loop {
        if total >= buf.len() {
            anyhow::bail!("Proxy CONNECT response too large");
        }
        let n = stream.read(&mut buf[total..]).await?;
        if n == 0 {
            anyhow::bail!("Proxy closed connection during CONNECT");
        }
        total += n;
        if let Some(_idx) = buf[..total].windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&buf[..total]);
            let first_line = headers.lines().next().unwrap_or("");
            if !first_line.contains(" 200") {
                anyhow::bail!("Proxy CONNECT failed: {first_line}");
            }
            return Ok(stream);
        }
    }
}

async fn read_gateway(ws: &mut WsStream) -> Result<Value> {
    loop {
        let m = ws
            .next()
            .await
            .context("Gateway closed before HELLO")?
            .context("Gateway read error during HELLO")?;
        if let WsMessage::Text(t) = m {
            return serde_json::from_str(&t).context("Gateway payload not JSON");
        }
    }
}

fn display_name(user: &Value) -> String {
    if let Some(global) = user["global_name"].as_str()
        && !global.is_empty()
    {
        return global.to_string();
    }
    if let Some(u) = user["username"].as_str() {
        return format!("@{u}");
    }
    "unknown".to_string()
}

/// Escape Discord markdown-active chars in short titles. Body text is left as-is
/// (users deliberately pass markdown bodies).
fn escape_md(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('~', "\\~")
}

/// Reject-feedback prompt is stored as HTML (Telegram-flavored).
/// Strip HTML tags for Discord.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

