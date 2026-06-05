# 应用模板 v2：服务绑定 + 在线 CRUD

版本：1.0
日期：2026-06-04
状态：设计中

---

## 1. 背景

当前模板系统的两个根本问题：

1. **模板是硬编码 TS 文件**（`frontend/src/shared/templates/*.ts`），任何增删改要改源码、重新构建前端。运营/租户无法维护。
2. **与 managed service 抽象冲突**：
   - 平台已有 `database_clusters` + `database_instances`（共享 MySQL/PG/MongoDB，租户拿账号）
   - 平台已有 `s3_targets`（共享 S3，租户拿凭证）
   - 但模板里 Postgres/MySQL/Redis/MinIO 都是 **per-tenant pod**
   - 依赖 DB 的应用模板（Nextcloud / n8n / Docmost）通过 `postCreate*.template.ts` 创建陪同的 DB pod，绕过 managed service

目标：模板**声明依赖**，部署时把依赖**绑定到 managed service** 或**新建实例**。同时支持在线 CRUD。

---

## 2. 核心概念变化

### 2.1 旧：模板 = 一组 K8s 资源定义

```
Template "Nextcloud"
  ├── App: nextcloud container
  ├── PostCreate: 起一个 mariadb pod
  └── 用户自己把 DB 连接 string 配进 env
```

### 2.2 新：模板 = 一组 App 定义 + 一组**服务依赖声明**

```
Template "Nextcloud"
  ├── App: nextcloud container
  └── Requires:
       ├── service "db": type=mysql, version>=10.6
       │     → 部署时让用户：[bind existing] 或 [provision new managed instance]
       │     → 自动注入 env: DB_HOST, DB_USER, DB_PASS, DB_NAME
       └── service "objstore" (optional): type=s3
             → 部署时让用户：[bind existing target] 或 [skip]
             → 自动注入 env: S3_ENDPOINT, S3_BUCKET, S3_KEY, S3_SECRET
```

依赖关系**声明在模板里**，注入逻辑**做在后端**，租户**永远不需要手填连接串**。

---

## 3. 数据模型

### 3.1 表：`app_templates`

```sql
CREATE TABLE app_templates (
    id              CHAR(36)     NOT NULL PRIMARY KEY,
    slug            VARCHAR(64)  NOT NULL UNIQUE,        -- "nextcloud", "n8n"
    name            VARCHAR(128) NOT NULL,               -- "Nextcloud"
    icon_url        VARCHAR(512) NULL,
    category        VARCHAR(32)  NOT NULL DEFAULT 'app', -- 'app' | 'database' | 'internal'
    description     TEXT         NULL,
    -- 谁能看到：
    visibility      VARCHAR(16)  NOT NULL DEFAULT 'PUBLIC',  -- PUBLIC | PRIVATE | ORG
    owner_user_id   CHAR(36)     NULL,                       -- PRIVATE 时谁建的
    owner_project_id CHAR(36)    NULL,                       -- ORG 时哪个项目
    -- 模板正文：app spec JSON（兼容旧的 AppTemplateContentModel 但扁平化）
    spec            JSON         NOT NULL,
    -- 服务依赖（数组）—— 见 §3.2
    requirements    JSON         NOT NULL DEFAULT (JSON_ARRAY()),
    -- 输入参数（用户能填的开关，如镜像 tag、管理员密码等）
    inputs          JSON         NOT NULL DEFAULT (JSON_ARRAY()),
    is_active       TINYINT(1)   NOT NULL DEFAULT 1,
    version         INT          NOT NULL DEFAULT 1,
    created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_at_visibility (visibility),
    INDEX idx_at_owner_user (owner_user_id),
    INDEX idx_at_owner_project (owner_project_id)
);
```

### 3.2 `requirements` JSON 结构（声明依赖）

```jsonc
[
  {
    "key": "db",                           // env 前缀
    "kind": "database",                    // database | objstore | cache
    "engine": "mysql",                     // mysql | mariadb | postgres | mongodb | redis | s3
    "min_version": "10.6",
    "required": true,
    "label": "Database",
    "env_mapping": {                       // 怎么注入到容器
      "host": "DB_HOST",
      "port": "DB_PORT",
      "name": "DB_NAME",
      "user": "DB_USER",
      "password": "DB_PASS"
    },
    "binding_modes": ["managed", "provision"]
    // managed   = 选一个已有 database_instance / s3_target 绑定
    // provision = 现场新建（调 POST /projects/:pid/databases）
    // 都不允许 = 部署时报错
  },
  {
    "key": "objstore",
    "kind": "objstore",
    "engine": "s3",
    "required": false,
    "label": "Object Storage (optional)",
    "env_mapping": {
      "endpoint": "S3_ENDPOINT",
      "bucket": "S3_BUCKET",
      "access_key": "S3_KEY",
      "secret_key": "S3_SECRET"
    },
    "binding_modes": ["managed"]
  }
]
```

