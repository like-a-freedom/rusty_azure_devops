use std::sync::Arc;

use async_trait::async_trait;
use azure_core::Error;
use azure_core::error::ErrorKind;
use azure_core::http::policies::{Policy, PolicyResult};
use azure_core::http::{Context, Request};

/// Intercepts SDK HTTP requests and rewrites `api-version=7.1-preview` to the
/// configured target version. This is needed because `azure_devops_rust_api`
/// hardcodes `7.1-preview` in all generated request builders, but on-prem
/// Azure DevOps Server instances only support up to `6.0`.
///
/// The policy is only used when `base_contains_collection` is true (on-prem).
/// Cloud requests keep the default `7.1-preview`.
#[derive(Debug, Clone)]
pub(crate) struct DowngradeApiVersionPolicy {
    target_version: String,
}

impl DowngradeApiVersionPolicy {
    pub fn new(target_version: impl Into<String>) -> Self {
        Self {
            target_version: target_version.into(),
        }
    }
}

#[async_trait]
impl Policy for DowngradeApiVersionPolicy {
    async fn send(
        &self,
        ctx: &Context,
        request: &mut Request,
        next: &[Arc<dyn Policy>],
    ) -> PolicyResult {
        let first = next.first().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                "DowngradeApiVersionPolicy: no next policy in pipeline",
            )
        })?;

        let pairs: Vec<_> = {
            let url = request.url_mut();
            url.query_pairs()
                .map(|(k, v)| {
                    if k.as_ref() == "api-version" && v.as_ref() == "7.1-preview" {
                        (k.into_owned(), self.target_version.clone())
                    } else {
                        (k.into_owned(), v.into_owned())
                    }
                })
                .collect()
        };

        {
            let mut qp = request.url_mut().query_pairs_mut();
            qp.clear();
            for (k, v) in &pairs {
                qp.append_pair(k, v);
            }
        }

        first.send(ctx, request, &next[1..]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, Clone)]
    struct CaptureNextPolicy {
        url: Arc<Mutex<Option<String>>>,
    }

    impl CaptureNextPolicy {
        fn new() -> (Self, Arc<Mutex<Option<String>>>) {
            let url = Arc::new(Mutex::new(None));
            (Self { url: url.clone() }, url)
        }
    }

    #[async_trait]
    impl Policy for CaptureNextPolicy {
        async fn send(
            &self,
            _ctx: &Context,
            request: &mut Request,
            _next: &[Arc<dyn Policy>],
        ) -> PolicyResult {
            *self.url.lock().unwrap() = Some(request.url().as_str().to_string());
            Err(Error::new(ErrorKind::Other, "captured"))
        }
    }

    fn make_request(raw_url: &str) -> Request {
        let url: url::Url = raw_url.parse().unwrap();
        Request::new(url, azure_core::http::Method::Get)
    }

    #[tokio::test]
    async fn rewrites_7_1_preview_to_target() {
        let policy = DowngradeApiVersionPolicy::new("6.0");
        let (next, captured) = CaptureNextPolicy::new();
        let next_list: Vec<Arc<dyn Policy>> = vec![Arc::new(next)];
        let mut req =
            make_request("https://example.com/_apis/wit/workitems/123?api-version=7.1-preview");

        let _ = policy.send(&Context::new(), &mut req, &next_list).await;
        let url = captured.lock().unwrap().take().unwrap();
        assert!(url.contains("api-version=6.0"), "got: {}", url);
        assert!(!url.contains("7.1-preview"), "got: {}", url);
    }

    #[tokio::test]
    async fn preserves_other_query_params() {
        let policy = DowngradeApiVersionPolicy::new("6.0");
        let (next, captured) = CaptureNextPolicy::new();
        let next_list: Vec<Arc<dyn Policy>> = vec![Arc::new(next)];
        let mut req = make_request(
            "https://example.com/_apis/wit/workitems/123?fields=System.Title,id&api-version=7.1-preview&expand=relations",
        );

        let _ = policy.send(&Context::new(), &mut req, &next_list).await;
        let url = captured.lock().unwrap().take().unwrap();
        assert!(url.contains("fields=System.Title%2Cid"), "got: {}", url);
        assert!(url.contains("api-version=6.0"), "got: {}", url);
        assert!(url.contains("expand=relations"), "got: {}", url);
    }

    #[tokio::test]
    async fn leaves_non_matching_api_version_untouched() {
        let policy = DowngradeApiVersionPolicy::new("6.0");
        let (next, captured) = CaptureNextPolicy::new();
        let next_list: Vec<Arc<dyn Policy>> = vec![Arc::new(next)];
        let mut req = make_request("https://example.com/_apis/wit/workitems/123?api-version=7.0");

        let _ = policy.send(&Context::new(), &mut req, &next_list).await;
        let url = captured.lock().unwrap().take().unwrap();
        assert!(url.contains("api-version=7.0"), "got: {}", url);
    }

    #[tokio::test]
    async fn noop_on_empty_query() {
        let policy = DowngradeApiVersionPolicy::new("6.0");
        let (next, captured) = CaptureNextPolicy::new();
        let next_list: Vec<Arc<dyn Policy>> = vec![Arc::new(next)];
        let mut req = make_request("https://example.com/_apis/wit/workitems/123");

        let _ = policy.send(&Context::new(), &mut req, &next_list).await;
        let url = captured.lock().unwrap().take().unwrap();
        assert!(!url.contains("api-version"), "got: {}", url);
    }

    #[tokio::test]
    async fn errors_when_no_next_policy() {
        let policy = DowngradeApiVersionPolicy::new("6.0");
        let mut req =
            make_request("https://example.com/_apis/wit/workitems/123?api-version=7.1-preview");

        let result = policy.send(&Context::new(), &mut req, &[]).await;
        assert!(result.is_err());
    }
}
