use std::sync::Arc;

use russh::client;
use russh_keys::key;

use crate::error::{AppError, AppResult};

struct Handler;

#[async_trait::async_trait]
impl client::Handler for Handler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // First-connect trust: accept any host key during provisioning
        Ok(true)
    }
}

/// Data returned from a completed node install.
pub struct InstallResult {
    /// Kubeconfig YAML (master only; None for workers).
    pub kubeconfig: Option<String>,
    /// Whether NVIDIA GPU devices were found on this node.
    pub has_gpu: bool,
    pub gpu_model: Option<String>,
    pub gpu_count: u32,
    /// Primary network interface name (e.g. "eth0", "ens3").
    pub main_iface: String,
}

pub struct NodeInstaller<'a> {
    ip: &'a str,
    password: &'a str,
    platform_pub_key: &'a str,
    _platform_priv_key: &'a str,
}

impl<'a> NodeInstaller<'a> {
    pub fn new(
        ip: &'a str,
        password: &'a str,
        platform_pub_key: &'a str,
        platform_priv_key: &'a str,
    ) -> Self {
        Self { ip, password, platform_pub_key, _platform_priv_key: platform_priv_key }
    }

    pub async fn run(
        &self,
        hostname: &str,
        master_ip: &str,
        k3s_token: &str,
        is_master: bool,
        storage_path: &str,
    ) -> AppResult<InstallResult> {
        let config = Arc::new(client::Config::default());
        let mut session = client::connect(config, (self.ip, 22u16), Handler)
            .await
            .map_err(|e| AppError::Ssh(format!("connect to {}: {e}", self.ip)))?;

        let authed = session
            .authenticate_password("root", self.password)
            .await
            .map_err(|e| AppError::Ssh(format!("auth: {e}")))?;
        if !authed {
            return Err(AppError::Ssh("SSH password authentication failed".into()));
        }

        // ── 1. Authorize platform SSH key ────────────────────────────────────
        self.exec(
            &mut session,
            &format!(
                "mkdir -p ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
                self.platform_pub_key
            ),
        ).await?;

        // ── 2. Timezone ──────────────────────────────────────────────────────
        self.exec(&mut session, "timedatectl set-timezone Asia/Shanghai || true").await?;

        // ── 3. Base packages ─────────────────────────────────────────────────
        self.exec(
            &mut session,
            "DEBIAN_FRONTEND=noninteractive apt-get update -qq && \
             apt-get install -y -qq nslcd libnss-ldap libpam-ldap nscd wget",
        ).await?;

        // ── 4. Storage mount point ───────────────────────────────────────────
        self.exec(
            &mut session,
            &format!("mkdir -p '{storage_path}' && chmod 1777 '{storage_path}'"),
        ).await?;

        // ── 5. Detect primary network interface ──────────────────────────────
        let main_iface = self
            .exec_output(
                &mut session,
                "ip route get 1.1.1.1 2>/dev/null | grep -oP 'dev \\K\\S+' | head -1",
            )
            .await?
            .trim()
            .to_string();
        let main_iface = if main_iface.is_empty() { "eth0".to_string() } else { main_iface };

        // ── 6. Detect NVIDIA GPU ─────────────────────────────────────────────
        let nvidia_dev = self
            .exec_output(&mut session, "ls /dev/nvidia0 2>/dev/null")
            .await?;
        let has_gpu = !nvidia_dev.trim().is_empty();

        let (gpu_model, gpu_count) = if has_gpu {
            // Install NVIDIA container toolkit
            self.exec(
                &mut session,
                "curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
                 gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg && \
                 curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
                 sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
                 tee /etc/apt/sources.list.d/nvidia-container-toolkit.list && \
                 apt-get update -qq && apt-get install -y -qq nvidia-container-toolkit && \
                 nvidia-ctk runtime configure --runtime=containerd && \
                 systemctl restart containerd",
            ).await?;

            let info = self
                .exec_output(
                    &mut session,
                    "nvidia-smi --query-gpu=name,count --format=csv,noheader 2>/dev/null | head -1",
                )
                .await?;
            let parts: Vec<&str> = info.trim().splitn(2, ',').collect();
            let model = parts.first().map(|s| s.trim().to_string());
            let count: u32 = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);
            (model, count)
        } else {
            (None, 0)
        };

        // ── 7. node_exporter ─────────────────────────────────────────────────
        self.exec(
            &mut session,
            "NE_VER=1.8.2; \
             if ! command -v node_exporter >/dev/null 2>&1; then \
               wget -q https://github.com/prometheus/node_exporter/releases/download/v${NE_VER}/node_exporter-${NE_VER}.linux-amd64.tar.gz -O /tmp/ne.tar.gz && \
               tar -xzf /tmp/ne.tar.gz -C /tmp && \
               mv /tmp/node_exporter-${NE_VER}.linux-amd64/node_exporter /usr/local/bin/node_exporter && \
               chmod +x /usr/local/bin/node_exporter && \
               rm -rf /tmp/ne.tar.gz /tmp/node_exporter-${NE_VER}.linux-amd64; \
             fi; \
             cat > /etc/systemd/system/node_exporter.service << 'SVCEOF'\n\
[Unit]\nDescription=Node Exporter\nAfter=network.target\n\
[Service]\nUser=nobody\nExecStart=/usr/local/bin/node_exporter\nRestart=always\n\
[Install]\nWantedBy=multi-user.target\nSVCEOF\n\
             systemctl daemon-reload && \
             systemctl enable node_exporter && \
             systemctl start node_exporter || systemctl restart node_exporter",
        ).await?;

