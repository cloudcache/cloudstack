# QuickStack 用户·订阅·配额 设计与接口文档

版本：1.1  
日期：2026-05-23  
状态：正式

---

## 目录

1. [设计概述](#1-设计概述)
2. [数据模型](#2-数据模型)
3. [配额流转模型](#3-配额流转模型)
4. [订阅状态机](#4-订阅状态机)
5. [项目成员角色](#5-项目成员角色)
6. [API — 认证](#6-api--认证)
7. [API — 个人中心](#7-api--个人中心)
8. [API — 订阅计划（用户侧）](#8-api--订阅计划用户侧)
9. [API — 账单与钱包](#9-api--账单与钱包)
10. [API — 项目管理](#10-api--项目管理)
11. [API — 管理员：用户管理](#11-api--管理员用户管理)
12. [API — 管理员：订阅管理](#12-api--管理员订阅管理)
13. [API — 管理员：项目管理](#13-api--管理员项目管理)
14. [API — 管理员：账单管理](#14-api--管理员账单管理)
15. [错误码](#15-错误码)

---

## 1. 设计概述

### 1.1 核心原则

| 原则 | 说明 |
|------|------|
| **用户无配额** | `users` 表不存储任何配额字段；配额由订阅计划定义 |
| **配额归 Project** | 订阅计划的配额池分配给 Project，运行时检查 Project 配额 |
| **默认 Project** | 用户首次激活订阅时，系统自动创建一个 `is_default=1` 的 Project，并将计划全量配额写入该 Project |
| **管理员可覆盖** | 管理员可在不换计划的前提下单独调整某个用户默认 Project 的各项配额 |
| **计费可选** | `billing_enabled` 关闭时订阅免费，计费系统静默 |

### 1.2 核心实体关系

```
subscription_plans ──(1:N)── user_subscriptions ──(N:1)── users
                                                              │
                                              auto-create     │
                                        ┌────────────────────┘
                                        ▼
                                   projects (is_default=1)
                                        │
                              (owner allocates quota to)
                                        │
                                   projects (is_default=0)
                                        │
                              ┌─────────┴──────────┐
                              ▼                     ▼
                            apps            database_instances
```

---

## 2. 数据模型

### 2.1 users（用户表）

用户表**不含任何配额字段**。资源限制来自订阅计划 → Project。

| 列 | 类型 | 说明 |
|----|------|------|
| `id` | CHAR(36) | UUID 主键 |
| `username` | VARCHAR(64) | 与 LLDAP uid 一致，唯一 |
| `email` | VARCHAR(255) | 唯一 |
| `display_name` | VARCHAR(128) | 显示名，可空 |
| `ldap_dn` | VARCHAR(512) | LLDAP 完整 DN |
| `ldap_uid` | INT UNSIGNED | POSIX UID（容器 securityContext） |
| `ldap_gid` | INT UNSIGNED | POSIX GID |
| `is_global_admin` | TINYINT(1) | 全局管理员标志 |
| `is_active` | TINYINT(1) | 账号启用状态 |
| `created_at` | DATETIME | 创建时间 |
| `updated_at` | DATETIME | 更新时间 |

### 2.2 subscription_plans（订阅计划表）

定义一套命名的资源配额池及定价。

| 列 | 类型 | 说明 |
|----|------|------|
| `id` | CHAR(36) | UUID |
| `name` | VARCHAR(64) | 内部 slug（唯一），如 `free`、`pro` |
| `display_name` | VARCHAR(128) | 前端显示名 |
| `description` | TEXT | 计划描述 |
| `price_monthly` | DECIMAL(10,2) | 月费（0 = 免费） |
| `price_annually` | DECIMAL(10,2) | 年费折扣价，NULL = 不支持年付 |
| `quota_cpu_mcores` | INT UNSIGNED | CPU 毫核上限（0 = 不限） |
| `quota_mem_mb` | INT UNSIGNED | 内存 MB 上限 |
| `quota_storage_gb` | INT UNSIGNED | 存储 GB 上限 |
| `quota_bandwidth_gb` | INT UNSIGNED | 月出流量 GB 上限 |
| `quota_domain_count` | INT UNSIGNED | 域名总数上限 |
| `quota_db_instance_count` | INT UNSIGNED | 数据库实例数上限 |
| `quota_project_count` | INT UNSIGNED | 可创建项目数上限（0 = 不限） |
| `quota_app_count` | INT UNSIGNED | 应用实例总数上限 |
| `quota_request_million` | INT UNSIGNED | 月请求数上限（百万） |
| `is_active` | TINYINT(1) | 计划是否启用 |
| `is_public` | TINYINT(1) | 用户是否可自助订阅 |
| `sort_order` | SMALLINT | 前端排序权重 |

### 2.3 user_subscriptions（用户订阅表）

| 列 | 类型 | 说明 |
|----|------|------|
| `id` | CHAR(36) | UUID |
| `user_id` | CHAR(36) | 关联 users.id |
| `plan_id` | CHAR(36) | 关联 subscription_plans.id |
| `status` | ENUM | `PENDING` / `ACTIVE` / `OVERDUE` / `EXPIRED` / `CANCELLED` |
| `billing_cycle` | ENUM | `MONTHLY` / `ANNUALLY` / `LIFETIME` / `CUSTOM` |
| `started_at` | DATETIME | 激活时间 |
| `expires_at` | DATETIME | 到期时间（NULL = 永不过期） |
| `auto_renew` | TINYINT(1) | 是否自动续费 |
| `cancelled_at` | DATETIME | 取消时间 |
| `cancel_reason` | VARCHAR(512) | 取消原因 |
| `price_paid` | DECIMAL(10,2) | 订阅时实付价格（计划价格可能变化） |
| `created_by` | CHAR(36) | 管理员 user_id；NULL 表示用户自订 |

### 2.4 projects（项目表）

Project 是资源隔离与配额分配的基本单元，对应 K8s Namespace。

| 列 | 类型 | 说明 |
|----|------|------|
| `id` | CHAR(36) | UUID |
| `name` | VARCHAR(128) | K8s namespace slug（唯一，小写字母/数字/连字符） |
| `display_name` | VARCHAR(255) | 显示名 |
| `owner_id` | CHAR(36) | 所有者 user_id |
| `is_active` | TINYINT(1) | 项目启用状态 |
| `is_default` | TINYINT(1) | **订阅激活时自动创建的默认项目** |
| `quota_cpu_mcores` | INT UNSIGNED | 已分配 CPU 毫核上限 |
| `quota_mem_mb` | INT UNSIGNED | 已分配内存 MB 上限 |
| `quota_storage_gb` | INT UNSIGNED | 已分配存储 GB 上限 |
| `quota_apps` | INT UNSIGNED | 已分配应用数上限 |
| `quota_db_instances` | INT UNSIGNED | 已分配数据库实例数上限 |
| `quota_bandwidth_gb` | INT UNSIGNED | 已分配月出流量 GB 上限 |
| `quota_domain_count` | INT UNSIGNED | 已分配域名数上限 |
| `quota_request_million` | INT UNSIGNED | 已分配月请求数上限（百万） |

### 2.5 project_members（项目成员表）

| 列 | 类型 | 说明 |
|----|------|------|
| `project_id` | CHAR(36) | PK 部分 |
| `user_id` | CHAR(36) | PK 部分 |
| `role` | ENUM | `ADMIN` / `OPERATOR` / `OBSERVER` |
| `added_by` | CHAR(36) | 邀请人 user_id |
| `added_at` | DATETIME | 加入时间 |

---

## 3. 配额流转模型

```
┌─────────────────────────────────┐
│      subscription_plans         │
│  quota_cpu_mcores = 8000        │
│  quota_mem_mb     = 16384       │
│  quota_app_count  = 20          │
│  quota_project_count = 3        │  ← 用户最多可创建 3 个项目
│  ...                            │
└───────────────┬─────────────────┘
                │ 订阅激活时
                │ 全量写入 →
                ▼
┌──────────────────────────────────────────┐
│   projects  (is_default=1, 自动创建)      │
│   quota_cpu_mcores   = 8000              │
│   quota_mem_mb       = 16384             │
│   quota_apps         = 20               │
│   ...                                    │
└───────────────┬──────────────────────────┘
                │ 用户自行拆分（通过管理员 API）
                ├─────────────────────────────┐
                ▼                             ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│ projects (project-A)    │    │ projects (project-B)    │
│ quota_cpu_mcores = 4000 │    │ quota_cpu_mcores = 4000 │
│ quota_apps = 10         │    │ quota_apps = 10         │
└─────────────────────────┘    └─────────────────────────┘
```

**关键规则：**
- `subscription_plans.quota_project_count` 限制用户可拥有的项目总数（0 = 不限）
- Project 的 `quota_*` 字段是"已分配"值，由管理员或用户（通过后续管理界面）写入
- 应用/数据库创建时，检查所属 Project 的对应配额是否充足
- 配额值 `0` 统一表示不限制

### 3.1 默认 Project 的生命周期

| 事件 | 行为 |
|------|------|
| 订阅首次激活（ACTIVE） | 若无默认 Project → 自动创建（名称：`{username}-default`）；将计划全量配额写入该 Project |
| 换计划（admin_assign_plan） | 默认 Project 存在则直接覆写配额；不存在则自动创建 |
| 管理员单项覆盖 | 只更新指定字段，其余字段保持不变 |
| 订阅取消 + expiry_action=RESET | 默认 Project 所有配额字段清零 |
| 订阅取消 + expiry_action=KEEP（默认） | 配额字段保留，应用继续运行 |

---

## 4. 订阅状态机

```
                  ┌──────────┐
                  │  PENDING │ ← 管理员预创建但未激活
                  └────┬─────┘
                       │ 激活（admin 操作或用户付款）
                       ▼
              ┌─────────────────┐
       ┌──────│     ACTIVE      │──────┐
       │      └────────┬────────┘      │
       │               │               │
       │ 欠费（cron）  │ 到期（cron）  │ 取消
       ▼               ▼               ▼
  ┌─────────┐    ┌─────────┐    ┌───────────┐
  │ OVERDUE │    │ EXPIRED │    │ CANCELLED │
  └────┬────┘    └─────────┘    └───────────┘
       │
       │ 续费成功（admin/cron）
       ▼
    ACTIVE
```

| 状态 | 说明 |
|------|------|
| `PENDING` | 已创建，待激活（管理员预分配未生效） |
| `ACTIVE` | 正常使用，配额生效 |
| `OVERDUE` | 欠费状态，服务受限但未停止（由 cron 任务扫描写入） |
| `EXPIRED` | 已到期，超过 `expires_at` |
| `CANCELLED` | 已取消，记录留存 |

> 同一用户同一时刻最多有一条 `ACTIVE` 或 `OVERDUE` 的订阅记录。

---

## 5. 项目成员角色

| 角色 | 权限 |
|------|------|
| `OBSERVER` | 查看项目/成员/应用/日志，不可写 |
| `OPERATOR` | `OBSERVER` + 部署/暂停/恢复应用，管理数据库，管理环境变量和端口 |
| `ADMIN` | `OPERATOR` + 邀请/移除成员、修改成员角色、修改项目显示名、删除项目 |
| 全局管理员 | 绕过所有项目级权限检查，可执行任意操作 |

**保护规则：**
- 每个项目必须至少保留一名 `ADMIN` 成员（last-admin guard）
- 项目所有者（`owner_id`）不能被降级或移除，除非先转移所有权
- 转移所有权：目标用户必须已是项目成员，完成后自动升级为 `ADMIN`

---

## 6. API — 认证

所有公开端点（无需 Authorization）：

### 6.1 登录

```
POST /auth/login
```

**请求：**
```json
{
  "username": "alice",
  "password": "secret123",
  "totp_code": "123456"   // 可选；启用 TOTP 时必填
}
```

**响应 200：**
```json
{
  "token": "eyJ...",
  "user": {
    "id": "uuid",
    "username": "alice",
    "email": "alice@example.com",
    "display_name": "Alice",
    "is_global_admin": false
  }
}
```

> 若账号启用了 TOTP 但未传 `totp_code`，返回 401，消息为 `TOTP_REQUIRED`。

### 6.2 注册

```
POST /auth/register
```

受 `registration_enabled` 平台配置控制（默认开放）。

**请求：**
```json
{
  "username": "bob",
  "email": "bob@example.com",
  "password": "secret123",
  "display_name": "Bob"
}
```

**响应 201：**
```json
{
  "id": "uuid",
  "message": "注册成功"
}
```

> `registration_require_approval=1` 时账号初始 `is_active=0`，需管理员审批后方可登录。

### 6.3 忘记密码

```
POST /auth/forgot-password
```

```json
{ "email": "alice@example.com" }
```

**响应 200：** 始终返回成功（防止用户枚举），重置链接发送至邮箱，有效期 1 小时。

### 6.4 重置密码

```
POST /auth/reset-password
```

```json
{
  "token": "hex-token-from-email",
  "new_password": "newSecret123"
}
```

**响应 200：** 密码更新，所有 session 立即失效。

### 6.5 当前用户信息

```
GET /api/v1/auth/me
```

**响应 200：**
```json
{
  "id": "uuid",
  "username": "alice",
  "email": "alice@example.com",
  "display_name": "Alice",
  "is_global_admin": false,
  "subscription": {
    "status": "ACTIVE",
    "plan_name": "pro",
    "plan_display_name": "专业版",
    "expires_at": "2026-06-23T00:00:00"
  },
  "created_at": "2026-01-01T00:00:00"
}
```

### 6.6 登出

```
POST /api/v1/auth/logout
```

**响应 204**。当前 session token 立即失效。

### 6.7 TOTP 管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api/v1/auth/totp/setup` | 生成 TOTP secret，返回 `{ secret, qr_url }` |
| `POST` | `/api/v1/auth/totp/verify` | 验证 code 并启用 TOTP，请求体 `{ "code": "123456" }` |
| `POST` | `/api/v1/auth/totp/disable` | 禁用 TOTP |

---

## 7. API — 个人中心

### 7.1 查看个人资料

```
GET /api/v1/profile
```

**响应 200：**
```json
{
  "id": "uuid",
  "username": "alice",
  "email": "alice@example.com",
  "display_name": "Alice",
  "is_global_admin": false,
  "subscription": {
    "id": "sub-uuid",
    "status": "ACTIVE",
    "billing_cycle": "MONTHLY",
    "expires_at": "2026-06-23T00:00:00",
    "auto_renew": true,
    "plan_name": "pro",
    "plan_display_name": "专业版",
    "plan_quota": {
      "cpu_mcores": 8000,
      "mem_mb": 16384,
      "storage_gb": 100,
      "bandwidth_gb": 200,
      "domain_count": 10,
      "db_instance_count": 5,
      "project_count": 3,
      "app_count": 20,
      "request_million": 100
    }
  },
  "wallet": {
    "balance": "99.50",
    "currency": "CNY"
  },
  "totp_enabled": true,
  "created_at": "2026-01-01T00:00:00"
}
```

### 7.2 修改个人资料

```
PUT /api/v1/profile
```

```json
{ "display_name": "Alice Smith" }
```

**响应 204**

### 7.3 修改密码

```
POST /api/v1/profile/change-password
```

```json
{
  "current_password": "oldSecret",
  "new_password": "newSecret123"
}
```

**响应 204**。密码修改后所有其他 session 失效。

### 7.4 Session 管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/profile/sessions` | 列出当前有效 session（含 IP、User-Agent） |
| `DELETE` | `/api/v1/profile/sessions/:id` | 撤销指定 session |
| `DELETE` | `/api/v1/profile/sessions/all` | 撤销所有其他 session（保留当前） |

**Session 列表响应：**
```json
[
  {
    "id": "uuid",
    "ip_addr": "1.2.3.4",
    "user_agent": "Mozilla/5.0...",
    "created_at": "2026-05-01T10:00:00",
    "expires_at": "2026-05-08T10:00:00"
  }
]
```

### 7.5 SSH 公钥管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/profile/ssh-keys` | 列出所有 SSH 公钥 |
| `POST` | `/api/v1/profile/ssh-keys` | 添加 SSH 公钥 |
| `GET` | `/api/v1/profile/ssh-keys/:id` | 查看公钥详情（含完整 public_key） |
| `DELETE` | `/api/v1/profile/ssh-keys/:id` | 删除 SSH 公钥 |

**添加公钥请求：**
```json
{
  "name": "My MacBook",
  "public_key": "ssh-ed25519 AAAA..."
}
```

**添加公钥响应 201：**
```json
{
  "id": "uuid",
  "fingerprint": "SHA256:abc123 (ssh-ed25519)"
}
```

---

## 8. API — 订阅计划（用户侧）

### 8.1 查看可用计划列表

```
GET /api/v1/plans
```

返回所有 `is_active=1 AND is_public=1` 的计划。

**响应 200：**
```json
[
  {
    "id": "uuid",
    "name": "free",
    "display_name": "免费版",
    "description": "适合个人试用",
    "price_monthly": "0.00",
    "price_annually": null,
    "quota": {
      "cpu_mcores": 2000,
      "mem_mb": 2048,
      "storage_gb": 10,
      "bandwidth_gb": 50,
      "domain_count": 3,
      "db_instance_count": 2,
      "project_count": 2,
      "app_count": 5,
      "request_million": 1
    },
    "is_active": true,
    "is_public": true,
    "sort_order": 0
  }
]
```

### 8.2 查看计划详情

```
GET /api/v1/plans/:id
```

### 8.3 查看我的订阅

```
GET /api/v1/subscription
```

**响应 200：**
```json
{
  "subscription": {
    "id": "uuid",
    "plan_id": "uuid",
    "plan_name": "pro",
    "plan_display_name": "专业版",
    "status": "ACTIVE",
    "billing_cycle": "MONTHLY",
    "started_at": "2026-05-23T00:00:00",
    "expires_at": "2026-06-23T00:00:00",
    "auto_renew": true,
    "cancelled_at": null,
    "cancel_reason": null,
    "price_paid": "29.00",
    "created_at": "2026-05-23T00:00:00"
  }
}
```

`subscription` 为 `null` 表示无订阅记录。

### 8.4 订阅计划（自助）

```
POST /api/v1/subscription
```

受 `subscription_self_service` 平台配置控制。

**请求：**
```json
{
  "plan_id": "uuid",
  "billing_cycle": "MONTHLY",
  "auto_renew": true
}
```

**行为：**
1. 检查 `subscription_allow_downgrade` 配置，如禁止降级则拒绝
2. 若 `billing_enabled=1` 且计划有价格，从钱包扣款
3. 取消当前 ACTIVE/OVERDUE 订阅
4. 创建新的 ACTIVE 订阅
5. 将计划配额写入默认 Project（自动创建 Project 如不存在）

**响应 201：**
```json
{
  "subscription_id": "uuid",
  "status": "ACTIVE",
  "expires_at": "2026-06-23T00:00:00"
}
```

### 8.5 取消订阅

```
DELETE /api/v1/subscription
```

```json
{ "reason": "不再需要" }
```

**响应 204**。若 `subscription_expiry_action=RESET`，默认 Project 配额清零。

---

## 9. API — 账单与钱包

### 9.1 钱包余额

```
GET /api/v1/billing/wallet
```

```json
{ "balance": "99.50", "currency": "CNY" }
```

### 9.2 交易记录

```
GET /api/v1/billing/transactions?type=DEDUCTION&page=1&per_page=20
```

`type` 可选值：`RECHARGE` / `DEDUCTION` / `REFUND` / `ADJUSTMENT`

### 9.3 当前用量

```
GET /api/v1/billing/usage
```

```json
{
  "active_apps": 3,
  "active_databases": 1,
  "mtd_cost": "12.34",
  "last_snapshot": {
    "time": "2026-05-23T10:00:00",
    "cpu_mcores": 1500,
    "mem_mb": 3072,
    "storage_gb": 8,
    "hourly_cost": "0.0040"
  }
}
```

### 9.4 用量历史

```
GET /api/v1/billing/usage/history?start=2026-05-01&end=2026-05-31&page=1
```

### 9.5 账单列表 / 详情

```
GET /api/v1/billing/invoices
GET /api/v1/billing/invoices/:id
```

---

## 10. API — 项目管理

### 10.1 列出我的项目

```
GET /api/v1/projects
```

**响应 200：**
```json
[
  {
    "id": "uuid",
    "name": "alice-default",
    "display_name": "alice 的默认项目",
    "owner_id": "uuid",
    "owner_username": "alice",
    "is_active": true,
    "is_default": true,
    "my_role": "ADMIN",
    "member_count": 1,
    "quota": {
      "cpu_mcores": 8000,
      "mem_mb": 16384,
      "storage_gb": 100,
      "apps": 20,
      "db_instances": 5,
      "bandwidth_gb": 200,
      "domain_count": 10,
      "request_million": 100
    },
    "created_at": "2026-05-23T00:00:00"
  }
]
```

### 10.2 创建项目

```
POST /api/v1/projects
```

受 `allow_user_create_projects` 平台配置控制。

```json
{
  "name": "my-app",
  "display_name": "我的应用项目"
}
```

- `name` 规则：小写字母/数字/连字符，以字母开头，最长 63 字符（对应 K8s namespace 命名规范）
- 创建者自动成为该项目的 `ADMIN` 成员

**响应 201：** `{ "id": "uuid", "name": "my-app" }`

### 10.3 查看项目详情

```
GET /api/v1/projects/:id
```

最低权限：`OBSERVER`。响应包含成员列表和完整配额。

### 10.4 更新项目

```
PUT /api/v1/projects/:id
```

最低权限：`ADMIN`。用户侧**只能修改** `display_name`，配额由管理员控制。

```json
{ "display_name": "新名称" }
```

### 10.5 删除项目

```
DELETE /api/v1/projects/:id
```

最低权限：`ADMIN`。若项目下有正在运行的应用则拒绝删除。

### 10.6 退出项目

```
POST /api/v1/projects/:id/leave
```

项目所有者不能退出（需先转移所有权），最后一名 `ADMIN` 不能退出。

### 10.7 转移所有权

```
POST /api/v1/projects/:id/transfer
```

发起人须为所有者或全局管理员，目标用户须已是项目成员。

```json
{ "to_user_id": "uuid" }
```

### 10.8 成员管理

| 方法 | 路径 | 说明 | 最低权限 |
|------|------|------|---------|
| `GET` | `/api/v1/projects/:id/members` | 列出成员 | OBSERVER |
| `POST` | `/api/v1/projects/:id/members` | 邀请成员 | ADMIN |
| `PUT` | `/api/v1/projects/:id/members/:user_id` | 修改成员角色 | ADMIN |
| `DELETE` | `/api/v1/projects/:id/members/:user_id` | 移除成员 | ADMIN |

**邀请成员请求：**
```json
{
  "user_id": "uuid",
  "role": "OPERATOR"
}
```

或通过用户名邀请：
```json
{
  "username": "bob",
  "role": "OBSERVER"
}
```

**修改成员角色：**
```json
{ "role": "ADMIN" }
```

---

## 11. API — 管理员：用户管理

所有 `/admin/*` 接口要求 `is_global_admin=true`。

### 11.1 列出用户

```
GET /api/v1/admin/users?search=alice&is_active=true&page=1&per_page=20
```

**响应 200：**
```json
{
  "data": [
    {
      "id": "uuid",
      "username": "alice",
      "email": "alice@example.com",
      "display_name": "Alice",
      "ldap_uid": 1001,
      "ldap_gid": 1001,
      "is_global_admin": false,
      "is_active": true
    }
  ],
  "total": 42,
  "page": 1,
  "per_page": 20
}
```

### 11.2 创建用户

```
POST /api/v1/admin/users
```

```json
{
  "username": "charlie",
  "email": "charlie@example.com",
  "password": "initialPass123",
  "display_name": "Charlie",
  "is_global_admin": false
}
```

- 同步在 LLDAP 创建用户并设置密码
- 自动分配至默认用户组（`ldap.default_user_group_id` 配置）
- 自动初始化钱包

**响应 201：** `{ "id": "uuid" }`

### 11.3 查看用户详情

```
GET /api/v1/admin/users/:id
```

**响应 200：**
```json
{
  "id": "uuid",
  "username": "alice",
  "email": "alice@example.com",
  "display_name": "Alice",
  "ldap_uid": 1001,
  "ldap_gid": 1001,
  "is_global_admin": false,
  "is_active": true,
  "wallet_balance": "99.50",
  "subscription": {
    "status": "ACTIVE",
    "plan": "专业版",
    "expires_at": "2026-06-23T00:00:00"
  },
  "created_at": "2026-01-01T00:00:00",
  "updated_at": "2026-05-23T00:00:00"
}
```

### 11.4 更新用户

```
PUT /api/v1/admin/users/:id
```

```json
{
  "display_name": "Alice Smith",
  "email": "alice2@example.com",
  "is_active": true,
  "is_global_admin": false
}
```

所有字段均可选，未传字段保持不变。管理员不能移除自己的 `is_global_admin` 标志。

**响应 204**

### 11.5 删除用户

```
DELETE /api/v1/admin/users/:id
```

同步从 LLDAP 删除，不能删除自己。**响应 204**

### 11.6 重置用户密码

```
POST /api/v1/admin/users/:id/reset-password
```

```json
{ "new_password": "newPass123" }
```

同步更新 LLDAP 密码，撤销该用户所有 session。**响应 204**

### 11.7 查看用户用量摘要

```
GET /api/v1/admin/users/:id/usage
```

```json
{
  "active_apps": 3,
  "active_databases": 1,
  "last_snapshot": {
    "time": "2026-05-23T10:00:00",
    "cpu_mcores": 1500,
    "mem_mb": 3072,
    "storage_gb": 8,
    "hourly_cost": "0.0040"
  }
}
```

---

## 12. API — 管理员：订阅管理

### 12.1 订阅计划 CRUD

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/admin/plans` | 列出所有计划（含下线计划），支持分页 |
| `POST` | `/api/v1/admin/plans` | 创建计划 |
| `GET` | `/api/v1/admin/plans/:id` | 查看计划详情（含活跃订阅数） |
| `PUT` | `/api/v1/admin/plans/:id` | 更新计划（所有字段可选） |
| `DELETE` | `/api/v1/admin/plans/:id` | 删除计划（有活跃订阅时拒绝） |

**创建计划请求：**
```json
{
  "name": "pro",
  "display_name": "专业版",
  "description": "适合中小团队",
  "price_monthly": "29.00",
  "price_annually": "290.00",
  "quota_cpu_mcores": 8000,
  "quota_mem_mb": 16384,
  "quota_storage_gb": 100,
  "quota_bandwidth_gb": 200,
  "quota_domain_count": 10,
  "quota_db_instance_count": 5,
  "quota_project_count": 3,
  "quota_app_count": 20,
  "quota_request_million": 100,
  "is_public": true,
  "sort_order": 1
}
```

### 12.2 订阅列表

```
GET /api/v1/admin/subscriptions?user_id=&plan_id=&status=ACTIVE&page=1
```

### 12.3 修改订阅

```
PUT /api/v1/admin/subscriptions/:id
```

```json
{
  "expires_at": "2026-12-31T00:00:00Z",
  "auto_renew": false,
  "status": "ACTIVE",
  "cancel_reason": null
}
```

将 `status` 改为 `ACTIVE` 时，会重新将计划配额写入默认 Project。

有效状态值：`PENDING` / `ACTIVE` / `OVERDUE` / `EXPIRED` / `CANCELLED`

### 12.4 强制取消订阅

```
DELETE /api/v1/admin/subscriptions/:id
```

```json
{
  "reason": "违规使用",
  "reset_quotas": true
}
```

`reset_quotas=true` 时将默认 Project 所有配额清零。

### 12.5 查看 / 管理用户当前订阅与配额

```
GET  /api/v1/admin/users/:id/subscription
POST /api/v1/admin/users/:id/subscription
```

#### GET — 查看订阅与配额现状

**响应 200：**
```json
{
  "subscription": {
    "id": "uuid",
    "plan_id": "uuid",
    "plan_name": "pro",
    "plan_display_name": "专业版",
    "status": "ACTIVE",
    "billing_cycle": "MONTHLY",
    "started_at": "2026-05-23T00:00:00",
    "expires_at": "2026-06-23T00:00:00",
    "auto_renew": false,
    "cancelled_at": null,
    "cancel_reason": null,
    "price_paid": "0.00",
    "created_at": "2026-05-23T00:00:00",
    "plan_quota": {
      "cpu_mcores": 8000,
      "mem_mb": 16384,
      "storage_gb": 100,
      "bandwidth_gb": 200,
      "domain_count": 10,
      "db_instances": 5,
      "apps": 20,
      "request_million": 100,
      "project_count": 3
    }
  },
  "default_project_allocation": {
    "project_id": "uuid",
    "project_name": "alice-default",
    "display_name": "alice 的默认项目",
    "cpu_mcores": 8000,
    "mem_mb": 16384,
    "storage_gb": 100,
    "bandwidth_gb": 200,
    "domain_count": 10,
    "db_instances": 5,
    "apps": 20,
    "request_million": 100
  }
}
```

`plan_quota` 是计划定义的原始值，`default_project_allocation` 是实际分配值（可能因管理员覆盖而不同）。

#### POST — 模式一：分配新计划（免扣款）

```json
{
  "plan_id": "uuid",
  "billing_cycle": "CUSTOM",
  "expires_at": "2026-12-31T00:00:00Z",
  "auto_renew": false,
  "skip_quota_apply": false
}
```

- 取消现有 ACTIVE/OVERDUE 订阅
- 创建新 ACTIVE 订阅，`price_paid=0.00`
- `skip_quota_apply=false`（默认）：将计划全量配额写入默认 Project
- 如同时传入单项配额字段，在计划配额写入后叠加覆盖

**响应 201：**
```json
{
  "mode": "plan_assigned",
  "subscription_id": "uuid",
  "status": "ACTIVE",
  "plan_price": "29.00"
}
```

#### POST — 模式二：单项配额覆盖（不换计划）

不传 `plan_id`，只传需要修改的配额字段：

```json
{
  "quota_cpu_mcores": 16000,
  "quota_mem_mb": 32768
}
```

- 订阅记录不变，只更新默认 Project 对应字段
- 未传字段保持原值
- 要求用户当前有 ACTIVE 或 OVERDUE 订阅

**响应 200：**
```json
{ "mode": "quota_patched" }
```

#### POST — 模式三：分配计划 + 同时覆盖部分配额

```json
{
  "plan_id": "uuid",
  "billing_cycle": "CUSTOM",
  "quota_cpu_mcores": 16000
}
```

先按计划写入所有配额，再用 `quota_cpu_mcores=16000` 覆盖 CPU 配额，其余字段取计划值。

---

## 13. API — 管理员：项目管理

### 13.1 列出所有项目

```
GET /api/v1/admin/projects?search=&owner_id=&is_active=true&page=1&per_page=20
```

响应含 `is_default`、`member_count`、`app_count` 及完整 8 项配额。

### 13.2 创建项目（指定所有者）

```
POST /api/v1/admin/projects
```

```json
{
  "name": "team-alpha",
  "display_name": "Alpha 团队",
  "owner_id": "uuid",
  "quota_cpu_mcores": 4000,
  "quota_mem_mb": 8192,
  "quota_storage_gb": 50,
  "quota_apps": 10,
  "quota_db_instances": 3,
  "quota_bandwidth_gb": 100,
  "quota_domain_count": 5,
  "quota_request_million": 50
}
```

**响应 201：** `{ "id": "uuid", "name": "team-alpha" }`

### 13.3 查看项目详情（管理员视图）

```
GET /api/v1/admin/projects/:id
```

响应额外包含 `owner_email`、`stats.db_count` 等字段。

### 13.4 更新项目

```
PUT /api/v1/admin/projects/:id
```

```json
{
  "display_name": "新名称",
  "is_active": true,
  "owner_id": "uuid",
  "quota_cpu_mcores": 4000,
  "quota_bandwidth_gb": 100
}
```

所有字段可选。修改 `owner_id` 时自动将新所有者升级为 `ADMIN` 成员。

### 13.5 强制删除项目

```
DELETE /api/v1/admin/projects/:id
```

即使有运行中的应用也强制删除。**响应 204**

### 13.6 成员管理（管理员版）

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/admin/projects/:id/members` | 列出成员 |
| `POST` | `/api/v1/admin/projects/:id/members` | 添加成员 |
| `PUT` | `/api/v1/admin/projects/:id/members/:user_id` | 修改成员角色 |
| `DELETE` | `/api/v1/admin/projects/:id/members/:user_id` | 移除成员 |

---

## 14. API — 管理员：账单管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/v1/admin/billing/wallets` | 列出所有用户钱包（含余额） |
| `POST` | `/api/v1/admin/billing/recharge` | 给用户充值 |
| `POST` | `/api/v1/admin/billing/adjustment` | 调整余额（正/负） |
| `GET` | `/api/v1/admin/billing/invoices` | 列出所有账单 |
| `POST` | `/api/v1/admin/billing/invoices` | 为用户生成账单（汇总指定周期用量快照） |
| `POST` | `/api/v1/admin/billing/invoices/:id/pay` | 标记账单已付款（从钱包扣款） |

**充值请求：**
```json
{
  "user_id": "uuid",
  "amount": "100.00",
  "description": "线下充值"
}
```

**余额调整请求：**
```json
{
  "user_id": "uuid",
  "amount": "-10.00",
  "description": "退款扣回"
}
```

---

## 15. 错误码

| HTTP | code | 场景 |
|------|------|------|
| 400 | `BAD_REQUEST` | 参数校验失败、密码过短、不合法的 billing_cycle 等 |
| 401 | `UNAUTHORIZED` | Token 无效/过期；`TOTP_REQUIRED`；`TOTP_INVALID` |
| 403 | `FORBIDDEN` | 权限不足；账号已禁用；自助订阅已关闭；禁止降级 |
| 404 | `NOT_FOUND` | 资源不存在 |
| 409 | `CONFLICT` | username/email 已存在；计划名重复；删除有活跃订阅的计划 |
| 500 | `INTERNAL_ERROR` | 服务器内部错误 |

---

## 附录：平台配置键（platform_config）

| key | 默认值 | 说明 |
|-----|--------|------|
| `registration_enabled` | `1` | 是否开放用户自主注册 |
| `registration_require_approval` | `0` | 注册是否需要管理员审批 |
| `allow_user_create_projects` | `1` | 是否允许普通用户自主创建项目 |
| `subscription_self_service` | `1` | 是否允许用户自助订阅/升降级 |
| `subscription_allow_downgrade` | `1` | 是否允许用户降级计划 |
| `subscription_expiry_action` | `KEEP` | 订阅到期/取消后配额处理：`KEEP` 保留 \| `RESET` 清零 |
| `billing_enabled` | `0` | 是否启用计费扣款（关闭时订阅免费） |
| `billing_currency` | `CNY` | 计费货币单位 |
| `price_cpu_mcores_hour` | `0.0001` | 每 mCore·小时 价格 |
| `price_mem_mb_hour` | `0.0001` | 每 MB·小时 价格 |
| `price_storage_gb_month` | `0.10` | 每 GB·月 价格 |
| `frontend_url` | `` | 前端访问地址（用于邮件链接） |
| `platform_display_name` | `QuickStack` | 平台显示名称（用于邮件签名） |
