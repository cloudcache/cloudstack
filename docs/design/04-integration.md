# QuickStack 集成设计文档

版本：1.0  
日期：2026-05-22  
状态：草稿

---

## 1. LLDAP 集成

### 1.1 概述

LLDAP（Lightweight LDAP）作为平台唯一身份源。平台不存储用户密码，所有认证通过 LDAP Bind 完成。

### 1.2 连接配置

```toml
# config/default.toml
[ldap]
url              = "ldap://lldap.internal:3890"
base_dn          = "dc=example,dc=com"
bind_dn          = "cn=admin,dc=example,dc=com"
bind_password    = "${LDAP_BIND_PASSWORD}"   # 从环境变量读取
user_ou          = "ou=people"
group_ou         = "ou=groups"
user_filter      = "(&(objectClass=person)(uid={}))"
sync_interval    = 600                        # 秒
connection_timeout = 5                        # 秒
pool_size        = 5
```

### 1.3 登录认证流程

```
POST /api/v1/auth/login { username, password, totp_code? }
  │
  ├─ 1. 构造 User DN：uid={username},{user_ou},{base_dn}
  │
  ├─ 2. ldap3::LdapConn::bind(user_dn, password)
  │       错误类型：
  │         InvalidCredentials → 401（密码错误）
  │         NoSuchObject       → 401（用户不存在）
  │         其他 LDAP 错误     → 500
  │
  ├─ 3. Bind 成功后，执行 Admin Bind 查询用户属性：
  │       ldap3::LdapConn::bind(bind_dn, bind_password)
  │       search(
  │         base = "{user_ou},{base_dn}",
  │         scope = Scope::One,
  │         filter = "(&(objectClass=person)(uid={username}))",
  │         attrs = ["uid","mail","cn","uidNumber","gidNumber","memberOf"]
  │       )
  │
  ├─ 4. 提取属性，Upsert users 表：
  │       INSERT INTO users (id, username, email, display_name,
  │                          ldap_dn, ldap_uid, ldap_gid, ...)
  │       ON DUPLICATE KEY UPDATE
  │         email=..., display_name=..., ldap_dn=...,
  │         ldap_uid=..., ldap_gid=..., updated_at=NOW()
  │
  ├─ 5. 同步全局管理员状态：
  │       查询 LDAP 组：cn=lldap_admin,{group_ou},{base_dn}
  │       检查 member 是否包含 user_dn
  │       → 更新 users.is_global_admin
  │
  ├─ 6. 检查 users.is_active = 1，否则 → 403
  │
  ├─ 7. 若 totp_credentials.enabled = true：
  │       验证 totp_code（TOTP-RS 库）
  │       验证失败 → 401 TOTP_INVALID
  │
  ├─ 8. 生成 JWT（HS256，payload：user_id，exp：+24h）
  │       写入 user_sessions 表（token_hash = SHA256(jwt)）
  │
  └─ 9. 返回 token + user 信息
```

### 1.4 LDAP 组到角色映射

LLDAP 中的组约定，管理员在 LLDAP Web UI 中手动创建和维护：

| LLDAP 组名 | 平台角色 | 说明 |
|-----------|----------|------|
| `lldap_admin` | GlobalAdmin | 用户登录时自动检测 |
| `qs_project_{name}_admin` | 项目 ADMIN | 定时同步 |
| `qs_project_{name}_op` | 项目 OPERATOR | 定时同步 |
| `qs_project_{name}_obs` | 项目 OBSERVER | 定时同步 |

### 1.5 定时同步任务

每隔 `ldap.sync_interval` 秒执行一次全量同步：

```rust
async fn sync_ldap_roles(pool: &MySqlPool, ldap: &LdapClient) {
    // 1. 查询所有 qs_project_* 开头的 LDAP 组
    let groups = ldap.search_groups("qs_project_*").await?;

    // 2. 解析组名，提取 project_name 和 role
    for group in groups {
        let (project_name, role) = parse_group_name(&group.cn)?;
        let project = db::projects::find_by_name(pool, &project_name).await?;
        let members = ldap.get_group_members(&group.dn).await?;

        // 3. 同步 project_members 表
        //    - 新增 LDAP 组中有但表中没有的成员
        //    - 删除 LDAP 组中没有但表中有的成员（仅删除来自 LDAP 的，手动添加的保留）
        db::project_members::sync_from_ldap(pool, &project.id, &role, &members).await?;
    }
}
```

### 1.6 节点 LDAP 认证配置

安装时，installer 在每个节点生成并写入以下配置文件：

**`/etc/nslcd.conf`（模板渲染）：**
```
uid nslcd
gid nslcd
uri {ldap_url}
base {base_dn}
binddn {bind_dn}
bindpw {bind_password}
base passwd {user_ou},{base_dn}
base group {group_ou},{base_dn}
filter passwd (objectClass=person)
map passwd uid uid
map passwd uidNumber uidNumber
map passwd gidNumber gidNumber
map passwd homeDirectory homeDirectory
map passwd loginShell loginShell
```

