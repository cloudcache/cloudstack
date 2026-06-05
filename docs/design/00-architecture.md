# QuickStack 架构设计文档

版本：1.0  
日期：2026-05-22  
状态：草稿

---

## 1. 系统概述

QuickStack 是一个面向多租户的自托管 PaaS（Platform as a Service）平台，构建于 K3s（轻量 Kubernetes）之上。平台提供应用全生命周期管理、共享数据库服务、基于 LLDAP 的统一身份认证，以及基于共享存储的持久化能力，目标是让用户无需了解 Kubernetes 即可部署和运维容器化应用。

### 1.1 核心设计原则

| 原则 | 说明 |
|------|------|
| 身份统一 | 所有组件（平台、节点 OS、容器内部）共用同一 LLDAP 身份源 |
| 存储扁平化 | 所有节点挂载同一共享文件系统，消除分布式存储编排复杂性 |
| 安全默认 | 容器默认非特权运行，SecurityContext 自动注入，权限最小化 |
| 零命令行安装 | 安装和节点管理全程通过 Web UI 完成 |
| 角色清晰 | 全局管理员 / 项目管理员 / 操作员 / 观察员 四级权限，职责不重叠 |

---

## 2. 系统组件架构

### 2.1 关键边界原则

> **所有基础设施组件均独立部署，均不进入 K3s 调度管理。**
> QuickStack 后端通过 REST API / SQL / SSH 对它们进行"管理"（推送配置、创建资源），
> 而非"安装"（安装由管理员在对应节点完成，然后在后台注册连接信息）。

| 服务 | 运行形态 | QuickStack 管理方式 | 管理 API 端点 |
|------|----------|---------------------|---------------|
| **QuickStack 后端** | systemd 服务，K3s Master 宿主机 | — | — |
| **pingora-proxy-manager** | 独立进程，LB 节点 | 调用其 REST API 推送/删除路由规则 | `/admin/proxy-managers` |
| **MySQL 平台库** | 独立 MySQL 实例 | 连接参数在 `DATABASE_URL` / config.toml 配置 | `/admin/platform-config` |
| **MySQL Galera 集群** | 独立 Galera 集群 | 通过 SQL 创建租户 DB/User，存入 K8s Secret | `/admin/db-clusters` |
| **PostgreSQL 集群** | 独立 PG 实例 | 通过 SQL 创建租户 DB/User，存入 K8s Secret | `/admin/db-clusters` |
| **LLDAP** | 独立进程，基础设施节点 | 读取用户/组属性，LDAP Bind 验证 | `/admin/platform-config` (ldap_*) |
| **K3s 节点** | K3s 原生 | SSH 自动化安装 K3s agent + nslcd + GPU | `/admin/nodes` |

### 2.2 组件架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         外部访问层                                  │
│   用户浏览器          租户应用用户          CI/CD Webhook           │
└──────────┬───────────────────┬──────────────────────┬──────────────┘
           │                   │                      │
           ▼                   ▼                      │
╔══════════════════════════════════════════════════════════════════╗
║  【独立 LB 节点】  pingora-proxy-manager（systemd，非 K3s Pod）  ║
║                                                                  ║
║  :80/:443  外部流量 + ACME HTTP-01/DNS-01 证书自动签发           ║
║  :81       管理 REST API（JWT 认证，仅内网；QuickStack 调用）     ║
║                                                                  ║
║  cloud.example.com         *.apps.example.com / 自定义域名       ║
║  → Master:3000             → K3s node:{NodePort}（轮询）        ║
║                                                                  ║
║  QuickStack 通过 POST/DELETE /hosts 动态推送路由规则             ║
║  应用暂停时 POST /hosts/{domain}/locations → 返回 503 维护页     ║
╚════════════════════════════════╦═════════════════════════════════╝
                                 ║ NodePort 直连（无 Ingress 层）
              ┌──────────────────╩──────────────────┐
              ▼                                     ▼
