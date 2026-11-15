// SPDX-License-Identifier: MPL-2.0

#[allow(dead_code)]
pub mod account;
#[allow(dead_code)]
pub mod login;
#[allow(dead_code)]
pub mod oauth;
pub mod usage;

#[cfg(test)]
mod tests;

#[allow(unused_imports, dead_code)]
pub use login::{
    GrokLoginEvent, GrokLoginState, GrokLoginStatus, GrokLoginSuccess, prepare,
    prepare_host_import, prepare_targeted,
};

use std::path::PathBuf;

use chrono::{Duration, Utc};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, RETRY_AFTER};

use crate::account_storage::{AccountStorageError, ProviderAccountStorage, ProviderAccountTokens};
use crate::error::GrokError;
use crate::model::UsageSnapshot;

#[allow(dead_code)]
pub const DEFAULT_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const REFRESH_BEFORE_EXPIRY: Duration = Duration::minutes(5);

#[allow(dead_code)]
pub async fn fetch(
    client: &reqwest::Client,
    account_id: &str,
    account_dir: PathBuf,
) -> Result<UsageSnapshot, GrokError> {
    fetch_at(
        client,
        account_id,
        account_dir,
        DEFAULT_BILLING_URL,
        oauth::TOKEN_URL,
    )
    .await
}

#[allow(dead_code)]
pub async fn fetch_at(
    client: &reqwest::Client,
    account_id: &str,
    account_dir: PathBuf,
    billing_url: &str,
    token_url: &str,
) -> Result<UsageSnapshot, GrokError> {
    let host_auth = account::host_auth_file_path();
    fetch_with_host(
        client,
        account_id,
        account_dir,
        billing_url,
        token_url,
        host_auth.as_deref(),
    )
    .await
}

/// `host_auth` is the Grok CLI's `auth.json`, read (never written) for accounts
/// imported from the CLI.
pub(crate) async fn fetch_with_host(
    client: &reqwest::Client,
    account_id: &str,
    account_dir: PathBuf,
    billing_url: &str,
    token_url: &str,
    host_auth: Option<&std::path::Path>,
) -> Result<UsageSnapshot, GrokError> {
    let root = account_dir
        .parent()
        .ok_or_else(|| GrokError::AccountStorage("invalid account directory".to_string()))?;
    let storage = ProviderAccountStorage::new(root);
    let metadata = storage
        .load_metadata(account_id)
        .map_err(map_account_storage_error)?;
    let mut tokens = storage
        .load_tokens(account_id)
        .map_err(map_account_storage_error)?;

    let renew = Renewal {
        client,
        storage: &storage,
        account_id,
        metadata: &metadata,
        token_url,
        host_auth,
    };

    renew.drop_shared_refresh_token(&mut tokens);

    if tokens.expires_at <= Utc::now() + REFRESH_BEFORE_EXPIRY {
        match renew.run(&mut tokens).await {
            // A borrowed token the CLI hasn't replaced yet is still good until it
            // really expires (or is refused below); keep using it.
            Err(GrokError::HostSignInExpired) if tokens.expires_at > Utc::now() => {}
            result => result?,
        }
    }

    let mut response = send_billing_request(client, billing_url, &tokens.access_token).await?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        renew.run(&mut tokens).await?;
        response = send_billing_request(client, billing_url, &tokens.access_token).await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(GrokError::Unauthorized);
        }
    }

    let status = response.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after_secs = parse_retry_after(response.headers());
        return Err(GrokError::RateLimited { retry_after_secs });
    }

    let response = response
        .error_for_status()
        .map_err(|source| GrokError::UsageEndpoint {
            status: status.as_u16(),
            source,
        })?;

    let body = response.text().await.map_err(GrokError::UsageRequest)?;
    let claims = oauth::decode_jwt_claims(&tokens.access_token).or_else(|| {
        tokens
            .token_id
            .as_deref()
            .and_then(oauth::decode_jwt_claims)
    });
    let email = (!metadata.email.trim().is_empty())
        .then_some(metadata.email.as_str())
        .or_else(|| {
            claims
                .as_ref()
                .and_then(|c| c.email.as_deref())
                .filter(|s| !s.trim().is_empty())
        });
    let display_name = claims
        .as_ref()
        .and_then(|c| c.name.as_deref())
        .filter(|s| !s.trim().is_empty());
    let provider_account_id = metadata
        .provider_account_id
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| claims.as_ref().and_then(|c| c.sub.as_deref()));

    let snapshot = usage::parse_billing_snapshot(&body, email, display_name, provider_account_id)?;
    let _ = storage.save_snapshot(account_id, &snapshot);
    Ok(snapshot)
}

/// How an account gets a new access token. An account signed in through Ledger
/// refreshes with its own refresh token. An account imported from the Grok CLI
/// has none on purpose: refreshing a copied token would rotate it and sign the
/// CLI out, so it takes the CLI's current token instead and leaves refreshing
/// to the CLI.
struct Renewal<'a> {
    client: &'a reqwest::Client,
    storage: &'a ProviderAccountStorage,
    account_id: &'a str,
    metadata: &'a crate::account_storage::ProviderAccountMetadata,
    token_url: &'a str,
    host_auth: Option<&'a std::path::Path>,
}

