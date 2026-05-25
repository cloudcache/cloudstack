pub mod auth;
pub mod billing;
pub mod builds;
pub mod cluster;
pub mod clusters_admin;
pub mod databases;
pub mod events;
pub mod ipam;
pub mod network;
pub mod nodes;
pub mod object_storage;
pub mod platform;
pub mod pools;
pub mod profile;
pub mod projects;
pub mod proxy_managers;
pub mod quota;
pub mod registries;
pub mod subscriptions;
pub mod domains;
pub mod apps;
pub mod stripe;
pub mod users;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};

use crate::{auth::middleware::require_auth, state::AppState};

pub fn router(state: AppState) -> Router<AppState> {
    let authed = Router::new()
        // ── Auth (self) ───────────────────────────────────────────────────────
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/totp/setup", post(auth::totp_setup))
        .route("/auth/totp/verify", post(auth::totp_verify))
        .route("/auth/totp/disable", post(auth::totp_disable))

        // ── Profile (self-service) ────────────────────────────────────────────
        .route("/profile", get(profile::get_profile).put(profile::update_profile))
        .route("/profile/change-password", post(profile::change_password))
        .route("/profile/sessions", get(profile::list_sessions))
        .route("/profile/sessions/all", delete(profile::revoke_all_sessions))
        .route("/profile/sessions/:id", delete(profile::revoke_session))
        .route("/profile/ssh-keys", get(profile::list_ssh_keys).post(profile::add_ssh_key))
        .route("/profile/ssh-keys/:id", get(profile::get_ssh_key).delete(profile::delete_ssh_key))

        // ── Plan catalogue (self) ─────────────────────────────────────────────
        .route("/plans", get(subscriptions::list_plans))
        .route("/plans/:id", get(subscriptions::get_plan))

        // ── My subscription (self) ────────────────────────────────────────────
        .route("/subscription", get(subscriptions::get_my_subscription)
                                    .post(subscriptions::subscribe)
                                    .delete(subscriptions::cancel_my_subscription))

        // ── Billing (self) ────────────────────────────────────────────────────
        .route("/billing/wallet", get(billing::get_wallet))
        .route("/billing/transactions", get(billing::list_transactions))
        .route("/billing/usage", get(billing::get_current_usage))
        .route("/billing/usage/history", get(billing::list_usage_history))
        .route("/billing/invoices", get(billing::list_invoices))
        .route("/billing/invoices/:id", get(billing::get_invoice))
        // ── Stripe top-up ────────────────────────────────────────────────────
        .route("/billing/topup", post(stripe::create_topup))
        .route("/billing/topup/config", get(stripe::topup_config))
        .route("/billing/topup/history", get(stripe::topup_history))

        // ── Users (admin) ─────────────────────────────────────────────────────
        .route("/admin/users", get(users::list).post(users::create))
        .route("/admin/users/:id", get(users::get).put(users::update).delete(users::delete_user))
        .route("/admin/users/:id/reset-password", post(users::reset_password))
        .route("/admin/users/:id/usage", get(users::get_usage))

        // ── Admin billing ─────────────────────────────────────────────────────
        .route("/admin/billing/wallets", get(billing::admin_list_wallets))
        .route("/admin/billing/recharge", post(billing::admin_recharge))
        .route("/admin/billing/adjustment", post(billing::admin_adjust_balance))
        .route("/admin/billing/invoices", get(billing::admin_list_invoices).post(billing::admin_generate_invoice))
        .route("/admin/billing/invoices/:id/pay", post(billing::admin_mark_paid))

        // ── Network usage (user) ──────────────────────────────────────────────
        .route("/projects/:project_id/network-usage", get(billing::get_project_network_usage))
        .route("/billing/overdue", get(billing::get_overdue_status))
        // ── Admin: network charges ────────────────────────────────────────────
        .route("/admin/billing/network-charges/compute", post(billing::admin_compute_network_charges))
        .route("/admin/billing/network-charges/collect", post(billing::admin_collect_network_charges))
        .route("/admin/billing/overdue", get(billing::admin_list_overdue))

        // ── Projects (user) ───────────────────────────────────────────────────
        .route("/projects", get(projects::list).post(projects::create))
        .route("/projects/:id", get(projects::get).put(projects::update).delete(projects::delete_project))
        .route("/projects/:id/leave", post(projects::leave_project))
        .route("/projects/:id/transfer", post(projects::transfer_owner))
        .route("/projects/:id/members", get(projects::list_members).post(projects::add_member))
        .route("/projects/:id/members/:user_id", put(projects::update_member).delete(projects::remove_member))

        // ── Admin: projects ───────────────────────────────────────────────────
        .route("/admin/projects", get(projects::admin_list).post(projects::admin_create))
        .route("/admin/projects/:id", get(projects::admin_get).put(projects::admin_update).delete(projects::admin_delete))
        .route("/admin/projects/:id/members", get(projects::list_members).post(projects::admin_add_member))
        .route("/admin/projects/:id/members/:user_id", put(projects::admin_update_member).delete(projects::admin_remove_member))

        // ── Monitoring ────────────────────────────────────────────────────────
        .route("/monitoring/app-status", get(apps::monitoring_app_status))
        .route("/monitoring/apps", get(apps::monitoring_apps))
        .route("/monitoring/managed-volumes", get(apps::monitoring_managed_volumes))

        // ── Apps ──────────────────────────────────────────────────────────────
        .route("/apps/:app_id", get(apps::get_by_id))
        .route("/projects/:project_id/apps", get(apps::list).post(apps::create))
        .route("/projects/:project_id/apps/:app_id", get(apps::get).put(apps::update).delete(apps::delete_app))
        .route("/projects/:project_id/apps/:app_id/deploy", post(apps::deploy))
        .route("/projects/:project_id/apps/:app_id/pause", post(apps::pause))
        .route("/projects/:project_id/apps/:app_id/resume", post(apps::resume))
        .route("/projects/:project_id/apps/:app_id/scale", post(apps::scale))
        .route("/projects/:project_id/apps/:app_id/logs", get(apps::logs_stream))
        .route("/projects/:project_id/apps/:app_id/terminal", get(apps::terminal_ws))
        .route("/projects/:project_id/apps/:app_id/env", get(apps::list_env).post(apps::set_env))
        .route("/projects/:project_id/apps/:app_id/env/:env_id", delete(apps::delete_env))
        .route("/projects/:project_id/apps/:app_id/ports", get(apps::list_ports).post(apps::add_port))
        .route("/projects/:project_id/apps/:app_id/ports/:port_id", delete(apps::delete_port))
        .route("/projects/:project_id/apps/:app_id/metrics", get(apps::metrics_current))
        .route("/projects/:project_id/apps/:app_id/metrics/history", get(apps::metrics_history))
        .route("/projects/:project_id/apps/:app_id/events", get(apps::list_events))
        .route("/projects/:project_id/apps/:app_id/pods", get(apps::list_pods))
        .route("/projects/:project_id/apps/:app_id/webhook/regenerate", post(apps::regenerate_webhook))
        .route("/projects/:project_id/apps/:app_id/deployments", get(apps::deployment_history))
        .route("/projects/:project_id/apps/:app_id/db-credentials", get(apps::db_credentials))

        // ── App: inline file mounts ───────────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/files",
            get(apps::list_file_mounts).post(apps::set_file_mount))
        .route("/projects/:project_id/apps/:app_id/files/:file_id",
            get(apps::get_file_mount).delete(apps::delete_file_mount))

        // ── App: extra hostPath volumes ───────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/volumes",
            get(apps::list_extra_volumes).post(apps::add_extra_volume))
        .route("/projects/:project_id/apps/:app_id/volumes/:vol_id",
            delete(apps::delete_extra_volume))

        // ── App: build jobs (GIT-source) ──────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/builds",
            get(builds::list_builds).post(builds::trigger_build))
        .route("/projects/:project_id/apps/:app_id/builds/:build_id",
            get(builds::get_build).delete(builds::cancel_build))
        .route("/projects/:project_id/apps/:app_id/builds/:build_id/logs",
            get(builds::build_logs))

        // ── App: backup schedules ─────────────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/backups",
            get(object_storage::list_backups).post(object_storage::create_backup))
        .route("/projects/:project_id/apps/:app_id/backups/:backup_id",
            put(object_storage::update_backup).delete(object_storage::delete_backup))

        // ── S3 targets (user: pick active targets for backup) ─────────────────
        .route("/s3-targets", get(object_storage::list_targets_user))

        // ── Backup file listing (aggregate) ──────────────────────────────────
        .route("/backups", get(object_storage::list_all_backups))
        .route("/backups/:s3_target_id/file", delete(object_storage::delete_backup_file))
        .route("/backups/:s3_target_id/download", get(object_storage::download_backup_file))

        // ── Network / IP allocations (user) ───────────────────────────────────
        .route("/projects/:project_id/network/pools",       get(network::list_project_pools))
        .route("/projects/:project_id/network/allocations", get(network::list_project_allocations))
        .route("/projects/:project_id/apps/:app_id/network",
            get(network::get_app_network).delete(network::release_app_network))
        .route("/projects/:project_id/apps/:app_id/network/reassign",
            post(network::reassign_app_network))

        // ── Quota ─────────────────────────────────────────────────────────────
        .route("/projects/:project_id/quota", get(quota::get_project_quota))
        .route("/projects/:project_id/quota/violations", get(quota::list_violations))
        .route("/admin/projects/:id/quota/enforce", post(quota::admin_enforce))
        .route("/admin/apps/:app_id/suspend", post(quota::admin_suspend_app))
        .route("/admin/apps/:app_id/unsuspend", post(quota::admin_unsuspend_app))

        // ── App: basic auth ───────────────────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/basic-auth",
            get(apps::get_basic_auth).put(apps::put_basic_auth).delete(apps::delete_basic_auth))

        // ── App: managed hostPath volumes ─────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/managed-volumes",
            get(apps::list_managed_volumes).post(apps::create_managed_volume))
        .route("/projects/:project_id/apps/:app_id/managed-volumes/:vid",
            put(apps::update_managed_volume).delete(apps::delete_managed_volume))
        .route("/projects/:project_id/apps/:app_id/managed-volumes/:vid/usage",
            get(apps::managed_volume_usage))
        .route("/projects/:project_id/managed-volumes/shareable",
            get(apps::list_shareable_volumes))

        // ── App: volume backup schedules ──────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/managed-volumes/:vid/backups",
            get(apps::list_volume_backups).post(apps::create_volume_backup))
        .route("/projects/:project_id/apps/:app_id/managed-volumes/:vid/backups/:bid",
            put(apps::update_volume_backup).delete(apps::delete_volume_backup))
        .route("/projects/:project_id/apps/:app_id/managed-volumes/:vid/backups/:bid/run",
            post(apps::run_volume_backup))

        // ── App: DB management tools ──────────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/db-tools",
            get(apps::list_db_tools).post(apps::deploy_db_tool))
        .route("/projects/:project_id/apps/:app_id/db-tools/:tool",
            get(apps::get_db_tool).delete(apps::delete_db_tool))

        // ── Domains ───────────────────────────────────────────────────────────
        .route("/projects/:project_id/apps/:app_id/domains", get(domains::list).post(domains::create))
        .route("/projects/:project_id/apps/:app_id/domains/:domain_id", delete(domains::delete_domain))

        // ── Databases ─────────────────────────────────────────────────────────
        .route("/projects/:project_id/databases", get(databases::list).post(databases::create))
        .route("/projects/:project_id/databases/:db_id", get(databases::get).delete(databases::delete_db))
        .route("/projects/:project_id/databases/:db_id/credentials", get(databases::credentials))

        // ── Admin: DB clusters ────────────────────────────────────────────────
        .route("/admin/db-clusters", get(databases::list_clusters).post(databases::create_cluster))
        .route("/admin/db-clusters/:id", get(databases::get_cluster).put(databases::update_cluster).delete(databases::delete_cluster))

        // ── Admin: K3s nodes ──────────────────────────────────────────────────
        .route("/admin/nodes", get(nodes::list).post(nodes::add))
        .route("/admin/nodes/metrics/aggregate", get(nodes::metrics_aggregate))
        .route("/admin/nodes/:id", get(nodes::get).delete(nodes::delete_node))
        .route("/admin/nodes/:id/labels", put(nodes::update_labels))
        .route("/admin/nodes/:id/health", get(nodes::health))
        .route("/admin/nodes/:id/metrics", get(nodes::metrics))
        .route("/admin/nodes/:id/cordon", post(nodes::cordon))
        .route("/admin/nodes/:id/uncordon", post(nodes::uncordon))

        // ── Admin: proxy managers (independent LB nodes) ──────────────────────
        .route("/admin/proxy-managers", get(proxy_managers::list).post(proxy_managers::create))
        .route("/admin/proxy-managers/:id", get(proxy_managers::get).put(proxy_managers::update).delete(proxy_managers::delete))
        .route("/admin/proxy-managers/:id/test", post(proxy_managers::test_connection))

        // ── Admin: subscription plans ─────────────────────────────────────────
        .route("/admin/plans", get(subscriptions::admin_list_plans).post(subscriptions::admin_create_plan))
        .route("/admin/plans/:id", get(subscriptions::admin_get_plan).put(subscriptions::admin_update_plan).delete(subscriptions::admin_delete_plan))

        // ── Admin: user subscription management ───────────────────────────────
        .route("/admin/subscriptions", get(subscriptions::admin_list_subscriptions))
        .route("/admin/subscriptions/:id", put(subscriptions::admin_update_subscription).delete(subscriptions::admin_cancel_subscription))
        .route("/admin/users/:id/subscription", get(subscriptions::admin_get_user_subscription).post(subscriptions::admin_assign_plan))

        // ── Admin: resource pools ─────────────────────────────────────────────
        .route("/admin/resource-pools", get(pools::list).post(pools::create))
        .route("/admin/resource-pools/:id", get(pools::get).put(pools::update).delete(pools::delete))

        // ── Admin: K3s clusters ───────────────────────────────────────────────
        .route("/admin/clusters", get(clusters_admin::list).post(clusters_admin::create))
        .route("/admin/clusters/:id", get(clusters_admin::get).put(clusters_admin::update).delete(clusters_admin::delete))

        // ── Admin: cluster-wide storage config ───────────────────────────────
        .route("/admin/cluster/storage", get(cluster::get_storage).put(cluster::update_storage))

        // ── Admin: S3 targets ─────────────────────────────────────────────────
        .route("/admin/s3-targets",
            get(object_storage::list_targets).post(object_storage::create_target))
        .route("/admin/s3-targets/:id",
            get(object_storage::get_target)
            .put(object_storage::update_target)
            .delete(object_storage::delete_target))
        .route("/admin/s3-targets/test", post(object_storage::test_target))

        // ── Admin: image registries ───────────────────────────────────────────
        .route("/admin/registries", get(registries::list).post(registries::create))
        .route("/admin/registries/:id", get(registries::get).put(registries::update_registry).delete(registries::delete))
        .route("/admin/registries/:id/images", get(registries::list_images))

        // ── Admin: IPAM ───────────────────────────────────────────────────────
        .route("/admin/ip-pools", get(ipam::list_pools).post(ipam::create_pool))
        .route("/admin/ip-pools/:id", get(ipam::get_pool).put(ipam::update_pool).delete(ipam::delete_pool))
        .route("/admin/ip-pools/:id/allocations", get(ipam::list_allocations).post(ipam::allocate))
        .route("/admin/ip-pools/:id/allocations/:ip", delete(ipam::release))

        // ── Admin: platform config ────────────────────────────────────────────
        .route("/admin/platform-config", get(platform::list_config).post(platform::set_config))

        .layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        // Public auth
        .route("/auth/login", post(auth::login))
        .route("/auth/register", post(auth::register))
        .route("/auth/registration-status", get(auth::registration_status))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .nest("/api/v1", authed)
        // Webhooks (public, validated inside handler)
        .route("/webhooks/:webhook_id", post(apps::webhook))
        .route("/stripe/webhook", post(stripe::stripe_webhook))
        // Maintenance page served by pingora for paused apps
        .route("/_qs/maintenance", get(maintenance_handler))
}

async fn maintenance_handler() -> impl axum::response::IntoResponse {
    (
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../assets/maintenance.html"),
    )
}
