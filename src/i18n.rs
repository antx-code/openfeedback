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
    pub timeout_notice: &'static str,
    pub escalated_notice: &'static str,
}

impl Locale {
    pub fn messages(self) -> Messages {
        match self {
            Self::En => Messages {
                approve_button: "\u{2705} Approve",
                reject_button: "\u{274C} Reject",
                prompt_text: "Please approve or reject this request.",
                approved_callback: "Approved \u{2714}",
                rejected_callback: "Rejected \u{2714}",
                reject_feedback_prompt: "\u{274C} <b>Rejected.</b> Reply to this message to add a reason (or ignore to skip).",
                timeout_notice: "\u{23F0} Request timed out \u{2014} no response received.",
                escalated_notice: "\u{23E9} No response \u{2014} escalated to the next channel.",
            },
            Self::ZhCN => Messages {
                approve_button: "\u{2705} \u{6279}\u{51C6}",
                reject_button: "\u{274C} \u{62D2}\u{7EDD}",
                prompt_text: "\u{8BF7}\u{6279}\u{51C6}\u{6216}\u{62D2}\u{7EDD}\u{6B64}\u{8BF7}\u{6C42}\u{3002}",
                approved_callback: "\u{5DF2}\u{6279}\u{51C6} \u{2714}",
                rejected_callback: "\u{5DF2}\u{62D2}\u{7EDD} \u{2714}",
                reject_feedback_prompt: "\u{274C} <b>\u{5DF2}\u{62D2}\u{7EDD}\u{3002}</b>\u{56DE}\u{590D}\u{6B64}\u{6D88}\u{606F}\u{4EE5}\u{6DFB}\u{52A0}\u{539F}\u{56E0}\u{FF08}\u{5FFD}\u{7565}\u{5219}\u{8DF3}\u{8FC7}\u{FF09}\u{3002}",
                timeout_notice: "\u{23F0} \u{8BF7}\u{6C42}\u{5DF2}\u{8D85}\u{65F6} \u{2014} \u{672A}\u{6536}\u{5230}\u{54CD}\u{5E94}\u{3002}",
                escalated_notice: "\u{23E9} \u{65E0}\u{54CD}\u{5E94} \u{2014} \u{5DF2}\u{5347}\u{7EA7}\u{81F3}\u{4E0B}\u{4E00}\u{4E2A}\u{901A}\u{9053}\u{3002}",
            },
            Self::ZhTW => Messages {
                approve_button: "\u{2705} \u{6279}\u{51C6}",
                reject_button: "\u{274C} \u{62D2}\u{7D55}",
                prompt_text: "\u{8ACB}\u{6279}\u{51C6}\u{6216}\u{62D2}\u{7D55}\u{6B64}\u{8ACB}\u{6C42}\u{3002}",
                approved_callback: "\u{5DF2}\u{6279}\u{51C6} \u{2714}",
                rejected_callback: "\u{5DF2}\u{62D2}\u{7D55} \u{2714}",
                reject_feedback_prompt: "\u{274C} <b>\u{5DF2}\u{62D2}\u{7D55}\u{3002}</b>\u{56DE}\u{8986}\u{6B64}\u{8A0A}\u{606F}\u{4EE5}\u{9644}\u{4E0A}\u{539F}\u{56E0}\u{FF08}\u{5FFD}\u{7565}\u{5247}\u{8DF3}\u{904E}\u{FF09}\u{3002}",
                timeout_notice: "\u{23F0} \u{8ACB}\u{6C42}\u{5DF2}\u{8D85}\u{6642} \u{2014} \u{672A}\u{6536}\u{5230}\u{56DE}\u{61C9}\u{3002}",
                escalated_notice: "\u{23E9} \u{7121}\u{56DE}\u{61C9} \u{2014} \u{5DF2}\u{5347}\u{7D1A}\u{81F3}\u{4E0B}\u{4E00}\u{500B}\u{901A}\u{9053}\u{3002}",
            },
        }
    }
}
