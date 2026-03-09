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
            },
            Self::ZhCN => Messages {
                approve_button: "\u{2705} \u{6279}\u{51C6}",
                reject_button: "\u{274C} \u{62D2}\u{7EDD}",
                prompt_text: "\u{8BF7}\u{6279}\u{51C6}\u{6216}\u{62D2}\u{7EDD}\u{6B64}\u{8BF7}\u{6C42}\u{3002}",
                approved_callback: "\u{5DF2}\u{6279}\u{51C6} \u{2714}",
                rejected_callback: "\u{5DF2}\u{62D2}\u{7EDD} \u{2714}",
            },
            Self::ZhTW => Messages {
                approve_button: "\u{2705} \u{6279}\u{51C6}",
                reject_button: "\u{274C} \u{62D2}\u{7D55}",
                prompt_text: "\u{8ACB}\u{6279}\u{51C6}\u{6216}\u{62D2}\u{7D55}\u{6B64}\u{8ACB}\u{6C42}\u{3002}",
                approved_callback: "\u{5DF2}\u{6279}\u{51C6} \u{2714}",
                rejected_callback: "\u{5DF2}\u{62D2}\u{7D55} \u{2714}",
            },
        }
    }
}
