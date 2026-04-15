use serde::{Deserialize, Serialize};

pub const TASK_STATUS_PENDING: &str = "pending";
pub const TASK_STATUS_QUEUED: &str = "queued";
pub const TASK_STATUS_RUNNING: &str = "running";
pub const TASK_STATUS_SUCCEEDED: &str = "succeeded";
pub const TASK_STATUS_FAILED: &str = "failed";
pub const TASK_STATUS_CANCELLED: &str = "cancelled";
pub const TASK_STATUS_TIMED_OUT: &str = "timed_out";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => TASK_STATUS_PENDING,
            TaskStatus::Queued => TASK_STATUS_QUEUED,
            TaskStatus::Running => TASK_STATUS_RUNNING,
            TaskStatus::Succeeded => TASK_STATUS_SUCCEEDED,
            TaskStatus::Failed => TASK_STATUS_FAILED,
            TaskStatus::Cancelled => TASK_STATUS_CANCELLED,
            TaskStatus::TimedOut => TASK_STATUS_TIMED_OUT,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Succeeded
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::TimedOut
        )
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    pub url: Option<String>,
    pub script: Option<String>,
    pub metadata_json: Option<String>,
    pub fingerprint_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub kind: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub input: TaskInput,
    pub fingerprint_profile_id: Option<String>,
    pub fingerprint_profile_version: Option<i64>,
    pub created_at: String,
}
