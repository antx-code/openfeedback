use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::TelegramConfig;
use crate::types::{Decision, FeedbackRequest, FeedbackResponse};

use super::Provider;

const POLL_INTERVAL: Duration = Duration::from_secs(3);

pub struct TelegramProvider {
    config: TelegramConfig,
    client: Client,
    base_url: String,
}

// --- Telegram API types ---

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
    parse_mode: String,
    reply_markup: Option<InlineKeyboardMarkup>,
}

#[derive(Debug, Serialize)]
struct InlineKeyboardMarkup {
    inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Debug, Serialize)]
struct InlineKeyboardButton {
    text: String,
    callback_data: String,
}

#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Message {
    message_id: i64,
}

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    callback_query: Option<CallbackQuery>,
    message: Option<ReplyMessage>,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    id: String,
    from: User,
    message: Option<CallbackMessage>,
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CallbackMessage {
    message_id: i64,
    chat: Chat,
}

#[derive(Debug, Deserialize)]
struct ReplyMessage {
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
    reply_to_message: Option<Box<ReplyToMessage>>,
}

#[derive(Debug, Deserialize)]
struct ReplyToMessage {
    message_id: i64,
}

#[derive(Debug, Deserialize)]
struct User {
    id: i64,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Chat {
    id: i64,
}

impl User {
    fn display_name(&self) -> String {
        if let Some(ref username) = self.username {
            format!("@{username}")
        } else if let Some(ref last) = self.last_name {
            format!("{} {last}", self.first_name)
        } else {
            self.first_name.clone()
        }
    }
}

impl TelegramProvider {
    pub fn new(config: TelegramConfig) -> Self {
        let base_url = format!("https://api.telegram.org/bot{}", config.bot_token);
        Self {
            config,
            client: Client::new(),
            base_url,
        }
    }

    fn is_trusted(&self, user_id: i64) -> bool {
        self.config.trusted_user_ids.is_empty()
            || self.config.trusted_user_ids.contains(&user_id)
    }

    async fn send_message(&self, request: &FeedbackRequest) -> Result<i64> {
        let text = format!(
            "\u{1F50D} <b>{}</b>\n\n{}\n\n<i>Please approve or reject this request.</i>",
            escape_html(&request.title),
            escape_html(&request.body),
        );

        let keyboard = InlineKeyboardMarkup {
            inline_keyboard: vec![vec![
                InlineKeyboardButton {
                    text: "\u{2705} Approve".to_string(),
                    callback_data: "approve".to_string(),
                },
                InlineKeyboardButton {
                    text: "\u{274C} Reject".to_string(),
                    callback_data: "reject".to_string(),
                },
            ]],
        };

        let body = SendMessageRequest {
            chat_id: self.config.chat_id,
            text,
            parse_mode: "HTML".to_string(),
            reply_markup: Some(keyboard),
        };

        let resp: TelegramResponse<Message> = self
            .client
            .post(format!("{}/sendMessage", self.base_url))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if !resp.ok {
            anyhow::bail!(
                "Telegram sendMessage failed: {}",
                resp.description.unwrap_or_default()
            );
        }

        let msg_id = resp.result.context("No message in response")?.message_id;
        info!(message_id = msg_id, "Feedback request sent to Telegram");
        Ok(msg_id)
    }

    async fn answer_callback(&self, callback_id: &str, text: &str) -> Result<()> {
        #[derive(Serialize)]
        struct Req {
            callback_query_id: String,
            text: String,
        }
        self.client
            .post(format!("{}/answerCallbackQuery", self.base_url))
            .json(&Req {
                callback_query_id: callback_id.to_string(),
                text: text.to_string(),
            })
            .send()
            .await?;
        Ok(())
    }

    async fn edit_message_reply_markup(&self, chat_id: i64, message_id: i64) -> Result<()> {
        #[derive(Serialize)]
        struct Req {
            chat_id: i64,
            message_id: i64,
            reply_markup: InlineKeyboardMarkup,
        }
        // Remove inline keyboard after decision
        self.client
            .post(format!("{}/editMessageReplyMarkup", self.base_url))
            .json(&Req {
                chat_id,
                message_id,
                reply_markup: InlineKeyboardMarkup {
                    inline_keyboard: vec![],
                },
            })
            .send()
            .await?;
        Ok(())
    }