**`/etc/nsswitch.conf`：**
```
passwd:   files ldap
group:    files ldap
shadow:   files ldap
gshadow:  files
```

**`/etc/pam.d/common-auth`（追加）：**
```
auth sufficient pam_ldap.so
auth required   pam_unix.so try_first_pass
```

---

## 2. Kubernetes（K3s）集成

### 2.1 连接配置

```rust
// 在 K3s Master 节点内运行时，使用 in-cluster config
let client = Client::try_default().await?;

// 开发环境或外部访问时，使用 kubeconfig
let config = Config::from_kubeconfig(&KubeConfigOptions::default()).await?;
let client = Client::try_from(config)?;
```

### 2.2 命名空间管理

```rust
// 创建项目时创建 namespace
async fn create_namespace(client: &Client, project_name: &str) -> Result<()> {
    let ns: Namespace = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Namespace",
        "metadata": {
            "name": project_name,
            "labels": {
                "qs/managed": "true",
                "qs/project": project_name
            }
        }
    }))?;
    let api: Api<Namespace> = Api::all(client.clone());
    api.create(&PostParams::default(), &ns).await?;
    Ok(())
}
```

### 2.3 Deployment 生成规则

#### 标签约定

```yaml
metadata:
  labels:
    qs-app: "{app_id}"          # 用于 Service/Ingress selector
    qs-project: "{project_name}"
    qs-user: "{username}"
    app.kubernetes.io/name: "{app_name}"
```

#### ConfigMap（环境变量）

```
名称：env-{app_id}
命名空间：{project_name}
data：所有 is_secret=false 的 env_var
```

#### Secret（敏感环境变量 + Registry 凭据）

```
名称：secret-{app_id}
data：所有 is_secret=true 的 env_var（Base64）

名称：registry-{app_id}（私有镜像时）
type: kubernetes.io/dockerconfigjson
data：.dockerconfigjson
```

#### Service（NodePort 类型）

每个应用创建一个 `NodePort` 类型的 K8s Service，Pingora 直接将外部流量路由到此端口，无需 Ingress 中间层。

```yaml
apiVersion: v1
kind: Service
metadata:
  name: svc-{app_id}
  namespace: {project_name}
  labels:
    qs-app: "{app_id}"
spec:
  type: NodePort
  selector:
    qs-app: "{app_id}"
  ports:
    - port: {container_port}
      targetPort: {container_port}
      nodePort: {nodeport}         # 平台分配，范围 30000-32767，写回 app_ports.nodeport
      protocol: TCP
```

#### NodePort 分配逻辑

```rust
async fn allocate_nodeport(pool: &MySqlPool) -> Result<u16> {
    // 查询已使用的 nodeport 集合
    let used: Vec<u16> = sqlx::query_scalar!(
        "SELECT nodeport FROM app_ports WHERE nodeport IS NOT NULL"
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .flatten()
    .collect();

    // 从 30000 开始找第一个未使用的端口
    // 跳过 30100（Registry 固定占用）
    let reserved = std::collections::HashSet::from([30100u16]);
    for port in 30000u16..=32767 {
        if !used.contains(&port) && !reserved.contains(&port) {
            return Ok(port);
        }
    }
    Err(AppError::QuotaExceeded("NodePort 端口已耗尽（上限 2767 个应用）".into()))
}
```

#### pingora 路由注册（与 Service 创建同步完成）

Service 创建后，平台立即通过 pingora REST API 注册路由；详细实现见第 3.3 节。

```rust
// 简化调用入口，Service 成功创建后调用
async fn register_app_domains(
    pingora: &PingoraClient,
    app: &App,
    nodeport: u16,
    any_node_ip: &str,
) -> Result<()> {
    for domain in &app.domains {
        register_app_domain(pingora, domain, nodeport, any_node_ip).await?;
    }
    Ok(())
}
```

### 2.5 NetworkPolicy 生成

节点外流量经由 NodePort 到达 Pod，NodePort 流量由 kube-proxy 转发，**不受 NetworkPolicy 中 `ingress.from` 规则限制**（这是 K8s 的标准行为）。因此 NetworkPolicy 主要控制 Pod 间通信。

```rust
fn build_network_policy(app: &App) -> Option<NetworkPolicy> {
    match app.network_policy {
        NetworkPolicy::AllowAll => {
            None  // 不创建 NetworkPolicy，K8s 默认允许所有
        }
        NetworkPolicy::NamespaceOnly => {
            // 仅允许同命名空间 Pod 互访，外部 NodePort 流量不受限
            // ingress: [{ from: [{ podSelector: {} }] }]
            // egress:  [{ to:   [{ podSelector: {} }] }]
            Some(build_namespace_only_policy(app))
        }
        NetworkPolicy::DenyAll => {
            // 阻断所有 Pod 间通信（出站和入站）
            // NodePort 入站流量依然可达（K8s 规范）
            // ingress: []  egress: []
            Some(build_deny_all_policy(app))
        }
        NetworkPolicy::InternetOnly => {
            // 仅允许出站访问外网（非集群内私有 IP 段）
            // ingress: []
            // egress: [{ to: [{ ipBlock: { cidr: 0.0.0.0/0,
            //   except: [10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16] } }] }]
            Some(build_internet_only_policy(app))
        }
    }
}
```

