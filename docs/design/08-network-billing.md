# QuickStack 网络用量计费设计文档

版本：1.0  
日期：2026-05-23  
状态：正式

---

## 目录

1. [设计概述](#1-设计概述)
2. [数据模型](#2-数据模型)
3. [LB 指标采集](#3-lb-指标采集)
4. [P95 算法](#4-p95-算法)
5. [请求/响应体均值](#5-请求响应体均值)
6. [欠费管理](#6-欠费管理)
7. [计费配置参数](#7-计费配置参数)
8. [API 接口](#8-api-接口)
9. [后台任务](#9-后台任务)
10. [计费流程图](#10-计费流程图)

---

## 1. 设计概述

### 1.1 背景

QuickStack 通过 Pingora 反向代理承载用户应用的外部流量。每个应用绑定一或多个域名，Pingora 统计每个域名的入/出流量、请求数及请求/响应体大小。平台需对这些数据进行采集、聚合，并按月计算网络用量费用。

### 1.2 两种计费模式

| 模式 | 说明 | 适用场景 |
|------|------|---------|
| **P95 出流量带宽计费** | 电信运营商常见的"保底带宽 + 峰值带宽"计费方式。每月采集所有5分钟采样点，去掉最高的5%突发后，取第95百分位带宽值乘以单价 | 出流量大、波动明显的应用；适合按带宽租用 |
| **按 GB 计费** | 对月总出流量按固定单价（CNY/GB）计费 | 低流量用户或平台未配置P95单价时的降级策略 |

### 1.3 为什么首选 P95

- **公平**：屏蔽偶发性流量尖峰（如一次大文件推送），避免用户因短暂突发被收取极高费用。
- **对齐运营商成本**：带宽资源按峰值容量采购，P95 更准确地反映实际承诺带宽。
- **可预期**：用户可预估月账单；按量计费在高峰期费用难以预测。

当平台配置 `price_egress_p95_mbps_month = 0` 时，系统自动降级为按 GB 计费。

---

## 2. 数据模型

### 2.1 `lb_bandwidth_samples`（LB 带宽采样表）

每 5 分钟一条记录，记录某个应用在该时间窗口内通过 LB 产生的流量数据。

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | BIGINT UNSIGNED AUTO_INCREMENT | 主键 |
| `app_id` | CHAR(36) NOT NULL | 关联 apps.id |
| `project_id` | CHAR(36) NOT NULL | 冗余，加速按项目聚合 |
| `sampled_at` | DATETIME NOT NULL | 时间窗口起始点（对齐到5分钟整点，如 10:05:00、10:10:00） |
| `duration_secs` | SMALLINT UNSIGNED NOT NULL | 采样窗口实际时长（通常 300，但首次可能不足） |
| `ingress_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 窗口内入流量（字节） |
| `egress_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 窗口内出流量（字节） |
| `req_count` | INT UNSIGNED NOT NULL DEFAULT 0 | 窗口内请求总数 |
| `req_body_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 窗口内请求体总字节 |
| `resp_body_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 窗口内响应体总字节 |

索引：
```sql
UNIQUE KEY uq_app_window (app_id, sampled_at)
KEY idx_project_time (project_id, sampled_at)
KEY idx_sampled_at (sampled_at)
```

> `ON DUPLICATE KEY UPDATE` 语义：同一 (app_id, sampled_at) 若已存在（例如 scraper 重试），则累加各流量字段，不覆盖。这保证了 scraper 幂等重试的正确性。

### 2.2 `monthly_network_charges`（月度网络费用表）

每月月末（或管理员触发）对每个项目计算一次，结果写入此表。

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | BIGINT UNSIGNED AUTO_INCREMENT | 主键 |
| `project_id` | CHAR(36) NOT NULL | 关联 projects.id |
| `user_id` | CHAR(36) NOT NULL | 冗余，关联 users.id（项目所有者） |
| `billing_month` | CHAR(7) NOT NULL | 格式 `YYYY-MM`，如 `2026-05` |
| `p95_egress_mbps` | DECIMAL(12,4) NOT NULL DEFAULT 0 | P95 出流量带宽（Mbps） |
| `total_egress_gb` | DECIMAL(16,6) NOT NULL DEFAULT 0 | 月总出流量（GB） |
| `total_ingress_gb` | DECIMAL(16,6) NOT NULL DEFAULT 0 | 月总入流量（GB） |
| `mean_req_body_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 月平均请求体大小（字节） |
| `mean_resp_body_bytes` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 月平均响应体大小（字节） |
| `total_req_count` | BIGINT UNSIGNED NOT NULL DEFAULT 0 | 月总请求数 |
| `egress_charge` | DECIMAL(10,4) NOT NULL DEFAULT 0 | 本月出流量应收费用（CNY） |
| `status` | ENUM('PENDING','CHARGED','WAIVED') NOT NULL DEFAULT 'PENDING' | 收费状态 |
| `computed_at` | DATETIME NOT NULL | 计算时间 |
| `charged_at` | DATETIME NULL | 实际扣款时间 |

索引：
```sql
UNIQUE KEY uq_project_month (project_id, billing_month)
KEY idx_user_month (user_id, billing_month)
KEY idx_status (status)
```

### 2.3 `overdue_charges`（欠费利息记录表）

记录每日对余额为负的用户收取的欠费利息。

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | BIGINT UNSIGNED AUTO_INCREMENT | 主键 |
| `user_id` | CHAR(36) NOT NULL | 关联 users.id |
| `charge_date` | DATE NOT NULL | 计息日期 |
| `overdue_balance` | DECIMAL(12,4) NOT NULL | 计息时的欠费余额（负数的绝对值） |
| `fee_pct` | DECIMAL(8,6) NOT NULL | 当日利率（如 0.0005 = 0.05%） |
| `fee_amount` | DECIMAL(10,4) NOT NULL | 本日利息金额（CNY） |
| `status` | ENUM('PENDING','APPLIED','WAIVED') NOT NULL DEFAULT 'PENDING' | 处理状态 |
| `applied_at` | DATETIME NULL | 实际扣款时间 |

索引：
```sql
UNIQUE KEY uq_user_date (user_id, charge_date)
KEY idx_status (status)
```

---

## 3. LB 指标采集

### 3.1 Pingora 统计接口

Pingora 反向代理暴露一个 HTTP 接口，返回各域名的累计计数器（delta 模式：每次读取后自动清零）：

```
GET http://{pingora_stats_addr}/stats/domains
```

响应格式（JSON）：

```json
{
  "domains": [
    {
      "domain": "myapp.example.com",
      "ingress_bytes": 1048576,
      "egress_bytes": 5242880,
      "req_count": 1234,
      "req_body_bytes": 204800,
      "resp_body_bytes": 4096000
    }
  ]
}
```

> **Delta 计数器**：Pingora 在响应后将各计数器清零。因此每次 scrape 得到的是"上次读取到本次"之间的增量，无需做差值计算。若 scraper 宕机后重启，Pingora 侧的数据会在下次读取时完整返回，不会丢失（Pingora 侧持续累积直到被读取）。

### 3.2 域名到应用的映射

通过 `app_domains` 表将域名映射到 app_id：

```sql
SELECT app_id, project_id
  FROM app_domains
 WHERE domain = ?
   AND is_active = 1
```

一个域名只归属一个应用；一个应用可有多个域名，同一5分钟窗口内各域名的流量累加到同一条 `lb_bandwidth_samples` 记录。

### 3.3 采样时间对齐

采样时间戳对齐到5分钟整点：

```rust
fn floor_to_5min(ts: DateTime<Utc>) -> DateTime<Utc> {
    let secs = ts.timestamp();
    let aligned = secs - (secs % 300);
    DateTime::from_timestamp(aligned, 0).unwrap()
}
```

`duration_secs` 记录实际距上次采样的秒数（首次 scrape 后才能计算，通常为 300）。

### 3.4 `src/proxy/stats.rs` 设计

```rust
pub struct LbStatsScraper {
    http_client: reqwest::Client,
    stats_url:   String,          // platform_config.pingora_stats_url
    last_run_at: Option<Instant>,
}

impl LbStatsScraper {
    /// 每 5 分钟调用一次（由 tokio::spawn 定时任务驱动）
    pub async fn scrape_and_store(&mut self, state: &AppState) -> AppResult<()> {
        let resp = self.http_client.get(&self.stats_url).send().await?;
        let stats: PingoraStatsResponse = resp.json().await?;

        let now = Utc::now();
        let window_start = floor_to_5min(now);
        let duration_secs = self.last_run_at
            .map(|t| t.elapsed().as_secs() as u16)
            .unwrap_or(300);
        self.last_run_at = Some(Instant::now());

        for domain_stat in stats.domains {
            let Some((app_id, project_id)) = lookup_app(&state.db, &domain_stat.domain).await?
            else { continue; };

            sqlx::query!(r#"
                INSERT INTO lb_bandwidth_samples
                    (app_id, project_id, sampled_at, duration_secs,
                     ingress_bytes, egress_bytes, req_count, req_body_bytes, resp_body_bytes)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    ingress_bytes   = ingress_bytes   + VALUES(ingress_bytes),
                    egress_bytes    = egress_bytes    + VALUES(egress_bytes),
                    req_count       = req_count       + VALUES(req_count),
                    req_body_bytes  = req_body_bytes  + VALUES(req_body_bytes),
                    resp_body_bytes = resp_body_bytes + VALUES(resp_body_bytes)
            "#, app_id, project_id, window_start, duration_secs,
                domain_stat.ingress_bytes, domain_stat.egress_bytes,
                domain_stat.req_count, domain_stat.req_body_bytes, domain_stat.resp_body_bytes
            ).execute(&state.db).await?;
        }
        Ok(())
    }
}
```

---

## 4. P95 算法

### 4.1 带宽值计算

将每个采样窗口的字节数转换为 Mbps：

```
egress_mbps = egress_bytes / duration_secs × 8 / 1_000_000
```

例：5分钟窗口（300秒）内出流量 150 MB：

```
egress_mbps = (150 × 1024 × 1024) / 300 × 8 / 1_000_000 ≈ 4.194 Mbps
```

### 4.2 最近秩法（Nearest-Rank）

设一个月共有 N 个采样点（最多 8928 个，即 31天 × 288次/天），计算步骤：

1. 将所有采样点的 `egress_mbps` 升序排列：`[v₁, v₂, …, vₙ]`
2. P95 索引：`k = ceil(0.95 × N)`（从1开始计数）
3. P95值：`p95_egress_mbps = vₖ`

```rust
fn compute_p95(mut values: Vec<f64>) -> f64 {
    if values.is_empty() { return 0.0; }
    values.sort_by(f64::total_cmp);
    let k = ((0.95 * values.len() as f64).ceil() as usize).max(1);
    values[k - 1]
}
```

### 4.3 月度账单计算

```
egress_charge = p95_egress_mbps × price_egress_p95_mbps_month
```

### 4.4 降级为按 GB 计费

当 `price_egress_p95_mbps_month = 0` 时：

```
egress_charge = total_egress_gb × price_egress_gb
              + total_ingress_gb × price_ingress_gb
              + (total_req_body_bytes / 1024³) × price_req_body_gb
```

### 4.5 数据量不足时的处理

| 采样数 N | 处理方式 |
|---------|---------|
| 0 | `egress_charge = 0`，记录但不扣款 |
| 1–19 | P95 退化为最大值（样本过少，取 k=N） |
| ≥ 20 | 正常 nearest-rank 计算 |

---

## 5. 请求/响应体均值

月度计算时，从 `lb_bandwidth_samples` 聚合：

```sql
SELECT
    SUM(req_body_bytes)  AS total_req_body,
    SUM(resp_body_bytes) AS total_resp_body,
    SUM(req_count)       AS total_req_count
  FROM lb_bandwidth_samples
 WHERE project_id = ?
   AND sampled_at >= ? AND sampled_at < ?
```

然后：

```
mean_req_body_bytes  = total_req_body  / total_req_count  (整数除法，向下取整)
mean_resp_body_bytes = total_resp_body / total_req_count
```

**用途：**
- 流量画像：识别大文件上传/下载型应用 vs API 型应用。
- 异常告警：请求体均值突然增大可能表示用户误上传大文件或遭受攻击。
- 未来扩展：可基于请求体大小实施单独的"上传流量"收费策略。

当 `total_req_count = 0` 时，均值字段记为 0，不做除法。

---

## 6. 欠费管理

### 6.1 触发条件

钱包余额 < 0 时，用户进入欠费状态。欠费状态不立即停服，而是每日收取利息，直至：
- 用户充值使余额 ≥ 0（利息停止累积），或
- 宽限期（`billing_overdue_grace_days`）届满后管理员介入暂停服务。

### 6.2 日利息计算

```
fee_amount = |wallet_balance| × billing_overdue_daily_fee_pct / 100
```

示例：余额 -200.00 CNY，日利率 0.05%：

```
fee_amount = 200.00 × 0.0005 = 0.10 CNY/天
```

### 6.3 原子操作

欠费利息扣款以数据库事务执行，确保一致性：

```sql
BEGIN;
  -- 1. 幂等检查：当日记录已 APPLIED 则跳过
  SELECT status FROM overdue_charges
   WHERE user_id = ? AND charge_date = ? FOR UPDATE;

  -- 2. 扣款
  UPDATE wallets
     SET balance = balance - :fee_amount
   WHERE user_id = ?;

  -- 3. 记录交易流水
  INSERT INTO wallet_transactions
    (user_id, amount, type, description, created_at)
  VALUES (?, -:fee_amount, 'OVERDUE_FEE', '欠费日息 YYYY-MM-DD', NOW());

  -- 4. 更新利息记录状态
  UPDATE overdue_charges
     SET status = 'APPLIED', applied_at = NOW()
   WHERE user_id = ? AND charge_date = ?;
COMMIT;
```

### 6.4 宽限期

宽限期内，`overdue_charges` 记录生成但 `status = PENDING`（不立即扣款），给用户充值机会。宽限期届满后，后台任务将 PENDING 记录批量 APPLIED。

| 配置键 | 默认值 | 说明 |
|--------|-------|------|
| `billing_overdue_grace_days` | 3 | 欠费后宽限天数，期间生成账单但不扣款 |

### 6.5 欠费状态与订阅状态联动

| 情况 | 订阅状态变化 |
|------|------------|
| 余额 < 0 且在宽限期内 | 不变（仍 ACTIVE） |
| 余额 < 0 且超出宽限期 | `ACTIVE` → `OVERDUE` |
| 用户充值使余额 ≥ 0 | `OVERDUE` → `ACTIVE`（自动恢复） |

---

## 7. 计费配置参数

以下键存储在 `platform_config` 表中：

| 键名 | 默认值 | 类型 | 说明 |
|------|-------|------|------|
| `pingora_stats_url` | `http://127.0.0.1:9081/stats/domains` | STRING | Pingora 统计接口地址 |
| `price_egress_p95_mbps_month` | `0` | DECIMAL | 月度P95出流量单价（CNY/Mbps），0 = 禁用P95改用按GB计费 |
| `price_egress_gb` | `0.08` | DECIMAL | 出流量单价（CNY/GB），仅在禁用P95时生效 |
| `price_ingress_gb` | `0` | DECIMAL | 入流量单价（CNY/GB），默认免费 |
| `price_req_body_gb` | `0` | DECIMAL | 请求体流量单价（CNY/GB），默认0 |
| `billing_overdue_grace_days` | `3` | INT | 欠费宽限天数 |
| `billing_overdue_daily_fee_pct` | `0.05` | DECIMAL | 日欠费利率（百分比，如 0.05 = 0.05%） |
| `network_billing_enabled` | `0` | BOOL | 是否启用网络用量计费 |

> 所有计费配置修改均立即生效，不影响历史已计算的账单记录。

---

## 8. API 接口

### 8.1 用户侧：项目月度网络用量

```
GET /api/v1/projects/:pid/network-usage?months=24
```

**权限**：项目 OBSERVER 及以上。

**查询参数：**
- `months`：返回最近 N 个月，默认 24，最大 24。

**响应 200：**

```json
{
  "project_id": "uuid",
  "records": [
    {
      "billing_month": "2026-05",
      "p95_egress_mbps": 12.34,
      "total_egress_gb": 38.9,
      "total_ingress_gb": 12.1,
      "total_req_count": 1200000,
      "mean_req_body_bytes": 1024,
      "mean_resp_body_bytes": 8192,
      "egress_charge": "98.72",
      "status": "CHARGED",
      "charged_at": "2026-06-01T02:00:00"
    }
  ]
}
```

### 8.2 用户侧：欠费状态

```
GET /api/v1/billing/overdue
```

**权限**：已登录用户（查询自身）。

**响应 200：**

```json
{
  "is_overdue": true,
  "wallet_balance": "-45.20",
  "grace_days_remaining": 1,
  "total_pending_fee": "0.23",
  "records": [
    {
      "charge_date": "2026-05-21",
      "overdue_balance": "43.10",
      "fee_amount": "0.22",
      "status": "APPLIED"
    },
    {
      "charge_date": "2026-05-22",
      "overdue_balance": "45.20",
      "fee_amount": "0.23",
      "status": "PENDING"
    }
  ]
}
```

### 8.3 管理员：触发计算月度网络费用

```
POST /api/v1/admin/billing/network-charges/compute
```

**权限**：全局管理员。

**请求体：**

```json
{
  "billing_month": "2026-05",
  "project_ids": ["uuid1", "uuid2"]   // 可选，不传则计算所有项目
}
```

**行为：**
1. 从 `lb_bandwidth_samples` 聚合指定月份所有采样点。
2. 按 P95 或按 GB 算法（取决于平台配置）计算费用。
3. `UPSERT` 到 `monthly_network_charges`，状态设为 `PENDING`。
4. 不扣款（仅计算）。

**响应 200：**

```json
{
  "billing_month": "2026-05",
  "computed_count": 42,
  "total_charge_cny": "1234.56"
}
```

### 8.4 管理员：执行扣款

```
POST /api/v1/admin/billing/network-charges/collect
```

**权限**：全局管理员。

**请求体：**

```json
{
  "billing_month": "2026-05",
  "dry_run": false   // true 则只返回预览，不实际扣款
}
```

**行为（对每条 PENDING 记录）：**
1. 从用户钱包扣除 `egress_charge`。
2. 插入 `wallet_transactions` 流水记录（type = `NETWORK_CHARGE`）。
3. 将 `monthly_network_charges.status` 更新为 `CHARGED`，写入 `charged_at`。
4. 若钱包余额不足，仍执行扣款（余额变负），由欠费管理流程跟进。

**响应 200：**

```json
{
  "billing_month": "2026-05",
  "collected_count": 40,
  "skipped_count": 2,
  "total_collected_cny": "1234.56"
}
```

### 8.5 管理员：查看欠费用户列表

```
GET /api/v1/admin/billing/overdue?status=PENDING&page=1&per_page=20
```

**权限**：全局管理员。

**响应 200：**

```json
{
  "data": [
    {
      "user_id": "uuid",
      "username": "alice",
      "email": "alice@example.com",
      "wallet_balance": "-200.00",
      "overdue_since": "2026-05-20",
      "total_pending_fee": "0.50",
      "subscription_status": "OVERDUE"
    }
  ],
  "total": 5,
  "page": 1,
  "per_page": 20
}
```

---

## 9. 后台任务

以下后台任务在 `src/main.rs` 中通过 `tokio::spawn` 启动：

### 9.1 LB 统计采集任务

```
任务名：lb_stats_scraper
间隔：每 5 分钟
入口：LbStatsScraper::scrape_and_store()
```

| 步骤 | 说明 |
|------|------|
| 1 | 调用 `GET {pingora_stats_url}` |
| 2 | 解析每条域名统计 |
| 3 | 查询 `app_domains` 获取 app_id / project_id |
| 4 | `UPSERT` 到 `lb_bandwidth_samples` |
| 5 | 更新 `last_run_at` 以计算下次 duration_secs |

**容错**：HTTP 超时或 Pingora 不可用时，记录错误日志并跳过本次，不影响下次采集。

### 9.2 欠费利息计算任务

```
任务名：overdue_charge_applier
触发：每日 UTC 00:05（宽限期检查）
幂等：以 (user_id, charge_date) 作为唯一键，重复运行安全
```

| 步骤 | 说明 |
|------|------|
| 1 | 扫描所有钱包余额 < 0 的用户 |
| 2 | 检查欠费天数是否超过宽限期 |
| 3 | 若超过宽限期：执行扣款事务（见第6.3节） |
| 4 | 若在宽限期内：插入 `status=PENDING` 的记录，不扣款 |
| 5 | 超出宽限期且订阅仍 ACTIVE：更新为 OVERDUE |

---

## 10. 计费流程图

### 10.1 数据采集与月度计算

```
┌──────────────┐    GET /stats/domains     ┌─────────────────┐
│   Pingora    │ ◄──────────────────────── │  lb_stats_scraper│
│  (反向代理)  │ ──────────────────────── ►│  (每5分钟)      │
└──────────────┘   delta 计数器（读后清零）└────────┬────────┘
                                                    │
                                                    │ UPSERT (app_id, sampled_at)
                                                    ▼
                                          ┌──────────────────────┐
                                          │  lb_bandwidth_samples │
                                          │  (5分钟粒度采样表)    │
                                          └──────────┬───────────┘
                                                     │
                                    月末 / 管理员触发 │
                                                     ▼
                                          ┌──────────────────────┐
                                          │  P95 / 按GB 计算引擎  │
                                          │  compute_monthly()    │
                                          └──────────┬───────────┘
                                                     │
                                                     │ UPSERT status=PENDING
                                                     ▼
                                          ┌──────────────────────┐
                                          │ monthly_network_charges│
                                          └──────────┬───────────┘
                                                     │
                                       管理员 POST /collect
                                                     ▼
                                          ┌──────────────────────┐
                                          │    wallets (扣款)     │
                                          │ wallet_transactions   │
                                          └──────────┬───────────┘
                                                     │
                                              若余额 < 0
                                                     ▼
                                          ┌──────────────────────┐
                                          │   overdue_charges     │
                                          │   (每日利息计算)      │
                                          └──────────────────────┘
```

### 10.2 欠费处理状态机

```
                     余额充足
                   ┌──────────┐
                   │  正常状态 │
                   └────┬─────┘
                        │ 扣款后余额 < 0
                        ▼
               ┌────────────────┐
               │  欠费（宽限期） │  ← ACTIVE 订阅，每日生成 PENDING 利息记录
               └────────┬───────┘
            ┌───────────┴──────────┐
            │ 用户充值余额 ≥ 0      │ 宽限期届满
            ▼                      ▼
    ┌──────────────┐     ┌──────────────────┐
    │  恢复正常状态 │     │  欠费超限         │  ← OVERDUE 订阅，服务受限
    └──────────────┘     │  利息开始扣款     │
                         └──────────────────┘
                                  │ 充值 + 结清利息
                                  ▼
                         ┌──────────────────┐
                         │  恢复 ACTIVE      │
                         └──────────────────┘
```
