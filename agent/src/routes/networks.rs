use axum::{extract::State, Json};
use bollard::network::{CreateNetworkOptions, InspectNetworkOptions};
use std::collections::HashMap;

use crate::state::AgentState;
use crate::types::{EnsureNetworkRequest, EnsureNetworkResponse};

/// POST /networks/ensure — create a Docker network if it doesn't exist.
pub async fn ensure(
    State(state): State<AgentState>,
    Json(req): Json<EnsureNetworkRequest>,
) -> Result<Json<EnsureNetworkResponse>, (axum::http::StatusCode, String)> {
    // Check if network already exists
    match state
        .docker
        .inspect_network(&req.name, None::<InspectNetworkOptions<String>>)
        .await
    {
        Ok(net) => {
            return Ok(Json(EnsureNetworkResponse {
                network_id: net.id.unwrap_or_default(),
                created: false,
            }));
        }
        Err(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. }) => {
            // Network doesn't exist — create it below
        }
        Err(e) => {
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string(),
            ));
        }
    }

    // Build IPAM config
    let ipam_config = bollard::models::IpamConfig {
        subnet: Some(req.subnet.clone()),
        gateway: req.gateway.clone(),
        ..Default::default()
    };
    let ipam = bollard::models::Ipam {
        config: Some(vec![ipam_config]),
        ..Default::default()
    };

    // Driver options — set the Linux bridge name on the host
    let mut driver_opts: HashMap<&str, &str> = HashMap::new();
    let bridge_name_val;
    if let Some(bridge_name) = &req.bridge_name {
        bridge_name_val = bridge_name.clone();
        driver_opts.insert(
            "com.docker.network.bridge.name",
            &bridge_name_val,
        );
    }

    let create_opts = CreateNetworkOptions {
        name: req.name.as_str(),
        driver: "bridge",
        ipam,
        options: driver_opts,
        ..Default::default()
    };

    let resp = state.docker.create_network(create_opts).await.map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            e.to_string(),
        )
    })?;

    Ok(Json(EnsureNetworkResponse {
        network_id: resp.id.unwrap_or_default(),
        created: true,
    }))
}