### 3.3 表：`app_template_bindings`（运行时：app → 服务实例的绑定）

跟踪一个已部署 app 绑定到了哪个 managed 资源，方便删 app 时同步清理。

```sql
CREATE TABLE app_template_bindings (
    id              CHAR(36)    NOT NULL PRIMARY KEY,
    app_id          CHAR(36)    NOT NULL,
    requirement_key VARCHAR(64) NOT NULL,        -- 对应 requirements[].key (如 "db")
    binding_kind    VARCHAR(16) NOT NULL,        -- 'database_instance' | 's3_target'
    binding_ref_id  CHAR(36)    NOT NULL,        -- database_instances.id 或 s3_targets.id
    provisioned     TINYINT(1)  NOT NULL DEFAULT 0,  -- 1=部署时新建（删 app 时也删）
    created_at      DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_atb_app (app_id),
    CONSTRAINT fk_atb_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
);
```

---

## 4. 流程

### 4.1 租户从模板部署

```
1. 租户打开 [从模板创建应用]
2. 前端 GET /api/v1/templates → 列出对当前用户可见的（PUBLIC + own PRIVATE + project ORG）
3. 租户选 Nextcloud
4. 前端读 requirements[]:
     - "db" required → 弹出"配置数据库"步骤
         · 选项 A: [绑定现有] 下拉显示 project 已有 database_instances
         · 选项 B: [新建] 下拉显示 admin 暴露给本 project 的 database_clusters
     - "objstore" optional → 折叠展示，默认 skip
5. 租户确认 → POST /api/v1/projects/:pid/apps/from-template
     body: {
       template_id, inputs: {...},
       bindings: [
         { key: "db", mode: "provision", cluster_id: "...", db_name_hint: "nc" },
         { key: "objstore", mode: "skip" }
       ]
     }
6. 后端按 requirements 顺序处理 bindings：
     - mode=managed: 校验 binding_ref_id 归属本 project
     - mode=provision: 调 databases::create() / s3 provision
   写入 app_template_bindings
7. 把每个 binding 的连接信息按 env_mapping 注入 app 的 env_vars
8. 部署 app（走现有 deploy 流程）
```

### 4.2 删除 app

```
1. DELETE /projects/:pid/apps/:id
2. 查 app_template_bindings WHERE app_id=?
3. 对 provisioned=1 的：调对应 managed service 的 delete
4. 删 app（现有逻辑）
5. CASCADE 自动清 bindings 行
```

### 4.3 管理员 / 租户 CRUD

| 端点 | 谁能用 | 说明 |
|------|--------|------|
| `GET /templates` | 任何登录用户 | 列可见模板（按 visibility 过滤） |
| `GET /templates/:id` | 任何可见 | 详情 |
| `POST /admin/templates` | global admin | 建 PUBLIC 模板 |
| `PUT /admin/templates/:id` | global admin | 改 PUBLIC 模板 |
| `DELETE /admin/templates/:id` | global admin | 删 |
| `POST /projects/:pid/templates` | project OWNER | 建 ORG/PRIVATE 模板（限本项目） |
| `PUT /projects/:pid/templates/:id` | 同上 | |
| `DELETE /projects/:pid/templates/:id` | 同上 | |

可视化编辑器：
- 表单填基础字段（slug / name / icon / category / visibility）
- "App spec" 段：复用现有 app 编辑表单（镜像、端口、卷、env、健康检查）
- "Requirements" 段：列表 + Add 按钮，每行选 kind + engine + required + env mapping
- "Inputs" 段：列表，每行 key/label/default/isEnvVar/randomGenerated（继承现有 inputSettings）

---

## 5. 兼容现有 40 个硬编码模板

**迁移策略**：用一个 seed 脚本，把现有的 TS 模板批量导入 `app_templates` 表。

