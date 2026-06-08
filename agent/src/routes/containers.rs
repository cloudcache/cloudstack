use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    Json,
};
use bollard::container::{LogOutput, LogsOptions};
use futures::StreamExt;
use serde::Deserialize;

use crate::state::AgentState;
use crate::types::{ContainerInfo, RunContainerRequest, RunContainerResponse};

/// POST /containers/run — create + start a container.
pub async fn run(
    State(state): State<AgentState>,
    Json(req): Json<RunContainerRequest>,
) -> Result<Json<RunContainerResponse>, (axum::http::StatusCode, String)> {
    crate::docker_ops::run_container(&state.docker, &req, &state.files_dir)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("run_container: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })
}

/// POST /containers/:id/stop
pub async fn stop(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    crate::docker_ops::stop_container(&state.docker, &id)
        .await
        .map(|_| axum::http::StatusCode::NO_CONTENT)
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })
}

/// DELETE /containers/:id
pub async fn remove(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, String)> {
    crate::docker_ops::remove_container(&state.docker, &id)
        .await
        .map(|_| axum::http::StatusCode::NO_CONTENT)
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })
}

/// GET /containers/:id/inspect — terminal state + exit code (for build completion).
pub async fn inspect(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> Result<Json<crate::types::InspectResponse>, (axum::http::StatusCode, String)> {
    crate::docker_ops::inspect_container(&state.docker, &id)
        .await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// GET /containers — list all running qs-* containers.
pub async fn list(
    State(state): State<AgentState>,
) -> Result<Json<Vec<ContainerInfo>>, (axum::http::StatusCode, String)> {
    use bollard::container::ListContainersOptions;
    use std::collections::HashMap;

    let mut filters = HashMap::new();
    filters.insert("name", vec!["qs-"]);

    let containers = state
        .docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        }))
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            )
        })?;

    let infos: Vec<ContainerInfo> = containers
        .into_iter()
        .map(|c| ContainerInfo {
            id: c.id.unwrap_or_default(),
            name: c
                .names
                .and_then(|n| n.into_iter().next())
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string(),
            image: c.image.unwrap_or_default(),
            state: c.state.unwrap_or_default(),
            status: c.status.unwrap_or_default(),
            created: c.created.unwrap_or(0),
        })
        .collect();

    Ok(Json(infos))
}

// ── Logs (SSE) ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_tail")]
    pub tail: String,
    #[serde(default)]
    pub follow: bool,
}

fn default_tail() -> String {
    "100".to_string()
}

/// GET /containers/:id/logs?tail=100&follow=true — stream container logs as SSE.
pub async fn logs(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (axum::http::StatusCode, String)>
{
    let opts = LogsOptions::<String> {
        follow: query.follow,
        stdout: true,
        stderr: true,
        tail: query.tail,
        ..Default::default()
    };

    let log_stream = state.docker.logs(&id, Some(opts));

    let sse_stream = log_stream.filter_map(|item| async {
        match item {
            Ok(output) => {
                let line = match output {
                    LogOutput::StdOut { message } => String::from_utf8_lossy(&message).to_string(),
                    LogOutput::StdErr { message } => String::from_utf8_lossy(&message).to_string(),
                    LogOutput::Console { message } => {
                        String::from_utf8_lossy(&message).to_string()
                    }
                    LogOutput::StdIn { message } => String::from_utf8_lossy(&message).to_string(),
                };
                Some(Ok::<_, Infallible>(Event::default().data(line.trim_end())))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(sse_stream))
}

// ── Exec (WebSocket) ─────────────────────────────────────────────────────────

/// GET /containers/:id/exec — WebSocket terminal session.
pub async fn exec_ws(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_exec(state, id, socket))
}

async fn handle_exec(state: AgentState, container_id: String, socket: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    use bollard::exec::{CreateExecOptions, StartExecResults};
    use futures::SinkExt;

    let exec = match state
        .docker
        .create_exec(
            &container_id,
            CreateExecOptions {
                cmd: Some(vec!["sh"]),
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                tty: Some(true),
                ..Default::default()
            },
        )
        .await
    {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("create_exec: {e}");
            return;
        }
    };

    let started = match state.docker.start_exec(&exec.id, None).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("start_exec: {e}");
            return;
        }
    };

    let StartExecResults::Attached { mut output, mut input } = started else {
        tracing::error!("exec not attached");
        return;
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    tokio::select! {
        // WS → Docker stdin
        _ = async {
            while let Some(Ok(msg)) = ws_rx.next().await {
                if let Message::Binary(data) = msg {
                    use tokio::io::AsyncWriteExt;
                    if input.write_all(&data).await.is_err() {
                        break;
                    }
                }
            }
        } => {},
        // Docker stdout → WS
        _ = async {
            while let Some(Ok(output_chunk)) = output.next().await {
                let bytes: Vec<u8> = match output_chunk {
                    LogOutput::StdOut { message } => message.to_vec(),
                    LogOutput::StdErr { message } => message.to_vec(),
                    LogOutput::Console { message } => message.to_vec(),
                    _ => continue,
                };
                if ws_tx.send(Message::Binary(bytes)).await.is_err() {
                    break;
                }
            }
        } => {},
    }
}
