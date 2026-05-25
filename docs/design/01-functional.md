# QuickStack 功能设计文档

版本：1.0  
日期：2026-05-22  
状态：草稿

---

## 1. 用户与身份管理

### 1.1 用户来源

平台不维护独立密码，所有用户由 LLDAP 统一管理。用户首次登录平台后，在 `users` 表自动创建本地记录（Provisioning），同步 LDAP 属性：

| LDAP 属性 | 平台字段 | 说明 |
|-----------|----------|------|
| `uid` | `username` | 唯一标识 |
| `mail` | `email` | 邮箱 |
| `cn` | `display_name` | 显示名 |
| `uidNumber` | `ldap_uid` | POSIX UID，用于容器 securityContext |
| `gidNumber` | `ldap_gid` | POSIX GID |
| DN | `ldap_dn` | 完整 DN |

### 1.2 全局管理员

LLDAP 中属于 `lldap_admin` 组的用户，登录时自动设置 `is_global_admin = true`。全局管理员可：
- 管理所有项目和用户
- 配置数据库集群（Galera / PostgreSQL）
- 配置平台参数（存储路径、域名、LB 等）
- 管理 K3s 节点
- 为用户分配订阅计划、调整 Project 配额

### 1.3 配额体系

配额来自**订阅计划**，分配到 **Project**，运行时检查所属 Project 配额。`0` 表示不限制。  
详细设计见 [05-user-subscription-quota.md](./05-user-subscription-quota.md)。

```
流转路径：
  subscription_plans.quota_* → 激活订阅 → 写入默认 Project.quota_*
                                          → 管理员可按需调整各 Project 配额

创建应用前检查（Project 维度）：
  项目已用CPU + 新应用CPU ≤ Project.quota_cpu_mcores（0 = 不限）
```

| 配额维度 | 字段 | 单位 |
|----------|------|------|
| CPU | quota_cpu_mcores | 毫核（1000 = 1 核） |
| 内存 | quota_mem_mb | MB |
| 存储 | quota_storage_gb | GB |
| 应用数 | quota_apps | 个 |
| 数据库实例数 | quota_db_instances | 个 |
| 月出流量 | quota_bandwidth_gb | GB |
| 域名数 | quota_domain_count | 个 |
| 月请求数 | quota_request_million | 百万次 |

### 1.4 双因素认证（2FA）

- 基于 TOTP（RFC 6238）
- 用户在个人设置页启用，扫描二维码绑定 Authenticator App
- 启用后登录流程：LDAP 验证 → TOTP 验证 → 签发 JWT
- Secret 加密存储在平台 MySQL

---

## 2. 安装程序

### 2.1 运行方式

```bash
# 下载并在任意 Linux 机器（首台节点）运行
curl -fsSL https://get.quickstack.io | sh
# 或直接运行二进制
./quickstack-installer --port 8080
```

浏览器访问 `http://{machine_ip}:8080` 进入 Web 安装向导。

### 2.2 安装向导步骤

#### Step 1：环境预检

后端在当前机器执行一系列检测，实时返回结果：

| 检测项 | 通过条件 | 阻塞安装 |
|--------|----------|---------|
| 操作系统 | Linux x86_64 / arm64 | 是 |
| 运行用户 | root 或具备 sudo | 是 |
| 端口 6443 | 未占用 | 是 |
| 端口 80/443 | 未占用（LB 节点） | 警告 |
| curl/wget | 可执行 | 是 |
| 外网连通 | 能访问 get.k3s.io | 警告 |
| /storage 路径 | 存在且可写 | 警告（可稍后配置）|

#### Step 2：平台基础配置

| 字段 | 类型 | 说明 |
|------|------|------|
| 平台管理域名 | string | 如 `cloud.example.com` |
| 应用二级域名后缀 | string | 如 `apps.example.com` |
| 默认时区 | select | 默认 `Asia/Shanghai` |
| 平台显示名称 | string | 默认 `QuickStack` |

