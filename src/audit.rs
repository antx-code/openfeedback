use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::types::FeedbackResponse;

pub fn log_response(audit_file: &str, response: &FeedbackResponse) -> Result<()> {
    let path = Path::new(audit_file);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create audit dir: {}", parent.display()))?;
    }

    let line = serde_json::to_string(response)
        .context("Failed to serialize audit entry")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open audit file: {}", path.display()))?;

    writeln!(file, "{line}")
        .context("Failed to write audit entry")?;

    tracing::debug!(path = audit_file, "Audit entry written");
    Ok(())
}