┌───────────────────────────┐   ┌──────────────────────────────────┐
│  【K3s Master 节点】       │   │      【K3s Worker 节点群】        │
│                           │   │                                  │
│  ┌─────────────────────┐  │   │  ┌───────┐  ┌───────┐  ┌──────┐ │
│  │  QuickStack 后端     │  │   │  │Node 1 │  │Node 2 │  │Node N│ │
│  │  systemd，非 K8s Pod │  │   │  │       │  │       │  │(GPU) │ │
│  │  Rust/axum :3000    │◄─┼───┤  │ Pods  │  │ Pods  │  │ Pods │ │
│  │  读 k3s.yaml → K8s  │  │   │  └──┬────┘  └──┬────┘  └──┬───┘ │
│  └─────────────────────┘  │   │     └──NodePort─┴──────────┘    │
│                           │   │                                  │
│  ┌─────────────────────┐  │   │  K3s Agent + nslcd + /storage   │
│  │  Docker Registry    │  │   └──────────────────────────────────┘
│  │  K8s Deployment     │  │
│  │  NodePort :30100    │  │
│  └─────────────────────┘  │
│  K3s server（禁用内置代理）│
└───────────────────────────┘

╔═══════════════════════════════════════════════════════════════════╗
║  【独立基础设施节点群】（均非 K3s Pod，由管理员独立安装后注册）   ║
║                                                                   ║
║  ┌──────────────┐  ┌─────────────────┐  ┌──────────────────────┐ ║
║  │    LLDAP     │  │ MySQL（平台库）  │  │  MySQL Galera 集群   │ ║
║  │ :3890 (LDAP) │  │ QuickStack 元数  │  │  租户数据库服务      │ ║
║  │ :17170 (Web) │  │ 据存储           │  │  后端通过 SQL 管理   │ ║
║  └──────────────┘  └─────────────────┘  └──────────────────────┘ ║
║                                                                   ║
║  ┌───────────────────────────────────────────────────────────┐   ║
║  │  PostgreSQL 集群   租户数据库服务，后端通过 SQL 管理       │   ║
║  └───────────────────────────────────────────────────────────┘   ║
║                                                                   ║
║  所有节点挂载共享存储 /storage（NFS 或分布式 FS）                 ║
╚═══════════════════════════════════════════════════════════════════╝
```

---

## 3. 技术栈

### 3.1 后端

| 层次 | 技术 | 版本 | 说明 |
|------|------|------|------|
| HTTP 框架 | axum | 0.7 | 异步 REST + WebSocket |
| 异步运行时 | tokio | 1.x | 全特性 |
| 数据库 ORM | sqlx | 0.7 | MySQL 原生驱动，编译期 SQL 检查 |
| Kubernetes | kube-rs | 0.88 | K8s API + controller-runtime |
| K8s 资源定义 | k8s-openapi | 0.21 | v1.29 API |
| LDAP 客户端 | ldap3 | 0.11 | 异步 LDAP 操作 |
| SSH 客户端 | russh | 0.44 | 节点管理用 SSH |
| JWT | jsonwebtoken | 9 | HS256/RS256 |
| 密码哈希 | argon2 | 0.5 | 替代 bcrypt |
| TOTP | totp-rs | 5 | 2FA |
| 字段加密 | aes-gcm | 0.10 | AES-256-GCM |
| 序列化 | serde + serde_json | 1.x | |
| HTTP 客户端 | reqwest | 0.12 | Pingora API / 外部调用 |
| 日志 | tracing + tracing-subscriber | 0.1 | 结构化日志 |
| 配置 | config | 0.14 | 多源配置合并 |
| RSA 密钥生成 | rsa | 0.9 | SSH 密钥对 |
| UUID | uuid | 1.x | v4 |
| 时间 | chrono | 0.4 | |
| 错误处理 | thiserror + anyhow | — | |

### 3.2 前端

| 层次 | 技术 | 版本 | 说明 |
|------|------|------|------|
| 框架 | React + Next.js | 18 / 14 | App Router |
| UI 组件 | Radix UI + shadcn/ui | — | |
| 样式 | Tailwind CSS | 3.x | |
| 状态管理 | Zustand | 4.x | |
| 表单 | React Hook Form + Zod | — | |
| 实时通信 | WebSocket（原生）| — | 日志 / 终端 / 安装进度 |
| 数据请求 | fetch + SWR | — | |

### 3.3 基础设施

| 组件 | 技术 | 说明 |
|------|------|------|
| 容器编排 | K3s | 轻量 Kubernetes（禁用所有内置代理组件）|
| 反向代理 + SSL | pingora-proxy-manager | 基于 Cloudflare Pingora，REST 管理 API :81，流量入口 :80/:443 |
| 镜像构建 | BuildKit | Dockerfile 构建，K8s Job |
| 镜像仓库 | Docker Registry v2 | K8s Deployment，NodePort :30100 |
| 平台数据库 | MySQL 8.0+ | 元数据存储 |
| 身份认证 | LLDAP | LDAP 目录服务 |
| 租户数据库 | MySQL Galera / PostgreSQL | 共享数据库服务 |

---

## 4. 部署拓扑

### 4.1 最小生产部署（3 节点）

```
┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐
│   基础设施节点    │   │   K3s Master 节点 │   │  K3s Worker 节点 │
│                  │   │                  │   │                  │
│  - LLDAP              │   │  - K3s server         │   │  - K3s agent     │
│  - MySQL (平台)       │   │  - QuickStack(systemd)│   │  - 用户 Pods     │
│  - MySQL Galera       │   │  - Registry (K8s)     │   │  - nslcd         │
│  - PostgreSQL         │   │  - nslcd              │   │  挂载 /storage   │
│  - pingora-proxy-mgr  │   │  挂载 /storage        │   │                  │
│                  │   │  - nslcd         │   │                  │
│  挂载 /storage   │   │  挂载 /storage   │   │                  │
└──────────────────┘   └──────────────────┘   └──────────────────┘
         │                      │                      │
         └──────────────────────┴──────────────────────┘
                           共享存储（NFS/分布式FS）
                                /storage
