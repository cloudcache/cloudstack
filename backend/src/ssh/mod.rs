//! SSH-based node provisioning with real-time step logging.

use std::sync::Arc;

use russh::client;
use russh_keys::key;

use crate::error::{AppError, AppResult};

pub mod provision_logger;
pub use provision_logger::ProvisionLogger;

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
    port: u16,
    /// SSH password. Empty → authenticate with the platform private key instead
    /// (the backend already has key access to root@node).
    password: &'a str,
    platform_pub_key: &'a str,
    platform_priv_key: &'a str,
    ldap_url: &'a str,
    ldap_base_dn: &'a str,
}

impl<'a> NodeInstaller<'a> {
    pub fn new(
        ip: &'a str,
        port: u16,
        password: &'a str,
        platform_pub_key: &'a str,
        platform_priv_key: &'a str,
        ldap_url: &'a str,
        ldap_base_dn: &'a str,
    ) -> Self {
        Self {
            ip,
            port,
            password,
            platform_pub_key,
            platform_priv_key,
            ldap_url,
            ldap_base_dn,
        }
    }

    // ── K3s provisioning ─────────────────────────────────────────────────────

    pub async fn run(
        &self,
        hostname: &str,
        master_ip: &str,
        k3s_token: &str,
        is_master: bool,
        storage_path: &str,
        pool_cidr: Option<&str>,
        pool_gateway: Option<&str>,
        logger: &ProvisionLogger,
    ) -> AppResult<InstallResult> {
        let resume_after = logger.last_ok_step().await;
        let mut session = self.connect(logger).await?;

        // Steps 1–7: common base setup
        let (main_iface, has_gpu, gpu_model, gpu_count, _is_rhel) =
            self.run_common_steps(&mut session, storage_path, logger, resume_after).await?;

        // ── 7.5 Detect number of physical NICs ─────────────────────────────
        // If only 1 NIC, bridge runs standalone (ipMasq=true, no master).
        // If ≥2 NICs, bridge binds to main_iface for L2 direct access.
        let nic_count_str = self
            .exec_step_output(
                &mut session,
                logger,
                7,
                "count_nics",
                "ls /sys/class/net | grep -vcE '^(lo|docker|veth|br-|cni|flannel|dummy)' || echo 1",
            )
            .await?;
        let nic_count: u32 = nic_count_str.trim().parse().unwrap_or(1);
        let multi_nic = nic_count >= 2;

        if !multi_nic {
            tracing::info!(
                node_ip = self.ip,
                "single NIC detected — bridge CNI will run standalone (ipMasq=true, no L2 direct)"
            );
        }

        // Build bridge CNI conflist based on NIC count
        let cni_conflist = match (pool_cidr, pool_gateway) {
            (Some(cidr), Some(gw)) => {
                let (master_line, ip_masq) = if multi_nic {
                    (format!("      \"master\": \"{main_iface}\",\n"), "false")
                } else {
                    (String::new(), "true")
                };
                Some(format!(
                    r#"{{
  "cniVersion": "0.3.1",
  "name": "qs-bridge",
  "plugins": [
    {{
      "type": "bridge",
      "bridge": "br-qs",
      "isGateway": true,
      "ipMasq": {ip_masq},
{master_line}      "ipam": {{
        "type": "host-local",
        "ranges": [[{{"subnet": "{cidr}", "gateway": "{gw}"}}]],
        "routes": [{{"dst": "0.0.0.0/0"}}],
        "dataDir": "/var/lib/cni/networks"
      }}
    }},
    {{"type": "portmap", "capabilities": {{"portMappings": true}}}}
  ]
}}"#
                ))
            }
            _ => None,
        };

        // ── 8. Write bridge CNI config BEFORE K3s install ────────────────────
        // containerd defaults to /etc/cni/net.d for config and /opt/cni/bin
        // for binaries. K3s with --flannel-backend=none does NOT override
        // these paths. We pre-write the config so containerd sees it the
        // moment K3s starts — eliminates the "cni plugin not initialized"
        // race window. Binaries are symlinked after install (step 10).
        if let Some(ref conflist) = cni_conflist {
            self.exec_step(
                &mut session,
                logger,
                8,
                "write_bridge_cni",
                &format!(
                    "mkdir -p /etc/cni/net.d && \
                     rm -f /etc/cni/net.d/10-flannel.conflist \
                           /var/lib/rancher/k3s/agent/etc/cni/net.d/10-flannel.conflist \
                           /var/lib/rancher/k3s/agent/etc/cni/net.d/10-bridge.conflist 2>/dev/null; \
                     ip link delete cni0 2>/dev/null; \
                     cat > /etc/cni/net.d/10-bridge.conflist <<'CNIEOF'\n\
{conflist}\n\
CNIEOF",
                ),
            )
            .await?;
        }

        // ── 9. K3s install ──────────────────────────────────────────────────
        let kubeconfig = if is_master {
            self.exec_step(
                &mut session,
                logger,
                9,
                "install_k3s_master",
                &format!(
                    "curl -sfL https://get.k3s.io | \
                     INSTALL_K3S_EXEC='server \
                       --disable traefik \
                       --disable servicelb \
                       --flannel-backend=none \
                       --service-cidr=10.96.0.0/12 \
                       --node-ip={} \
                       --write-kubeconfig-mode=644' \
                     K3S_TOKEN={k3s_token} sh -",
                    self.ip
                ),
            )
            .await?;

            // Symlink K3s-bundled CNI binaries to /opt/cni/bin (now that K3s data dir exists).
            // Also clean stale Multus binaries from previous installs.
            self.exec_step(
                &mut session,
                logger,
                10,
                "symlink_cni_binaries",
                "mkdir -p /opt/cni/bin && \
                 rm -f /opt/cni/bin/multus-shim /opt/cni/bin/passthru 2>/dev/null; \
                 K3S_BIN=/var/lib/rancher/k3s/data/current/bin && \
                 for p in bridge host-local loopback portmap; do \
                   ln -sf $K3S_BIN/$p /opt/cni/bin/$p; \
                 done",
            )
            .await?;

            // Wait for kubeconfig
            self.exec_step(
                &mut session,
                logger,
                11,
                "wait_kubeconfig",
                "i=0; until [ -f /etc/rancher/k3s/k3s.yaml ] || [ $i -ge 30 ]; \
                 do sleep 2; i=$((i+1)); done",
            )
            .await?;

            let raw = self
                .exec_step_output(&mut session, logger, 12, "read_kubeconfig", "cat /etc/rancher/k3s/k3s.yaml")
                .await?;
            let kc = raw.replace("127.0.0.1", self.ip);

            // coredns hostNetwork — no overlay, system pods use host network
            let _ = self.exec_step(
                &mut session,
                logger,
                13,
                "coredns_hostnetwork",
                "k3s kubectl -n kube-system patch deploy coredns --type=json \
                 -p='[{\"op\":\"add\",\"path\":\"/spec/template/spec/hostNetwork\",\"value\":true},\
                      {\"op\":\"add\",\"path\":\"/spec/template/spec/dnsPolicy\",\"value\":\"ClusterFirstWithHostNet\"}]' \
                 2>/dev/null || true",
            ).await;

            // NVIDIA device plugin
            if has_gpu {
                let _ = self.exec_step(
                    &mut session,
                    logger,
                    14,
                    "deploy_nvidia_plugin",
                    "k3s kubectl apply -f \
                     https://raw.githubusercontent.com/NVIDIA/k8s-device-plugin/v0.14.5/nvidia-device-plugin.yml \
                     2>/dev/null || true",
                ).await;
            }

            Some(kc)
        } else {
            self.exec_step(
                &mut session,
                logger,
                9,
                "install_k3s_worker",
                &format!(
                    "curl -sfL https://get.k3s.io | \
                     INSTALL_K3S_EXEC='agent --node-ip={}' \
                     K3S_URL=https://{master_ip}:6443 K3S_TOKEN={k3s_token} sh -",
                    self.ip
                ),
            )
            .await?;

            // Symlink K3s-bundled CNI binaries to /opt/cni/bin (now that K3s data dir exists).
            // Also clean stale Multus binaries from previous installs.
            self.exec_step(
                &mut session,
                logger,
                10,
                "symlink_cni_binaries",
                "mkdir -p /opt/cni/bin && \
                 rm -f /opt/cni/bin/multus-shim /opt/cni/bin/passthru 2>/dev/null; \
                 K3S_BIN=/var/lib/rancher/k3s/data/current/bin && \
                 for p in bridge host-local loopback portmap; do \
                   ln -sf $K3S_BIN/$p /opt/cni/bin/$p; \
                 done",
            )
            .await?;

            None
        };

        let _ = hostname;

        session
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .map_err(|e| AppError::Ssh(format!("disconnect: {e}")))?;

        Ok(InstallResult {
            kubeconfig,
            has_gpu,
            gpu_model,
            gpu_count,
            main_iface,
        })
    }

    // ── Docker provisioning ──────────────────────────────────────────────────

    pub async fn run_docker(
        &self,
        _hostname: &str,
        storage_path: &str,
        agent_url: &str,
        agent_port: u16,
        node_id: &str,
        agent_token: &str,
        backend_url: &str,
        logger: &ProvisionLogger,
    ) -> AppResult<InstallResult> {
        let resume_after = logger.last_ok_step().await;
        let mut session = self.connect(logger).await?;

        // Steps 1–7: common base setup
        let (main_iface, has_gpu, gpu_model, gpu_count, is_rhel) =
            self.run_common_steps(&mut session, storage_path, logger, resume_after).await?;

        // ── 8. Docker Engine ─────────────────────────────────────────────────
        self.exec_step(
            &mut session,
            logger,
            8,
            "install_docker",
            "if ! command -v docker >/dev/null 2>&1; then \
               curl -fsSL https://get.docker.com | sh; \
             fi && \
             systemctl enable docker && systemctl start docker",
        )
        .await?;

        // NVIDIA container toolkit for Docker
        if has_gpu {
            let nvidia_cmd = if is_rhel {
                "curl -s -L https://nvidia.github.io/libnvidia-container/stable/rpm/nvidia-container-toolkit.repo | \
                 tee /etc/yum.repos.d/nvidia-container-toolkit.repo && \
                 dnf install -y nvidia-container-toolkit && \
                 nvidia-ctk runtime configure --runtime=docker && \
                 systemctl restart docker".to_string()
            } else {
                "curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
                 gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg 2>/dev/null && \
                 curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
                 sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
                 tee /etc/apt/sources.list.d/nvidia-container-toolkit.list && \
                 export DEBIAN_FRONTEND=noninteractive && \
                 apt-get update -qq && apt-get install -y -qq nvidia-container-toolkit && \
                 nvidia-ctk runtime configure --runtime=docker && \
                 systemctl restart docker".to_string()
            };
            self.exec_step(&mut session, logger, 9, "install_nvidia_docker", &nvidia_cmd).await?;
        }

        // ── 9/10. Install qs-agent ───────────────────────────────────────────
        let step_idx = if has_gpu { 10 } else { 9 };
        self.exec_step(
            &mut session,
            logger,
            step_idx,
            "install_agent",
            &format!(
                "wget -q '{agent_url}' -O /usr/local/bin/qs-agent && \
                 chmod +x /usr/local/bin/qs-agent && \
                 mkdir -p /var/lib/qs-agent/files && \
                 cat > /etc/systemd/system/qs-agent.service << 'SVCEOF'\n\
[Unit]\nDescription=QuickStack Docker Agent\nAfter=network.target docker.service\nRequires=docker.service\n\
[Service]\nExecStart=/usr/local/bin/qs-agent \
  --port {agent_port} \
  --node-id {node_id} \
  --agent-token {agent_token} \
  --backend-url {backend_url} \
  --files-dir /var/lib/qs-agent/files\n\
Restart=always\nRestartSec=5\n\
[Install]\nWantedBy=multi-user.target\nSVCEOF\n\
                 systemctl daemon-reload && \
                 systemctl enable qs-agent && \
                 systemctl start qs-agent"
            ),
        )
        .await?;

        session
            .disconnect(russh::Disconnect::ByApplication, "", "en")
            .await
            .map_err(|e| AppError::Ssh(format!("disconnect: {e}")))?;

        Ok(InstallResult {
            kubeconfig: None,
            has_gpu,
            gpu_model,
            gpu_count,
            main_iface,
        })
    }

    // ── Shared steps (1–7) ───────────────────────────────────────────────────

    /// Runs steps 1–7 that are identical for K3s and Docker provisioning:
    /// SSH key, timezone, LDAP, storage, NIC detection, GPU detection, node_exporter.
    ///
    /// Supports Debian/Ubuntu (apt), Rocky/CentOS/RHEL (dnf/yum).
    ///
    /// `resume_after`: skip steps with index <= this value (from previous attempt).
    /// Returns `(main_iface, has_gpu, gpu_model, gpu_count, is_rhel)`.
    async fn run_common_steps(
        &self,
        session: &mut client::Handle<Handler>,
        storage_path: &str,
        logger: &ProvisionLogger,
        resume_after: u16,
    ) -> AppResult<(String, bool, Option<String>, u32, bool)> {
        if resume_after > 0 {
            tracing::info!("resuming provision for {} from step {}", logger.node_id, resume_after + 1);
        }

        // ── 0. Detect distro family ─────────────────────────────────────────
        // Reads ID_LIKE from /etc/os-release to determine package manager.
        // "debian" → apt, "rhel" / "fedora" → dnf/yum.
        let distro_raw = self
            .exec_step_output(
                session, logger, 0, "detect_distro",
                ". /etc/os-release 2>/dev/null && echo \"${ID_LIKE:-$ID}\"",
            )
            .await?;
        let distro_family = distro_raw.trim().to_lowercase();
        let is_rhel = distro_family.contains("rhel")
            || distro_family.contains("fedora")
            || distro_family.contains("centos")
            || distro_family.contains("rocky")
            || distro_family.contains("alma");

        tracing::info!(
            node_ip = self.ip,
            distro = %distro_family,
            pkg_mgr = if is_rhel { "dnf" } else { "apt" },
            "detected distro family"
        );

        // ── 1. SSH key ───────────────────────────────────────────────────────
        if resume_after < 1 {
            self.exec_step(
                session,
                logger,
                1,
                "authorize_ssh_key",
                &format!(
                    "mkdir -p ~/.ssh && echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
                    self.platform_pub_key
                ),
            ).await?;
        } else {
            let log_id = logger.step_begin(1, "authorize_ssh_key").await?;
            logger.step_finish(log_id, "SKIPPED", Some("already done")).await;
        }

        // ── 2. Timezone + firewall/SELinux prep ─────────────────────────────
        if resume_after < 2 {
            if is_rhel {
                // RHEL-based: disable firewalld (K3s manages iptables directly),
                // set SELinux to permissive (K3s + bridge CNI need permissive or
                // a custom policy). Also set timezone.
                self.exec_step(
                    session, logger, 2, "set_timezone_and_firewall",
                    "timedatectl set-timezone Asia/Shanghai || true; \
                     systemctl disable --now firewalld 2>/dev/null || true; \
                     if command -v setenforce >/dev/null 2>&1; then \
                       setenforce 0 || true; \
                       sed -i 's/^SELINUX=enforcing/SELINUX=permissive/' /etc/selinux/config 2>/dev/null || true; \
                     fi",
                ).await?;
            } else {
                self.exec_step(
                    session, logger, 2, "set_timezone",
                    "timedatectl set-timezone Asia/Shanghai || true",
                ).await?;
            }
        } else {
            let log_id = logger.step_begin(2, "set_timezone").await?;
            logger.step_finish(log_id, "SKIPPED", Some("already done")).await;
        }

        // ── 3/4. Packages + LDAP ────────────────────────────────────────────
        let ldap_configured = !self.ldap_url.is_empty()
            && !self.ldap_base_dn.is_empty();

        if resume_after < 4 {
            if ldap_configured {
                let install_cmd = if is_rhel {
                    format!(
                        "dnf install -y nss-pam-ldapd nscd wget curl && \
                         authselect select sssd with-mkhomedir --force 2>/dev/null || true"
                    )
                } else {
                    format!(
                        "export DEBIAN_FRONTEND=noninteractive && \
                         echo 'nslcd nslcd/ldap-uris string {ldap_url}' | debconf-set-selections && \
                         echo 'nslcd nslcd/ldap-base string {ldap_base}' | debconf-set-selections && \
                         echo 'libnss-ldap libnss-ldap/nsswitch note' | debconf-set-selections && \
                         echo 'libpam-ldap libpam-ldap/dblogin boolean false' | debconf-set-selections && \
                         apt-get update -qq && \
                         apt-get install -y -qq -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' \
                           nslcd libnss-ldap libpam-ldap nscd wget curl",
                        ldap_url = self.ldap_url,
                        ldap_base = self.ldap_base_dn,
                    )
                };
                self.exec_step(session, logger, 3, "install_packages", &install_cmd).await?;

                self.exec_step(
                    session,
                    logger,
                    4,
                    "configure_ldap",
                    &format!(
                        "cat > /etc/nslcd.conf << 'NSLCDEOF'\n\
uid nslcd\ngid nslcd\nuri {ldap_url}\nbase {ldap_base}\n\
tls_reqcert never\nNSLCDEOF\n\
                         chmod 640 /etc/nslcd.conf && \
                         sed -i 's/^passwd:.*/passwd: files ldap/' /etc/nsswitch.conf && \
                         sed -i 's/^group:.*/group: files ldap/' /etc/nsswitch.conf && \
                         sed -i 's/^shadow:.*/shadow: files ldap/' /etc/nsswitch.conf && \
                         systemctl stop nslcd nscd 2>/dev/null; \
                         timeout 30 systemctl start nslcd || true && \
                         timeout 30 systemctl start nscd || true",
                        ldap_url = self.ldap_url,
                        ldap_base = self.ldap_base_dn,
                    ),
                ).await.map_err(|_| AppError::Ssh(
                    format!("LDAP service start failed — server {} unreachable", self.ldap_url)
                ))?;
            } else {
                let install_cmd = if is_rhel {
                    "dnf install -y wget curl".to_string()
                } else {
                    "export DEBIAN_FRONTEND=noninteractive && \
                     apt-get update -qq && \
                     apt-get install -y -qq wget curl".to_string()
                };
                self.exec_step(session, logger, 3, "install_packages", &install_cmd).await?;

                let log_id = logger.step_begin(4, "configure_ldap").await?;
                logger.step_finish(log_id, "SKIPPED", Some("LDAP not configured")).await;
            }
        } else {
            let log_id = logger.step_begin(3, "install_packages").await?;
            logger.step_finish(log_id, "SKIPPED", Some("already done")).await;
            let log_id = logger.step_begin(4, "configure_ldap").await?;
            logger.step_finish(log_id, "SKIPPED", Some("already done")).await;
        }

        // ── 5. Storage ──────────────────────────────────────────────────────
        if resume_after < 5 {
            self.exec_step(
                session, logger, 5, "setup_storage",
                &format!("mkdir -p '{storage_path}' && chmod 1777 '{storage_path}'"),
            ).await?;
        } else {
            let log_id = logger.step_begin(5, "setup_storage").await?;
            logger.step_finish(log_id, "SKIPPED", Some("already done")).await;
        }

        // ── 6. Detect primary NIC (always re-run — need return value) ────────
        let main_iface = self
            .exec_step_output(
                session, logger, 6, "detect_nic",
                "ip route get 1.1.1.1 2>/dev/null | awk '/dev /{for(i=1;i<=NF;i++)if($i==\"dev\")print $(i+1)}' | head -1",
            )
            .await?
            .trim()
            .to_string();
        let main_iface = if main_iface.is_empty() {
            "eth0".to_string()
        } else {
            main_iface
        };

        // ── 7. Detect GPU + toolkit (always re-run — need return value) ──────
        let nvidia_dev = self
            .exec_step_output(session, logger, 7, "detect_gpu", "ls /dev/nvidia0 2>/dev/null")
            .await?;
        let has_gpu = !nvidia_dev.trim().is_empty();

        let (gpu_model, gpu_count) = if has_gpu {
            let info = self
                .exec_step_output(
                    session, logger, 7, "detect_gpu_info",
                    "nvidia-smi --query-gpu=name,count --format=csv,noheader 2>/dev/null | head -1",
                )
                .await?;
            let parts: Vec<&str> = info.trim().splitn(2, ',').collect();
            let model = parts.first().map(|s| s.trim().to_string());
            let count: u32 = parts
                .get(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);

            if resume_after < 7 {
                let nvidia_cmd = if is_rhel {
                    "curl -s -L https://nvidia.github.io/libnvidia-container/stable/rpm/nvidia-container-toolkit.repo | \
                     tee /etc/yum.repos.d/nvidia-container-toolkit.repo && \
                     dnf install -y nvidia-container-toolkit".to_string()
                } else {
                    "curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
                     gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg 2>/dev/null && \
                     curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
                     sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
                     tee /etc/apt/sources.list.d/nvidia-container-toolkit.list && \
                     export DEBIAN_FRONTEND=noninteractive && \
                     apt-get update -qq && apt-get install -y -qq nvidia-container-toolkit".to_string()
                };
                self.exec_step(session, logger, 7, "install_nvidia_toolkit", &nvidia_cmd).await?;
            }

            (model, count)
        } else {
            (None, 0)
        };

        // ── 7. node_exporter ─────────────────────────────────────────────────
        self.exec_step(
            session,
            logger,
            7,
            "install_node_exporter",
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

        Ok((main_iface, has_gpu, gpu_model, gpu_count, is_rhel))
    }

    // ── SSH connection ───────────────────────────────────────────────────────

    async fn connect(
        &self,
        logger: &ProvisionLogger,
    ) -> AppResult<client::Handle<Handler>> {
        let log_id = logger.step_begin(0, "ssh_connect").await?;

        let config = Arc::new(client::Config::default());
        let mut session = client::connect(config, (self.ip, self.port), Handler)
            .await
            .map_err(|e| {
                let msg = format!("connect to {}:{}: {e}", self.ip, self.port);
                // Fire-and-forget: log the failure
                let logger = logger.clone();
                let msg2 = msg.clone();
                tokio::spawn(async move {
                    logger.step_finish(log_id, "FAILED", Some(&msg2)).await;
                });
                AppError::Ssh(msg)
            })?;

        // Password auth when a password is supplied; otherwise authenticate with
        // the platform private key (the backend can already SSH to root@node).
        let authed = if !self.password.is_empty() {
            session
                .authenticate_password("root", self.password)
                .await
                .map_err(|e| AppError::Ssh(format!("password auth: {e}")))?
        } else {
            let keypair = russh_keys::decode_secret_key(self.platform_priv_key, None)
                .map_err(|e| AppError::Ssh(format!("parse platform private key: {e}")))?;
            session
                .authenticate_publickey("root", Arc::new(keypair))
                .await
                .map_err(|e| AppError::Ssh(format!("key auth: {e}")))?
        };
        if !authed {
            let how = if self.password.is_empty() { "platform key" } else { "password" };
            logger.step_finish(log_id, "FAILED", Some(&format!("{how} auth failed"))).await;
            return Err(AppError::Ssh(format!("SSH {how} authentication failed")));
        }

        logger.step_finish(log_id, "OK", Some(&format!("connected to {}", self.ip))).await;
        Ok(session)
    }

    // ── Logged exec methods ──────────────────────────────────────────────────

    /// Execute a command with step-level logging. Captures stdout/stderr,
    /// flushes output to DB incrementally, checks cancel between steps.
    async fn exec_step(
        &self,
        session: &mut client::Handle<Handler>,
        logger: &ProvisionLogger,
        step_index: u16,
        step_name: &str,
        cmd: &str,
    ) -> AppResult<()> {
        // Check cancel before starting
        if logger.is_cancelled().await {
            let log_id = logger.step_begin(step_index, step_name).await?;
            logger.step_finish(log_id, "CANCELLED", None).await;
            return Err(AppError::Ssh("provision cancelled by admin".into()));
        }

        let log_id = logger.step_begin(step_index, step_name).await?;

        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| AppError::Ssh(format!("open channel: {e}")))?;
        channel
            .exec(true, cmd)
            .await
            .map_err(|e| AppError::Ssh(format!("exec '{step_name}': {e}")))?;

        let mut output_buf = String::new();
        let mut exit_code: Option<u32> = None;

        loop {
            match channel.wait().await {
                Some(russh::ChannelMsg::Data { data }) => {
                    let chunk = String::from_utf8_lossy(&data);
                    output_buf.push_str(&chunk);
                    // Flush to DB every ~4KB to avoid too many writes
                    if output_buf.len() >= 4096 {
                        logger.step_append_output(log_id, &output_buf).await;
                        output_buf.clear();
                    }
                }
                Some(russh::ChannelMsg::ExtendedData { data, ext }) => {
                    // ext == 1 is stderr
                    let chunk = String::from_utf8_lossy(&data);
                    output_buf.push_str(&chunk);
                    if output_buf.len() >= 4096 {
                        logger.step_append_output(log_id, &output_buf).await;
                        output_buf.clear();
                    }
                }
                Some(russh::ChannelMsg::ExitStatus { exit_status }) => {
                    exit_code = Some(exit_status);
                }
                Some(russh::ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }

        // Flush remaining output
        if !output_buf.is_empty() {
            logger.step_append_output(log_id, &output_buf).await;
        }

        match exit_code {
            Some(0) | None => {
                logger.step_finish(log_id, "OK", None).await;
                Ok(())
            }
            Some(code) => {
                let err = format!("[exit {}] {}", code, step_name);
                logger.step_finish(log_id, "FAILED", Some(&err)).await;
                Err(AppError::Ssh(format!("command exited {code}: {step_name}")))
            }
        }
    }

    /// Execute a command, capture and return its stdout, with step logging.
    async fn exec_step_output(
        &self,
        session: &mut client::Handle<Handler>,
        logger: &ProvisionLogger,
        step_index: u16,
        step_name: &str,
        cmd: &str,
    ) -> AppResult<String> {
        if logger.is_cancelled().await {
            let log_id = logger.step_begin(step_index, step_name).await?;
            logger.step_finish(log_id, "CANCELLED", None).await;
            return Err(AppError::Ssh("provision cancelled by admin".into()));
        }

        let log_id = logger.step_begin(step_index, step_name).await?;

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
                Some(russh::ChannelMsg::ExtendedData { data, .. }) => {
                    // Capture stderr in log but don't include in return value
                    let chunk = String::from_utf8_lossy(&data);
                    logger.step_append_output(log_id, &format!("[stderr] {chunk}")).await;
                }
                Some(russh::ChannelMsg::ExitStatus { .. })
                | None
                | Some(russh::ChannelMsg::Eof) => break,
                _ => {}
            }
        }

        logger.step_finish(log_id, "OK", Some(&output)).await;
        Ok(output)
    }
}

/// Generate an RSA-4096 platform SSH keypair.
/// Returns `(private_key_pem, public_key_openssh_line)`.
pub fn generate_keypair() -> AppResult<(String, String)> {
    use rsa::pkcs8::LineEnding;
    use rsa::{pkcs8::EncodePrivateKey, RsaPrivateKey};

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
