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
        return Err(ElevenLabsError::LoginRequired);
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
