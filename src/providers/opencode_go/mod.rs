// SPDX-License-Identifier: MPL-2.0

pub mod account;
pub mod login;
pub mod opencode;
pub mod storage;

use crate::config::{Config, ManagedOpenCodeGoAccountConfig};
use crate::error::OpenCodeGoError;
use crate::model::{ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot, UsageWindow};
use chrono::{DateTime, Utc};
use serde::Deserialize;

pub use storage::load_api_key;

const OPENCODE_GO_API_URL: &str = "https://opencode.ai/zen/go/v1/usage";
pub(crate) const OPENCODE_API_KEY_ENV: &str = "OPENCODE_API_KEY";
pub(crate) const OPENCODE_GO_API_KEY_ENV: &str = "OPENCODE_GO_API_KEY";

#[derive(Debug, Deserialize)]
struct OpenCodeGoUsageResponse {
    usage: Option<OpenCodeGoUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeGoUsage {
    rolling: Option<OpenCodeGoUsageWindow>,
    weekly: Option<OpenCodeGoUsageWindow>,
    monthly: Option<OpenCodeGoUsageWindow>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeGoUsageWindow {
    percent: Option<f64>,
    #[serde(rename = "resetsAt")]
    resets_at: Option<String>,
}

pub fn sync_managed_accounts(config: &mut Config) -> bool {
    let original_len = config.opencode_go_managed_accounts.len();
    config.opencode_go_managed_accounts.retain(|account| {
        account.api_key_source.starts_with("env:") || !account.api_key_source.is_empty()
    });
    config.opencode_go_managed_accounts.len() != original_len
}

pub async fn fetch(
    client: &reqwest::Client,
    account: &ManagedOpenCodeGoAccountConfig,
) -> Result<UsageSnapshot, OpenCodeGoError> {
    let api_key = load_api_key(&account.id)
        .ok()
        .filter(|key| !key.is_empty())
        .or_else(|| environment_api_key().map(|(key, _)| key))
        .ok_or(OpenCodeGoError::LoginRequired)?;

    fetch_at(client, &api_key, OPENCODE_GO_API_URL).await
}

pub(crate) fn environment_api_key() -> Option<(String, &'static str)> {
    [
        (OPENCODE_API_KEY_ENV, "env:OPENCODE_API_KEY"),
        (OPENCODE_GO_API_KEY_ENV, "env:OPENCODE_GO_API_KEY"),
    ]
    .into_iter()
    .find_map(|(name, source)| {
        std::env::var(name)
            .ok()
            .filter(|key| !key.is_empty())
            .map(|key| (key, source))
    })
}

async fn fetch_at(
    client: &reqwest::Client,
    api_key: &str,
    endpoint: &str,
) -> Result<UsageSnapshot, OpenCodeGoError> {
    let response = client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(OpenCodeGoError::UsageRequest)?;

    match response.status() {
        reqwest::StatusCode::UNAUTHORIZED => return Err(OpenCodeGoError::LoginRequired),
        reqwest::StatusCode::FORBIDDEN => return Err(OpenCodeGoError::EntitlementRequired),
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            let retry_after_secs = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok());
            return Err(OpenCodeGoError::RateLimited { retry_after_secs });
        }
        status if status.is_server_error() => {
            return Err(OpenCodeGoError::UsageHttp {
                status: status.as_u16(),
            });
        }
        _ => {}
    }

    let response = response
        .error_for_status()
        .map_err(OpenCodeGoError::UsageEndpoint)?;
    let body = response
        .text()
        .await
        .map_err(OpenCodeGoError::UsageEndpoint)?;
    parse(&body, Utc::now())
}

pub fn parse(body: &str, updated_at: DateTime<Utc>) -> Result<UsageSnapshot, OpenCodeGoError> {
    let response: OpenCodeGoUsageResponse =
        serde_json::from_str(body).map_err(OpenCodeGoError::DecodeUsage)?;
    let mut windows = Vec::new();
    if let Some(usage) = response.usage {
        push_window(&mut windows, "5 Hour", usage.rolling, Some(5 * 3600));
        push_window(&mut windows, "Weekly", usage.weekly, Some(7 * 24 * 3600));
        push_window(&mut windows, "Monthly", usage.monthly, Some(30 * 24 * 3600));
    }
    if windows.is_empty() {
        return Err(OpenCodeGoError::NoUsageData);
    }

    Ok(UsageSnapshot {
        provider: ProviderId::OpenCodeGo,
        source: "API Key".to_string(),
        updated_at,
        headline: UsageHeadline(0),
        windows,
        provider_cost: None,
        extra_usage: None,
        identity: ProviderIdentity::default(),
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    })
}

