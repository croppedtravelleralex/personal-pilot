use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::app::state::AppState;

use super::{
    auth::auth_middleware,
    behavior_handlers::{
        create_behavior_profile, create_identity_profile, create_network_profile,
        create_session_profile, create_site_behavior_policy, delete_behavior_profile,
        delete_identity_profile, delete_network_profile, delete_session_profile,
        delete_site_behavior_policy, get_behavior_profile, get_identity_profile,
        get_network_profile, get_session_profile, get_site_behavior_policy, list_behavior_profiles,
        list_identity_profiles, list_network_profiles, list_session_profiles,
        list_site_behavior_policies, patch_behavior_profile, patch_identity_profile,
        patch_network_profile, patch_session_profile, patch_site_behavior_policy,
    },
    handlers::{
        browser_extract_text, browser_get_final_url, browser_get_html, browser_get_title,
        browser_open, cancel_task, check_proxy_trust_cache, create_fingerprint_profile,
        create_platform_template, create_proxy, create_store_platform_override, create_task,
        explain_proxy_selection, get_fingerprint_profile, get_platform_template, get_proxy,
        get_store_platform_override, get_task, get_task_logs, get_task_runs, get_verify_batch,
        health, list_fingerprint_profiles, list_platform_templates, list_proxies,
        list_store_platform_overrides, list_verify_batches, maintain_proxy_trust_cache,
        repair_proxy_trust_cache, repair_proxy_trust_cache_batch, retry_task,
        scan_proxy_trust_cache, smoke_test_proxy, status, verify_batch_proxies, verify_proxy,
    },
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/tasks", post(create_task))
        .route("/browser/open", post(browser_open))
        .route("/browser/html", post(browser_get_html))
        .route("/browser/title", post(browser_get_title))
        .route("/browser/final-url", post(browser_get_final_url))
        .route("/browser/text", post(browser_extract_text))
        .route(
            "/behavior-profiles",
            post(create_behavior_profile).get(list_behavior_profiles),
        )
        .route(
            "/behavior-profiles/:id",
            get(get_behavior_profile)
                .patch(patch_behavior_profile)
                .delete(delete_behavior_profile),
        )
        .route(
            "/identity-profiles",
            post(create_identity_profile).get(list_identity_profiles),
        )
        .route(
            "/identity-profiles/:id",
            get(get_identity_profile)
                .patch(patch_identity_profile)
                .delete(delete_identity_profile),
        )
        .route(
            "/network-profiles",
            post(create_network_profile).get(list_network_profiles),
        )
        .route(
            "/network-profiles/:id",
            get(get_network_profile)
                .patch(patch_network_profile)
                .delete(delete_network_profile),
        )
        .route(
            "/session-profiles",
            post(create_session_profile).get(list_session_profiles),
        )
        .route(
            "/session-profiles/:id",
            get(get_session_profile)
                .patch(patch_session_profile)
                .delete(delete_session_profile),
        )
        .route(
            "/site-behavior-policies",
            post(create_site_behavior_policy).get(list_site_behavior_policies),
        )
        .route(
            "/site-behavior-policies/:id",
            get(get_site_behavior_policy)
                .patch(patch_site_behavior_policy)
                .delete(delete_site_behavior_policy),
        )
        .route(
            "/fingerprint-profiles",
            post(create_fingerprint_profile).get(list_fingerprint_profiles),
        )
        .route(
            "/platform-templates",
            post(create_platform_template).get(list_platform_templates),
        )
        .route("/platform-templates/:id", get(get_platform_template))
        .route(
            "/store-platform-overrides",
            post(create_store_platform_override).get(list_store_platform_overrides),
        )
        .route(
            "/store-platform-overrides/:id",
            get(get_store_platform_override),
        )
        .route("/proxies", post(create_proxy).get(list_proxies))
        .route(
            "/proxies/verify-batch",
            post(verify_batch_proxies).get(list_verify_batches),
        )
        .route("/proxies/verify-batch/:id", get(get_verify_batch))
        .route("/fingerprint-profiles/:id", get(get_fingerprint_profile))
        .route("/proxies/:id", get(get_proxy))
        .route("/proxies/:id/explain", get(explain_proxy_selection))
        .route(
            "/proxies/:id/trust-cache-check",
            get(check_proxy_trust_cache),
        )
        .route(
            "/proxies/:id/trust-cache-repair",
            post(repair_proxy_trust_cache),
        )
        .route("/proxies/trust-cache-scan", get(scan_proxy_trust_cache))
        .route(
            "/proxies/trust-cache-repair-batch",
            post(repair_proxy_trust_cache_batch),
        )
        .route(
            "/proxies/trust-cache-maintenance",
            post(maintain_proxy_trust_cache),
        )
        .route("/proxies/:id/smoke", post(smoke_test_proxy))
        .route("/proxies/:id/verify", post(verify_proxy))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/runs", get(get_task_runs))
        .route("/tasks/:id/logs", get(get_task_logs))
        .route("/tasks/:id/retry", post(retry_task))
        .route("/tasks/:id/cancel", post(cancel_task))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_middleware))
}