        // ── 8. K3s install ───────────────────────────────────────────────────
        // host-gw: simple L3 routing between nodes, no VXLAN overhead.
        // Requires all nodes on the same L2 segment (same VLAN/broadcast domain).
        let kubeconfig = if is_master {
            self.exec(
                &mut session,
                &format!(
                    "curl -sfL https://get.k3s.io | \
                     INSTALL_K3S_EXEC='server \
                       --disable traefik \
                       --disable servicelb \
                       --flannel-backend=host-gw \
                       --cluster-cidr=10.244.0.0/16 \
                       --service-cidr=10.96.0.0/12 \
                       --node-ip={} \
                       --write-kubeconfig-mode=644' \
                     K3S_TOKEN={k3s_token} sh -",
                    self.ip
                ),
            ).await?;

            // Wait for K3s to write kubeconfig (up to 60 s)
            self.exec(
                &mut session,
                "i=0; until [ -f /etc/rancher/k3s/k3s.yaml ] || [ $i -ge 30 ]; \
                 do sleep 2; i=$((i+1)); done",
            ).await?;

            let raw = self
                .exec_output(&mut session, "cat /etc/rancher/k3s/k3s.yaml")
                .await?;
            let kc = raw.replace("127.0.0.1", self.ip);

            // ── 9. Deploy Multus CNI (master only) ───────────────────────────
            // Multus enables secondary network interfaces on pods (macvlan/ipvlan)
            // for fixed per-container IPs from our IPAM pools.
            // Tolerate errors here — admin can re-trigger via cluster update.
            let _ = self.exec(
                &mut session,
                "k3s kubectl apply -f \
                 https://raw.githubusercontent.com/k8snetworkplumbingwg/multus-cni/master/deployments/multus-daemonset-thick.yml \
                 2>/dev/null || true",
            ).await;

            // ── 10. NVIDIA device plugin (if GPU master) ─────────────────────
            if has_gpu {
                let _ = self.exec(
                    &mut session,
                    "k3s kubectl apply -f \
                     https://raw.githubusercontent.com/NVIDIA/k8s-device-plugin/v0.14.5/nvidia-device-plugin.yml \
                     2>/dev/null || true",
                ).await;
            }

            Some(kc)
        } else {
            self.exec(
                &mut session,
                &format!(
                    "curl -sfL https://get.k3s.io | \
                     INSTALL_K3S_EXEC='agent --node-ip={}' \
                     K3S_URL=https://{master_ip}:6443 K3S_TOKEN={k3s_token} sh -",
                    self.ip
                ),
            ).await?;

            // NVIDIA device plugin runs as a DaemonSet on the master — workers
            // just need the container toolkit (installed above) + the GPU label.
            None
        };

        let _ = hostname; // hostname used by callers for K8s node identification

        session.disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .map_err(|e| AppError::Ssh(format!("disconnect: {e}")))?;

        Ok(InstallResult { kubeconfig, has_gpu, gpu_model, gpu_count, main_iface })
    }

    async fn exec(
        &self,
        session: &mut client::Handle<Handler>,
        cmd: &str,
    ) -> AppResult<()> {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| AppError::Ssh(format!("open channel: {e}")))?;
        channel
            .exec(true, cmd)
            .await
            .map_err(|e| AppError::Ssh(format!("exec '{cmd}': {e}")))?;

        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                    if exit_status != 0 {
                        return Err(AppError::Ssh(format!("command exited {exit_status}: {cmd}")));
                    }
                    break;
                }
                Some(russh::ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }
        Ok(())
    }

    async fn exec_output(
        &self,
        session: &mut client::Handle<Handler>,
        cmd: &str,
    ) -> AppResult<String> {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| AppError::Ssh(format!("open channel: {e}")))?;
        channel
            .exec(true, cmd)
            .await
            .map_err(|e| AppError::Ssh(format!("exec: {e}")))?;

        let mut output = String::new();
        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { data }) => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(russh::ChannelMsg::ExitStatus { .. }) | None | Some(russh::ChannelMsg::Eof) => break,
                _ => {}
            }
        }
        Ok(output)
    }
}

/// Generate an RSA-4096 platform SSH keypair.
/// Returns `(private_key_pem, public_key_openssh_line)`.
pub fn generate_keypair() -> AppResult<(String, String)> {
    use rsa::{pkcs8::EncodePrivateKey, RsaPrivateKey};
    use rsa::pkcs8::LineEnding;

    let mut rng = rand::thread_rng();
    let priv_key =
        RsaPrivateKey::new(&mut rng, 4096).map_err(|e| AppError::Crypto(e.to_string()))?;

    let pem = priv_key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| AppError::Crypto(e.to_string()))?
        .to_string();

    use russh_keys::PublicKeyBase64;
    let key_pair = russh_keys::decode_secret_key(&pem, None)
        .map_err(|e| AppError::Crypto(format!("decode secret key: {e}")))?;
    let pub_b64 = key_pair.public_key_base64();
    let algo = key_pair.name();

    Ok((pem, format!("{algo} {pub_b64} quickstack")))
}
