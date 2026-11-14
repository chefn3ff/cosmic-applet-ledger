// SPDX-License-Identifier: MPL-2.0

pub mod account;
pub mod login;
pub mod quota;
pub mod storage;

use crate::config::{Config, ManagedFishAccountConfig};
use crate::error::FishError;
use crate::model::UsageSnapshot;
use storage::normalize_api_key;

pub use account::discover_accounts;
pub use login::{FishLoginEvent, FishLoginState, FishLoginStatus};

/// Pay-as-you-go API credit, the balance that audio API calls spend. This is
/// the only endpoint read: no package or subscription call.
const FISH_API_URL: &str = "https://api.fish.audio/wallet/self/api-credit";

pub fn sync_managed_accounts(config: &mut Config) -> bool {
    let original_len = config.fish_managed_accounts.len();
    let discovered = discover_accounts(config);
    config.fish_managed_accounts.retain(|account| {
        discovered
            .iter()
            .any(|discovered_account| discovered_account.id == account.id)
    });
    config.fish_managed_accounts.len() != original_len
}

pub async fn fetch(
    client: &reqwest::Client,
    account: &ManagedFishAccountConfig,
) -> Result<UsageSnapshot, FishError> {
    fetch_with_endpoint(client, account, FISH_API_URL).await
}

pub(crate) async fn fetch_with_endpoint(
    _client: &reqwest::Client,
    account: &ManagedFishAccountConfig,
    endpoint: &str,
) -> Result<UsageSnapshot, FishError> {
    let client = crate::runtime::http_client_without_redirects();
    let stored_key = storage::load_api_key(&account.id).map_err(|_| FishError::LoginRequired)?;
    let api_key = normalize_api_key(&stored_key).map_err(|error| {
        if error == "API key is required" {
            FishError::LoginRequired
        } else {
            FishError::InvalidApiKey
        }
    })?;

    let response = send_request(&client, endpoint, &api_key).await?;
    let status = response.status();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok());
    if let Some(error) = error_for_status(status, retry_after) {
        return Err(error);
    }

    let updated_at = chrono::Utc::now();
    let body = response.text().await.map_err(FishError::UsageRequest)?;
    quota::parse(&body, updated_at)
}

/// Maps a non-success status to its error: 401 and 403 mean the key is bad,
/// 429 is a rate limit (Retry-After is parsed when sent; Fish documents none).
fn error_for_status(status: reqwest::StatusCode, retry_after: Option<&str>) -> Option<FishError> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Some(FishError::LoginRequired);
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Some(FishError::RateLimited {
            retry_after_secs: retry_after.and_then(|value| value.trim().parse().ok()),
        });
    }
    if !status.is_success() {
        return Some(FishError::UsageHttp {
            status: status.as_u16(),
        });
    }
    None
}

async fn send_request(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
) -> Result<reqwest::Response, FishError> {
    let mut authorization = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|_| FishError::InvalidApiKey)?;
    authorization.set_sensitive(true);

    client
        .get(endpoint)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, authorization)
        .header(reqwest::header::USER_AGENT, "cosmic-applet-ledger")
        .send()
        .await
        .map_err(FishError::UsageRequest)
}

#[cfg(test)]
mod tests {
    use super::error_for_status;
    use crate::error::{AppError, FishError};
    use reqwest::StatusCode;

    #[test]
    fn unauthorized_and_forbidden_ask_for_login() {
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let error = error_for_status(status, None).expect("auth failure is an error");
            assert!(matches!(error, FishError::LoginRequired));
            let app_error = AppError::from(error);
            assert!(app_error.requires_user_action());
            assert!(!app_error.is_transient());
        }
    }

    #[test]
    fn rate_limit_is_transient_and_carries_retry_after() {
        let error =
            error_for_status(StatusCode::TOO_MANY_REQUESTS, Some("30")).expect("429 is an error");
        let app_error = AppError::from(error);
        assert_eq!(app_error.rate_limit_retry_after_secs(), Some(30));
        assert!(app_error.is_transient());
        assert!(!app_error.requires_user_action());
    }

    #[test]
    fn rate_limit_without_a_usable_retry_after_still_retries() {
        for header in [None, Some("soon"), Some("")] {
            let error =
                error_for_status(StatusCode::TOO_MANY_REQUESTS, header).expect("429 is an error");
            assert!(matches!(
                error,
                FishError::RateLimited {
                    retry_after_secs: None
                }
            ));
        }
    }

    #[test]
    fn other_failures_are_http_errors() {
        assert!(matches!(
            error_for_status(StatusCode::INTERNAL_SERVER_ERROR, None),
            Some(FishError::UsageHttp { status: 500 })
        ));
        assert!(matches!(
            error_for_status(StatusCode::BAD_REQUEST, None),
            Some(FishError::UsageHttp { status: 400 })
        ));
        assert!(error_for_status(StatusCode::OK, None).is_none());
    }
}