    async fn poll_for_response(
        &self,
        sent_message_id: i64,
        timeout: Duration,
        title: &str,
    ) -> Result<FeedbackResponse> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut offset: Option<i64> = None;

        loop {
            if tokio::time::Instant::now() >= deadline {
                info!("Timeout reached, no response received");
                return Ok(FeedbackResponse::timeout(title));
            }

            let remaining = deadline - tokio::time::Instant::now();
            let poll_timeout = remaining.min(Duration::from_secs(30));

            let mut url = format!(
                "{}/getUpdates?timeout={}&allowed_updates=[\"callback_query\",\"message\"]",
                self.base_url,
                poll_timeout.as_secs()
            );
            if let Some(off) = offset {
                url.push_str(&format!("&offset={off}"));
            }

            let resp: TelegramResponse<Vec<Update>> =
                self.client.get(&url).send().await?.json().await?;

            let updates = match resp.result {
                Some(u) => u,
                None => continue,
            };

            for update in updates {
                offset = Some(update.update_id + 1);

                // Handle button click
                if let Some(cb) = update.callback_query {
                    let data = cb.data.as_deref().unwrap_or("");
                    let cb_msg = match cb.message {
                        Some(ref m) => m,
                        None => continue,
                    };

                    // Must be our message
                    if cb_msg.message_id != sent_message_id {
                        continue;
                    }

                    if !self.is_trusted(cb.from.id) {
                        warn!(user_id = cb.from.id, "Untrusted user attempted action");
                        self.answer_callback(&cb.id, "You are not authorized.")
                            .await
                            .ok();
                        continue;
                    }

                    let decision = match data {
                        "approve" => Decision::Approved,
                        "reject" => Decision::Rejected,
                        _ => continue,
                    };

                    let label = if decision == Decision::Approved {
                        "Approved"
                    } else {
                        "Rejected"
                    };
                    self.answer_callback(&cb.id, &format!("{label} \u{2714}"))
                        .await
                        .ok();
                    self.edit_message_reply_markup(cb_msg.chat.id, cb_msg.message_id)
                        .await
                        .ok();

                    info!(
                        decision = label,
                        user = cb.from.display_name(),
                        "Response received"
                    );

                    return Ok(FeedbackResponse {
                        decision,
                        user: cb.from.display_name(),
                        user_id: cb.from.id,
                        feedback: None,
                        timestamp: Utc::now(),
                        request_title: title.to_string(),
                    });
                }

                // Handle text reply to our message (as feedback)
                if let Some(msg) = update.message {
                    if msg.chat.id != self.config.chat_id {
                        continue;
                    }
                    if let Some(ref reply_to) = msg.reply_to_message {
                        if reply_to.message_id == sent_message_id {
                            let user = msg.from.as_ref();
                            let user_id = user.map_or(0, |u| u.id);

                            if !self.is_trusted(user_id) {
                                warn!(user_id, "Untrusted user replied");
                                continue;
                            }

                            let feedback_text = msg.text.clone();
                            let user_name = user
                                .map(|u| u.display_name())
                                .unwrap_or_else(|| "unknown".to_string());

                            info!(
                                user = %user_name,
                                feedback = ?feedback_text,
                                "Text feedback received (treating as approval with feedback)"
                            );

                            // Text reply = approved with feedback
                            self.edit_message_reply_markup(
                                self.config.chat_id,
                                sent_message_id,
                            )
                            .await
                            .ok();

                            return Ok(FeedbackResponse {
                                decision: Decision::Approved,
                                user: user_name,
                                user_id,
                                feedback: feedback_text,
                                timestamp: Utc::now(),
                                request_title: title.to_string(),
                            });
                        }
                    }
                }
            }

            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

#[async_trait]
impl Provider for TelegramProvider {
    async fn send_and_wait(&self, request: &FeedbackRequest) -> Result<FeedbackResponse> {
        let msg_id = self.send_message(request).await?;
        let timeout = Duration::from_secs(request.timeout_secs);
        self.poll_for_response(msg_id, timeout, &request.title).await
    }
}

/// Escape special characters for Telegram HTML parse mode
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
