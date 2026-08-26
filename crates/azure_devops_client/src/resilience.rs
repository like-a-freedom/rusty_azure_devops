use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use base64::Engine;

use crate::AzureError;

/// Boxed, send-safe future used by `OnPremFallback` so the trait stays
/// object-safe. Optional lifetime so adapters can borrow from `self` / args.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, AzureError>> + Send + 'a>>;

/// Retry strategy: exposes a `RetryPolicy` callers apply to SDK operations.
/// Orthogonal to fallback — tests can inject one without the other.
pub trait RetryStrategy: Send + Sync + 'static {
    /// The retry policy the client should apply to transport operations. Callers
    /// use `policy.retry_async(...)` directly on their own typed futures.
    fn retry_policy(&self) -> &RetryPolicy;
}

/// On-prem HTTP fallback that bypasses the SDK. The adapter iterates API
/// versions and auth header variants. `None` for cloud URLs that never
/// fall back; `Some(ProductionFallback)` for on-prem.
pub trait OnPremFallback: Send + Sync + 'static {
    /// Direct HTTP GET that bypasses the SDK. Returns parsed JSON on success,
    /// `AzureError::Http(Fallback { errors })` if all attempts fail.
    fn get<'a>(&'a self, path: &'a str) -> BoxFuture<'a, serde_json::Value>;

    /// Like `get`, but overrides the `api-version` query parameter.
    fn get_with_api_version<'a>(
        &'a self,
        path: &'a str,
        api_version: &'a str,
    ) -> BoxFuture<'a, serde_json::Value>;
}

/// Simple, reusable retry policy with exponential backoff and jitter.
/// Configurable via envs: `RT_RETRY_MAX`, `RT_BASE_DELAY_MS`.
#[derive(Clone, Default)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
}

impl RetryPolicy {
    /// Default: 3 retries, 100ms base delay.
    pub const DEFAULT_MAX_RETRIES: u32 = 3;
    pub const DEFAULT_BASE_DELAY_MS: u64 = 100;

