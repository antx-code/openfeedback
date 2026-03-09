use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Locale {
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCN,
    #[serde(rename = "zh-TW")]
    ZhTW,
}

impl Default for Locale {
    fn default() -> Self {
        Self::En
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::En => write!(f, "en"),
            Self::ZhCN => write!(f, "zh-CN"),
            Self::ZhTW => write!(f, "zh-TW"),
        }
    }
}

pub struct Messages {
    pub approve_button: &'static str,
    pub reject_button: &'static str,
    pub prompt_text: &'static str,
    pub approved_callback: &'static str,
    pub rejected_callback: &'static str,
    pub reject_feedback_prompt: &'static str,
    pub reject_feedback_callback: &'static str,
    pub timeout_notice: &'static str,
}

impl Locale {
    pub fn messages(self) -> Messages {
        match self {
            Self::En => Messages {
                approve_button: "✅ Approve",
                reject_button: "❌ Reject",
                prompt_text: "Please approve or reject this request.",
                approved_callback: "Approved ✔",
                rejected_callback: "Rejected ✔",
                reject_feedback_prompt: "❌ <b>Rejected.</b> Reply to this message within 60 seconds to add a reason (or ignore to skip).",
                reject_feedback_callback: "Rejected with feedback ✔",
                timeout_notice: "⏰ Request timed out — no response received.",
            },
            Self::ZhCN => Messages {
                approve_button: "✅ 批准",
                reject_button: "❌ 拒绝",
                prompt_text: "请批准或拒绝此请求。",
                approved_callback: "已批准 ✔",
                rejected_callback: "已拒绝 ✔",
                reject_feedback_prompt: "❌ <b>已拒绝。</b>请在 60 秒内回复此消息以添加原因（忽略则跳过）。",
                reject_feedback_callback: "已拒绝并附上原因 ✔",
                timeout_notice: "⏰ 请求已超时 — 未收到响应。",
            },
            Self::ZhTW => Messages {
                approve_button: "✅ 批准",
                reject_button: "❌ 拒絕",
                prompt_text: "請批准或拒絕此請求。",
                approved_callback: "已批准 ✔",
                rejected_callback: "已拒絕 ✔",
                reject_feedback_prompt: "❌ <b>已拒絕。</b>請在 60 秒內回覆此訊息以附上原因（忽略則跳過）。",
                reject_feedback_callback: "已拒絕並附上原因 ✔",
                timeout_notice: "⏰ 請求已超時 — 未收到回應。",
            },
        }
    }
}
