use serde::{Deserialize, Serialize};

pub const RUN_STATUS_PENDING: &str = "pending";
pub const RUN_STATUS_RUNNING: &str = "running";
pub const RUN_STATUS_SUCCEEDED: &str = "succeeded";
pub const RUN_STATUS_FAILED: &str = "failed";
pub const RUN_STATUS_CANCELLED: &str = "cancelled";
pub const RUN_STATUS_TIMED_OUT: &str = "timed_out";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => RUN_STATUS_PENDING,
            RunStatus::Running => RUN_STATUS_RUNNING,
            RunStatus::Succeeded => RUN_STATUS_SUCCEEDED,
            RunStatus::Failed => RUN_STATUS_FAILED,
            RunStatus::Cancelled => RUN_STATUS_CANCELLED,
            RunStatus::TimedOut => RUN_STATUS_TIMED_OUT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub status: RunStatus,
    pub attempt: i32,
    pub runner_kind: String,
}