配置后展示 DNS 配置提示（需在 DNS 服务商配置泛解析）。

#### Step 3：MySQL 数据库配置

输入连接信息 → `[测试连接]` → 执行 Schema 初始化 DDL。

#### Step 4：LLDAP 配置

输入 LDAP 连接信息 → `[测试连接]` → 展示发现的用户数量和角色映射约定。

#### Step 5：pingora-proxy-manager 配置

填写已独立部署的 pingora-proxy-manager 实例信息（安装向导不负责安装 pingora 本身）：

| 字段 | 示例值 |
|------|--------|
| 节点 IP / 主机名 | `lb.internal` |
| 管理 API 地址 | `http://lb.internal:81/api` |
| 管理员用户名 | `admin` |
| 管理员密码 | `••••••••` |

按钮：`[测试连接]` → `POST /api/login`，成功后展示 pingora 连接确认。

#### Step 6：共享存储配置

填写 `/storage` 路径 → 检测挂载状态、可用空间、读写权限。

#### Step 7：节点接管

逐台添加节点（每次填写 IP + SSH 用户 + SSH 密码），后端执行：

```
1. SSH 密码连接
2. 上传平台 SSH 公钥 → authorized_keys（后续仅密钥认证）
3. 执行环境检查（OS、GPU、存储挂载、LDAP 状态）
4. 安装 K3s（Master 首台 / Worker 后续台）
5. GPU 节点：安装 nvidia-container-toolkit，配置 runtime class
6. 安装配置 nslcd（LDAP 用户认证），写入 nsswitch.conf / pam.d
7. 验证存储挂载
8. 节点打标签（qs/managed=true, gpu=true 等）
```

全程通过 WebSocket 实时推送安装日志到前端。

#### Step 8：Master 组件部署

自动依次执行：
1. 部署内部 Docker Registry（K8s Deployment，NodePort :30100）
2. 安装 QuickStack 二进制到 `/opt/quickstack/`，写入 systemd Unit 并启动
3. 通过 pingora API 注册平台管理域名路由（`POST /api/hosts`）并申请证书

#### Step 9：初始管理员验证

输入 LLDAP admin 用户名密码，执行一次完整登录验证后完成初始化。

#### Step 10：完成

展示平台访问地址，installer 退出，正式服务接管。

---

## 3. 项目管理

### 3.1 项目模型

项目对应 K8s 的一个 Namespace，是资源隔离的最小单元。

- 每个项目有唯一名称（同时作为 K8s namespace 名）
- 项目有一个 Owner（创建者），Owner 自动获得 ADMIN 角色
- 全局管理员可对任意项目执行全部操作

### 3.2 成员与角色

| 角色 | 添加/移除成员 | 删除项目 | 创建/编辑应用 | 部署/重启 | 暂停/恢复 | 删除应用 | 创建 DB 实例 | 删除 DB 实例 | 查看日志 | 终端 | 查看配置 |
|------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| GlobalAdmin | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓（含密码）|
| ADMIN | ✓ | ✓(owner) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓（含密码）|
| OPERATOR | ✗ | ✗ | ✓ | ✓ | ✗ | ✗ | ✓ | ✗ | ✓ | ✓ | ✓（含密码）|
| OBSERVER | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✓（密码隐藏）|

### 3.3 项目配额

全局管理员可为项目单独设置配额，用于控制整个项目的资源消耗上限，与用户配额取严格值。

---

## 4. 应用管理

### 4.1 应用类型

平台只管理**普通容器化应用**（`App`）。数据库服务通过独立的「数据库实例」模型管理，不再与 App 混用。

### 4.2 应用来源

| 来源类型 | 说明 |
|----------|------|
| `CONTAINER` | 指定已有镜像（支持私有仓库，填写用户名/密码） |
| `GIT` | 提供 Git 仓库 URL + 分支/Tag，平台自动 clone 并用 BuildKit 构建 |

### 4.3 应用生命周期