    /// Build a RetryPolicy from environment variables:
    /// - `RT_RETRY_MAX` (default 3)
    /// - `RT_BASE_DELAY_MS` (default 100)
    pub fn from_env() -> Self {
        let max = std::env::var("RT_RETRY_MAX")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(Self::DEFAULT_MAX_RETRIES)
            .max(1);
        let ms = std::env::var("RT_BASE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(Self::DEFAULT_BASE_DELAY_MS)
            .max(1);
        Self {
            max_retries: max,
            base_delay: Duration::from_millis(ms),
        }
    }

    /// Retry an async operation with exponential backoff and jitter.
    /// The provided closure is invoked up to `max_retries + 1` times.
    pub async fn retry_async<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut attempt = 0u32;
        loop {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    attempt += 1;
                    if attempt > self.max_retries {
                        return Err(e);
                    }
                    let max_delay = self.base_delay * (1u32 << attempt);
                    let max_ms = max_delay.as_millis() as u64;
                    let jitter = rand::random::<u64>() % (max_ms + 1);
                    let delay = Duration::from_millis(jitter.min(max_ms));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

/// Production retry adapter: wraps a `RetryPolicy`.
pub struct ProductionRetry {
    retry: RetryPolicy,
}

impl ProductionRetry {
    pub fn new(retry: RetryPolicy) -> Self {
        Self { retry }
    }

    /// Build from environment variables.
    pub fn from_env() -> Self {
        Self::new(RetryPolicy::from_env())
    }
}

impl RetryStrategy for ProductionRetry {
    fn retry_policy(&self) -> &RetryPolicy {
        &self.retry
    }
}

/// Production on-prem fallback: direct HTTP GET that bypasses the SDK.
/// Holds the on-prem context (endpoint, PAT, API versions) needed to
/// build direct HTTP requests.
pub struct ProductionFallback {
    endpoint: String,
    pat: String,
    api_versions: Vec<String>,
}

impl ProductionFallback {
    pub fn new(
        endpoint: impl Into<String>,
        pat: impl Into<String>,
        api_versions: Vec<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            pat: pat.into(),
            api_versions,
        }
    }

    /// Build the URL for an on-prem fallback request, joining endpoint, path,
    /// and api-version with the correct query separator (`?` or `&`).
    fn on_prem_url(endpoint: &str, path: &str, api: &str) -> String {
        let sep = if path.contains('?') { '&' } else { '?' };
        format!(
            "{}/{}{}api-version={}",
            endpoint.trim_end_matches('/'),
            path,
            sep,
            api
        )
    }
}

impl OnPremFallback for ProductionFallback {
    fn get<'a>(&'a self, path: &'a str) -> BoxFuture<'a, serde_json::Value> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .build()
                .map_err(|e| AzureError::Http(crate::http_error::HttpError::transport(e)))?;
            let mut errors: Vec<String> = Vec::new();
            for api in self.api_versions.iter() {
                let url_s = Self::on_prem_url(&self.endpoint, path, api);
                let auth_variants = [format!(":{}", self.pat), format!("user:{}", self.pat)];
                for auth_val in auth_variants.iter() {
                    let auth = format!(
                        "Basic {}",
                        base64::engine::general_purpose::STANDARD.encode(auth_val)
                    );
                    let resp_res = client
                        .get(&url_s)
                        .header(reqwest::header::AUTHORIZATION, auth)
                        .header(
                            reqwest::header::USER_AGENT,
                            "azsdk-rust-devops_rust_api/0.32.0",
                        )
                        .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
                        .send()
                        .await;
                    match resp_res {
                        Err(e) => {
                            errors.push(format!("{} {} -> request error: {}", api, auth_val, e));
                            continue;
                        }
                        Ok(resp) => {
                            let status = resp.status();
                            let body_text = resp.text().await.unwrap_or_default();
                            if !status.is_success() {
                                errors.push(format!(
                                    "{} {} -> {}: {}",
                                    api, auth_val, status, body_text
                                ));
                                continue;
                            }
                            match serde_json::from_str::<serde_json::Value>(&body_text) {
                                Ok(v) => return Ok(v),
                                Err(e) => {
                                    errors.push(format!(
                                        "{} {} -> decode failed: {} | body: {}",
                                        api, auth_val, e, body_text
                                    ));
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            Err(AzureError::Http(crate::http_error::HttpError::fallback(
                errors,
            )))
        })
    }

    fn get_with_api_version<'a>(
        &'a self,
        path: &'a str,
        api_version: &'a str,
    ) -> BoxFuture<'a, serde_json::Value> {
        Box::pin(async move {
            let client = reqwest::Client::builder()
                .build()
                .map_err(|e| AzureError::Http(crate::http_error::HttpError::transport(e)))?;
            let url_s = Self::on_prem_url(&self.endpoint, path, api_version);
            let auth_variants = [format!(":{}", self.pat), format!("user:{}", self.pat)];
            let mut errors: Vec<String> = Vec::new();
            for auth_val in auth_variants.iter() {
                let auth = format!(
                    "Basic {}",
                    base64::engine::general_purpose::STANDARD.encode(auth_val)
                );
                let resp_res = client
                    .get(&url_s)
                    .header(reqwest::header::AUTHORIZATION, auth)
                    .header(
                        reqwest::header::USER_AGENT,
                        "azsdk-rust-devops_rust_api/0.32.0",
                    )
                    .header(reqwest::header::ACCEPT, "application/json; charset=utf-8")
                    .send()
                    .await;
                match resp_res {
                    Err(e) => {
                        errors.push(format!("{} -> request error: {}", auth_val, e));
                        continue;
                    }
                    Ok(resp) => {
                        let status = resp.status();
                        let body_text = resp.text().await.unwrap_or_default();
                        if !status.is_success() {
                            errors.push(format!("{} -> {}: {}", auth_val, status, body_text));
                            continue;
                        }
                        match serde_json::from_str::<serde_json::Value>(&body_text) {
                            Ok(v) => return Ok(v),
                            Err(e) => {
                                errors.push(format!(
                                    "{} -> decode failed: {} | body: {}",
                                    auth_val, e, body_text
                                ));
                                continue;
                            }
                        }
                    }
                }
            }
            Err(AzureError::Http(crate::http_error::HttpError::fallback(
                errors,
            )))
        })
    }
}

/// No-op retry for tests: zero retries, immediate fail.
pub struct NoOpRetry;

impl RetryStrategy for NoOpRetry {
    fn retry_policy(&self) -> &RetryPolicy {
        const NOOP: RetryPolicy = RetryPolicy {
            max_retries: 0,
            base_delay: Duration::ZERO,
        };
        &NOOP
    }
}

/// No-op fallback for tests: always returns `AzureError::NotImplemented`.
pub struct NoOpFallback;

impl OnPremFallback for NoOpFallback {
    fn get<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, serde_json::Value> {
        Box::pin(async { Err(AzureError::NotImplemented) })
    }
    fn get_with_api_version<'a>(
        &'a self,
        _path: &'a str,
        _api_version: &'a str,
    ) -> BoxFuture<'a, serde_json::Value> {
        Box::pin(async { Err(AzureError::NotImplemented) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn retry_succeeds_after_retries() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
        };
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let result = policy
            .retry_async(move || {
                let c = c.clone();
                async move {
                    let prev = c.fetch_add(1, Ordering::SeqCst) + 1;
                    if prev < 3 { Err("fail") } else { Ok(42) }
                }
            })
            .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn retry_gives_up() {
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
        };
        let calls = Arc::new(AtomicU32::new(0));
        let c = calls.clone();
        let res = policy
            .retry_async(move || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err::<(), &str>("permanent")
                }
            })
            .await;
        assert!(res.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn on_prem_url_without_query_params_appends_api_version_with_question_mark() {
        let url = ProductionFallback::on_prem_url(
            "https://tfs.example.com/tfs",
            "_apis/wit/workitems/123",
            "6.0",
        );
        assert_eq!(
            url,
            "https://tfs.example.com/tfs/_apis/wit/workitems/123?api-version=6.0"
        );
    }

    #[test]
    fn on_prem_url_with_query_params_appends_api_version_with_ampersand() {
        let url = ProductionFallback::on_prem_url(
            "https://tfs.example.com/tfs",
            "_apis/wit/workitems/123?fields=System.Id,System.Tags",
            "6.0",
        );
        assert_eq!(
            url,
            "https://tfs.example.com/tfs/_apis/wit/workitems/123?fields=System.Id,System.Tags&api-version=6.0"
        );
    }

    #[test]
    fn on_prem_url_with_multiple_query_params_appends_api_version_with_ampersand() {
        let url = ProductionFallback::on_prem_url(
            "https://tfs.example.com/tfs",
            "_apis/wit/workitems/123?fields=System.Id&expand=relations",
            "7.0",
        );
        assert_eq!(
            url,
            "https://tfs.example.com/tfs/_apis/wit/workitems/123?fields=System.Id&expand=relations&api-version=7.0"
        );
    }

    #[test]
    fn on_prem_url_trims_trailing_slash_from_endpoint() {
        let url = ProductionFallback::on_prem_url(
            "https://tfs.example.com/tfs/",
            "_apis/projects",
            "6.0",
        );
        assert_eq!(
            url,
            "https://tfs.example.com/tfs/_apis/projects?api-version=6.0"
        );
    }

    #[test]
    fn noop_retry_policy_is_zero() {
        let r = NoOpRetry.retry_policy();
        assert_eq!(r.max_retries, 0);
    }

    #[tokio::test]
    async fn noop_fallback_returns_not_implemented() {
        let result = NoOpFallback.get("_apis/projects").await;
        assert!(matches!(result, Err(AzureError::NotImplemented)));
    }

    #[test]
    fn retry_from_env_uses_defaults_when_no_env_vars() {
        let policy = RetryPolicy::from_env();
        assert_eq!(policy.max_retries, RetryPolicy::DEFAULT_MAX_RETRIES);
        assert_eq!(
            policy.base_delay,
            Duration::from_millis(RetryPolicy::DEFAULT_BASE_DELAY_MS)
        );
    }

    #[test]
    fn retry_from_env_uses_env_vars() {
        unsafe {
            std::env::set_var("RT_RETRY_MAX", "5");
            std::env::set_var("RT_BASE_DELAY_MS", "200");
        }
        let policy = RetryPolicy::from_env();
        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.base_delay, Duration::from_millis(200));
        unsafe {
            std::env::remove_var("RT_RETRY_MAX");
            std::env::remove_var("RT_BASE_DELAY_MS");
        }
    }

    #[test]
    fn production_retry_new_stores_values() {
        let p = ProductionRetry::new(RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(50),
        });
        assert_eq!(p.retry.max_retries, 5);
        assert_eq!(p.retry.base_delay, Duration::from_millis(50));
    }

    #[test]
    fn production_retry_implements_retry_strategy() {
        let p = ProductionRetry::new(RetryPolicy {
            max_retries: 2,
            base_delay: Duration::ZERO,
        });
        let strategy: &dyn RetryStrategy = &p;
        let policy = strategy.retry_policy();
        assert_eq!(policy.max_retries, 2);
    }

    #[test]
    fn retry_policy_has_min_one() {
        unsafe {
            std::env::set_var("RT_RETRY_MAX", "0");
            std::env::set_var("RT_BASE_DELAY_MS", "0");
        }
        let policy = RetryPolicy::from_env();
        assert_eq!(policy.max_retries, 1);
        assert_eq!(policy.base_delay, Duration::from_millis(1));
        unsafe {
            std::env::remove_var("RT_RETRY_MAX");
            std::env::remove_var("RT_BASE_DELAY_MS");
        }
    }
}