### 2.6 BuildKit Job

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: build-{build_job_id}
  namespace: registry-and-build
spec:
  ttlSecondsAfterFinished: 3600
  template:
    spec:
      restartPolicy: Never
      initContainers:
        - name: git-clone
          image: alpine/git:latest
          command: ["git", "clone", "--branch", "{branch}", "{git_url}", "/workspace"]
          env:
            - name: GIT_TOKEN
              valueFrom:
                secretKeyRef:
                  name: git-token-{app_id}
                  key: token
          volumeMounts:
            - name: workspace
              mountPath: /workspace
      containers:
        - name: buildkit
          image: moby/buildkit:latest
          args:
            - --addr
            - unix:///run/buildkit/buildkitd.sock
          securityContext:
            privileged: true        # BuildKit 需要特权
          command:
            - buildctl
            - build
            - --frontend=dockerfile.v0
            - --local
            - context=/workspace
            - --local
            - dockerfile=/workspace/{dockerfile_path}
            - --output
            - type=image,name={registry_host}/{project}/{app}:{commit_hash},push=true
          volumeMounts:
            - name: workspace
              mountPath: /workspace
      volumes:
        - name: workspace
          emptyDir: {}
```

### 2.7 应用暂停操作

暂停/恢复的详细实现见第 3.4 节（pingora-proxy-manager 集成）。此处仅记录 K8s 侧操作：

```rust
// 暂停：仅 K8s 侧缩容，路由层由 deploy_service 调用 pingora 处理
async fn k8s_scale_zero(client: &Client, app: &App) -> Result<()> {
    Api::<Deployment>::namespaced(client.clone(), &app.project_name)
        .patch(
            &format!("app-{}", app.id),
            &PatchParams::apply("quickstack"),
            &Patch::MergePatch(json!({ "spec": { "replicas": 0 } })),
        ).await?;
    Ok(())
}

// 恢复：K8s 侧恢复副本数，路由层由 deploy_service 调用 pingora 处理
async fn k8s_scale_restore(client: &Client, app: &App) -> Result<()> {
    Api::<Deployment>::namespaced(client.clone(), &app.project_name)
        .patch(
            &format!("app-{}", app.id),
            &PatchParams::apply("quickstack"),
            &Patch::MergePatch(json!({ "spec": { "replicas": app.replicas } })),
        ).await?;
    Ok(())
}
```

### 2.8 实时 Pod 状态监听

```rust
// 使用 kube-rs Watch API 实时监听 Pod 变化，通过 WebSocket 推送前端
async fn watch_pod_status(client: &Client, project_name: &str, app_id: &str, tx: Sender<PodStatus>) {
    let pods: Api<Pod> = Api::namespaced(client.clone(), project_name);
    let lp = ListParams::default()
        .labels(&format!("qs-app={}", app_id));

    let mut stream = watcher(pods, lp).boxed();
    while let Some(event) = stream.next().await {
        match event {
            Ok(Event::Applied(pod)) => { tx.send(extract_status(&pod)).await?; }
            Ok(Event::Deleted(pod)) => { tx.send(PodStatus::Deleted(pod.name())).await?; }
            _ => {}
        }
    }
}
```

---

## 3. pingora-proxy-manager 集成

### 3.1 概述

[pingora-proxy-manager](https://github.com/DDULDDUCK/pingora-proxy-manager) 基于 Cloudflare Pingora 框架，提供：
- HTTP/HTTPS 反向代理，流量入口 `:80/:443`
- 内建 ACME 客户端，支持 HTTP-01 和 DNS-01 证书自动签发与续签
- REST 管理 API，基础路径 `http://{host}:81/api`，JWT 认证

QuickStack 后端在启动时通过 `POST /api/login` 获取 JWT，缓存在内存中并在过期前自动续签；之后所有路由操作均携带 `Authorization: Bearer {jwt}` 头。

### 3.2 PingoraClient 封装