impl Renewal<'_> {
    async fn run(&self, tokens: &mut ProviderAccountTokens) -> Result<(), GrokError> {
        if tokens.refresh_token.trim().is_empty() {
            return self.adopt_host_token(tokens);
        }
        refresh_account_tokens(
            self.client,
            self.storage,
            self.account_id,
            tokens,
            self.token_url,
        )
        .await
    }

    /// Accounts imported before 0.6.0 kept a copy of the CLI's refresh token. While
    /// that copy is still the CLI's current one, refreshing it would sign the CLI
    /// out, so it is dropped and the account becomes a borrowing (imported) one.
    fn drop_shared_refresh_token(&self, tokens: &mut ProviderAccountTokens) {
        if tokens.refresh_token.trim().is_empty() {
            return;
        }
        let Some(host) = self.host_auth.and_then(account::read_host_credentials) else {
            return;
        };
        if host.refresh_token.as_deref() != Some(tokens.refresh_token.as_str())
            || !host_is_same_account(&host, self.metadata)
        {
            return;
        }
        tokens.refresh_token.clear();
        if let Err(error) = self.storage.save_tokens(self.account_id, tokens) {
            // The cleared copy is held in memory and retried by `load_tokens`.
            tracing::warn!(account_id = self.account_id, %error, "could not save after dropping a copied Grok refresh token");
        }
        tracing::info!(
            account_id = self.account_id,
            "dropped a refresh token shared with the Grok CLI"
        );
    }

    fn adopt_host_token(&self, tokens: &mut ProviderAccountTokens) -> Result<(), GrokError> {
        let host = self
            .host_auth
            .and_then(account::read_host_credentials)
            .ok_or(GrokError::HostSignInExpired)?;
        if !host_is_same_account(&host, self.metadata) || host.access_token == tokens.access_token {
            return Err(GrokError::HostSignInExpired);
        }
        let claims = oauth::decode_jwt_claims(&host.access_token);
        tokens.access_token = host.access_token;
        tokens.expires_at = host
            .expires_at
            .or_else(|| {
                claims
                    .and_then(|c| c.exp)
                    .and_then(|exp| chrono::DateTime::from_timestamp(exp, 0))
            })
            .unwrap_or_else(|| Utc::now() + Duration::hours(1));
        // Only the access token is kept; the CLI's refresh token is never copied.
        tokens.refresh_token.clear();
        self.storage
            .save_tokens(self.account_id, tokens)
            .map_err(map_account_storage_error)
    }
}

/// The CLI's file can hold a different account than the one imported, after
/// `grok login` as someone else. Its token is used only for the same user.
fn host_is_same_account(
    host: &account::HostGrokCredentials,
    metadata: &crate::account_storage::ProviderAccountMetadata,
) -> bool {
    let host_user = host
        .user_id
        .clone()
        .or_else(|| oauth::decode_jwt_claims(&host.access_token).and_then(|c| c.sub));
    match (host_user, metadata.provider_account_id.as_deref()) {
        (Some(host_user), Some(expected)) => host_user == expected,
        _ => {
            let host_email = host.email.as_deref().unwrap_or_default().trim();
            !host_email.is_empty() && host_email.eq_ignore_ascii_case(metadata.email.trim())
        }
    }
}

async fn refresh_account_tokens(
    client: &reqwest::Client,
    storage: &ProviderAccountStorage,
    account_id: &str,
    tokens: &mut ProviderAccountTokens,
    token_url: &str,
) -> Result<(), GrokError> {
    if tokens.refresh_token.trim().is_empty() {
        return Err(GrokError::RefreshUnavailable);
    }
    let refreshed = oauth::refresh_token(client, &tokens.refresh_token, token_url).await?;
    tokens.access_token = refreshed.access_token;
    tokens.refresh_token = refreshed.refresh_token;
    tokens.expires_at = refreshed.expires_at;
    if let Some(scope_str) = refreshed.scope {
        tokens.scope = scope_str.split_whitespace().map(String::from).collect();
    }
    if let Some(id_token) = refreshed.id_token {
        tokens.token_id = Some(id_token);
    }
    storage
        .save_tokens(account_id, tokens)
        .map_err(map_account_storage_error)?;
    Ok(())
}

async fn send_billing_request(
    client: &reqwest::Client,
    billing_url: &str,
    access_token: &str,
) -> Result<reqwest::Response, GrokError> {
    let bearer = format!("Bearer {access_token}");
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&bearer).map_err(GrokError::InvalidBearerHeader)?,
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    client
        .get(billing_url)
        .headers(headers)
        .send()
        .await
        .map_err(GrokError::UsageRequest)
}

fn parse_retry_after(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

fn map_account_storage_error(error: AccountStorageError) -> GrokError {
    if error.is_missing() {
        GrokError::CredentialsMissing
    } else {
        GrokError::AccountStorage(error.to_string())
    }
}
