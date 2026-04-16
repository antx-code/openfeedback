use anyhow::Result;
use tracing::info;

use super::{build, Provider};
use crate::config::Config;
use crate::types::{Decision, FeedbackRequest, FeedbackResponse, TimeoutKind};

/// Resolve which primary/secondary providers to use, given a config and an
/// optional CLI override.
///
/// - If `override_primary` is set, only that provider runs (no failover).
/// - Otherwise, use `config.default_provider` as primary and
///   `config.failover_provider` as optional secondary.
pub struct Plan {
    pub primary: Box<dyn Provider>,
    pub primary_name: String,
    pub secondary: Option<Box<dyn Provider>>,
    pub secondary_name: Option<String>,
    pub escalate_after_secs: u64,
}

pub fn plan(config: &Config, override_primary: Option<&str>) -> Result<Plan> {
    if let Some(name) = override_primary {
        config.validate_provider(name)?;
        return Ok(Plan {
            primary: build(name, config)?,
            primary_name: name.to_string(),
            secondary: None,
            secondary_name: None,
            escalate_after_secs: 0,
        });
    }

    let primary_name = config.default_provider.clone();
    let primary = build(&primary_name, config)?;

    let (secondary, secondary_name, escalate_after) = match config.failover_provider.as_deref() {
        Some(fp) => {
            let after = config
                .escalate_after_secs
                .unwrap_or(config.default_timeout / 2)
                .max(1);
            (Some(build(fp, config)?), Some(fp.to_string()), after)
        }
        None => (None, None, 0),
    };

    Ok(Plan {
        primary,
        primary_name,
        secondary,
        secondary_name,
        escalate_after_secs: escalate_after,
    })
}

/// Run the plan: primary first, escalate to secondary only on timeout.
/// Any non-timeout decision (approve/reject) from primary is final.
pub async fn run(plan: Plan, total_timeout: u64, request: FeedbackRequest) -> Result<FeedbackResponse> {
    let Plan {
        primary,
        primary_name,
        secondary,
        secondary_name,
        escalate_after_secs,
    } = plan;

    // No secondary → primary owns the full budget and any timeout is final.
    let primary_budget = if secondary.is_some() {
        escalate_after_secs.min(total_timeout)
    } else {
        total_timeout
    };

    let primary_kind = if secondary.is_some() {
        TimeoutKind::Escalated
    } else {
        TimeoutKind::Final
    };

    let primary_req = FeedbackRequest {
        timeout_secs: primary_budget,
        timeout_kind: primary_kind,
        ..request.clone()
    };

    info!(
        primary = %primary_name,
        budget = primary_budget,
        has_secondary = secondary.is_some(),
        "Dispatching to primary provider"
    );

    let resp = primary.send_and_wait(&primary_req).await?;

    // Any non-timeout decision ends the flow.
    if resp.decision != Decision::Timeout || secondary.is_none() {
        return Ok(resp);
    }

    // Escalate.
    let secondary = secondary.expect("checked above");
    let secondary_name = secondary_name.expect("paired with secondary");
    let remaining = total_timeout.saturating_sub(primary_budget);

    info!(
        secondary = %secondary_name,
        budget = remaining,
        "Primary timed out — escalating"
    );

    let secondary_req = FeedbackRequest {
        timeout_secs: remaining,
        timeout_kind: TimeoutKind::Final,
        ..request
    };

    let mut resp = secondary.send_and_wait(&secondary_req).await?;
    // Decorate with escalation lineage.
    if resp.escalated_from.is_none() {
        resp.escalated_from = Some(primary_name);
    }
    Ok(resp)
}
