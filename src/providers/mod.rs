pub mod discord;
pub mod orchestrator;
pub mod telegram;

use anyhow::Result;
use async_trait::async_trait;

use crate::types::{FeedbackRequest, FeedbackResponse};

#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a feedback request and block until a response is received or timeout.
    async fn send_and_wait(&self, request: &FeedbackRequest) -> Result<FeedbackResponse>;
}

/// Build a provider by name from the given config. Centralizes the `match` so
/// both the single-provider and failover paths stay in sync.
pub fn build(
    name: &str,
    config: &crate::config::Config,
) -> Result<Box<dyn Provider>> {
    match name {
        "telegram" => {
            let tg = config
                .telegram
                .clone()
                .ok_or_else(|| anyhow::anyhow!("telegram config missing"))?;
            Ok(Box::new(telegram::TelegramProvider::new(tg, config.locale)))
        }
        "discord" => {
            let dc = config
                .discord
                .clone()
                .ok_or_else(|| anyhow::anyhow!("discord config missing"))?;
            Ok(Box::new(discord::DiscordProvider::new(dc, config.locale)))
        }
        other => anyhow::bail!("Unknown provider: {other}"),
    }
}
