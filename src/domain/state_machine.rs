use anyhow::{anyhow, Result};

use super::task::TaskStatus;

pub fn can_transition(from: &TaskStatus, to: &TaskStatus) -> bool {
    matches!(
        (from, to),
        (TaskStatus::Pending, TaskStatus::Queued)
            | (TaskStatus::Pending, TaskStatus::Cancelled)
            | (TaskStatus::Queued, TaskStatus::Running)
            | (TaskStatus::Queued, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Succeeded)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::TimedOut)
            | (TaskStatus::Failed, TaskStatus::Queued)
            | (TaskStatus::TimedOut, TaskStatus::Queued)
    )
}

pub fn ensure_transition(from: &TaskStatus, to: &TaskStatus) -> Result<()> {
    if can_transition(from, to) {
        Ok(())
    } else {
        Err(anyhow!(
            "invalid task status transition: {} -> {}",
            from,
            to
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_transitions() {
        assert!(can_transition(&TaskStatus::Pending, &TaskStatus::Queued));
        assert!(can_transition(&TaskStatus::Queued, &TaskStatus::Running));
        assert!(can_transition(&TaskStatus::Running, &TaskStatus::Succeeded));
        assert!(can_transition(&TaskStatus::Running, &TaskStatus::TimedOut));
        assert!(can_transition(&TaskStatus::Failed, &TaskStatus::Queued));
        assert!(can_transition(&TaskStatus::TimedOut, &TaskStatus::Queued));
    }

    #[test]
    fn rejects_invalid_transitions() {
        assert!(!can_transition(&TaskStatus::Pending, &TaskStatus::Running));
        assert!(!can_transition(&TaskStatus::Queued, &TaskStatus::Succeeded));
        assert!(!can_transition(&TaskStatus::Succeeded, &TaskStatus::Queued));
        assert!(!can_transition(&TaskStatus::Cancelled, &TaskStatus::Queued));
    }
}