fn push_window(
    windows: &mut Vec<UsageWindow>,
    label: &str,
    window: Option<OpenCodeGoUsageWindow>,
    window_seconds: Option<i64>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(percent) = window.percent else {
        return;
    };
    let reset_at = window
        .resets_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    windows.push(UsageWindow {
        label: label.to_string(),
        used_percent: percent as f32,
        reset_description: reset_at.map(|value| value.to_rfc3339()),
        reset_at,
        window_seconds,
        group: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const UPDATED_AT: &str = "2026-08-25T12:00:00Z";

    fn updated_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(UPDATED_AT)
            .unwrap()
            .with_timezone(&Utc)
    }

    async fn server(
        status: u16,
        retry_after: Option<u64>,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let retry_after = retry_after
            .map(|value| format!("retry-after: {value}\r\n"))
            .unwrap_or_default();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 4096];
            let received = stream.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..received]).to_string();
            let response = format!(
                "HTTP/1.1 {status} status\r\ncontent-type: application/json\r\n{retry_after}content-length: 2\r\nconnection: close\r\n\r\n{{}}"
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (format!("http://{address}/usage"), handle)
    }

    #[tokio::test]
    async fn fetch_maps_unauthorized_to_login_required() {
        let (endpoint, handle) = server(401, None).await;

        let error = fetch_at(&reqwest::Client::new(), "test-key", &endpoint)
            .await
            .unwrap_err();

        assert!(matches!(error, OpenCodeGoError::LoginRequired));
        assert!(error.requires_user_action());
        assert!(
            handle
                .await
                .unwrap()
                .contains("authorization: Bearer test-key\r\n")
        );
    }

    #[tokio::test]
    async fn fetch_maps_forbidden_to_entitlement_without_reauthentication() {
        let (endpoint, _handle) = server(403, None).await;

        let error = fetch_at(&reqwest::Client::new(), "test-key", &endpoint)
            .await
            .unwrap_err();

        assert!(matches!(error, OpenCodeGoError::EntitlementRequired));
        assert!(!error.requires_user_action());
        assert!(!error.is_transient());
    }

    #[tokio::test]
    async fn fetch_maps_rate_limit_retry_after() {
        let (endpoint, _handle) = server(429, Some(42)).await;

        let error = fetch_at(&reqwest::Client::new(), "test-key", &endpoint)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            OpenCodeGoError::RateLimited {
                retry_after_secs: Some(42)
            }
        ));
        assert!(!error.requires_user_action());
        assert!(error.is_transient());
    }

    #[test]
    fn parses_all_usage_windows() {
        let snapshot = parse(
            r#"{
                "usage": {
                    "rolling": {"status":"active","percent":45.5,"resetsAt":"2026-08-25T18:00:00Z"},
                    "weekly": {"status":"active","percent":30.0,"resetsAt":"2026-08-29T00:00:00Z"},
                    "monthly": {"status":"active","percent":15.0,"resetsAt":"2026-09-01T00:00:00Z"}
                }
            }"#,
            updated_at(),
        )
        .unwrap();

        assert_eq!(snapshot.provider, ProviderId::OpenCodeGo);
        assert_eq!(snapshot.windows.len(), 3);
        assert_eq!(snapshot.windows[0].label, "5 Hour");
        assert_eq!(snapshot.windows[1].label, "Weekly");
        assert_eq!(snapshot.windows[2].label, "Monthly");
    }

    #[test]
    fn parses_percentages_and_reset_timestamps() {
        let snapshot = parse(
            r#"{"usage":{"rolling":{"percent":45.5,"resetsAt":"2026-08-25T18:00:00Z"}}}"#,
            updated_at(),
        )
        .unwrap();
        let window = &snapshot.windows[0];
        assert!((window.used_percent - 45.5).abs() < 0.001);
        assert_eq!(window.window_seconds, Some(5 * 3600));
        assert_eq!(
            window.reset_at.unwrap().to_rfc3339(),
            "2026-08-25T18:00:00+00:00"
        );
        assert_eq!(
            window.reset_description.as_deref(),
            Some("2026-08-25T18:00:00+00:00")
        );
    }

    #[test]
    fn parses_rate_limited_status() {
        let snapshot = parse(
            r#"{"usage":{"weekly":{"status":"rate-limited","percent":80.25}}}"#,
            updated_at(),
        )
        .unwrap();
        assert!((snapshot.windows[0].used_percent - 80.25).abs() < 0.001);
    }

    #[test]
    fn parses_missing_individual_windows() {
        let snapshot = parse(
            r#"{"usage":{"rolling":{"percent":10},"weekly":{"percent":20}}}"#,
            updated_at(),
        )
        .unwrap();
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].label, "5 Hour");
        assert_eq!(snapshot.windows[1].label, "Weekly");
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(
            parse("not-json", updated_at()),
            Err(OpenCodeGoError::DecodeUsage(_))
        ));
    }

    #[test]
    fn rejects_empty_usage_object() {
        assert!(matches!(
            parse(r#"{"usage":{}}"#, updated_at()),
            Err(OpenCodeGoError::NoUsageData)
        ));
    }

    #[test]
    fn parses_valid_data_for_every_window() {
        let snapshot = parse(
            r#"{"usage":{"rolling":{"percent":1},"weekly":{"percent":2},"monthly":{"percent":3}}}"#,
            updated_at(),
        )
        .unwrap();
        assert_eq!(snapshot.windows[0].window_seconds, Some(18_000));
        assert_eq!(snapshot.windows[1].window_seconds, Some(604_800));
        assert_eq!(snapshot.windows[2].window_seconds, Some(2_592_000));
    }

    #[test]
    fn sync_managed_accounts_filters_empty_sources() {
        let now = Utc::now();
        let mut config = Config {
            opencode_go_managed_accounts: vec![
                ManagedOpenCodeGoAccountConfig {
                    id: "go-1".to_string(),
                    label: "Configured".to_string(),
                    api_key_source: "stored".to_string(),
                    created_at: now,
                    updated_at: now,
                    last_authenticated_at: None,
                },
                ManagedOpenCodeGoAccountConfig {
                    id: "go-2".to_string(),
                    label: "Empty".to_string(),
                    api_key_source: String::new(),
                    created_at: now,
                    updated_at: now,
                    last_authenticated_at: None,
                },
            ],
            ..Config::default()
        };
        assert!(sync_managed_accounts(&mut config));
        assert_eq!(config.opencode_go_managed_accounts.len(), 1);
    }
}