```

### 4.2 扩展部署

- LB 节点：独立部署，支持双活（Keepalived + VIP）
- Master 节点：支持 3 节点 HA（etcd 集群）
- Worker 节点：无限水平扩展，GPU 节点混合调度
- 基础设施节点：LLDAP/MySQL 可独立高可用部署

---

## 5. 网络架构

### 5.1 流量路径

```
外部 HTTP/HTTPS 请求（pingora-proxy-manager 终结 TLS，ACME 自动证书）
  │
  ▼
pingora-proxy-manager（:80/:443）
  ├── cloud.example.com
  │     └── 反向代理 → Master:3000（QuickStack 后端，systemd 进程）
  │
  ├── {app}.apps.example.com
  │     └── 反向代理 → node1:{nodeport}, node2:{nodeport}, ...（轮询）
  │           └── K8s kube-proxy 将 NodePort 流量转发至目标 Pod
  │
  └── custom-domain.com
        └── 反向代理 → node1:{nodeport}, ...
              （平台通过 POST /hosts REST API 动态注册，证书自动签发）

路由无 Ingress 中间层，pingora 直接按 Host 头匹配路由到对应 NodePort。
应用暂停时，平台通过 POST /hosts/{domain}/locations 将路由指向
QuickStack 自身的 /_qs/maintenance 端点（返回 503 维护页面）。
```

### 5.2 K8s 内部网络

```
命名空间隔离：
  quickstack          平台组件
  registry-and-build  构建和镜像仓库
  {project_name}      每个项目独立命名空间

NetworkPolicy 模式（每个 App 独立配置）：
  ALLOW_ALL        默认，允许所有入站/出站
  NAMESPACE_ONLY   仅允许同命名空间内通信
  DENY_ALL         完全隔离（仅 Ingress 流量）
  INTERNET_ONLY    仅允许出站访问互联网
```

### 5.3 端口规划

| 端口 | 服务 | 说明 |
|------|------|------|
| 80 / 443 | pingora-proxy-manager | 外部流量入口，SSL 终结 |
| 81 | pingora 管理 API | REST API，JWT 认证，仅内网 |
| 3000 | QuickStack 服务 | 平台后端 systemd，pingora 代理 |
| 6443 | K3s API Server | Kubernetes API（仅内网）|
| 3890 | LLDAP | LDAP 协议（仅内网）|
| 17170 | LLDAP Web UI | LLDAP 管理界面（仅内网）|
| 30100 | Docker Registry | K8s NodePort（仅内网）|
| 30000–32767 | 用户应用 NodePort | 每应用一个，pingora 按 Host 路由 |
| 8080 | Installer | 仅安装期间开放，完成后关闭 |

---

## 6. 安全架构

### 6.1 认证链

```
用户 → LLDAP LDAP Bind（验证密码）
     → QuickStack JWT（会话令牌，1天有效期）
     → 可选 TOTP 2FA（第二因子）

节点管理 → SSH RSA-4096 密钥对（平台生成，私钥 AES-256-GCM 加密存于 MySQL）
          → 首次接管用密码，之后仅密钥
