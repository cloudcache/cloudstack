use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Deserialize;

use crate::error::{AppError, AppResult};

use super::types::{MetricPoint, MetricSelector, MetricSeries};

// ── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait MetricsStore: Send + Sync {
    async fn write(&self, points: &[MetricPoint]) -> AppResult<()>;
    async fn query_range(
        &self,
        selector: MetricSelector,
        start: i64,
        end: i64,
        step_secs: u32,
    ) -> AppResult<Vec<MetricSeries>>;
    async fn query_latest(&self, selector: MetricSelector) -> AppResult<Vec<MetricPoint>>;
    fn backend_name(&self) -> &'static str;
}

// ── NullStore ─────────────────────────────────────────────────────────────────

pub struct NullStore;

#[async_trait]
impl MetricsStore for NullStore {
    async fn write(&self, _: &[MetricPoint]) -> AppResult<()> { Ok(()) }
    async fn query_range(&self, _: MetricSelector, _: i64, _: i64, _: u32) -> AppResult<Vec<MetricSeries>> { Ok(vec![]) }
    async fn query_latest(&self, _: MetricSelector) -> AppResult<Vec<MetricPoint>> { Ok(vec![]) }
    fn backend_name(&self) -> &'static str { "none" }
}

// ── VictoriaMetrics ───────────────────────────────────────────────────────────
//
// Write:  POST {endpoint}/api/v1/import/prometheus   (Prometheus text format)
// Range:  GET  {endpoint}/api/v1/query_range         (PromQL)
// Latest: GET  {endpoint}/api/v1/query               (PromQL instant)

pub struct VictoriaMetricsStore {
    endpoint: String,           // e.g. "http://vm:8428"  — no trailing slash
    token: Option<String>,      // Bearer token, if any
    client: reqwest::Client,
}

