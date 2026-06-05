mod auth;
mod docker_ops;
mod routes;
mod state;
mod types;

use std::net::SocketAddr;

use axum::{
    middleware,
    routing::{delete, get, post},
    Extension, Router,
};
use bollard::Docker;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::auth::AgentToken;
use crate::state::AgentState;

#[derive(Parser)]
#[command(name = "qs-agent", about = "QuickStack Docker node agent")]
struct Args {
    /// HTTP listen port
    #[arg(long, env = "QS_AGENT_PORT", default_value = "9800")]
    port: u16,

    /// Backend URL for heartbeat registration (unused for now, reserved)
    #[arg(long, env = "QS_BACKEND_URL", default_value = "")]
    backend_url: String,

    /// This node's ID (matches cluster_nodes.id in backend DB)
    #[arg(long, env = "QS_NODE_ID")]
    node_id: String,

    /// Shared secret token — must match the backend's agent_token
    #[arg(long, env = "QS_AGENT_TOKEN")]
    agent_token: String,

    /// Directory for writing inline file mounts
    #[arg(long, env = "QS_FILES_DIR", default_value = "/var/lib/qs-agent/files")]
    files_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "qs_agent=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let docker = Docker::connect_with_local_defaults()
        .map_err(|e| anyhow::anyhow!("failed to connect to Docker socket: {e}"))?;

    // Verify Docker is reachable
    let version = docker.version().await?;
    tracing::info!(
        "connected to Docker {} (API {})",
        version.version.as_deref().unwrap_or("?"),
        version.api_version.as_deref().unwrap_or("?"),
    );

    // Ensure files directory exists
    tokio::fs::create_dir_all(&args.files_dir).await?;

    let state = AgentState {
        docker,
        node_id: args.node_id.clone(),
        files_dir: args.files_dir,
    };

    let token = AgentToken(args.agent_token);

    let app = Router::new()
        // Container lifecycle
        .route("/containers/run", post(routes::containers::run))
        .route("/containers/:id/stop", post(routes::containers::stop))
        .route("/containers/:id", delete(routes::containers::remove))
        .route("/containers/:id/logs", get(routes::containers::logs))
        .route("/containers/:id/exec", get(routes::containers::exec_ws))
        .route("/containers", get(routes::containers::list))
        // Network management
        .route("/networks/ensure", post(routes::networks::ensure))
        // Health / status (no auth required for health checks)
        .route("/status", get(routes::status::health))
        // Auth layer — protects all routes above except /status
        .layer(middleware::from_fn(auth::require_token))
        .layer(Extension(token))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    tracing::info!("qs-agent listening on {addr} (node_id={})", args.node_id);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