```
STOPPED
  │ deploy（手动/Webhook）
  ▼
DEPLOYING ──失败──► FAILED
  │成功
  ▼
RUNNING
  │ pause（ADMIN）
  ▼
PAUSED ──── resume（ADMIN）──► DEPLOYING
  │
  │ 暂停期间：replicas=0，pingora 路由重写到维护页
  │ 所有者/ADMIN/OBSERVER 可查看配置、日志（历史）
```

### 4.4 应用配置

#### 资源配置

| 字段 | 说明 | 单位 |
|------|------|------|
| CPU 预留 | Kubernetes requests.cpu | 毫核 |
| CPU 上限 | Kubernetes limits.cpu | 毫核 |
| 内存预留 | Kubernetes requests.memory | MB |
| 内存上限 | Kubernetes limits.memory | MB |
| 副本数 | spec.replicas | 个 |

#### 调度配置

| 选项 | 默认 | 说明 |
|------|------|------|
| 多副本分散调度 | 开启 | podAntiAffinity，副本分布在不同节点 |
| GPU 支持 | 关闭 | 开启后调度到 GPU 节点，设置 gpu_count |
| GPU 型号 | 不限 | 通过 nodeSelector 指定 |

#### 健康检查

支持 HTTP GET 和 TCP 两种方式，可配置检查路径、端口、周期、超时、失败阈值。

#### 网络策略

| 策略 | 说明 |
|------|------|
| ALLOW_ALL | 不限制（默认）|
| NAMESPACE_ONLY | 仅同项目 Pod 互访 |
| DENY_ALL | 完全隔离 Pod 间通信（NodePort 入站流量不受影响）|
| INTERNET_ONLY | 仅允许出站访问外网 |

### 4.5 应用暂停（内部审计状态）

暂停由 ADMIN 触发，填写可选原因。暂停时：

1. K8s Deployment `spec.replicas` 设为 0（保留 Deployment 对象，配置不丢失）
2. pingora `POST /api/hosts/{domain}/locations` 将路由重写到 QuickStack 的 `/_qs/maintenance`（返回 503 维护页）
3. 外部访问该域名得到维护页面，内部 ADMIN / OPERATOR / OBSERVER 仍可通过平台查看配置和历史日志
4. 恢复后：replicas 还原，pingora `DELETE /api/hosts/{domain}/locations` 移除重写规则，流量恢复到 NodePort

### 4.6 Webhook 触发部署

每个应用可生成唯一 `webhook_id`，CI/CD 系统通过 `POST /webhooks/{webhook_id}` 触发部署（不需要认证）。

---

## 5. 域名与 SSL

### 5.1 系统自动分配域名

创建应用时，平台自动分配一个二级域名：

```
{app_name}-{project_id_short}.{app_subdomain_base}
如：myapp-abc12.apps.example.com
```

此域名不可删除，只要应用存在就一直有效。

### 5.2 用户自定义域名

用户可为应用绑定任意自定义域名，平台：
1. 调用 pingora `POST /api/hosts` 注册路由（hostname → 应用 NodePort）
2. 调用 pingora `POST /api/certs` 触发 ACME HTTP-01 证书申请
3. 展示 CNAME/A 记录配置提示（需用户先将域名 DNS 解析到 pingora 节点 IP）

### 5.3 SSL 证书

SSL 证书全部由 pingora-proxy-manager 内建 ACME 客户端管理。

| 域名类型 | 证书方案 |
|----------|----------|
| 平台管理域名 | 单域证书（HTTP-01，安装时通过 `POST /api/certs` 申请）|
| 系统分配应用域名 `{app}.apps.example.com` | 单域证书（HTTP-01，创建应用时自动申请）|
| 用户自定义域名 | 单域证书（HTTP-01，绑定域名时触发，需用户已将域名 DNS 解析到 LB）|
| 泛域名 `*.apps.example.com`（可选）| DNS-01，需在 pingora 管理界面配置 DNS provider，由管理员手动操作 |

