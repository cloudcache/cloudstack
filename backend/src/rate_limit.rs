/// Simple in-memory IP-based rate limiter.
///
/// Uses a sliding-window counter per IP address. Expired entries are lazily
/// cleaned up every 60 seconds to prevent unbounded growth.
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};
use tokio::sync::Mutex;

struct Entry {
    hits: Vec<Instant>,
}

pub struct RateLimiter {
    state: Mutex<HashMap<IpAddr, Entry>>,
    max_requests: u32,
    window: Duration,
    last_cleanup: Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(HashMap::new()),
            max_requests,
            window,
            last_cleanup: Mutex::new(Instant::now()),
        })
    }

    async fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let cutoff = now - self.window;

        let mut map = self.state.lock().await;

        // Lazy cleanup
        {
            let mut last = self.last_cleanup.lock().await;
            if now.duration_since(*last) > Duration::from_secs(60) {
                map.retain(|_, e| e.hits.last().map_or(false, |t| *t > cutoff));
                *last = now;
            }
        }

        let entry = map.entry(ip).or_insert_with(|| Entry { hits: Vec::new() });
        entry.hits.retain(|t| *t > cutoff);

        if entry.hits.len() >= self.max_requests as usize {
            return false;
        }
        entry.hits.push(now);
        true
    }
}

/// Axum middleware: rate-limits by client IP.
///
/// Requires `Extension<Arc<RateLimiter>>` and `ConnectInfo<SocketAddr>`.
pub async fn rate_limit(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(limiter): Extension<Arc<RateLimiter>>,
    req: Request,
    next: Next,
) -> Response {
    if !limiter.check(addr.ip()).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded — try again later",
        )
            .into_response();
    }
    next.run(req).await
}