```rust
pub struct PingoraClient {
    http:         reqwest::Client,
    api_base:     String,               // http://{host}:81/api
    username:     String,
    password:     String,               // 解密后的明文
    token:        Arc<RwLock<String>>,  // 缓存的 JWT
    token_expiry: Arc<RwLock<Instant>>,
}

impl PingoraClient {
    /// 登录获取 JWT，启动时和 token 过期后调用
    async fn refresh_token(&self) -> Result<()> {
        let resp: serde_json::Value = self.http
            .post(format!("{}/login", self.api_base))
            .json(&json!({ "username": &self.username, "password": &self.password }))
            .send().await?
            .json().await?;
        let token = resp["token"].as_str()
            .ok_or(AppError::ProxyManager("登录响应无 token 字段".into()))?
            .to_string();
        *self.token.write().await = token;
        *self.token_expiry.write().await = Instant::now() + Duration::from_secs(3600);
        Ok(())
    }

    async fn bearer(&self) -> Result<String> {
        if Instant::now() >= *self.token_expiry.read().await {
            self.refresh_token().await?;
        }
        Ok(format!("Bearer {}", self.token.read().await))
    }

    // ── Proxy Hosts ─────────────────────────────────────────────

    /// 创建代理 Host（新应用域名 / 系统域名注册）
    pub async fn create_host(&self, req: &CreateHostRequest) -> Result<()> {
        self.http
            .post(format!("{}/hosts", self.api_base))
            .header("Authorization", self.bearer().await?)
            .json(req)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// 删除代理 Host（应用删除 / 域名解绑）
    pub async fn delete_host(&self, domain: &str) -> Result<()> {
        self.http
            .delete(format!("{}/hosts/{}", self.api_base, urlencoding::encode(domain)))
            .header("Authorization", self.bearer().await?)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// 更新 Host 的 Location（应用暂停：将 / 重写到 /_qs/maintenance）
    pub async fn set_maintenance_location(&self, domain: &str, qs_host: &str) -> Result<()> {
        let body = json!({
            "path": "/",
            "upstream_ip":   qs_host,       // QuickStack 后端 IP（Master 节点）
            "upstream_port": 3000,
            "path_rewrite":  "/_qs/maintenance"
        });
        self.http
            .post(format!("{}/hosts/{}/locations", self.api_base,
                          urlencoding::encode(domain)))
            .header("Authorization", self.bearer().await?)
            .json(&body)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// 删除暂停 Location（恢复应用时移除维护重写规则）
    pub async fn remove_maintenance_location(&self, domain: &str) -> Result<()> {
        let body = json!({ "path": "/" });
        self.http
            .delete(format!("{}/hosts/{}/locations", self.api_base,
                            urlencoding::encode(domain)))
            .header("Authorization", self.bearer().await?)
            .json(&body)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    // ── SSL Certificates ─────────────────────────────────────────

    /// 申请 Let's Encrypt 证书（创建自定义域名时调用）
    pub async fn request_cert(&self, domains: &[&str]) -> Result<()> {
        let body = json!({ "domains": domains });
        self.http
            .post(format!("{}/certs", self.api_base))
            .header("Authorization", self.bearer().await?)
            .json(&body)
            .send().await?
            .error_for_status()?;
        Ok(())
    }

    /// 轮询证书状态（同步到 app_domains.cert_status）
    pub async fn list_certs(&self) -> Result<Vec<CertInfo>> {
        let resp: serde_json::Value = self.http
            .get(format!("{}/certs", self.api_base))
            .header("Authorization", self.bearer().await?)
            .send().await?
            .json().await?;
        // 解析并返回 CertInfo { domain, status, expiry }
        parse_certs(resp)
    }

    // ── Health Check ─────────────────────────────────────────────

    pub async fn health_check(&self) -> Result<bool> {
        Ok(self.http
            .get(format!("{}/stats/realtime", self.api_base))
            .header("Authorization", self.bearer().await?)
            .send().await?
            .status()
            .is_success())
    }
}

#[derive(serde::Serialize)]
pub struct CreateHostRequest {
    pub domain:        String,
    pub scheme:        String,       // "https"
    pub upstream_ip:   String,       // 任意一个 K3s 节点 IP（或 Master IP）
    pub upstream_port: u16,          // NodePort
    pub ssl_force:     bool,         // true = HTTP→HTTPS 重定向
}
```

### 3.3 应用路由注册（创建/更新应用时）

每个应用只有**一个 NodePort**，在所有 K3s 节点上均可访问（kube-proxy 保证）。`upstream_ip` 填写任意 Ready 节点 IP 即可；pingora 内部不做多上游轮询（单节点 NodePort 已经由 kube-proxy 负载均衡到多个 Pod）。

```rust
async fn register_app_domain(
    pingora: &PingoraClient,
    domain: &AppDomain,
    nodeport: u16,
    any_node_ip: &str,   // 取第一个 Ready Worker 节点 IP
) -> Result<()> {
    pingora.create_host(&CreateHostRequest {
        domain:        domain.hostname.clone(),
        scheme:        "https".into(),
        upstream_ip:   any_node_ip.into(),
        upstream_port: nodeport,
        ssl_force:     domain.redirect_https,
    }).await?;

    // 若是自定义域名，同时触发证书申请
    if !domain.is_system_domain {
        pingora.request_cert(&[&domain.hostname]).await?;
    }
    Ok(())
}
```

