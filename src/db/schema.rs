pub const CREATE_TASKS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    input_json TEXT NOT NULL,
    form_input_redacted_json TEXT,
    network_policy_json TEXT,
    behavior_policy_json TEXT,
    execution_intent_json TEXT,
    fingerprint_profile_json TEXT,
    fingerprint_profile_id TEXT,
    fingerprint_profile_version INTEGER,
    identity_profile_id TEXT,
    behavior_profile_id TEXT,
    behavior_profile_version INTEGER,
    network_profile_id TEXT,
    session_profile_id TEXT,
    persona_id TEXT,
    platform_id TEXT,
    manual_gate_request_id TEXT,
    proxy_id TEXT,
    requested_region TEXT,
    proxy_mode TEXT,
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

pub const CREATE_DASHBOARD_ONBOARDING_DRAFTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS dashboard_onboarding_drafts (
    id TEXT PRIMARY KEY,
    share_token TEXT NOT NULL UNIQUE,
    share_expires_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    login_url TEXT NOT NULL,
    site_key TEXT NOT NULL,
    success_hint TEXT,
    behavior_profile_id TEXT,
    identity_profile_id TEXT,
    session_profile_id TEXT,
    fingerprint_profile_id TEXT,
    proxy_id TEXT,
    credential_mode TEXT NOT NULL DEFAULT 'alias',
    credential_ref TEXT,
    inferred_contract_json TEXT,
    final_contract_json TEXT,
    site_policy_id TEXT,
    site_policy_version INTEGER,
    shadow_task_id TEXT,
    active_success_task_id TEXT,
    active_failure_task_id TEXT,
    continuity_task_id TEXT,
    evidence_summary_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PROXY_HEALTH_SNAPSHOTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_health_snapshots (
    id TEXT PRIMARY KEY,
    proxy_id TEXT NOT NULL,
    overall_score REAL NOT NULL,
    grade TEXT NOT NULL,
    identity_score REAL,
    privacy_score REAL,
    fraud_score REAL,
    mail_reputation_score REAL,
    network_quality_score REAL,
    site_access_score REAL,
    browser_privacy_score REAL,
    probe_ok INTEGER NOT NULL DEFAULT 0,
    probe_latency_ms INTEGER,
    error TEXT,
    summary_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(proxy_id) REFERENCES proxies(id)
);
"#;

pub const CREATE_PROXY_HEALTH_SNAPSHOTS_PROXY_CREATED_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxy_health_snapshots_proxy_created
ON proxy_health_snapshots(proxy_id, created_at DESC);
"#;

pub const ALL_SCHEMA_SQL: [&str; 36] = [
    CREATE_TASKS_TABLE_SQL,
    CREATE_RUNS_TABLE_SQL,
    CREATE_ARTIFACTS_TABLE_SQL,
    CREATE_LOGS_TABLE_SQL,
    CREATE_FINGERPRINT_PROFILES_TABLE_SQL,
    CREATE_BEHAVIOR_PROFILES_TABLE_SQL,
    CREATE_SITE_BEHAVIOR_POLICIES_TABLE_SQL,
    CREATE_IDENTITY_PROFILES_TABLE_SQL,
    CREATE_NETWORK_PROFILES_TABLE_SQL,
    CREATE_SESSION_PROFILES_TABLE_SQL,
    CREATE_NETWORK_POLICIES_TABLE_SQL,
    CREATE_CONTINUITY_POLICIES_TABLE_SQL,
    CREATE_PLATFORM_TEMPLATES_TABLE_SQL,
    CREATE_STORE_PLATFORM_OVERRIDES_TABLE_SQL,
    CREATE_PERSONA_PROFILES_TABLE_SQL,
    CREATE_MANUAL_GATE_REQUESTS_TABLE_SQL,
    CREATE_CONTINUITY_EVENTS_TABLE_SQL,
    CREATE_PERSONA_HEALTH_SNAPSHOTS_TABLE_SQL,
    CREATE_PROXIES_TABLE_SQL,
    CREATE_PROXY_HARVEST_RUNS_TABLE_SQL,
    CREATE_PROXY_HARVEST_SOURCES_TABLE_SQL,
    CREATE_PROXY_SITE_STATS_TABLE_SQL,
    CREATE_BEHAVIOR_SITE_STATS_TABLE_SQL,
    CREATE_PROXY_SESSION_BINDINGS_TABLE_SQL,
    CREATE_PROXIES_SELECTION_INDEX_SQL,
    CREATE_PROXY_SESSION_BINDINGS_LOOKUP_INDEX_SQL,
    CREATE_PROXIES_VERIFY_STATE_INDEX_SQL,
    CREATE_PROXIES_ENDPOINT_DEDUPE_INDEX_SQL,
    CREATE_VERIFY_BATCHES_CREATED_AT_INDEX_SQL,
    CREATE_TASKS_KIND_STATUS_INDEX_SQL,
    CREATE_PROXIES_PROVIDER_REGION_VERIFY_INDEX_SQL,
    CREATE_PROVIDER_RISK_SNAPSHOTS_TABLE_SQL,
    CREATE_PROVIDER_REGION_RISK_SNAPSHOTS_TABLE_SQL,
    CREATE_DASHBOARD_ONBOARDING_DRAFTS_TABLE_SQL,
    CREATE_PROXY_HEALTH_SNAPSHOTS_TABLE_SQL,
    CREATE_PROXY_HEALTH_SNAPSHOTS_PROXY_CREATED_INDEX_SQL,
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

pub const CREATE_BEHAVIOR_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS behavior_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft',
    tags_json TEXT,
    profile_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_SITE_BEHAVIOR_POLICIES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS site_behavior_policies (
    id TEXT PRIMARY KEY,
    version INTEGER NOT NULL DEFAULT 1,
    site_key TEXT NOT NULL,
    page_archetype TEXT,
    action_kind TEXT,
    behavior_profile_id TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    required INTEGER NOT NULL DEFAULT 0,
    override_json TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(behavior_profile_id) REFERENCES behavior_profiles(id)
);
"#;

pub const CREATE_IDENTITY_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS identity_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft',
    fingerprint_profile_id TEXT,
    behavior_profile_id TEXT,
    network_profile_id TEXT,
    identity_json TEXT NOT NULL,
    secret_aliases_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(fingerprint_profile_id) REFERENCES fingerprint_profiles(id),
    FOREIGN KEY(behavior_profile_id) REFERENCES behavior_profiles(id)
);
"#;

pub const CREATE_NETWORK_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS network_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft',
    network_policy_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_SESSION_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS session_profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft',
    continuity_mode TEXT NOT NULL,
    retention_policy_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_NETWORK_POLICIES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS network_policies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    country_anchor TEXT,
    region_anchor TEXT,
    allow_same_country_fallback INTEGER NOT NULL DEFAULT 0,
    allow_same_region_fallback INTEGER NOT NULL DEFAULT 0,
    provider_preference TEXT,
    allowed_regions_json TEXT,
    network_policy_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_CONTINUITY_POLICIES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS continuity_policies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    session_ttl_seconds INTEGER NOT NULL DEFAULT 86400,
    heartbeat_interval_seconds INTEGER NOT NULL DEFAULT 300,
    site_group_mode TEXT NOT NULL DEFAULT 'host',
    recovery_enabled INTEGER NOT NULL DEFAULT 1,
    protect_on_login_loss INTEGER NOT NULL DEFAULT 1,
    policy_json TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PLATFORM_TEMPLATES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS platform_templates (
    id TEXT PRIMARY KEY,
    platform_id TEXT NOT NULL,
    name TEXT NOT NULL,
    warm_paths_json TEXT NOT NULL DEFAULT '[]',
    revisit_paths_json TEXT NOT NULL DEFAULT '[]',
    stateful_paths_json TEXT NOT NULL DEFAULT '[]',
    write_operation_paths_json TEXT NOT NULL DEFAULT '[]',
    high_risk_paths_json TEXT NOT NULL DEFAULT '[]',
    allowed_regions_json TEXT,
    preferred_locale TEXT,
    preferred_timezone TEXT,
    continuity_checks_json TEXT,
    identity_markers_json TEXT,
    login_loss_signals_json TEXT,
    recovery_steps_json TEXT,
    behavior_defaults_json TEXT,
    event_chain_templates_json TEXT,
    page_semantics_json TEXT,
    readiness_level TEXT NOT NULL DEFAULT 'draft',
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_STORE_PLATFORM_OVERRIDES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS store_platform_overrides (
    id TEXT PRIMARY KEY,
    store_id TEXT NOT NULL,
    platform_id TEXT NOT NULL,
    admin_origin TEXT,
    entry_origin TEXT,
    entry_paths_json TEXT,
    warm_paths_json TEXT,
    revisit_paths_json TEXT,
    stateful_paths_json TEXT,
    high_risk_paths_json TEXT,
    recovery_steps_json TEXT,
    login_loss_signals_json TEXT,
    identity_markers_json TEXT,
    behavior_defaults_json TEXT,
    event_chain_templates_json TEXT,
    page_semantics_json TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PERSONA_PROFILES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS persona_profiles (
    id TEXT PRIMARY KEY,
    store_id TEXT NOT NULL,
    platform_id TEXT NOT NULL,
    device_family TEXT NOT NULL DEFAULT 'desktop',
    country_anchor TEXT NOT NULL,
    region_anchor TEXT,
    locale TEXT NOT NULL,
    timezone TEXT NOT NULL,
    fingerprint_profile_id TEXT NOT NULL,
    behavior_profile_id TEXT,
    network_policy_id TEXT NOT NULL,
    continuity_policy_id TEXT NOT NULL,
    credential_ref TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(fingerprint_profile_id) REFERENCES fingerprint_profiles(id),
    FOREIGN KEY(behavior_profile_id) REFERENCES behavior_profiles(id),
    FOREIGN KEY(network_policy_id) REFERENCES network_policies(id),
    FOREIGN KEY(continuity_policy_id) REFERENCES continuity_policies(id)
);
"#;

pub const CREATE_MANUAL_GATE_REQUESTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS manual_gate_requests (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    persona_id TEXT,
    store_id TEXT,
    platform_id TEXT,
    requested_action_kind TEXT NOT NULL,
    requested_url TEXT,
    reason_code TEXT NOT NULL,
    reason_summary TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    resolution_note TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY(task_id) REFERENCES tasks(id),
    FOREIGN KEY(persona_id) REFERENCES persona_profiles(id)
);
"#;

pub const CREATE_CONTINUITY_EVENTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS continuity_events (
    id TEXT PRIMARY KEY,
    persona_id TEXT,
    store_id TEXT,
    platform_id TEXT,
    task_id TEXT,
    run_id TEXT,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    event_json TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(task_id) REFERENCES tasks(id),
    FOREIGN KEY(run_id) REFERENCES runs(id),
    FOREIGN KEY(persona_id) REFERENCES persona_profiles(id)
);
"#;

pub const CREATE_PERSONA_HEALTH_SNAPSHOTS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS persona_health_snapshots (
    id TEXT PRIMARY KEY,
    persona_id TEXT NOT NULL,
    store_id TEXT,
    platform_id TEXT,
    status TEXT NOT NULL,
    active_session_count INTEGER NOT NULL DEFAULT 0,
    continuity_score REAL NOT NULL DEFAULT 0,
    login_risk_count INTEGER NOT NULL DEFAULT 0,
    last_event_type TEXT,
    last_task_at TEXT,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(persona_id) REFERENCES persona_profiles(id)
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
    source_label TEXT,
    proxy_health_score REAL,
    proxy_health_grade TEXT,
    proxy_health_checked_at TEXT,
    proxy_health_summary_json TEXT,
    last_seen_at TEXT,
    promoted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PROXY_HARVEST_RUNS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_harvest_runs (
    id TEXT PRIMARY KEY,
    source_label TEXT,
    source_kind TEXT,
    fetched_count INTEGER NOT NULL DEFAULT 0,
    accepted_count INTEGER NOT NULL DEFAULT 0,
    deduped_count INTEGER NOT NULL DEFAULT 0,
    rejected_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    summary_json TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT
);
"#;

pub const CREATE_PROXY_HARVEST_SOURCES_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_harvest_sources (
    source_label TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    source_tier TEXT NOT NULL DEFAULT 'public',
    for_demo INTEGER NOT NULL DEFAULT 1,
    for_prod INTEGER NOT NULL DEFAULT 0,
    validation_mode TEXT,
    expected_geo_quality TEXT,
    cost_class TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    config_json TEXT,
    interval_seconds INTEGER NOT NULL DEFAULT 300,
    base_proxy_score REAL NOT NULL DEFAULT 1.0,
    quarantine_until TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    backoff_until TEXT,
    last_run_started_at TEXT,
    last_run_finished_at TEXT,
    last_run_status TEXT,
    last_error TEXT,
    health_score REAL NOT NULL DEFAULT 100.0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
"#;

pub const CREATE_PROXY_SITE_STATS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS proxy_site_stats (
    proxy_id TEXT NOT NULL,
    site_key TEXT NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_success_at TEXT,
    last_failure_at TEXT,
    last_failure_scope TEXT,
    last_browser_failure_signal TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (proxy_id, site_key),
    FOREIGN KEY(proxy_id) REFERENCES proxies(id)
);
"#;

pub const CREATE_BEHAVIOR_SITE_STATS_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS behavior_site_stats (
    behavior_profile_id TEXT NOT NULL,
    site_key TEXT NOT NULL,
    page_archetype TEXT NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    timeout_count INTEGER NOT NULL DEFAULT 0,
    abort_count INTEGER NOT NULL DEFAULT 0,
    avg_added_latency_ms INTEGER,
    last_success_at TEXT,
    last_failure_at TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (behavior_profile_id, site_key, page_archetype),
    FOREIGN KEY(behavior_profile_id) REFERENCES behavior_profiles(id)
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
    fingerprint_profile_id TEXT,
    site_key TEXT,
    requested_region TEXT,
    requested_provider TEXT,
    cookies_json TEXT,
    cookie_updated_at TEXT,
    local_storage_json TEXT,
    session_storage_json TEXT,
    storage_updated_at TEXT,
    last_success_at TEXT,
    last_failure_at TEXT,
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

pub const CREATE_PROXIES_ENDPOINT_DEDUPE_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_proxies_endpoint_dedupe
ON proxies(scheme, host, port, username, provider, region);
"#;

pub const CREATE_VERIFY_BATCHES_CREATED_AT_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_verify_batches_created_at
ON verify_batches(created_at, id);
"#;

pub const CREATE_TASKS_KIND_STATUS_INDEX_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_tasks_kind_status
ON tasks(kind, status, created_at, id);
"#;
