# QuickStack 接口文档

版本：1.0  
日期：2026-05-22  
状态：草稿

---

## 概述

### Base URL

```
https://cloud.example.com/api/v1
```

### 认证

除登录接口外，所有接口需在请求头携带 JWT：

```
Authorization: Bearer {token}
```

### 通用响应格式

**成功：**
```json
{
  "data": { ... }
}
```

**分页列表：**
```json
{
  "data": [ ... ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 100
  }
}
```

**错误：**
```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "应用不存在"
  }
}
```

### 错误码

| HTTP 状态码 | code | 说明 |
|------------|------|------|
| 400 | `VALIDATION_ERROR` | 请求参数校验失败 |
| 401 | `UNAUTHORIZED` | 未登录或 Token 过期 |
| 403 | `FORBIDDEN` | 无权限执行此操作 |
| 404 | `NOT_FOUND` | 资源不存在 |
| 409 | `CONFLICT` | 资源已存在（如名称冲突）|
| 422 | `QUOTA_EXCEEDED` | 配额不足 |
| 422 | `CLUSTER_ERROR` | 数据库集群操作失败 |
| 500 | `INTERNAL_ERROR` | 服务器内部错误 |

### 权限标注说明

- `public` — 无需认证
- `auth` — 需要登录
- `admin` — 需要全局管理员
- `project:ADMIN` — 项目 ADMIN 角色
- `project:OPERATOR` — 项目 OPERATOR 或以上
- `project:OBSERVER` — 项目任意角色（含 OBSERVER）

---

## 1. 认证接口

### 1.1 登录

```
POST /auth/login
权限：public
```

**请求：**
```json
{
  "username": "alice",
  "password": "secret",
  "totp_code": "123456"   // 可选，仅 2FA 启用时必填
}
```

**响应 200：**
```json
{
  "data": {
    "token": "eyJ...",
    "expires_at": "2026-05-23T12:00:00Z",
    "user": {
      "id": "uuid",
      "username": "alice",
      "display_name": "Alice",
      "email": "alice@example.com",
      "is_global_admin": false
    }
  }
}
```

**响应 401（需要 2FA）：**
```json
{
  "error": {
    "code": "TOTP_REQUIRED",
    "message": "需要两步验证码"
  }
}
```

---

### 1.2 登出

```
POST /auth/logout
权限：auth
```

**响应 204**（无内容）

---

### 1.3 获取当前用户信息

```
GET /auth/me
权限：auth
```

**响应 200：**
```json
{
  "data": {
    "id": "uuid",
    "username": "alice",
    "display_name": "Alice",
    "email": "alice@example.com",
    "ldap_uid": 10001,
    "ldap_gid": 10001,
    "is_global_admin": false,
    "quotas": {
      "cpu_mcores": 4000,
      "mem_mb": 8192,
      "storage_gb": 100,
      "apps": 10,
      "db_instances": 5
    },
    "totp_enabled": true
  }
}
```

---

### 1.4 TOTP 管理

#### 生成 TOTP 二维码

```
POST /auth/totp/setup
权限：auth
```

**响应 200：**
```json
{
  "data": {
    "qr_code_url": "data:image/png;base64,...",
    "secret": "JBSWY3DPEHPK3PXP"
  }
}
```

#### 确认并启用 TOTP

```
POST /auth/totp/enable
权限：auth
```

**请求：**
```json
{ "totp_code": "123456" }
```

**响应 200：**
```json
{ "data": { "enabled": true } }
```

#### 禁用 TOTP

```
POST /auth/totp/disable
权限：auth
```

**请求：**
```json
{ "totp_code": "123456" }
```

**响应 200：**
```json
{ "data": { "enabled": false } }
```

---

## 2. 用户管理接口（全局管理员）

### 2.1 用户列表

```
GET /admin/users?page=1&per_page=20&search=alice
权限：admin
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "username": "alice",
      "display_name": "Alice",
      "email": "alice@example.com",
      "is_global_admin": false,
      "is_active": true,
      "ldap_uid": 10001,
      "created_at": "2026-01-01T00:00:00Z"
    }
  ],
  "pagination": { "page": 1, "per_page": 20, "total": 50 }
}
```