### 3.4 应用暂停/恢复的路由切换

暂停时，pingora 的 Location 重写将所有请求导向 QuickStack 的维护页端点；恢复时删除该 Location 规则，流量重新落回 NodePort。

```rust
async fn pause_app(client: &Client, pingora: &PingoraClient, app: &App, master_ip: &str) -> Result<()> {
    // 1. K8s Deployment replicas → 0
    Api::<Deployment>::namespaced(client.clone(), &app.project_name)
        .patch(&format!("app-{}", app.id), &PatchParams::apply("qs"),
               &Patch::MergePatch(json!({"spec":{"replicas":0}})))
        .await?;

    // 2. pingora：为每个域名添加 Location 重写到 /_qs/maintenance
    for domain in &app.domains {
        pingora.set_maintenance_location(&domain.hostname, master_ip).await?;
    }
    Ok(())
}

async fn resume_app(client: &Client, pingora: &PingoraClient, app: &App) -> Result<()> {
    // 1. K8s Deployment replicas 恢复
    Api::<Deployment>::namespaced(client.clone(), &app.project_name)
        .patch(&format!("app-{}", app.id), &PatchParams::apply("qs"),
               &Patch::MergePatch(json!({"spec":{"replicas": app.replicas}})))
        .await?;

    // 2. pingora：删除 Location 重写，流量恢复到 NodePort
    for domain in &app.domains {
        pingora.remove_maintenance_location(&domain.hostname).await?;
    }
    Ok(())
}
```

### 3.5 安装时初始化（注册平台管理域名）

```rust
async fn init_platform_route(pingora: &PingoraClient, platform_domain: &str, master_ip: &str) -> Result<()> {
    // 注册平台管理界面域名，上游为 Master 节点 :3000（QuickStack systemd 服务）
    pingora.create_host(&CreateHostRequest {
        domain:        platform_domain.into(),
        scheme:        "https".into(),
        upstream_ip:   master_ip.into(),
        upstream_port: 3000,
        ssl_force:     true,
    }).await?;

    // 申请平台域名证书
    pingora.request_cert(&[platform_domain]).await?;

    // 注意：*.apps.example.com 泛域名证书需 DNS-01，
    // 在 pingora 管理界面手动配置 DNS provider，或由管理员在安装后配置。
    // 系统分配的应用二级域名在第一次创建时逐个申请单域名证书（HTTP-01），
    // 无需泛域名证书。
    Ok(())
}
```

### 3.6 QuickStack 维护页端点

QuickStack 后端（axum）自身暴露 `/_qs/maintenance`，供 pingora Location 重写后访问：

```rust
// 在 api/mod.rs 路由注册中加入
.route("/_qs/maintenance", get(maintenance_handler))

async fn maintenance_handler() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../assets/maintenance.html"),
    )
}
```

`maintenance.html` 为静态资源，内嵌在二进制中（`include_str!`），显示「服务暂停维护」页面。

---

## 4. 共享数据库集群集成

### 4.1 MySQL Galera 集成

#### 连接管理

```rust
// 使用 sqlx 连接租户 DB 集群（独立连接池，与平台 DB 分开）
async fn get_cluster_pool(cluster: &DatabaseCluster) -> Result<MySqlPool> {
    let password = decrypt(&cluster.admin_password)?;
    let url = format!(
        "mysql://{}:{}@{}:{}/",
        cluster.admin_user, password, cluster.host, cluster.port
    );
    MySqlPool::connect(&url).await.map_err(|e| AppError::ClusterError(e.to_string()))
}
```

#### 创建数据库实例

