use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Locale;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_timeout")]
    pub default_timeout: u64,
    #[serde(default = "default_reject_feedback_timeout")]
    pub reject_feedback_timeout: u64,
    #[serde(default)]
    pub locale: Locale,

    /// Optional secondary provider. When set, the primary provider's
    /// timeout becomes a soft failover handoff.
    #[serde(default)]
    pub failover_provider: Option<String>,
    /// How long to wait on the primary before escalating.
    /// If unset, defaults to half of `default_timeout`.
    #[serde(default)]
    pub escalate_after_secs: Option<u64>,

    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: i64,
    #[serde(default)]
    pub trusted_user_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub bot_token: String,
    /// Discord snowflakes are 64-bit unsigned; store as string to avoid JSON precision issues.
    pub channel_id: String,
    /// Required when using a bot token — your Discord application ID.
    pub application_id: String,
    #[serde(default)]
    pub trusted_user_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_audit_file")]
    pub audit_file: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            audit_file: default_audit_file(),
        }
    }
}

fn default_provider() -> String {
    "telegram".to_string()
}

fn default_timeout() -> u64 {
    3600
}

fn default_reject_feedback_timeout() -> u64 {
    60
}

fn default_audit_file() -> String {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("openfeedback");
    base.join("audit.jsonl")
        .to_string_lossy()
        .into_owned()
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("openfeedback")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            anyhow::bail!(
                "Config file not found at {}\nRun `openfeedback init` to create one.",
                path.display()
            );
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate_provider(&self, provider: &str) -> Result<()> {
        match provider {
            "telegram" => {
                let tg = self
                    .telegram
                    .as_ref()
                    .context("telegram provider selected but [telegram] section missing")?;
                if tg.bot_token.is_empty() {
                    anyhow::bail!("telegram.bot_token cannot be empty");
                }
                if tg.chat_id == 0 {
                    anyhow::bail!("telegram.chat_id cannot be 0");
                }
            }
            "discord" => {
                let dc = self
                    .discord
                    .as_ref()
                    .context("discord provider selected but [discord] section missing")?;
                if dc.bot_token.is_empty() {
                    anyhow::bail!("discord.bot_token cannot be empty");
                }
                if dc.channel_id.is_empty() || dc.channel_id == "0" {
                    anyhow::bail!("discord.channel_id cannot be empty");
                }
                if dc.application_id.is_empty() {
                    anyhow::bail!("discord.application_id cannot be empty");
                }
            }
            other => anyhow::bail!("Unknown provider: {other}"),
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        self.validate_provider(&self.default_provider)?;
        if let Some(ref fp) = self.failover_provider {
            if fp == &self.default_provider {
                anyhow::bail!(
                    "failover_provider must differ from default_provider (both are {fp})"
                );
            }
            self.validate_provider(fp)?;
            // If escalate_after_secs is explicitly set, it must leave room for
            // the secondary to actually do something. Otherwise the secondary
            // would be invoked with a 0-second budget and time out instantly.
            if let Some(after) = self.escalate_after_secs
                && after >= self.default_timeout
            {
                anyhow::bail!(
                    "escalate_after_secs ({after}) must be less than default_timeout ({}) \
                     so the failover provider has a non-zero budget",
                    self.default_timeout
                );
            }
        }
        Ok(())
    }

    pub fn generate_default() -> String {
        r#"default_provider = "telegram"
default_timeout = 3600
# Seconds to wait for reject feedback (0 = skip)
reject_feedback_timeout = 60
# locale: "en" (default), "zh-CN", "zh-TW"
locale = "en"

# --- Optional failover ---
# If set, when the primary provider times out without a decision,
# openfeedback cleans up (removes buttons + sends an "escalated" notice)
# and hands off to this secondary provider.
# failover_provider = "discord"
# escalate_after_secs = 1800   # default: half of default_timeout

[telegram]
bot_token = "YOUR_BOT_TOKEN"
chat_id = 0
trusted_user_ids = []

# [discord]
# bot_token = "YOUR_BOT_TOKEN"
# application_id = "YOUR_APPLICATION_ID"
# channel_id = "YOUR_CHANNEL_ID"
# trusted_user_ids = []

[logging]
# audit_file = "~/.local/share/openfeedback/audit.jsonl"
"#
        .to_string()
    }
}
