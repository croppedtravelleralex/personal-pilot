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
    runner_id TEXT,
    heartbeat_at TEXT,
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
    result_json TEXT,
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

pub const CREATE_PROXIES_PROVIDER_REGION_VERIFY_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxies_provider_region_verify
ON proxies(provider, region, last_verify_status, last_verify_at);
"#;


pub const CREATE_PROVIDER_RISK_SNAPSHOTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS provider_risk_snapshots (
    provider TEXT PRIMARY KEY,
    success_count INTEGER NOT NULL,
    failure_count INTEGER NOT NULL,
    risk_hit INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PROVIDER_REGION_RISK_SNAPSHOTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS provider_region_risk_snapshots (
    provider TEXT NOT NULL,
    region TEXT NOT NULL,
    recent_failed_count INTEGER NOT NULL,
    risk_hit INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(provider, region)
);
"#;

pub const ALL_SCHEMA_SQL: [&str; 15] = [
    CREATE_TASKS_TABLE_SQL,
    CREATE_RUNS_TABLE_SQL,
    CREATE_ARTIFACTS_TABLE_SQL,
    CREATE_LOGS_TABLE_SQL,
    CREATE_FINGERPRINT_PROFILES_TABLE_SQL,
    CREATE_PROXIES_TABLE_SQL,
    CREATE_PROXY_SESSION_BINDINGS_TABLE_SQL,
    CREATE_PROXIES_SELECTION_INDEX_SQL,
    CREATE_PROXY_SESSION_BINDINGS_LOOKUP_INDEX_SQL,
    CREATE_PROXIES_VERIFY_STATE_INDEX_SQL,
    CREATE_VERIFY_BATCHES_CREATED_AT_INDEX_SQL,
    CREATE_TASKS_KIND_STATUS_INDEX_SQL,
    CREATE_PROXIES_PROVIDER_REGION_VERIFY_INDEX_SQL,
    CREATE_PROVIDER_RISK_SNAPSHOTS_TABLE_SQL,
    CREATE_PROVIDER_REGION_RISK_SNAPSHOTS_TABLE_SQL,
];


pub const CREATE_FINGERPRINT_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS fingerprint_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'active',
    tags_json TEXT,
    profile_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;


pub const CREATE_PROXIES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proxies (
    id TEXT PRIMARY KEY,
    scheme TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT,
    password TEXT,
    region TEXT,
    country TEXT,
    provider TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    score REAL NOT NULL DEFAULT 1.0,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_checked_at TEXT,
    last_used_at TEXT,
    cooldown_until TEXT,
    last_smoke_status TEXT,
    last_smoke_protocol_ok INTEGER,
    last_smoke_upstream_ok INTEGER,
    last_exit_ip TEXT,
    last_anonymity_level TEXT,
    last_smoke_at TEXT,
    last_verify_status TEXT,
    last_verify_geo_match_ok INTEGER,
    last_exit_country TEXT,
    last_exit_region TEXT,
    last_verify_at TEXT,
    last_probe_latency_ms INTEGER,
    last_probe_error TEXT,
    last_probe_error_category TEXT,
    last_verify_confidence REAL,
    last_verify_score_delta INTEGER,
    last_verify_source TEXT,
    cached_trust_score INTEGER,
    trust_score_cached_at TEXT,
    provider_risk_version_seen INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;


pub const CREATE_PROXY_SESSION_BINDINGS_TABLE_SQL: &str = r#"

CREATE TABLE IF NOT EXISTS verify_batches (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    requested_count INTEGER NOT NULL,
    accepted_count INTEGER NOT NULL,
    skipped_count INTEGER NOT NULL,
    stale_after_seconds INTEGER NOT NULL,
    task_timeout_seconds INTEGER NOT NULL,
    provider_summary_json TEXT,
    filters_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS proxy_session_bindings (
    session_key TEXT PRIMARY KEY,
    proxy_id TEXT NOT NULL,
    provider TEXT,
    region TEXT,
    last_used_at TEXT NOT NULL,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(proxy_id) REFERENCES proxies(id)
);
"#;


pub const CREATE_PROXIES_SELECTION_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxies_selection
ON proxies(status, provider, region, score DESC, last_used_at, created_at);
"#;

pub const CREATE_PROXY_SESSION_BINDINGS_LOOKUP_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxy_session_bindings_lookup
ON proxy_session_bindings(proxy_id, provider, region, expires_at, last_used_at);
"#;


pub const CREATE_PROXIES_VERIFY_STATE_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxies_verify_state
ON proxies(status, last_verify_status, last_verify_at, cooldown_until);
"#;

pub const CREATE_VERIFY_BATCHES_CREATED_AT_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_verify_batches_created_at
ON verify_batches(created_at, id);
"#;

pub const CREATE_TASKS_KIND_STATUS_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_tasks_kind_status
ON tasks(kind, status, created_at, id);
"#;