```rust
async fn create_mysql_instance(
    pool: &MySqlPool,
    db_name: &str,
    db_user: &str,
    password: &str,
) -> Result<()> {
    // 数据库名称安全校验（仅允许字母数字下划线）
    validate_db_name(db_name)?;

    // 使用格式化字符串（非绑定参数，因为 CREATE DATABASE 不支持参数化）
    // db_name 已经过白名单校验，安全
    sqlx::query(&format!(
        "CREATE DATABASE `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
        db_name
    )).execute(pool).await?;

    sqlx::query(&format!(
        "CREATE USER `{}`@`%` IDENTIFIED BY ?",
        db_user
    )).bind(password).execute(pool).await?;

    sqlx::query(&format!(
        "GRANT SELECT,INSERT,UPDATE,DELETE,CREATE,DROP,INDEX,ALTER,\
         CREATE TEMPORARY TABLES,LOCK TABLES,CREATE VIEW,SHOW VIEW,\
         CREATE ROUTINE,ALTER ROUTINE,EXECUTE,REFERENCES,TRIGGER \
         ON `{}`.* TO `{}`@`%`",
        db_name, db_user
    )).execute(pool).await?;

    sqlx::query("FLUSH PRIVILEGES").execute(pool).await?;
    Ok(())
}
```

#### 删除数据库实例

```rust
async fn drop_mysql_instance(pool: &MySqlPool, db_name: &str, db_user: &str) -> Result<()> {
    validate_db_name(db_name)?;

    sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", db_name))
        .execute(pool).await?;

    sqlx::query(&format!("DROP USER IF EXISTS `{}`@`%`", db_user))
        .execute(pool).await?;

    sqlx::query("FLUSH PRIVILEGES").execute(pool).await?;
    Ok(())
}
```

### 4.2 PostgreSQL 集成

#### 创建数据库实例

```rust
async fn create_pg_instance(
    pool: &PgPool,
    db_name: &str,
    db_user: &str,
    password: &str,
) -> Result<()> {
    validate_db_name(db_name)?;

    sqlx::query(&format!(
        "CREATE DATABASE \"{}\" ENCODING 'UTF8' LC_COLLATE 'en_US.UTF-8'",
        db_name
    )).execute(pool).await?;

    sqlx::query(&format!("CREATE USER \"{}\" WITH PASSWORD $1", db_user))
        .bind(password)
        .execute(pool).await?;

    sqlx::query(&format!(
        "GRANT ALL PRIVILEGES ON DATABASE \"{}\" TO \"{}\"",
        db_name, db_user
    )).execute(pool).await?;

    // 授予 schema 权限（PostgreSQL 15+ 需要额外步骤）
    // 连接到目标数据库执行
    let db_pool = PgPool::connect(&format!(
        "postgres://admin:pass@host/{}", db_name
    )).await?;

    sqlx::query(&format!(
        "GRANT ALL ON SCHEMA public TO \"{}\"", db_user
    )).execute(&db_pool).await?;

    Ok(())
}
```

### 4.3 K8s Secret 创建（数据库凭据注入）

```rust
async fn create_db_secret(
    client: &Client,
    namespace: &str,
    instance: &DatabaseInstance,
    cluster: &DatabaseCluster,
    password: &str,
) -> Result<String> {
    let secret_name = format!("db-{}", instance.id);
    let db_url = match cluster.cluster_type {
        ClusterType::MysqlGalera => format!(
            "mysql://{}:{}@{}:{}/{}",
            instance.db_user, password, cluster.host, cluster.port, instance.db_name
        ),
        ClusterType::Postgresql => format!(
            "postgresql://{}:{}@{}:{}/{}",
            instance.db_user, password, cluster.host, cluster.port, instance.db_name
        ),
    };

    let secret: Secret = serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": &secret_name,
            "namespace": namespace,
            "labels": { "qs/db-instance": &instance.id }
        },
        "type": "Opaque",
        "stringData": {
            "DB_HOST":     &cluster.host,
            "DB_PORT":     cluster.port.to_string(),
            "DB_NAME":     &instance.db_name,
            "DB_USER":     &instance.db_user,
            "DB_PASSWORD": password,
            "DB_URL":      &db_url
        }
    }))?;

    let api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    api.create(&PostParams::default(), &secret).await?;
    Ok(secret_name)
}
```

---

## 5. SSH 节点管理集成

### 5.1 SSH 客户端封装（russh）

```rust
pub struct SshClient {
    session: russh::client::Handle<Handler>,
}

impl SshClient {
    pub async fn connect(host: &str, port: u16, user: &str, auth: SshAuth) -> Result<Self> {
        let config = Arc::new(russh::client::Config {
            ..(russh::client::Config::default())
        });
        let handler = Handler::default();
        let mut session = russh::client::connect(config, (host, port), handler).await?;

        match auth {
            SshAuth::Password(pass) => {
                session.authenticate_password(user, pass).await?;
            }
            SshAuth::PrivateKey(key_pem) => {
                let key = russh_keys::decode_secret_key(&key_pem, None)?;
                session.authenticate_publickey(user, Arc::new(key)).await?;
            }
        }
        Ok(Self { session })
    }

    pub async fn exec(&self, command: &str) -> Result<ExecOutput> {
        let mut channel = self.session.channel_open_session().await?;
        channel.exec(true, command).await?;
        // 读取 stdout/stderr，等待 EOF
        let output = read_channel_output(&mut channel).await?;
        Ok(output)
    }

    pub async fn upload_file(&self, content: &[u8], remote_path: &str) -> Result<()> {
        // 使用 SCP 或 SFTP 上传
        let mut channel = self.session.channel_open_session().await?;
        channel.exec(true, &format!(
            "cat > {}", shell_escape(remote_path)
        )).await?;
        channel.data(content).await?;
        channel.eof().await?;
        Ok(())
    }
}
```

### 5.2 节点安装脚本编排

```rust
pub struct NodeInstaller {
    ssh: SshClient,
    config: PlatformConfig,
    progress_tx: mpsc::Sender<InstallLog>,
}

