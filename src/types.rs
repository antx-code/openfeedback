use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRequest {
    pub title: String,
    pub body: String,
    pub timeout_secs: u64,
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
