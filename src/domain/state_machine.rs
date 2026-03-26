use anyhow::{anyhow, Result};

use super::task::TaskStatus;

pub fn can_transition(from: &TaskStatus, to: &TaskStatus) -> bool {
    matches!(
        (from, to),
        (TaskStatus::Created, TaskStatus::Queued)
            | (TaskStatus::Created, TaskStatus::Cancelled)
            | (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Queued, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Succeeded)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Timeout)
    )
}

pub fn ensure_transition(from: &TaskStatus, to: &TaskStatus) -> Result<()> {
    if can_transition(from, to) {
        Ok(())
    } else {
        Err(anyhow!("invalid task status transition"))
    }
}
