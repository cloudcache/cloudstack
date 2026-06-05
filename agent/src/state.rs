use bollard::Docker;

#[derive(Clone)]
pub struct AgentState {
    pub docker: Docker,
    pub node_id: String,
    /// Directory where inline file mounts are written before container creation.
    pub files_dir: String,
}