pingora 自动处理证书续签，平台通过定期轮询 `GET /api/certs` 同步 `app_domains.cert_status`。

---

## 6. 存储

### 6.1 共享存储目录结构

```
/storage/                              ← 全局管理员配置挂载点
  {username}/                          ← 用户根目录（chown 由 initContainer 设置）
    home/                              ← 容器内 /home/{username}
    {appName}/
      data/                            ← 容器内 /data
      logs/                            ← 容器内 /logs
    {appName2}/
      ...
```

### 6.2 目录初始化

每次部署时，Pod 的 `initContainer`（以 root 身份运行）负责：
1. 创建用户目录、应用目录（`mkdir -p`）
2. `chown -R {ldap_uid}:{ldap_gid}` 设置目录所有者
3. initContainer 退出后，主容器以 LDAP 用户身份运行

### 6.3 额外挂载

用户可在应用配置中添加额外的 hostPath 挂载，路径必须以 `/storage/{username}/` 为前缀（平台强制校验，防止越权访问其他用户目录）。

---

## 7. Pod 标准容器规范

每个用户应用 Pod 由平台自动注入以下标准配置，用户无需手动配置。

### 7.1 标准环境变量

| 变量 | 值 | 说明 |
|------|----|------|
| `TZ` | `Asia/Shanghai`（可按应用配置）| 时区 |
| `HOME` | `/home/{username}` | 用户 Home |
| `USER` | `{username}` | 用户名 |

### 7.2 标准 Volume 挂载

| Volume | 宿主机路径 | 容器内路径 | 只读 |
|--------|-----------|-----------|------|
| etc-hosts | `/etc/hosts` | `/etc/hosts` | 是 |
| etc-passwd | `/etc/passwd` | `/etc/passwd` | 是 |
| etc-group | `/etc/group` | `/etc/group` | 是 |
| etc-shadow | `/etc/shadow` | `/etc/shadow` | 是 |
| etc-gshadow | `/etc/gshadow` | `/etc/gshadow` | 是 |
| etc-pam-d | `/etc/pam.d` | `/etc/pam.d` | 是 |
| nslcd-conf | `/etc/nslcd.conf` | `/etc/nslcd.conf` | 是 |
| nsswitch-conf | `/etc/nsswitch.conf` | `/etc/nsswitch.conf` | 是 |
| nslcd-socket | `/var/run/nslcd` | `/var/run/nslcd` | **否**（socket 需可写）|
| etc-sudoers | `/etc/sudoers` | `/etc/sudoers` | 是 |
| user-home | `/storage/{username}/home` | `/home/{username}` | 否 |
| app-data | `/storage/{username}/{app}/data` | `/data` | 否 |
| app-logs | `/storage/{username}/{app}/logs` | `/logs` | 否 |

### 7.3 SecurityContext

```yaml
runAsUser:                {ldap_uid}
runAsGroup:               {ldap_gid}
fsGroup:                  {ldap_gid}
runAsNonRoot:             true
allowPrivilegeEscalation: false
seccompProfile:
  type: RuntimeDefault
```

### 7.4 GPU 支持

当应用 `gpu_enabled = true` 时额外注入：

```yaml
runtimeClassName: nvidia
resources:
  limits:
    nvidia.com/gpu: {gpu_count}
nodeSelector:
  gpu: "true"
  # 若指定型号：
  accelerator: {gpu_model}
```

### 7.5 多副本分散调度

当 `replicas > 1` 且 `anti_affinity_enabled = true` 时注入：

```yaml
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchLabels:
              qs-app: "{app_id}"
          topologyKey: kubernetes.io/hostname
```

---

## 8. 共享数据库服务

### 8.1 数据库集群管理

全局管理员在「数据库集群」页面维护可用的数据库集群：

| 字段 | 说明 |
|------|------|
| 类型 | `MYSQL_GALERA` / `POSTGRESQL` |
| 主机名/VIP | 集群访问端点 |
| 端口 | 默认 3306 / 5432 |
| 管理员凭据 | 用于创建/删除数据库和用户 |
| 最大数据库数 | 0 = 不限 |

