pub const CREATE_TASKS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    input_json TEXT NOT NULL,
    network_policy_json TEXT,
    fingerprint_profile_json TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    queued_at TEXT,
    started_at TEXT,
    finished_at TEXT,
    result_json TEXT,
    error_message TEXT
);
"#;

pub const CREATE_RUNS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    runner_kind TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    error_message TEXT,
    FOREIGN KEY(task_id) REFERENCES tasks(id)
);
"#;

pub const CREATE_ARTIFACTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    run_id TEXT,
    kind TEXT NOT NULL,
    storage_path TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id),
    FOREIGN KEY(run_id) REFERENCES runs(id)
);
"#;

pub const CREATE_LOGS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS logs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    run_id TEXT,
    level TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id),
    FOREIGN KEY(run_id) REFERENCES runs(id)
);
"#;

pub const ALL_SCHEMA_SQL: [&str; 4] = [
    CREATE_TASKS_TABLE_SQL,
    CREATE_RUNS_TABLE_SQL,
    CREATE_ARTIFACTS_TABLE_SQL,
    CREATE_LOGS_TABLE_SQL,
];