---

### 2.2 更新用户配额

```
PUT /admin/users/{user_id}/quota
权限：admin
```

**请求：**
```json
{
  "quota_cpu_mcores": 4000,
  "quota_mem_mb": 8192,
  "quota_storage_gb": 100,
  "quota_apps": 10,
  "quota_db_instances": 5
}
```

**响应 200：**
```json
{ "data": { "updated": true } }
```

---

### 2.3 启用/禁用用户

```
PUT /admin/users/{user_id}/status
权限：admin
```

**请求：**
```json
{ "is_active": false }
```

---

## 3. 项目接口

### 3.1 项目列表

```
GET /projects?page=1&per_page=20
权限：auth
```

返回当前用户有权限的项目列表（全局管理员返回所有项目）。

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "name": "my-project",
      "display_name": "我的项目",
      "owner_id": "uuid",
      "my_role": "ADMIN",
      "app_count": 3,
      "db_instance_count": 2,
      "quotas": {
        "cpu_mcores": 0,
        "mem_mb": 0,
        "storage_gb": 0,
        "apps": 0,
        "db_instances": 0
      },
      "created_at": "2026-01-01T00:00:00Z"
    }
  ],
  "pagination": { ... }
}
```

---

### 3.2 创建项目

```
POST /projects
权限：auth
```

**请求：**
```json
{
  "name": "my-project",
  "display_name": "我的项目"
}
```

`name` 规则：小写字母、数字、连字符，3-63 位，不以连字符开头或结尾。

**响应 201：**
```json
{ "data": { "id": "uuid", "name": "my-project", ... } }
```

---

### 3.3 获取项目详情

```
GET /projects/{project_id}
权限：project:OBSERVER
```

**响应 200：** 项目详情 + 成员列表 + 资源使用量快照

---

### 3.4 更新项目

```
PUT /projects/{project_id}
权限：project:ADMIN
```

**请求：**
```json
{ "display_name": "新名称" }
```

---

### 3.5 删除项目

```
DELETE /projects/{project_id}
权限：project:ADMIN（owner 或 GlobalAdmin）
```

级联删除：K8s namespace、所有应用、所有 DB 实例、所有部署记录。

**响应 204**

---

### 3.6 更新项目配额

```
PUT /projects/{project_id}/quota
权限：admin
```

**请求：**
```json
{
  "quota_cpu_mcores": 16000,
  "quota_mem_mb": 32768,
  "quota_storage_gb": 500,
  "quota_apps": 20,
  "quota_db_instances": 10
}
```

---

### 3.7 成员管理

#### 获取成员列表

```
GET /projects/{project_id}/members
权限：project:OBSERVER
```

**响应 200：**
```json
{
  "data": [
    {
      "user_id": "uuid",
      "username": "alice",
      "display_name": "Alice",
      "role": "ADMIN",
      "added_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

#### 添加成员

```
POST /projects/{project_id}/members
权限：project:ADMIN
```

**请求：**
```json
{
  "user_id": "uuid",
  "role": "OPERATOR"
}
```

#### 更新成员角色

```
PUT /projects/{project_id}/members/{user_id}
权限：project:ADMIN
```

**请求：**
```json
{ "role": "OBSERVER" }
```

#### 移除成员

```
DELETE /projects/{project_id}/members/{user_id}
权限：project:ADMIN
```

**响应 204**

---

## 4. 应用接口

### 4.1 应用列表

```
GET /projects/{project_id}/apps?page=1&per_page=20
权限：project:OBSERVER
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "name": "my-app",
      "display_name": "我的应用",
      "source_type": "GIT",
      "status": "RUNNING",
      "replicas": 2,
      "ready_replicas": 2,
      "image": "registry.internal/ns/my-app:abc123",
      "domains": [
        { "hostname": "my-app-xyz.apps.example.com", "is_system_domain": true }
      ],
      "created_at": "2026-01-01T00:00:00Z",
      "updated_at": "2026-01-01T00:00:00Z"
    }
  ]
}
```

---

### 4.2 创建应用

```
POST /projects/{project_id}/apps
权限：project:OPERATOR
```

**请求（CONTAINER 来源）：**
```json
{
  "name": "my-app",
  "display_name": "我的应用",
  "source_type": "CONTAINER",
  "container_image": "nginx:latest",
  "container_registry_user": "user",
  "container_registry_pass": "pass",
  "replicas": 1,
  "cpu_reservation_mcores": 100,
  "cpu_limit_mcores": 500,
  "mem_reservation_mb": 128,
  "mem_limit_mb": 512,
  "timezone": "Asia/Shanghai",
  "network_policy": "ALLOW_ALL",
  "anti_affinity_enabled": true,
  "gpu_enabled": false,
  "ports": [
    { "container_port": 80, "protocol": "TCP" }
  ],
  "env_vars": [
    { "key_name": "ENV", "value": "production", "is_secret": false },
    { "key_name": "DB_PASS", "value": "secret123", "is_secret": true }
  ]
}
```

**请求（GIT 来源）：**
```json
{
  "name": "my-app",
  "source_type": "GIT",
  "git_url": "https://github.com/org/repo",
  "git_branch": "main",
  "git_token": "ghp_xxx",
  "dockerfile_path": "Dockerfile",
  "build_args": [
    { "key": "NODE_ENV", "value": "production" }
  ],
  ...
}
```

**响应 201：** 应用详情

---

### 4.3 获取应用详情

```
GET /projects/{project_id}/apps/{app_id}
权限：project:OBSERVER
```

**响应 200：** 完整应用配置（OBSERVER 角色下 is_secret=true 的 env_var.value 返回空字符串）

---

### 4.4 更新应用

```
PUT /projects/{project_id}/apps/{app_id}
权限：project:OPERATOR
```

**请求：** 同创建，字段可部分更新（PATCH 语义）

---

### 4.5 删除应用

```
DELETE /projects/{project_id}/apps/{app_id}
权限：project:ADMIN
```

级联删除：K8s Deployment/Service/Ingress/ConfigMap/Secret、域名记录、部署历史（保留审计）。

**响应 204**

---

### 4.6 部署应用

```
POST /projects/{project_id}/apps/{app_id}/deploy
权限：project:OPERATOR
```

**响应 202：**
```json
{
  "data": {
    "event_id": "uuid",
    "build_job_id": "uuid"    // GIT 来源时有值
  }
}
```

---

### 4.7 暂停应用

```
POST /projects/{project_id}/apps/{app_id}/pause
权限：project:ADMIN
```

**请求：**
```json
{ "reason": "违规内容审查" }
```

**响应 200：**
```json
{ "data": { "status": "PAUSED", "paused_at": "2026-05-22T12:00:00Z" } }
```

---

### 4.8 恢复应用

```
POST /projects/{project_id}/apps/{app_id}/resume
权限：project:ADMIN
```

**响应 202：**
```json
{ "data": { "event_id": "uuid", "status": "DEPLOYING" } }
```

---

### 4.9 应用扩缩容

```
POST /projects/{project_id}/apps/{app_id}/scale
权限：project:OPERATOR
```

**请求：**
```json
{ "replicas": 3 }
```

---

### 4.10 获取 Pod 状态

```
GET /projects/{project_id}/apps/{app_id}/pods
权限：project:OBSERVER
```

**响应 200：**
```json
{
  "data": [
    {
      "name": "my-app-7d9f9-xxxx",
      "node": "worker-1",
      "status": "Running",
      "ready": true,
      "restarts": 0,
      "age": "2h",
      "cpu_usage_mcores": 50,
      "mem_usage_mb": 128
    }
  ]
}
```

---

### 4.11 获取部署历史

```
GET /projects/{project_id}/apps/{app_id}/events?page=1&per_page=20
权限：project:OBSERVER
```

---

### 4.12 Webhook 触发部署

```
POST /webhooks/{webhook_id}
权限：public（仅需 webhook_id）
```

**响应 202：**
```json
{ "data": { "event_id": "uuid" } }
```

---

## 5. 域名接口

### 5.1 域名列表

```
GET /projects/{project_id}/apps/{app_id}/domains
权限：project:OBSERVER
```

### 5.2 添加域名

```
POST /projects/{project_id}/apps/{app_id}/domains
权限：project:OPERATOR
```

**请求：**
```json
{
  "hostname": "myapp.example.com",
  "target_port": 80,
  "ssl_enabled": true,
  "use_lets_encrypt": true,
  "basic_auth_enabled": false
}
```

**响应 201：**
```json
{
  "data": {
    "id": "uuid",
    "hostname": "myapp.example.com",
    "cert_status": "PENDING",
    "dns_hint": {
      "type": "CNAME",
      "name": "myapp.example.com",
      "value": "lb.example.com"
    }
  }
}
```

### 5.3 删除域名

```
DELETE /projects/{project_id}/apps/{app_id}/domains/{domain_id}
权限：project:OPERATOR
```

系统域名（`is_system_domain=true`）不可删除，返回 403。

---

## 6. 日志与终端接口

### 6.1 Pod 日志（SSE 流）

```
GET /projects/{project_id}/apps/{app_id}/logs?pod={pod_name}&tail=200&follow=true
权限：project:OBSERVER
Content-Type: text/event-stream
```

**SSE 事件格式：**
```
data: {"timestamp": "2026-05-22T12:00:00Z", "line": "[INFO] Server started"}

data: {"timestamp": "2026-05-22T12:00:01Z", "line": "[INFO] Listening on :8080"}
```

---

### 6.2 构建日志（SSE 流）

```
GET /projects/{project_id}/apps/{app_id}/build-logs/{build_job_id}?follow=true
权限：project:OBSERVER
Content-Type: text/event-stream
```

---

### 6.3 Web 终端（WebSocket）

```
GET /projects/{project_id}/apps/{app_id}/terminal?pod={pod_name}&container=app
权限：project:OPERATOR
Upgrade: websocket
```

**消息格式：**
- 客户端发送：`{"type": "input", "data": "ls -la\n"}`
- 服务端发送：`{"type": "output", "data": "total 0\n..."}`
- 服务端发送：`{"type": "resize", "cols": 80, "rows": 24}`（客户端可发）

---

## 7. 数据库实例接口

### 7.1 数据库实例列表

```
GET /projects/{project_id}/databases?page=1&per_page=20
权限：project:OBSERVER
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "db_name": "p_myproject_appdb",
      "db_user": "u_alice_a3f9x2",
      "db_password": "••••••••",    // OBSERVER 角色返回掩码
      "cluster_type": "MYSQL_GALERA",
      "cluster_host": "galera.internal",
      "cluster_port": 3306,
      "k8s_secret_name": "db-uuid",
      "status": "ACTIVE",
      "created_at": "2026-01-01T00:00:00Z",
      "connection_url": "mysql://u_alice_a3f9x2:••••••••@galera.internal:3306/p_myproject_appdb"
    }
  ]
}
```

---

### 7.2 创建数据库实例

```
POST /projects/{project_id}/databases
权限：project:OPERATOR
```

**请求：**
```json
{
  "cluster_id": "uuid",
  "db_name": "appdb"
}
```

`db_name` 规则：小写字母、数字、下划线，1-32 位。  
实际创建数据库名：`p_{project_name}_{db_name}`。

**响应 201：**
```json
{
  "data": {
    "id": "uuid",
    "db_name": "p_myproject_appdb",
    "db_user": "u_alice_a3f9x2",
    "db_password": "generated_password",  // 仅创建时返回明文一次
    "cluster_host": "galera.internal",
    "cluster_port": 3306,
    "k8s_secret_name": "db-uuid",
    "connection_url": "mysql://...@galera.internal:3306/p_myproject_appdb"
  }
}
```

---

### 7.3 查看数据库密码

```
GET /projects/{project_id}/databases/{db_id}/credentials
权限：project:OPERATOR（OBSERVER 返回 403）
```

**响应 200：**
```json
{
  "data": {
    "db_password": "actual_password",
    "connection_url": "mysql://u_alice_a3f9x2:actual_password@galera.internal:3306/p_myproject_appdb"
  }
}
```

---

### 7.4 删除数据库实例

```
DELETE /projects/{project_id}/databases/{db_id}
权限：project:ADMIN
```

**响应 204**

---

## 8. 节点管理接口

### 8.1 节点列表

```
GET /admin/nodes
权限：admin
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "hostname": "worker-1",
      "ip_address": "192.168.1.11",
      "node_role": "WORKER",
      "node_status": "READY",
      "has_gpu": true,
      "gpu_model": "NVIDIA A100",
      "gpu_count": 2,
      "cpu_capacity_mcores": 32000,
      "mem_capacity_mb": 131072,
      "storage_available": true,
      "ldap_auth_active": true,
      "last_seen_at": "2026-05-22T12:00:00Z"
    }
  ]
}
```

---

### 8.2 添加节点

```
POST /admin/nodes
权限：admin
```

**请求：**
```json
{
  "ip_address": "192.168.1.12",
  "ssh_user": "root",
  "ssh_password": "password",
  "node_role": "WORKER"
}
```

**响应 202：**
```json
{
  "data": {
    "task_id": "uuid",
    "ws_url": "/ws/admin/node-install/{task_id}"
  }
}
```

**WebSocket 日志推送（GET /ws/admin/node-install/{task_id}）：**
```json
{"level": "info",  "message": "SSH 连接 192.168.1.12 成功"}
{"level": "info",  "message": "上传 SSH 公钥到 authorized_keys"}
{"level": "info",  "message": "安装 K3s agent..."}
{"level": "info",  "message": "✓ K3s agent 启动成功"}
{"level": "info",  "message": "配置 LDAP 认证（nslcd）..."}
{"level": "info",  "message": "✓ 节点安装完成"}
{"level": "done",  "node_id": "uuid"}
```

---

### 8.3 节点操作

```
POST /admin/nodes/{node_id}/cordon
POST /admin/nodes/{node_id}/uncordon
POST /admin/nodes/{node_id}/drain
DELETE /admin/nodes/{node_id}
权限：admin
```

---

## 9. 数据库集群管理接口

### 9.1 集群列表

```
GET /admin/db-clusters
权限：admin
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "name": "galera-main",
      "cluster_type": "MYSQL_GALERA",
      "host": "galera.internal",
      "port": 3306,
      "is_active": true,
      "db_count": 25,
      "max_databases": 100,
      "description": "主 Galera 集群"
    }
  ]
}
```

---

### 9.2 创建集群

```
POST /admin/db-clusters
权限：admin
```

**请求：**
```json
{
  "name": "galera-main",
  "cluster_type": "MYSQL_GALERA",
  "host": "galera.internal",
  "port": 3306,
  "admin_user": "root",
  "admin_password": "admin_secret",
  "max_databases": 100,
  "description": "主 Galera 集群"
}
```

按钮：测试连接 `POST /admin/db-clusters/test-connection`（同结构请求，不保存）。

---

## 10. 平台配置接口

### 10.1 获取平台配置

```
GET /admin/platform-config
权限：admin
```

**响应 200：**
```json
{
  "data": {
    "platform_domain": "cloud.example.com",
    "app_subdomain_base": "apps.example.com",
    "default_timezone": "Asia/Shanghai",
    "platform_display_name": "QuickStack",
    "shared_storage_path": "/storage",
    "ldap_url": "ldap://lldap.internal:3890",
    "ldap_base_dn": "dc=example,dc=com",
    "ldap_bind_dn": "cn=admin,dc=example,dc=com",
    "ldap_bind_password": "••••••••",
    "ldap_sync_interval_sec": 600
  }
}
```

---

### 10.2 更新平台配置

```
PUT /admin/platform-config
权限：admin
```

**请求：** 只传需要更新的字段。

---

### 10.3 手动触发 LDAP 同步

```
POST /admin/ldap/sync
权限：admin
```

**响应 202：**
```json
{
  "data": {
    "task_id": "uuid",
    "message": "LDAP 同步任务已提交"
  }
}
```

---

## 11. 安装向导接口

> 仅 installer 运行期间可用（Base URL：`http://{machine_ip}:8080/installer/api`）

### 11.1 环境预检

```
GET /installer/api/preflight
```

**响应 200：**
```json
{
  "data": {
    "checks": [
      { "name": "os_arch",    "status": "pass", "message": "Linux x86_64" },
      { "name": "root_user",  "status": "pass", "message": "Running as root" },
      { "name": "port_6443",  "status": "pass", "message": "Port free" },
      { "name": "storage",    "status": "warn", "message": "/storage 不存在，可稍后挂载" },
      { "name": "internet",   "status": "pass", "message": "get.k3s.io reachable" }
    ],
    "can_proceed": true
  }
}
```

---

### 11.2 保存配置（各步骤共用）

```
POST /installer/api/config
```

**请求：** 分步提交，`step` 字段标识当前步骤：

```json
{
  "step": "platform",
  "platform_domain": "cloud.example.com",
  "app_subdomain_base": "apps.example.com",
  "default_timezone": "Asia/Shanghai"
}
```

```json
{
  "step": "database",
  "mysql_host": "mysql.internal",
  "mysql_port": 3306,
  "mysql_database": "quickstack",
  "mysql_user": "qs_admin",
  "mysql_password": "secret"
}
```

```json
{
  "step": "ldap",
  "ldap_url": "ldap://lldap.internal:3890",
  "ldap_base_dn": "dc=example,dc=com",
  "ldap_bind_dn": "cn=admin,dc=example,dc=com",
  "ldap_bind_password": "secret",
  "ldap_user_ou": "ou=people",
  "ldap_group_ou": "ou=groups"
}
```

---

### 11.3 测试连接

```
POST /installer/api/test-connection
```

**请求：**
```json
{ "type": "mysql", "host": "mysql.internal", "port": 3306, "user": "qs_admin", "password": "secret", "database": "quickstack" }
{ "type": "ldap",  "url": "ldap://...", "bind_dn": "...", "bind_password": "..." }
{ "type": "pingora", "api_base_url": "http://lb:81/api", "username": "admin", "password": "..." }
{ "type": "ssh",   "host": "192.168.1.11", "user": "root", "password": "..." }
```

**响应 200：**
```json
{ "data": { "success": true, "message": "连接成功，发现 15 个 LDAP 用户" } }
```

---

### 11.4 添加节点（安装阶段）

```
POST /installer/api/nodes
```

**请求：**
```json
{
  "ip_address": "192.168.1.11",
  "ssh_user": "root",
  "ssh_password": "password",
  "node_role": "MASTER"
}
```

**响应 202：**
```json
{ "data": { "task_id": "uuid" } }
```

**WebSocket 实时日志：**
```
ws://machine_ip:8080/installer/ws/node/{task_id}
```

---

### 11.5 获取安装状态

```
GET /installer/api/status
```

**响应 200：**
```json
{
  "data": {
    "current_step": 7,
    "steps": [
      { "step": 1, "name": "环境预检",    "status": "done" },
      { "step": 2, "name": "基础配置",    "status": "done" },
      { "step": 3, "name": "数据库配置",  "status": "done" },
      { "step": 4, "name": "LDAP 配置",  "status": "done" },
      { "step": 5, "name": "负载均衡",    "status": "done" },
      { "step": 6, "name": "存储配置",    "status": "done" },
      { "step": 7, "name": "节点接管",    "status": "in_progress" },
      { "step": 8, "name": "组件部署",    "status": "pending" },
      { "step": 9, "name": "管理员验证",  "status": "pending" },
      { "step": 10,"name": "完成",        "status": "pending" }
    ],
    "nodes": [
      {
        "ip": "192.168.1.10",
        "role": "MASTER",
        "status": "done",
        "has_gpu": false
      }
    ]
  }
}
```

---

### 11.6 完成安装

```
POST /installer/api/finalize
```

**请求：**
```json
{
  "admin_username": "admin",
  "admin_password": "admin_password"
}
```

验证 LDAP admin 登录后：
1. 完成平台初始化
2. 返回平台访问地址
3. installer 进程退出，正式服务接管

**响应 200：**
```json
{
  "data": {
    "platform_url": "https://cloud.example.com",
    "message": "安装完成！"
  }
}
```

---

## 12. 健康检查

```
GET /health
权限：public
```

**响应 200：**
```json
{
  "status": "ok",
  "version": "1.0.0",
  "db": "ok",
  "k8s": "ok"
}
```
