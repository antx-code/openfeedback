pub mod telegram;

use anyhow::Result;
use async_trait::async_trait;

use crate::types::{FeedbackRequest, FeedbackResponse};

#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a feedback request and block until a response is received or timeout.
    async fn send_and_wait(&self, request: &FeedbackRequest) -> Result<FeedbackResponse>;
}
