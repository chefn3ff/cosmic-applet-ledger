// SPDX-License-Identifier: MPL-2.0

use crate::account_storage::validated_account_dir;
use crate::config::{Config, ManagedFishAccountConfig, paths};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishAccount {
    pub id: String,
    pub label: String,
    pub config_dir: PathBuf,
}

pub fn discover_accounts(config: &Config) -> Vec<FishAccount> {
    config
        .fish_managed_accounts
        .iter()
        .filter_map(|managed| {
            let config_dir = validated_account_dir(&paths().fish_accounts_dir, &managed.id).ok()?;
            Some(FishAccount {
                id: managed.id.clone(),
                label: managed.label.clone(),
                config_dir,
            })
        })
        .collect()
}

pub fn apply_login_account(config: &mut Config, account: ManagedFishAccountConfig) {
    let account_id = account.id.clone();
    config
        .fish_managed_accounts
        .retain(|existing| existing.id != account_id);
    config.fish_managed_accounts.push(account);
}