impl NodeInstaller {
    pub async fn install_master(&self, join_token: Option<String>) -> Result<String> {
        self.log("info", "开始安装 K3s Master...").await;

        // 1. 安装 K3s server
        let install_cmd = format!(
            r#"curl -sfL https://get.k3s.io | \
               INSTALL_K3S_EXEC="server \
                 --disable traefik \
                 --node-name {hostname}" sh -"#,
            hostname = &self.config.hostname
        );
        self.ssh.exec(&install_cmd).await?;
        self.log("info", "✓ K3s server 安装完成").await;

        // 2. 读取 join token
        let token = self.ssh.exec("cat /var/lib/rancher/k3s/server/node-token").await?;
        self.log("info", "✓ 获取集群 Join Token").await;

        // 3. 配置 LDAP 认证
        self.setup_ldap_auth().await?;

        // 4. 检测并配置 GPU
        self.setup_gpu_if_present().await?;

        // 5. 验证存储挂载
        self.verify_storage().await?;

        // 6. 打节点标签
        self.label_node().await?;

        Ok(token.stdout.trim().to_string())
    }

    pub async fn install_worker(&self, master_ip: &str, join_token: &str) -> Result<()> {
        self.log("info", "开始安装 K3s Worker...").await;

        let install_cmd = format!(
            r#"curl -sfL https://get.k3s.io | \
               K3S_URL=https://{master}:6443 \
               K3S_TOKEN={token} \
               INSTALL_K3S_EXEC="agent --node-name {hostname}" sh -"#,
            master = master_ip,
            token = join_token,
            hostname = &self.config.hostname
        );
        self.ssh.exec(&install_cmd).await?;
        self.log("info", "✓ K3s agent 安装完成").await;

        self.setup_ldap_auth().await?;
        self.setup_gpu_if_present().await?;
        self.verify_storage().await?;
        self.label_node().await?;
        Ok(())
    }

    async fn setup_ldap_auth(&self) -> Result<()> {
        self.log("info", "配置 LDAP 认证（nslcd）...").await;

        // 安装依赖
        let pkg_manager = self.ssh.exec("which apt-get || which yum").await?;
        let install_cmd = if pkg_manager.stdout.contains("apt-get") {
            "DEBIAN_FRONTEND=noninteractive apt-get install -y nslcd libnss-ldapd libpam-ldapd"
        } else {
            "yum install -y nss-pam-ldapd"
        };
        self.ssh.exec(install_cmd).await?;

        // 上传渲染后的配置文件
        let nslcd_conf = render_nslcd_conf(&self.config.ldap);
        self.ssh.upload_file(nslcd_conf.as_bytes(), "/etc/nslcd.conf").await?;

        let nsswitch = render_nsswitch();
        self.ssh.upload_file(nsswitch.as_bytes(), "/etc/nsswitch.conf").await?;

        let pam_auth = render_pam_common_auth();
        self.ssh.upload_file(pam_auth.as_bytes(), "/etc/pam.d/common-auth").await?;

        // 启动服务
        self.ssh.exec("systemctl enable --now nslcd").await?;
        self.ssh.exec("systemctl restart nslcd").await?;

        // 验证
        let test = self.ssh.exec(
            &format!("getent passwd {} | head -1", &self.config.ldap.test_user)
        ).await?;
        if test.stdout.is_empty() {
            return Err(AppError::InstallError("LDAP 用户验证失败，请检查 LLDAP 配置".into()));
        }

        self.log("info", "✓ LDAP 认证配置完成").await;
        Ok(())
    }

    async fn setup_gpu_if_present(&self) -> Result<()> {
        let check = self.ssh.exec("ls /dev/nvidia0 2>/dev/null || nvidia-smi 2>/dev/null").await?;
        if check.exit_status != 0 {
            return Ok(());  // 无 GPU，跳过
        }

        self.log("info", "检测到 GPU，安装 nvidia-container-toolkit...").await;

        self.ssh.exec(
            "curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
             gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg && \
             curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
             sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
             tee /etc/apt/sources.list.d/nvidia-container-toolkit.list && \
             apt-get update && \
             apt-get install -y nvidia-container-toolkit"
        ).await?;

        self.ssh.exec("nvidia-ctk runtime configure --runtime=containerd").await?;
        self.ssh.exec("systemctl restart k3s || systemctl restart k3s-agent").await?;

        // 获取 GPU 信息写入数据库
        let gpu_info = self.ssh.exec("nvidia-smi --query-gpu=name,count --format=csv,noheader").await?;
        // 解析并更新 cluster_nodes 表

        self.log("info", "✓ GPU 运行时配置完成").await;
        Ok(())
    }

