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
    pub telegram: Option<TelegramConfig>,
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

    fn validate(&self) -> Result<()> {
        match self.default_provider.as_str() {
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
            other => anyhow::bail!("Unknown provider: {other}"),
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

[telegram]
bot_token = "YOUR_BOT_TOKEN"
chat_id = 0
trusted_user_ids = []

[logging]
# audit_file = "~/.local/share/openfeedback/audit.jsonl"
"#
        .to_string()
    }
}
