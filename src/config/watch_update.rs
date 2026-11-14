// SPDX-License-Identifier: MPL-2.0

use super::Config;

impl Config {
    /// Applies the fields another Ledger process changed. `keys` are config
    /// field names (the `set_changed!` keys in `config.rs`), so fields are
    /// copied by name through serde instead of a hand-kept list per field;
    /// a new field syncs without touching this file. An empty `keys` means
    /// the whole configuration was replaced.
    pub fn apply_watcher_update(&mut self, update: Self, keys: &[&str]) {
        if keys.is_empty() {
            *self = update;
            return;
        }
        let (Ok(serde_json::Value::Object(mut current)), Ok(serde_json::Value::Object(incoming))) =
            (serde_json::to_value(&*self), serde_json::to_value(&update))
        else {
            tracing::warn!("config watcher update could not be serialized; ignored");
            return;
        };
        let mut changed = false;
        for key in keys {
            if let Some(value) = incoming.get(*key) {
                current.insert((*key).to_string(), value.clone());
                changed = true;
            }
        }
        if !changed {
            return;
        }
        match serde_json::from_value(serde_json::Value::Object(current)) {
            Ok(merged) => *self = merged,
            Err(error) => {
                tracing::warn!(%error, "config watcher update could not be applied; ignored");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ManagedGrokAccountConfig, ManagedKimiAccountConfig, ManagedMinimaxAccountConfig,
        ManagedOpenCodeGoAccountConfig, ManagedZaiAccountConfig, ProviderEnablement,
    };
    use chrono::Utc;

    #[test]
    fn applies_minimax_watcher_keys_without_replacing_unrelated_configuration() {
        let mut config = Config::default();
        let mut update = Config {
            codex_enablement: crate::config::ProviderEnablement::Disabled,
            minimax_enablement: crate::config::ProviderEnablement::Disabled,
            selected_minimax_account_ids: vec!["minimax-2".to_string()],
            minimax_managed_accounts: vec![minimax_account("minimax-2")],
            ..Config::default()
        };
        update.minimax_managed_accounts[0].label = "Second Minimax".to_string();

        config.apply_watcher_update(
            update,
            &[
                "minimax_enablement",
                "selected_minimax_account_ids",
                "minimax_managed_accounts",
            ],
        );

        assert_eq!(
            config.minimax_enablement,
            crate::config::ProviderEnablement::Disabled
        );
        assert_eq!(config.selected_minimax_account_ids, ["minimax-2"]);
        assert_eq!(config.minimax_managed_accounts[0].label, "Second Minimax");
        assert_eq!(
            config.codex_enablement,
            crate::config::ProviderEnablement::Auto
        );
    }

    #[test]
    fn applies_kimi_watcher_keys_without_replacing_unrelated_configuration() {
        let mut config = Config::default();
        let mut update = Config {
            codex_enablement: crate::config::ProviderEnablement::Disabled,
            kimi_enablement: crate::config::ProviderEnablement::Disabled,
            selected_kimi_account_ids: vec!["kimi-2".to_string()],
            kimi_managed_accounts: vec![kimi_account("kimi-2")],
            ..Config::default()
        };
        update.kimi_managed_accounts[0].label = "Second Kimi".to_string();

        config.apply_watcher_update(
            update,
            &[
                "kimi_enablement",
                "selected_kimi_account_ids",
                "kimi_managed_accounts",
            ],
        );

        assert_eq!(
            config.kimi_enablement,
            crate::config::ProviderEnablement::Disabled
        );
        assert_eq!(config.selected_kimi_account_ids, ["kimi-2"]);
        assert_eq!(config.kimi_managed_accounts[0].label, "Second Kimi");
        assert_eq!(
            config.codex_enablement,
            crate::config::ProviderEnablement::Auto
        );
    }

    #[test]
    fn applies_zai_watcher_keys_without_replacing_unrelated_configuration() {
        let mut config = Config::default();
        let now = Utc::now();
        let update = Config {
            codex_enablement: ProviderEnablement::Disabled,
            zai_enablement: ProviderEnablement::Enabled,
            selected_zai_account_ids: vec!["zai-1".to_string()],
            zai_managed_accounts: vec![ManagedZaiAccountConfig {
                id: "zai-1".to_string(),
                label: "Z.AI Coding Plan".to_string(),
                api_key_source: "stored".to_string(),
                created_at: now,
                updated_at: now,
                last_authenticated_at: None,
            }],
            ..Config::default()
        };

        config.apply_watcher_update(
            update,
            &[
                "zai_enablement",
                "selected_zai_account_ids",
                "zai_managed_accounts",
            ],
        );

        assert_eq!(config.zai_enablement, ProviderEnablement::Enabled);
        assert_eq!(config.selected_zai_account_ids, ["zai-1"]);
        assert_eq!(config.zai_managed_accounts[0].id, "zai-1");
        assert_eq!(config.codex_enablement, ProviderEnablement::Auto);
    }

    #[test]
    fn applies_opencode_go_watcher_keys_without_replacing_unrelated_configuration() {
        let mut config = Config::default();
        let now = Utc::now();
        let update = Config {
            codex_enablement: ProviderEnablement::Disabled,
            opencode_go_enablement: ProviderEnablement::Enabled,
            selected_opencode_go_account_ids: vec!["go-1".to_string()],
            opencode_go_managed_accounts: vec![ManagedOpenCodeGoAccountConfig {
                id: "go-1".to_string(),
                label: "OpenCode Go".to_string(),
                api_key_source: "stored".to_string(),
                created_at: now,
                updated_at: now,
                last_authenticated_at: None,
            }],
            ..Config::default()
        };

        config.apply_watcher_update(
            update,
            &[
                "opencode_go_enablement",
                "selected_opencode_go_account_ids",
                "opencode_go_managed_accounts",
            ],
        );

        assert_eq!(config.opencode_go_enablement, ProviderEnablement::Enabled);
        assert_eq!(config.selected_opencode_go_account_ids, ["go-1"]);
        assert_eq!(config.opencode_go_managed_accounts[0].id, "go-1");
        assert_eq!(config.codex_enablement, ProviderEnablement::Auto);
    }

    #[test]
    fn applies_grok_watcher_keys_without_replacing_unrelated_configuration() {
        let mut config = Config::default();
        let now = Utc::now();
        let update = Config {
            grok_enablement: ProviderEnablement::Enabled,
            selected_grok_account_ids: vec!["grok-1".to_string()],
            grok_managed_accounts: vec![ManagedGrokAccountConfig {
                id: "grok-1".to_string(),
                label: "Grok User".to_string(),
                config_dir: std::path::PathBuf::from("/tmp/grok-1"),
                email: Some("grok@example.com".to_string()),
                provider_account_id: Some("user-grok".to_string()),
                team_id: None,
                plan: Some("SuperGrok".to_string()),
                created_at: now,
                updated_at: now,
                last_authenticated_at: None,
            }],
            ..Config::default()
        };

        config.apply_watcher_update(
            update,
            &[
                "grok_enablement",
                "selected_grok_account_ids",
                "grok_managed_accounts",
            ],
        );

        assert_eq!(config.grok_enablement, ProviderEnablement::Enabled);
        assert_eq!(config.selected_grok_account_ids, ["grok-1"]);
        assert_eq!(config.grok_managed_accounts[0].id, "grok-1");
        assert_eq!(config.codex_enablement, ProviderEnablement::Auto);
    }

    #[test]
    fn every_config_field_syncs_by_its_key() {
        let _demo = crate::demo_env::activate_for_test();
        let mut update = Config {
            refresh_interval_seconds: 1800,
            elevenlabs_ring_enabled: false,
            log_level: "debug".to_string(),
            ..Config::default()
        };
        crate::demo_env::apply_config(&mut update);
        for provider in crate::model::ProviderId::ALL {
            *crate::config::provider_enablement_mut(&mut update, provider) =
                ProviderEnablement::Disabled;
        }
        let update_json = serde_json::to_value(&update).unwrap();
        let default_json = serde_json::to_value(Config::default()).unwrap();

        for (key, expected) in update_json.as_object().unwrap() {
            if default_json[key] == *expected {
                continue;
            }
            let mut config = Config::default();
            config.apply_watcher_update(update.clone(), &[key.as_str()]);
            let applied = serde_json::to_value(&config).unwrap();
            assert_eq!(&applied[key], expected, "watcher did not sync `{key}`");
            for (other, value) in applied.as_object().unwrap() {
                if other != key {
                    assert_eq!(value, &default_json[other], "`{key}` changed `{other}`");
                }
            }
        }
    }

    #[test]
    fn openrouter_changes_sync_across_processes() {
        let mut config = Config::default();
        let update = Config {
            openrouter_enablement: ProviderEnablement::Disabled,
            selected_openrouter_account_ids: vec!["openrouter-2".to_string()],
            ..Config::default()
        };

        config.apply_watcher_update(
            update,
            &["openrouter_enablement", "selected_openrouter_account_ids"],
        );

        assert_eq!(config.openrouter_enablement, ProviderEnablement::Disabled);
        assert_eq!(config.selected_openrouter_account_ids, ["openrouter-2"]);
    }

    #[test]
    fn ignores_unknown_keys() {
        let mut config = Config::default();
        let update = Config {
            kimi_enablement: crate::config::ProviderEnablement::Disabled,
            ..Config::default()
        };

        config.apply_watcher_update(update, &["unknown"]);

        assert_eq!(
            config.kimi_enablement,
            crate::config::ProviderEnablement::Auto
        );
    }

    #[test]
    fn empty_keys_replace_the_entire_configuration() {
        let mut config = Config::default();
        let update = Config {
            kimi_enablement: crate::config::ProviderEnablement::Disabled,
            ..Config::default()
        };

        config.apply_watcher_update(update, &[]);

        assert_eq!(
            config.kimi_enablement,
            crate::config::ProviderEnablement::Disabled
        );
    }

    fn minimax_account(id: &str) -> ManagedMinimaxAccountConfig {
        let now = Utc::now();
        ManagedMinimaxAccountConfig {
            id: id.to_string(),
            label: id.to_string(),
            api_key_source: "stored".to_string(),
            created_at: now,
            updated_at: now,
            last_authenticated_at: None,
        }
    }

    fn kimi_account(id: &str) -> ManagedKimiAccountConfig {
        let now = Utc::now();
        ManagedKimiAccountConfig {
            id: id.to_string(),
            label: id.to_string(),
            api_key_source: "stored".to_string(),
            created_at: now,
            updated_at: now,
            last_authenticated_at: None,
        }
    }
}