```

### 6.2 数据加密

| 数据 | 加密方式 | 存储位置 |
|------|----------|----------|
| 用户密码 | 委托给 LLDAP | LLDAP 目录 |
| 数据库密码（租户 DB）| AES-256-GCM | MySQL platform_config |
| Registry 密码 | AES-256-GCM | MySQL |
| Git Token | AES-256-GCM | MySQL |
| SSH 私钥 | AES-256-GCM | MySQL |
| LDAP Bind 密码 | AES-256-GCM | MySQL |
| K8s Secrets | K8s 原生加密（etcd at-rest）| etcd |
| JWT 签名密钥 | 启动时生成，内存 + MySQL | MySQL |

### 6.3 容器安全默认值

```
所有用户 Pod 默认：
  runAsNonRoot: true
  allowPrivilegeEscalation: false
  seccompProfile: RuntimeDefault
  readOnlyRootFilesystem: false（可按应用配置）
  runAsUser: {ldap_uid}（来自 LLDAP）
  runAsGroup: {ldap_gid}
```

### 6.4 RBAC 层次

```
GlobalAdmin（全局管理员）
  └── 全部平台资源
  └── 集群节点管理
  └── 数据库集群管理
  └── 用户配额管理
  └── 平台配置管理

ProjectAdmin（项目管理员）
  └── 项目下全部资源
  └── 成员管理

ProjectOperator（项目操作员）
  └── 应用 CRUD + 部署
  └── 数据库实例 CRUD
  └── 日志 / 终端查看

ProjectObserver（项目观察员）
  └── 所有资源只读
  └── 日志查看（无终端）
  └── 密码字段隐藏
```

---

## 7. 安装程序架构

```
quickstack-installer（独立二进制，安装完成后废弃）
  │
  ├── 嵌入 Web UI 静态文件（include_dir!）
  ├── 临时 SQLite（记录安装进度，防中断后重试）
  ├── HTTP API（安装向导各步骤）
  └── WebSocket（节点安装实时日志推送）

安装流程：
  Step 1  环境预检
  Step 2  平台基础配置（域名、时区等）
  Step 3  MySQL 配置 + 初始化 Schema
  Step 4  LLDAP 配置 + 连接验证
  Step 5  pingora-proxy-manager 配置（管理 API 地址、JWT 凭据）
  Step 6  共享存储配置
  Step 7  节点接管（SSH → K3s + nslcd + GPU）
  Step 8  Master 组件部署（Registry K8s Deployment + QuickStack systemd 服务安装）
            + pingora 注册平台管理域名路由
  Step 9  初始管理员验证
  Step 10 完成，installer 退出，QuickStack systemd 服务接管
```

### 7.1 QuickStack 后端部署方式

后端以 systemd 服务运行在 Master 节点宿主机，**不进入 K3s 调度**，直接通过本地 kubeconfig 访问 K3s API：

```
安装路径：/opt/quickstack/quickstack
配置文件：/opt/quickstack/config.toml
K3s 访问：读取 /etc/rancher/k3s/k3s.yaml
systemd：/etc/systemd/system/quickstack.service
日志：journalctl -u quickstack -f
```

```ini
# /etc/systemd/system/quickstack.service
[Unit]
Description=QuickStack PaaS Backend
After=network.target k3s.service

[Service]
Type=simple
User=quickstack
ExecStart=/opt/quickstack/quickstack --config /opt/quickstack/config.toml
Restart=always
RestartSec=5
Environment=KUBECONFIG=/etc/rancher/k3s/k3s.yaml
Environment=QS_ENCRYPTION_KEY=<base64-32-bytes>

