// SPDX-License-Identifier: MPL-2.0

use crate::config::{Config, ManagedFishAccountConfig};
use crate::key_authentication::{
    KeyAuthenticationAdapter, KeyAuthenticationTarget, prepare as prepare_key_authentication,
    prepare_for_reauth as prepare_key_authentication_for_reauth,
};
use crate::providers::fish::storage;
use chrono::{DateTime, Utc};

pub use crate::key_authentication::{
    KeyAuthenticationEvent as FishLoginEvent, KeyAuthenticationState as FishLoginState,
    KeyAuthenticationStatus as FishLoginStatus,
};

pub(crate) struct FishKeyAuthentication;

impl KeyAuthenticationAdapter for FishKeyAuthentication {
    type Account = ManagedFishAccountConfig;

    fn new_account_id() -> String {
        format!(
            "fish-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        )
    }

    fn find_account(config: &Config, account_id: &str) -> Result<KeyAuthenticationTarget, String> {
        config
            .fish_managed_accounts
            .iter()
            .find(|account| account.id == account_id)
            .map(|account| KeyAuthenticationTarget {
                id: account.id.clone(),
                label: account.label.clone(),
                created_at: account.created_at,
            })
            .ok_or_else(|| "Fish Audio account not found".to_string())
    }

    fn discover_api_key() -> Option<String> {
        None
    }

    fn empty_key_error() -> String {
        "API key is required".to_string()
    }

    fn validate(
        config: &Config,
        account_id: &str,
        label: &str,
        api_key: &str,
    ) -> Result<(), String> {
        let api_key = storage::normalize_api_key(api_key)?;
        if label.trim().is_empty() {
            return Err("Account name is required".to_string());
        }
        if config
            .fish_managed_accounts
            .iter()
            .any(|account| account.id != account_id && account.label == label)
        {
            return Err("An account with this name already exists".to_string());
        }
        if config.fish_managed_accounts.iter().any(|account| {
            account.id != account_id
                && storage::load_api_key(&account.id)
                    .ok()
                    .and_then(|stored_key| storage::normalize_api_key(&stored_key).ok())
                    .is_some_and(|stored_key| stored_key == api_key)
        }) {
            return Err("An account with this API key already exists".to_string());
        }
        Ok(())
    }

    fn build_account(
        account_id: String,
        label: String,
        created_at: DateTime<Utc>,
        authenticated_at: DateTime<Utc>,
    ) -> Self::Account {
        ManagedFishAccountConfig {
            id: account_id,
            label,
            api_key_source: "stored".to_string(),
            created_at,
            updated_at: authenticated_at,
            last_authenticated_at: Some(authenticated_at),
        }
    }

    fn persist(account: &Self::Account, api_key: &str) -> Result<(), String> {
        storage::write_api_key(&account.id, api_key)
            .map_err(|error| format!("Failed to save Fish Audio API key: {error}"))
    }
}

pub fn prepare() -> FishLoginState {
    prepare_key_authentication::<FishKeyAuthentication>()
}

pub fn prepare_for_reauth(config: Config, account_id: &str) -> Result<FishLoginState, String> {
    prepare_key_authentication_for_reauth::<FishKeyAuthentication>(&config, account_id)
}

pub(crate) fn save(
    config: &Config,
    state: &mut FishLoginState,
) -> Result<ManagedFishAccountConfig, String> {
    state.save::<FishKeyAuthentication>(config)
}
