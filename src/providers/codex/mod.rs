// SPDX-License-Identifier: MPL-2.0

mod account;
mod login;
mod oauth;
mod opencode_import;
mod refresh;
#[cfg(test)]
mod tests;

use crate::account_storage::{
    AccountStorageError, ProviderAccountMetadata, ProviderAccountStorage, ProviderAccountTokens,
};
use crate::auth::{CodexAuth, account_id_from_id_token};
use crate::error::{CodexError, Result};
use crate::model::{
    ProviderCost, ProviderId, ProviderIdentity, UsageHeadline, UsageSnapshot, UsageWindow,
};
use chrono::{DateTime, Duration, Utc};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use account::{CodexAccount, apply_login_account, discover_accounts, sync_managed_accounts};
#[cfg(test)]
pub use login::CodexLoginSuccess;
pub use login::{CodexLoginEvent, CodexLoginState, CodexLoginStatus, prepare};
pub use opencode_import::{is_available as opencode_import_available, prepare_opencode_import};

use crate::config::ManagedCodexAccountConfig;

pub(crate) fn system_active_account_id(
    managed_accounts: &[ManagedCodexAccountConfig],
    auth_path: &Path,
) -> Option<String> {
    let content = std::fs::read_to_string(auth_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let tokens = &json["tokens"];
    let active_account_id = tokens["id_token"]
        .as_str()
        .and_then(account_id_from_id_token)
        .or_else(|| {
            tokens["access_token"]
                .as_str()
                .and_then(account_id_from_id_token)
        })?;
    managed_accounts.iter().find_map(|account| {
        if account.provider_account_id.as_deref() == Some(active_account_id.as_str()) {
            Some(account.id.clone())
        } else {
            None
        }
    })
}

const ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";
const REFRESH_BEFORE_EXPIRY: Duration = Duration::minutes(5);
const SESSION_WINDOW_SECONDS: i64 = 5 * 60 * 60;
const WEEKLY_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
const WINDOW_DURATION_TOLERANCE_SECONDS: u64 = 60;

pub async fn fetch(
    client: &reqwest::Client,
    account_id: &str,
    account_dir: PathBuf,
) -> Result<UsageSnapshot, CodexError> {
    fetch_at(
        client,
        account_id,
        account_dir,
        ENDPOINT,
        refresh::TOKEN_ENDPOINT,
    )
    .await
}

async fn fetch_at(
    client: &reqwest::Client,
    account_id: &str,
    account_dir: PathBuf,
    usage_endpoint: &str,
    token_endpoint: &str,
) -> Result<UsageSnapshot, CodexError> {
    let root = account_dir
        .parent()
        .ok_or_else(|| CodexError::AccountStorage("invalid account directory".to_string()))?;
    let storage = ProviderAccountStorage::new(root);
    let metadata = storage
        .load_metadata(account_id)
        .map_err(map_account_storage_error)?;
    let mut tokens = storage
        .load_tokens(account_id)
        .map_err(map_account_storage_error)?;

    drop_shared_refresh_token(&storage, account_id, &metadata, &mut tokens);

    if tokens.expires_at <= Utc::now() + REFRESH_BEFORE_EXPIRY {
        match renew_tokens(
            client,
            &storage,
            account_id,
            &metadata,
            &tokens,
            token_endpoint,
        )
        .await
        {
            Ok(renewed) => tokens = renewed,
            // A borrowed token OpenCode hasn't replaced yet is still good until it
            // really expires (or is refused below); keep using it.
            Err(CodexError::HostSignInExpired) if tokens.expires_at > Utc::now() => {}
            Err(error) => return Err(error),
        }
    }

    let auth = auth_from_storage(&metadata, &tokens);
    match fetch_oauth_at(client, &auth, usage_endpoint).await {
        Ok(snapshot) => {
            let _ = storage.save_snapshot(account_id, &snapshot);
            if let Some(metadata) = refreshed_metadata(account_id, &snapshot, &storage) {
                let _ = storage.save_metadata(account_id, &metadata);
            }
            Ok(snapshot)
        }
        Err(error) => {
            if should_refresh_on(&error) {
                tokens = renew_tokens(
                    client,
                    &storage,
                    account_id,
                    &metadata,
                    &tokens,
                    token_endpoint,
                )
                .await?;
                let refreshed_auth = auth_from_storage(&metadata, &tokens);
                let snapshot = fetch_oauth_at(client, &refreshed_auth, usage_endpoint).await?;
                let _ = storage.save_snapshot(account_id, &snapshot);
                if let Some(metadata) = refreshed_metadata(account_id, &snapshot, &storage) {
                    let _ = storage.save_metadata(account_id, &metadata);
                }
                return Ok(snapshot);
            }
            Err(error)
        }
    }
}

fn map_account_storage_error(error: AccountStorageError) -> CodexError {
    if error.is_missing() {
        CodexError::CredentialsMissing
    } else {
        CodexError::AccountStorage(error.to_string())
    }
}

/// An account signed in through Ledger refreshes with its own refresh token. An
/// account imported from OpenCode has none on purpose (refreshing a copy would
/// rotate it and sign OpenCode out), so it takes OpenCode's current token instead.
async fn renew_tokens(
    client: &reqwest::Client,
    storage: &ProviderAccountStorage,
    account_id: &str,
    metadata: &ProviderAccountMetadata,
    current: &ProviderAccountTokens,
    token_endpoint: &str,
) -> Result<ProviderAccountTokens, CodexError> {
    if current.refresh_token.is_empty() {
        return adopt_opencode_token(storage, account_id, metadata, current);
    }
    refresh_tokens(client, storage, account_id, current, token_endpoint).await
}

/// Accounts imported from OpenCode before 0.6.0 kept a copy of its refresh token.
/// While that copy is still OpenCode's current one, refreshing it would sign
/// OpenCode out, so it is dropped and the account borrows OpenCode's token instead.
fn drop_shared_refresh_token(
    storage: &ProviderAccountStorage,
    account_id: &str,
    metadata: &ProviderAccountMetadata,
    tokens: &mut ProviderAccountTokens,
) {
    if tokens.refresh_token.is_empty() {
        return;
    }
    let Some(crate::providers::opencode_auth::OpenCodeCredential::OAuth {
        access,
        refresh,
        account_id: opencode_account,
        ..
    }) = crate::providers::opencode_auth::discover("openai")
    else {
        return;
    };
    let opencode_account =
        opencode_account.or_else(|| crate::auth::account_id_from_id_token(&access));
    let same_account = matches!(
        (opencode_account.as_deref(), metadata.provider_account_id.as_deref()),
        (Some(theirs), Some(ours)) if theirs == ours
    );
    if !same_account || refresh != tokens.refresh_token {
        return;
    }
    tokens.refresh_token.clear();
    if let Err(error) = storage.save_tokens(account_id, tokens) {
        // The cleared copy is held in memory and retried by `load_tokens`.
        tracing::warn!(account_id, %error, "could not save after dropping a copied Codex refresh token");
    }
    tracing::info!(account_id, "dropped a refresh token shared with OpenCode");
}

/// Reads OpenCode's `auth.json` (never writes it) and takes its Codex access token,
/// only when it belongs to the same ChatGPT account and differs from the one held.
fn adopt_opencode_token(
    storage: &ProviderAccountStorage,
    account_id: &str,
    metadata: &ProviderAccountMetadata,
    current: &ProviderAccountTokens,
) -> Result<ProviderAccountTokens, CodexError> {
    let Some(crate::providers::opencode_auth::OpenCodeCredential::OAuth {
        access,
        expires,
        account_id: opencode_account,
        ..
    }) = crate::providers::opencode_auth::discover("openai")
    else {
        return Err(CodexError::HostSignInExpired);
    };
    let opencode_account =
        opencode_account.or_else(|| crate::auth::account_id_from_id_token(&access));
    let same_account = matches!(
        (opencode_account.as_deref(), metadata.provider_account_id.as_deref()),
        (Some(theirs), Some(ours)) if theirs == ours
    );
    let expires_at = DateTime::<Utc>::from_timestamp_millis(expires)
        .filter(|expires_at| *expires_at > Utc::now());
    let (true, Some(expires_at)) = (same_account && access != current.access_token, expires_at)
    else {
        return Err(CodexError::HostSignInExpired);
    };
    let tokens = ProviderAccountTokens {
        access_token: access,
        refresh_token: String::new(),
        expires_at,
        scope: current.scope.clone(),
        token_id: current.token_id.clone(),
    };
    storage
        .save_tokens(account_id, &tokens)
        .map_err(|error| CodexError::AccountStorage(error.to_string()))?;
    Ok(tokens)
}

async fn refresh_tokens(
    client: &reqwest::Client,
    storage: &ProviderAccountStorage,
    account_id: &str,
    current: &ProviderAccountTokens,
    token_endpoint: &str,
) -> Result<ProviderAccountTokens, CodexError> {
    let refresh_token = (!current.refresh_token.is_empty())
        .then_some(current.refresh_token.as_str())
        .ok_or(CodexError::RefreshUnavailable)?;
    let refreshed = refresh::refresh_access_token_at(client, token_endpoint, refresh_token).await?;
    let tokens = ProviderAccountTokens {
        access_token: refreshed.access_token,
        refresh_token: refreshed
            .refresh_token
            .unwrap_or_else(|| current.refresh_token.clone()),
        expires_at: refreshed.expires_at,
        scope: current.scope.clone(),
        token_id: current.token_id.clone(),
    };
    storage
        .save_tokens(account_id, &tokens)
        .map_err(|error| CodexError::AccountStorage(error.to_string()))?;
    Ok(tokens)
}

pub(crate) async fn fetch_oauth(
    client: &reqwest::Client,
    auth: &CodexAuth,
) -> Result<UsageSnapshot, CodexError> {
    fetch_oauth_at(client, auth, ENDPOINT).await
}

pub(super) async fn fetch_oauth_at(
    client: &reqwest::Client,
    auth: &CodexAuth,
    endpoint: &str,
) -> Result<UsageSnapshot, CodexError> {
    let mut headers = HeaderMap::new();
    let bearer = format!("Bearer {}", auth.access_token);
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&bearer).map_err(CodexError::InvalidBearerHeader)?,
    );
    if let Some(account_id) = &auth.account_id {
        headers.insert(
            "ChatGPT-Account-Id",
            HeaderValue::from_str(account_id).map_err(CodexError::InvalidAccountIdHeader)?,
        );
    }
    let response = client
        .get(endpoint)
        .headers(headers)
        .send()
        .await
        .map_err(CodexError::UsageRequest)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(CodexError::Unauthorized);
    }
    let status = response.status();
    if !status.is_success() {
        return Err(CodexError::UsageHttp {
            status: status.as_u16(),
            details: String::new(),
        });
    }
    let body = response.text().await.map_err(CodexError::UsageRequest)?;
    let body_len = body.len();
    let payload: CodexUsageResponse = serde_json::from_str(&body).map_err(|error| {
        tracing::warn!(body_len, error = %error, "failed to decode codex usage response");
        CodexError::DecodeUsageJson(error)
    })?;
    normalize_oauth(payload)
}