```typescript
// scripts/seed-templates.ts
import { allTemplates } from "./templates/all.templates";

for (const t of allTemplates) {
    await db.app_templates.upsert({
        slug: slugify(t.name),
        name: t.name,
        icon_url: t.iconName,
        category: databaseTemplates.includes(t) ? 'database' : 'app',
        visibility: 'PUBLIC',
        spec: t.templates[0],     // 旧模型嵌套 templates[]，新模型扁平
        requirements: deriveRequirements(t),  // 见下
        inputs: t.templates[0].inputSettings,
    });
}
```

`deriveRequirements()` 把现有 `postCreate*` 的隐式 DB 依赖转成 `requirements`：

| 旧模板 | 旧行为 | 新 requirements |
|--------|--------|-----------------|
| Nextcloud (postCreate 起 mariadb) | 隐式起 pod | `[{kind:database, engine:mariadb, required:true, ...}]` |
| n8n (postCreate 起 postgres) | 隐式起 pod | `[{kind:database, engine:postgres, required:true, ...}]` |
| Docmost | 起 postgres | `[{kind:database, engine:postgres, required:true, ...}]` |
| 纯 DB 模板 (Postgres/MySQL/etc) | 自己就是 DB pod | category='database' + 没有 requirements；**或弃用，引导用户走 managed** |
| MinIO | 自己就是 S3 pod | category='internal'（管理员组件，租户不可见） |

迁移完后**删 `frontend/src/shared/templates/` 整个目录** —— 不再有 TS 硬编码模板，全部走 DB。

---

## 6. 实施分期

### Phase 1：模板上数据库 + 后端 CRUD（最小可上线）
- 新表 `app_templates`
- 后端 `api/templates.rs`：list/get/create/update/delete + seed 脚本
- 前端模板选择器改成 fetch `/api/v1/templates`
- **不动 requirements 字段，先用 `requirements: []` 占位**
- 现有 `postCreate*` 暂时继续生效

**效果**：UI 能从 DB 读模板了，但仍然是 pod-per-tenant DB。改动有限，风险低。

### Phase 2：requirements 模型 + 绑定流程
- 新表 `app_template_bindings`
- 后端 `from-template` 路由解析 requirements，调 managed DB/S3 API
- 前端"从模板部署"加配置步骤（选 binding 或 provision）
- 把 Nextcloud / n8n / Docmost 等模板的 requirements 写好
- 删 `postCreate*` 函数（被 binding 流程取代）

**效果**：依赖 DB 的模板部署时自动走 managed service，不再起陪同 pod。

### Phase 3：管理员/租户在线 CRUD UI
- 管理员"模板管理"页面
- 项目层"我的私有模板"
- 可视化编辑器（基础字段 + spec + requirements + inputs）

**效果**：完全运行时管理，源码里再无模板。

---

## 7. 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| Spec 用 JSON 还是关系表 | **JSON 字段** | 模板结构嵌套深、变动频繁，关系表过度规范化得不偿失 |
| requirements 是否带版本 | **是**（`min_version`） | managed cluster 可能装的是 PG 13，模板要 PG 14 — 需要校验 |
| 模板版本管理 | **MVP 阶段：`version` 字段递增即可**，不留历史；后续按需加 `app_template_versions` 表 | 避免一开始就过度设计 |
| 删除已有 binding | **provisioned=1 才级联删** | 防止误删用户共享的 managed instance |
| 旧 `databaseTemplates` 怎么办 | **category='database' 但默认不在租户的"创建应用"列表里显示**；通过别的入口（"用模板部署一个独立 DB"）显式访问 | 保留能力但不诱导误用 |
| 模板 schema 用 JSON Schema 校验吗 | **后端用 serde + 自定义校验**，不引 JSON Schema 运行时 | 维护一份 zod + serde 已经足够 |

---

## 8. 风险

| 风险 | 缓解 |
|------|------|
| 已部署的 app 怎么办（没有 bindings 行） | seed 时**不动**，存量 app 继续按原 env 跑；只有从 Phase 2 起新建的 app 走 binding |
| Provisioned binding 在 managed cluster 上孤儿化 | 删 app 用事务：先删 managed 资源、再删 app；失败回滚 |
| 模板 spec JSON 与 app 表 schema drift | spec JSON 在 deploy 时被映射成 apps/app_ports/app_volumes 行 —— 加一个 spec_version 字段，后端用 match 路由 |
| 私有模板被用户互相看见 | `visibility=PRIVATE` 时 list/get 严格 WHERE owner_user_id = current_user_id |
