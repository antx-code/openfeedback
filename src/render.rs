use anyhow::{Context, Result};
use std::path::Path;

/// Read body content from file or use inline text.
pub fn load_body(body_file: Option<&str>, body_text: Option<&str>) -> Result<String> {
    match (body_file, body_text) {
        (Some(path), _) => {
            std::fs::read_to_string(Path::new(path))
                .with_context(|| format!("Failed to read body file: {path}"))
        }
        (_, Some(text)) => Ok(text.to_string()),
        (None, None) => anyhow::bail!("Either --body-file or --body must be provided"),
    }
}