fn auth_from_storage(
    metadata: &ProviderAccountMetadata,
    tokens: &ProviderAccountTokens,
) -> CodexAuth {
    CodexAuth {
        access_token: tokens.access_token.clone(),
        account_id: metadata.provider_account_id.clone(),
        refresh_token: Some(tokens.refresh_token.clone()).filter(|token| !token.is_empty()),
        id_token: None,
        expires_at: Some(tokens.expires_at),
    }
}

fn refreshed_metadata(
    account_id: &str,
    snapshot: &UsageSnapshot,
    storage: &ProviderAccountStorage,
) -> Option<ProviderAccountMetadata> {
    if snapshot.identity.email.is_none() && snapshot.identity.account_id.is_none() {
        return None;
    }
    let mut metadata = storage.load_metadata(account_id).ok()?;
    if let Some(email) = snapshot
        .identity
        .email
        .as_deref()
        .filter(|email| !email.is_empty())
    {
        metadata.email = email.to_ascii_lowercase();
    }
    if snapshot.identity.account_id.is_some() {
        metadata
            .provider_account_id
            .clone_from(&snapshot.identity.account_id);
    }
    metadata.updated_at = Utc::now();
    Some(metadata)
}

fn should_refresh_on(error: &CodexError) -> bool {
    match error {
        CodexError::Unauthorized => true,
        CodexError::UsageHttp { status, .. } => *status == 401 || *status == 403,
        _ => false,
    }
}