[Install]
WantedBy=multi-user.target
```

---

## 8. 模块边界与职责

| 模块 | 职责 | 不负责 |
|------|------|--------|
| `api/` | HTTP 请求解析、响应序列化、认证中间件 | 业务逻辑 |
| `k8s/` | K8s 资源 CRUD，Pod Spec 生成，NodePort 分配 | 业务规则 |
| `proxy/` | 调用 pingora REST API 推送/删除路由规则 | pingora 本身的安装和运维 |
| `auth/` | LDAP bind、JWT 签发验证、TOTP | 用户 UI 逻辑 |
| `ssh/` | SSH 连接、K3s/nslcd/GPU 自动化安装 | DB/LB 节点安装 |
| `db/` | MySQL 连接池、sqlx migrate | 业务规则 |
| `crypto/` | AES-256-GCM 加解密、SHA-256 | — |

---

## 9. 独立服务的管理员操作流程

独立部署的服务不由 QuickStack 安装，但由管理员通过 QuickStack Web 后台进行注册和管理。

### 9.1 pingora-proxy-manager（LB）

```
管理员操作：
  1. 在 LB 节点手动安装 pingora-proxy-manager（二进制 + systemd 或 docker）
  2. 设置 pingora 管理账号密码
  3. 在 QuickStack 后台 → 管理 → 负载均衡 → 添加实例
       POST /api/v1/admin/proxy-managers
       { "name": "primary-lb", "host": "10.0.0.5",
         "api_base_url": "http://10.0.0.5:81/api",
         "api_username": "admin", "api_password": "..." }
  4. QuickStack 验证连接后保存，后续所有域名路由由后端自动推送

QuickStack 对 pingora 的管理内容：
  - 应用部署时：自动 POST /hosts（注册上游）
  - 应用删除时：自动 DELETE /hosts
  - 应用暂停时：POST /hosts/{domain}/locations（重定向到维护页）
  - 应用恢复时：DELETE /hosts/{domain}/locations
  - SSL 证书：POST /certs（ACME 自动签发）
```

### 9.2 数据库集群（MySQL Galera / PostgreSQL）

```
管理员操作：
  1. 在 DB 节点手动安装 MySQL Galera 或 PostgreSQL
  2. 创建管理员账号
  3. 在 QuickStack 后台 → 管理 → 数据库集群 → 添加集群
       POST /api/v1/admin/db-clusters
       { "name": "galera-01", "cluster_type": "MYSQL_GALERA",
         "host": "10.0.0.10", "port": 3306,
         "admin_user": "root", "admin_password": "...",
         "max_databases": 100 }
  4. QuickStack 验证连接后保存

QuickStack 对 DB 集群的管理内容：
  - 租户创建 DB 实例时：CREATE DATABASE / CREATE USER / GRANT
  - 租户删除 DB 实例时：DROP DATABASE / DROP USER
  - 同时在项目 K8s Namespace 创建 Secret（DB_HOST/PORT/NAME/USER/PASS/URL）
```

### 9.3 LLDAP

```
管理员操作：
  1. 在基础设施节点手动安装 LLDAP
  2. 在 QuickStack 后台 → 管理 → 平台配置 → 设置 LDAP 参数
       POST /api/v1/admin/platform-config
       { "key": "ldap_url",  "value": "ldap://10.0.0.3:3890" }
       { "key": "ldap_bind_dn", "value": "cn=admin,dc=example,dc=com" }
       { "key": "ldap_bind_password", "value": "..." }  // 自动加密存储
       ...

QuickStack 对 LLDAP 的使用：
  - 用户登录时 LDAP Bind 验证密码
  - 同步 uidNumber/gidNumber 作为 Pod SecurityContext
  - 定期同步用户状态（激活/停用、管理员组成员）
```

### 9.4 K3s 节点（唯一由 QuickStack 安装的基础设施）

```
管理员操作：
  1. 准备一台干净的 Ubuntu 22.04 节点，开放 SSH
  2. 在 QuickStack 后台 → 管理 → 节点 → 添加节点
       POST /api/v1/admin/nodes
       { "hostname": "worker-01", "ip_address": "10.0.0.20",
         "node_role": "WORKER", "ssh_password": "..." }
  3. QuickStack 通过 SSH 自动完成：
       - 安装平台 SSH 公钥（后续无密码登录）
       - 安装 nslcd / libnss-ldap（LDAP 用户透传）
       - 创建 /storage 挂载点
       - 检测 NVIDIA GPU，安装 nvidia-container-toolkit
       - 安装 K3s agent，加入集群
```

---

## 10. 可观测性

| 方面 | 实现 |
|------|------|
| 结构化日志 | tracing + JSON 格式输出 |
| 审计日志 | deployment_events 表记录所有操作 |
| Pod 日志 | K8s Log API → SSE 实时流 |
| 构建日志 | BuildKit Job 日志 → SSE 实时流 |
| 节点指标 | K8s Metrics API（CPU / 内存 / 存储） |
| 健康检查 | GET /health（平台自身） |
| Pod 状态 | K8s Watch → WebSocket 推送前端 |
