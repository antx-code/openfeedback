use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Controls what notice is sent when a provider hits its timeout budget.
///
/// - `Final` — user-visible timeout, exits the CLI with code 2.
/// - `Escalated` — soft timeout during failover; the primary cleans up
///   and the orchestrator hands off to the secondary provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TimeoutKind {
    #[default]
    Final,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub title: String,
    pub body: String,
    pub timeout_secs: u64,
    pub reject_feedback_timeout_secs: u64,
    #[serde(default)]
    pub timeout_kind: TimeoutKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Approved,
    Rejected,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackResponse {
    pub decision: Decision,
    pub user: String,
    pub user_id: i64,
    pub feedback: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub request_title: String,
    /// Which provider actually produced this response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// If the decision came from a secondary provider after the primary
    /// escalated, the primary provider name is recorded here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalated_from: Option<String>,
}

impl FeedbackResponse {
    pub fn timeout(title: &str) -> Self {
        Self {
            decision: Decision::Timeout,
            user: String::new(),
            user_id: 0,
            feedback: None,
            timestamp: Utc::now(),
            request_title: title.to_string(),
            provider: None,
            escalated_from: None,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self.decision {
            Decision::Approved => 0,
            Decision::Rejected => 1,
            Decision::Timeout => 2,
        }
    }
}
