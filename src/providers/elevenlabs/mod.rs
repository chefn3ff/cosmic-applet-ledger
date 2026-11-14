// SPDX-License-Identifier: MPL-2.0

pub mod account;
pub mod login;
pub mod quota;
pub mod storage;

use crate::config::{Config, ManagedElevenLabsAccountConfig};
use crate::error::ElevenLabsError;
use crate::model::UsageSnapshot;
use storage::normalize_api_key;

pub use account::discover_accounts;
pub use login::{ElevenLabsLoginEvent, ElevenLabsLoginState, ElevenLabsLoginStatus};

const ELEVENLABS_API_URL: &str = "https://api.elevenlabs.io/v1/user/subscription";
const ELEVENLABS_STATS_URL: &str = "https://api.elevenlabs.io/v1/usage/character-stats";

pub fn sync_managed_accounts(config: &mut Config) -> bool {
    let original_len = config.elevenlabs_managed_accounts.len();
    let discovered = discover_accounts(config);
    config.elevenlabs_managed_accounts.retain(|account| {
        discovered
            .iter()
            .any(|discovered_account| discovered_account.id == account.id)
    });
    config.elevenlabs_managed_accounts.len() != original_len
}

pub async fn fetch(
    client: &reqwest::Client,
    account: &ManagedElevenLabsAccountConfig,
) -> Result<UsageSnapshot, ElevenLabsError> {
    fetch_with_endpoint(client, account, ELEVENLABS_API_URL).await
}

pub(crate) async fn fetch_with_endpoint(
    _client: &reqwest::Client,
    account: &ManagedElevenLabsAccountConfig,
    endpoint: &str,
) -> Result<UsageSnapshot, ElevenLabsError> {
    let client = crate::runtime::http_client_without_redirects();
    let stored_key =
        storage::load_api_key(&account.id).map_err(|_| ElevenLabsError::LoginRequired)?;
    let api_key = normalize_api_key(&stored_key).map_err(|error| {
        if error == "API key is required" {
            ElevenLabsError::LoginRequired
        } else {
            ElevenLabsError::InvalidApiKey
        }
    })?;

    let response = send_request(&client, endpoint, &api_key).await?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let body = response.text().await.unwrap_or_default();
        return Err(auth_error_from_body(&body));
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        return Err(ElevenLabsError::RateLimited { retry_after_secs });
    }
    if status.is_server_error() || !status.is_success() {
        return Err(ElevenLabsError::UsageHttp {
            status: status.as_u16(),
        });
    }

    let updated_at = chrono::Utc::now();
    let body = response
        .text()
        .await
        .map_err(ElevenLabsError::UsageRequest)?;

    let mut snapshot = quota::parse(&body, updated_at)?;

    // Per-key breakdown is optional: it needs workspace metrics access, and
    // the Credits row is still right without it.
    if let Some(start) = quota::period_start(&body) {
        let stats_url = format!(
            "{ELEVENLABS_STATS_URL}?start_unix={}&end_unix={}&include_workspace_metrics=true&breakdown_type=all_api_keys&metric=credits&aggregation_interval=day",
            start.timestamp_millis(),
            updated_at.timestamp_millis(),
        );
        if let Ok(stats) = send_request(&client, &stats_url, &api_key).await
            && stats.status().is_success()
            && let Ok(stats_body) = stats.text().await
        {
            quota::apply_key_usage(&mut snapshot, &stats_body);
        }
    }

    Ok(snapshot)
}

/// ElevenLabs answers 401 for a key without the User: Read permission too,
/// with `{"detail":{"status":"missing_permissions","message":...}}`. That key
/// is valid, so it gets its own message instead of "login required".
fn auth_error_from_body(body: &str) -> ElevenLabsError {
    let detail = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.get("detail").cloned());
    let status = detail
        .as_ref()
        .and_then(|detail| detail.get("status"))
        .and_then(serde_json::Value::as_str);
    if status == Some("missing_permissions") {
        let message = detail
            .as_ref()
            .and_then(|detail| detail.get("message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        tracing::warn!(
            elevenlabs_message = message,
            "ElevenLabs API key is missing a permission"
        );
        ElevenLabsError::MissingPermission
    } else {
        ElevenLabsError::LoginRequired
    }
}

async fn send_request(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
) -> Result<reqwest::Response, ElevenLabsError> {
    // ElevenLabs authenticates with a bare `xi-api-key` header, not Bearer.
    let mut api_key_header = reqwest::header::HeaderValue::from_str(api_key)
        .map_err(|_| ElevenLabsError::InvalidApiKey)?;
    api_key_header.set_sensitive(true);

    client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .header("xi-api-key", api_key_header)
        .header(reqwest::header::USER_AGENT, "cosmic-applet-ledger")
        .send()
        .await
        .map_err(ElevenLabsError::UsageRequest)
}

#[cfg(test)]
mod tests {
    use super::auth_error_from_body;
    use crate::error::ElevenLabsError;

    #[test]
    fn missing_permission_body_gets_its_own_error() {
        let body = r#"{"detail":{"status":"missing_permissions","message":"The API key you used is missing the permission user_read to execute this operation."}}"#;
        assert!(matches!(
            auth_error_from_body(body),
            ElevenLabsError::MissingPermission
        ));
    }

    #[test]
    fn other_auth_failures_still_ask_for_login() {
        for body in [
            r#"{"detail":{"status":"invalid_api_key","message":"Invalid API key"}}"#,
            "",
            "not json",
        ] {
            assert!(matches!(
                auth_error_from_body(body),
                ElevenLabsError::LoginRequired
            ));
        }
    }
}