impl VictoriaMetricsStore {
    pub fn new(endpoint: String, token: Option<String>) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            token,
            client: reqwest::Client::new(),
        }
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.token {
            Some(t) => req.bearer_auth(t),
            None    => req,
        }
    }

    /// Convert a MetricSelector to a PromQL selector string.
    /// e.g. `qs_app_cpu_used_mcores{app_id="xxx",project_id="yyy"}`
    fn to_promql(sel: &MetricSelector) -> String {
        if sel.labels.is_empty() {
            return sel.name.clone();
        }
        let matchers: Vec<String> = sel.labels.iter()
            .map(|(k, v)| format!(r#"{}="{}""#, k, v.replace('"', r#"\""#)))
            .collect();
        format!("{}{{{}}} ", sel.name, matchers.join(","))
    }
}

#[async_trait]
impl MetricsStore for VictoriaMetricsStore {
    /// Write points using Prometheus exposition text format.
    /// Each line: `metric{labels} value timestamp_ms`
    async fn write(&self, points: &[MetricPoint]) -> AppResult<()> {
        if points.is_empty() { return Ok(()); }

        let mut body = String::with_capacity(points.len() * 80);
        for p in points {
            body.push_str(&p.name);
            if !p.labels.is_empty() {
                body.push('{');
                let mut first = true;
                for (k, v) in &p.labels {
                    if !first { body.push(','); }
                    body.push_str(k);
                    body.push_str("=\"");
                    body.push_str(&v.replace('"', "\\\""));
                    body.push('"');
                    first = false;
                }
                body.push('}');
            }
            body.push(' ');
            body.push_str(&p.value.to_string());
            body.push(' ');
            // VictoriaMetrics /import/prometheus accepts milliseconds
            body.push_str(&(p.timestamp * 1000).to_string());
            body.push('\n');
        }

        let resp = self.auth(
            self.client
                .post(format!("{}/api/v1/import/prometheus", self.endpoint))
                .header("Content-Type", "text/plain")
                .body(body),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("vm write: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("vm write {status}: {text}")));
        }
        Ok(())
    }

    async fn query_range(
        &self,
        selector: MetricSelector,
        start: i64,
        end: i64,
        step_secs: u32,
    ) -> AppResult<Vec<MetricSeries>> {
        let query = Self::to_promql(&selector);
        let url = format!("{}/api/v1/query_range", self.endpoint);

        let resp = self.auth(
            self.client.get(&url)
                .query(&[
                    ("query", query.as_str()),
                    ("start", &start.to_string()),
                    ("end",   &end.to_string()),
                    ("step",  &step_secs.to_string()),
                ]),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("vm query_range: {e}")))?
        .json::<VmRangeResponse>()
        .await
        .map_err(|e| AppError::Internal(format!("vm query_range parse: {e}")))?;

        if resp.status != "success" {
            return Err(AppError::Internal(format!("vm query_range: {}", resp.error.unwrap_or_default())));
        }

        Ok(resp.data.result.into_iter().map(|r| MetricSeries {
            name: selector.name.clone(),
            labels: r.metric.into_iter().filter(|(k, _)| k != "__name__").collect::<BTreeMap<_,_>>(),
            points: r.values.into_iter()
                .filter_map(|v| {
                    let ts = v.0 as i64;
                    let val: f64 = v.1.parse().ok()?;
                    Some((ts, val))
                })
                .collect(),
        }).collect())
    }

    async fn query_latest(&self, selector: MetricSelector) -> AppResult<Vec<MetricPoint>> {
        let query = Self::to_promql(&selector);
        let now = chrono::Utc::now().timestamp();
        let url = format!("{}/api/v1/query", self.endpoint);

        let resp = self.auth(
            self.client.get(&url)
                .query(&[("query", query.as_str()), ("time", &now.to_string())]),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("vm query_latest: {e}")))?
        .json::<VmInstantResponse>()
        .await
        .map_err(|e| AppError::Internal(format!("vm query_latest parse: {e}")))?;

        if resp.status != "success" {
            return Err(AppError::Internal(format!("vm query: {}", resp.error.unwrap_or_default())));
        }

        Ok(resp.data.result.into_iter().filter_map(|r| {
            let ts = r.value.0 as i64;
            let val: f64 = r.value.1.parse().ok()?;
            let labels: BTreeMap<String, String> = r.metric.into_iter()
                .filter(|(k, _)| k != "__name__")
                .collect();
            Some(MetricPoint { name: selector.name.clone(), labels, timestamp: ts, value: val })
        }).collect())
    }

    fn backend_name(&self) -> &'static str { "victoria_metrics" }
}

// ── VictoriaMetrics response shapes ──────────────────────────────────────────

#[derive(Deserialize)]
struct VmRangeResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    data: VmRangeData,
}

#[derive(Deserialize)]
struct VmRangeData {
    result: Vec<VmRangeResult>,
}

#[derive(Deserialize)]
struct VmRangeResult {
    metric: BTreeMap<String, String>,
    values: Vec<(f64, String)>,   // [timestamp_secs, "value_str"]
}

#[derive(Deserialize)]
struct VmInstantResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    data: VmInstantData,
}

#[derive(Deserialize)]
struct VmInstantData {
    result: Vec<VmInstantResult>,
}

#[derive(Deserialize)]
struct VmInstantResult {
    metric: BTreeMap<String, String>,
    value: (f64, String),   // [timestamp_secs, "value_str"]
}

// ── Config + factory ──────────────────────────────────────────────────────────

pub struct MetricsConfig {
    pub endpoint: String,
    pub token: Option<String>,
    pub scrape_interval_secs: u32,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { endpoint: String::new(), token: None, scrape_interval_secs: 30 }
    }
}

pub fn build_store(cfg: MetricsConfig) -> Box<dyn MetricsStore> {
    if cfg.endpoint.is_empty() {
        tracing::info!("metrics: no endpoint configured — using NullStore");
        return Box::new(NullStore);
    }
    tracing::info!(endpoint = %cfg.endpoint, "metrics: VictoriaMetrics backend");
    Box::new(VictoriaMetricsStore::new(cfg.endpoint, cfg.token))
}