    async fn verify_storage(&self) -> Result<()> {
        let path = &self.config.shared_storage_path;
        let result = self.ssh.exec(
            &format!("mountpoint -q {} && touch {}/.qs_probe && rm {}/.qs_probe",
                     path, path, path)
        ).await?;

        if result.exit_status != 0 {
            self.log("warn", &format!("{} 不是挂载点，请确认共享存储已挂载", path)).await;
        } else {
            self.log("info", &format!("✓ 共享存储 {} 验证通过", path)).await;
        }
        Ok(())
    }

    async fn upload_ssh_public_key(&self) -> Result<()> {
        let pub_key = &self.config.ssh_public_key;
        self.ssh.exec("mkdir -p ~/.ssh && chmod 700 ~/.ssh").await?;
        self.ssh.exec(&format!(
            "echo '{}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys",
            pub_key
        )).await?;
        Ok(())
    }
}
```

### 5.3 SSH 密钥对生成

```rust
use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey};

pub fn generate_ssh_keypair() -> Result<(String, String)> {
    let mut rng = rand::thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 4096)?;
    let public_key = RsaPublicKey::from(&private_key);

    let private_pem = private_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)?.to_string();
    let public_openssh = public_key.to_public_key_openssh()?;  // OpenSSH 格式

    Ok((private_pem, public_openssh))
}
```

---

## 6. 字段加密集成

### 6.1 AES-256-GCM 实现

```rust
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use base64::{Engine, engine::general_purpose::STANDARD as B64};

pub struct CryptoService {
    cipher: Aes256Gcm,
}

impl CryptoService {
    pub fn new(key_b64: &str) -> Result<Self> {
        let key_bytes = B64.decode(key_b64)?;
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self { cipher: Aes256Gcm::new(key) })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| AppError::CryptoError)?;

        // 存储格式：Base64(nonce || ciphertext)
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(B64.encode(&combined))
    }

    pub fn decrypt(&self, encrypted_b64: &str) -> Result<String> {
        let combined = B64.decode(encrypted_b64)?;
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| AppError::CryptoError)?;
        Ok(String::from_utf8(plaintext)?)
    }
}
```

---

## 7. 集成依赖关系与初始化顺序

```
系统启动时的初始化顺序：

1. 加载配置（环境变量 + config 文件）
2. 初始化 CryptoService（从 QS_ENCRYPTION_KEY 加载密钥）
3. 连接 MySQL（platform DB），执行 pending migrations
4. 从 platform_config 表加载运行时配置
5. 初始化 LDAP 连接池
6. 初始化 K8s client（kube-rs in-cluster）
7. 初始化 PingoraClient（从 load_balancers 表加载）
8. 初始化 数据库集群连接池（从 database_clusters 表加载）
9. 启动 LDAP 定时同步任务（tokio::spawn）
10. 启动 K8s Pod 状态 Watch 任务
11. 启动 HTTP 服务（axum）

依赖关系：
  axum handlers
    → services（业务逻辑）
      → db（sqlx，MySQL platform DB）
      → k8s（kube-rs）
      → ldap（ldap3）
      → lb（PingoraClient）
      → cluster_db（sqlx，租户 DB 集群）
      → crypto（AES-256-GCM）
      → ssh（russh，仅节点管理）
```

---

## 8. 集成错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("LDAP 错误: {0}")]
    Ldap(String),

    #[error("Kubernetes 错误: {0}")]
    Kubernetes(#[from] kube::Error),

    #[error("SSH 错误: {0}")]
    Ssh(String),

    #[error("负载均衡 API 错误: {0}")]
    LoadBalancer(String),

    #[error("数据库集群错误: {0}")]
    ClusterError(String),

    #[error("加密错误")]
    CryptoError,

    #[error("配额不足: {0}")]
    QuotaExceeded(String),

    #[error("未授权")]
    Unauthorized,

    #[error("禁止访问")]
    Forbidden,

    #[error("资源不存在: {0}")]
    NotFound(String),

    #[error("资源已存在: {0}")]
    Conflict(String),

    #[error("安装错误: {0}")]
    InstallError(String),
}

// axum IntoResponse 实现，将 AppError 转换为标准 JSON 响应
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::Unauthorized   => (StatusCode::UNAUTHORIZED,  "UNAUTHORIZED"),
            AppError::Forbidden      => (StatusCode::FORBIDDEN,     "FORBIDDEN"),
            AppError::NotFound(_)    => (StatusCode::NOT_FOUND,     "NOT_FOUND"),
            AppError::Conflict(_)    => (StatusCode::CONFLICT,      "CONFLICT"),
            AppError::QuotaExceeded(_) => (StatusCode::UNPROCESSABLE_ENTITY, "QUOTA_EXCEEDED"),
            AppError::ClusterError(_)  => (StatusCode::UNPROCESSABLE_ENTITY, "CLUSTER_ERROR"),
            _                        => (StatusCode::INTERNAL_SERVER_ERROR,  "INTERNAL_ERROR"),
        };

        let body = json!({
            "error": { "code": code, "message": self.to_string() }
        });

        (status, Json(body)).into_response()
    }
}
```
