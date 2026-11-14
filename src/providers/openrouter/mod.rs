// SPDX-License-Identifier: MPL-2.0

pub mod account;
pub mod login;
pub mod quota;
pub mod storage;

use crate::config::{Config, ManagedOpenRouterAccountConfig};
use crate::error::OpenRouterError;
use crate::model::UsageSnapshot;
use storage::normalize_api_key;

pub use account::discover_accounts;
pub use login::{OpenRouterLoginEvent, OpenRouterLoginState, OpenRouterLoginStatus};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/key";
const OPENROUTER_CREDITS_URL: &str = "https://openrouter.ai/api/v1/credits";
const OPENROUTER_KEYS_URL: &str = "https://openrouter.ai/api/v1/keys";
const OPENROUTER_ANALYTICS_URL: &str = "https://openrouter.ai/api/v1/analytics/query";
const OPENROUTER_ACTIVITY_URL: &str = "https://openrouter.ai/api/v1/activity?group_by=workspace";

pub fn sync_managed_accounts(config: &mut Config) -> bool {
    let original_len = config.openrouter_managed_accounts.len();
    let discovered = discover_accounts(config);
    config.openrouter_managed_accounts.retain(|account| {
        discovered
            .iter()
            .any(|discovered_account| discovered_account.id == account.id)
    });
    config.openrouter_managed_accounts.len() != original_len
}

pub async fn fetch(
    client: &reqwest::Client,
    account: &ManagedOpenRouterAccountConfig,
) -> Result<UsageSnapshot, OpenRouterError> {
    fetch_with_endpoint(client, account, OPENROUTER_API_URL).await
}

pub(crate) async fn fetch_with_endpoint(
    _client: &reqwest::Client,
    account: &ManagedOpenRouterAccountConfig,
    endpoint: &str,
) -> Result<UsageSnapshot, OpenRouterError> {
    let client = crate::runtime::http_client_without_redirects();
    let stored_key =
        storage::load_api_key(&account.id).map_err(|_| OpenRouterError::LoginRequired)?;
    let api_key = normalize_api_key(&stored_key).map_err(|error| {
        if error == "API key is required" {
            OpenRouterError::LoginRequired
        } else {
            OpenRouterError::InvalidApiKey
        }
    })?;

    let response = send_request(&client, endpoint, &api_key).await?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(OpenRouterError::LoginRequired);
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok());
        return Err(OpenRouterError::RateLimited { retry_after_secs });
    }
    if status.is_server_error() || !status.is_success() {
        return Err(OpenRouterError::UsageHttp {
            status: status.as_u16(),
        });
    }

    let updated_at = chrono::Utc::now();
    let body = response
        .text()
        .await
        .map_err(OpenRouterError::UsageRequest)?;

    // Fetch the key list for management keys (their own usage is always zero)
    let keys_body = if quota::is_management_key(&body) {
        optional_body(&client, OPENROUTER_KEYS_URL, &api_key).await
    } else {
        None
    };

    // Fetch credits endpoint optionally (returns total_credits and total_usage)
    let credits_body = match send_request(&client, OPENROUTER_CREDITS_URL, &api_key).await {
        Ok(c_res) => {
            let status = c_res.status();
            let text = c_res.text().await.unwrap_or_default();
            if status.is_success() {
                Some(text)
            } else {
                None
            }
        }
        Err(_) => None,
    };

    // Fetch activity endpoint optionally (returns tokens grouped by date & model)
    let activity_body = match send_request(&client, OPENROUTER_ACTIVITY_URL, &api_key).await {
        Ok(a_res) => {
            let status = a_res.status();
            let text = a_res.text().await.unwrap_or_default();
            if status.is_success() {
                Some(text)
            } else {
                None
            }
        }
        Err(_) => None,
    };

    let mut snapshot = quota::parse(
        &body,
        credits_body.as_deref(),
        activity_body.as_deref(),
        keys_body.as_deref(),
        updated_at,
    )?;

    // `/activity` stops at the last completed UTC day; the analytics query
    // (management keys only) also covers today.
    if keys_body.is_some() {
        let today = chrono::Local::now().date_naive();
        let start = (today - chrono::Duration::days(6))
            .and_hms_opt(0, 0, 0)
            .and_then(|midnight| midnight.and_local_timezone(chrono::Local).earliest())
            .map_or(updated_at - chrono::Duration::days(7), |dt| {
                dt.with_timezone(&chrono::Utc)
            });
        let time_range = serde_json::json!({
            "start": start.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "end": updated_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        });
        let hourly_query = serde_json::json!({
            "metrics": ["tokens_total"],
            "granularity": "hour",
            "time_range": time_range,
            "limit": 1000,
        });
        let model_query = serde_json::json!({
            "metrics": ["tokens_total"],
            "dimensions": ["model"],
            "time_range": time_range,
        });
        let hourly_body = optional_analytics(&client, &api_key, &hourly_query).await;
        let model_body = optional_analytics(&client, &api_key, &model_query).await;
        quota::apply_analytics(
            &mut snapshot,
            hourly_body.as_deref(),
            model_body.as_deref(),
            today,
        );
    }

    Ok(snapshot)
}

async fn optional_analytics(
    client: &reqwest::Client,
    api_key: &str,
    query: &serde_json::Value,
) -> Option<String> {
    let response = client
        .post(OPENROUTER_ANALYTICS_URL)
        .header(reqwest::header::ACCEPT, "application/json")
        .bearer_auth(api_key)
        .header(reqwest::header::USER_AGENT, "cosmic-applet-ledger")
        .json(query)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

async fn optional_body(client: &reqwest::Client, endpoint: &str, api_key: &str) -> Option<String> {
    let response = send_request(client, endpoint, api_key).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

async fn send_request(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
) -> Result<reqwest::Response, OpenRouterError> {
    let authorization = format!("Bearer {api_key}");
    let mut authorization = reqwest::header::HeaderValue::from_str(&authorization)
        .map_err(|_| OpenRouterError::InvalidApiKey)?;
    authorization.set_sensitive(true);

    client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, authorization)
        .header(reqwest::header::USER_AGENT, "cosmic-applet-ledger")
        .send()
        .await
        .map_err(OpenRouterError::UsageRequest)
}