#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub plan_type: Option<String>,
    pub rate_limit: Option<CodexRateLimit>,
    pub credits: Option<CodexCredits>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimit {
    pub primary_window: Option<CodexWindow>,
    pub secondary_window: Option<CodexWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexWindow {
    pub used_percent: f32,
    pub limit_window_seconds: Option<i64>,
    pub reset_at: i64,
}

#[derive(Debug, Deserialize)]
struct CodexCredits {
    pub balance: Option<BalanceValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BalanceValue {
    Str(String),
    Num(f64),
}

impl BalanceValue {
    fn as_f64_str(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Num(n) => n.to_string(),
        }
    }
}

fn normalize_oauth(payload: CodexUsageResponse) -> Result<UsageSnapshot, CodexError> {
    let mut windows = Vec::new();
    if let Some(w) = payload
        .rate_limit
        .as_ref()
        .and_then(|r| r.primary_window.as_ref())
    {
        windows.push(normalize_window(
            codex_window_label(w.limit_window_seconds, "Session"),
            w.used_percent,
            w.reset_at,
            w.limit_window_seconds,
        ));
    }
    if let Some(w) = payload
        .rate_limit
        .as_ref()
        .and_then(|r| r.secondary_window.as_ref())
    {
        windows.push(normalize_window(
            codex_window_label(w.limit_window_seconds, "Weekly"),
            w.used_percent,
            w.reset_at,
            w.limit_window_seconds,
        ));
    }

    let provider_cost = payload.credits.as_ref().and_then(|c| {
        let raw = c.balance.as_ref()?.as_f64_str();
        match raw.parse::<f64>() {
            Ok(used) => Some(ProviderCost {
                used,
                limit: None,
                units: "credits".to_string(),
            }),
            Err(source) => {
                tracing::warn!(
                    balance = %raw,
                    error = %CodexError::InvalidCreditBalance { balance: raw.clone(), source },
                    "failed to parse codex credit balance"
                );
                None
            }
        }
    });

    if windows.is_empty() && provider_cost.is_none() {
        return Err(CodexError::NoUsageData);
    }

    Ok(UsageSnapshot {
        provider: ProviderId::Codex,
        source: "OAuth".to_string(),
        updated_at: Utc::now(),
        headline: UsageHeadline::first_available(&windows),
        windows,
        provider_cost,
        extra_usage: None,
        identity: ProviderIdentity {
            email: payload.email,
            account_id: payload.account_id,
            plan: payload.plan_type,
            display_name: None,
        },
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    })
}

fn codex_window_label(limit_window_seconds: Option<i64>, fallback: &'static str) -> &'static str {
    match limit_window_seconds {
        Some(seconds)
            if seconds.abs_diff(SESSION_WINDOW_SECONDS) <= WINDOW_DURATION_TOLERANCE_SECONDS =>
        {
            "Session"
        }
        Some(seconds)
            if seconds.abs_diff(WEEKLY_WINDOW_SECONDS) <= WINDOW_DURATION_TOLERANCE_SECONDS =>
        {
            "Weekly"
        }
        _ => fallback,
    }
}

fn normalize_window(
    label: &str,
    used_percent: f32,
    reset_at_epoch: i64,
    window_seconds: Option<i64>,
) -> UsageWindow {
    UsageWindow {
        label: label.to_string(),
        used_percent,
        reset_at: DateTime::from_timestamp(reset_at_epoch, 0),
        window_seconds,
        reset_description: None,
        group: None,
    }
}