### 8.2 租户数据库实例生命周期

#### 创建（OPERATOR 或 ADMIN）

```
1. 检查项目 db_instances 配额
2. 生成数据库名：p_{project_name}_{user_db_name}
3. 生成专属用户名：u_{username}_{random6}
4. 生成随机密码（32 位）
5. 在对应集群执行 CREATE DATABASE + CREATE USER + GRANT
6. 加密存储密码到 database_instances 表
7. 在同项目 K8s namespace 创建 Secret（DB_HOST/PORT/NAME/USER/PASS/URL）
8. 返回实例信息
```

MySQL GRANT 权限范围：
```sql
SELECT, INSERT, UPDATE, DELETE, CREATE, DROP, INDEX, ALTER,
CREATE TEMPORARY TABLES, LOCK TABLES, CREATE VIEW, SHOW VIEW,
CREATE ROUTINE, ALTER ROUTINE, EXECUTE, REFERENCES, TRIGGER
```
（无 SUPER、PROCESS、REPLICATION 等高权限）

PostgreSQL GRANT：
```sql
GRANT ALL PRIVILEGES ON DATABASE ... TO ...;
-- 不授予 pg_write_all_data, pg_read_all_data 等超级权限
```

#### 删除（ADMIN）

```
1. DROP USER（MySQL）/ DROP ROLE（PG）
2. DROP DATABASE
3. 删除 K8s Secret
4. database_instances.status = 'DROPPED'（保留审计记录）
```

### 8.3 密码查看权限

| 角色 | 密码可见性 |
|------|-----------|
| ADMIN / OPERATOR | 明文显示（点击「显示密码」）|
| OBSERVER | 隐藏，显示为 `••••••••` |

---

## 9. 构建系统

### 9.1 构建流程（GIT 来源应用）

```
1. 用 simple-git clone 仓库（或 K8s Job 内 clone）
2. 创建 BuildKit Job（K8s Job，镜像 moby/buildkit）
3. BuildKit 读取 Dockerfile，构建镜像
4. 推送到内部 Registry（registry-and-build namespace）
5. 触发 Deployment 滚动更新
```

### 9.2 构建日志

构建 Job 的标准输出通过 K8s Log API 获取，平台通过 SSE（Server-Sent Events）实时推送给前端。

---

## 10. 节点管理

### 10.1 节点列表

展示所有 K3s 节点的：
- 状态（Ready / NotReady）
- 角色（Master / Worker）
- CPU / 内存容量和使用量
- GPU 信息
- 存储挂载状态
- LDAP 认证状态（nslcd 是否运行）

### 10.2 添加节点

安装完成后，在平台「节点管理」页面可继续添加 Worker 节点，流程与安装向导 Step 7 完全相同（复用同一套 SSH 接管代码）。

### 10.3 节点维护

| 操作 | 说明 |
|------|------|
| 停止调度（Cordon）| kubectl cordon，不影响已运行 Pod |
| 恢复调度（Uncordon）| kubectl uncordon |
| 排空（Drain）| kubectl drain，将 Pod 迁移到其他节点 |
| 移除节点 | kubectl delete node + SSH 卸载 K3s agent |

---

## 11. 备份

### 11.1 S3 备份目标

全局管理员配置 S3 兼容存储（MinIO、阿里云 OSS、AWS S3 等）作为备份目标。

### 11.2 备份计划

每个应用可创建备份计划：
- Cron 表达式（如 `0 2 * * *` 每天凌晨 2 点）
- 保留天数
- 备份类型：文件备份（/data 目录打包）/ 数据库备份（dump）/ 两者

### 11.3 数据库备份

备份计划关联数据库实例时，平台自动：
- MySQL：执行 `mysqldump` 导出
- PostgreSQL：执行 `pg_dump` 导出
- 压缩后上传至 S3
