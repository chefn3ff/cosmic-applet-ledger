use super::{account_add_action, prepare_account_facts};
use crate::config::Config;
use crate::model::ProviderId;
use crate::providers::interface::{
    ProviderAccountAction, ProviderAccountAddAction, ProviderAccountStatusKind,
};

#[test]
fn cursor_account_facts_preserve_scan_account_contract() {
    use crate::account_storage::{
        NewProviderAccount, ProviderAccountStorage, ProviderAccountTokens,
    };
    use crate::config::{ManagedCursorAccountConfig, paths};
    use crate::model::{AppState, AuthState, ProviderAccountRuntimeState, ProviderHealth};
    use chrono::Utc;

    let mut env = crate::test_support::test_env();
    let root = tempfile::tempdir().unwrap();
    env.set("XDG_STATE_HOME", root.path());

    let now = Utc::now();
    let storage = ProviderAccountStorage::new(paths().cursor_accounts_dir.clone());
    storage
        .replace_account(
            "cursor-1".to_string(),
            NewProviderAccount {
                provider: ProviderId::Cursor,
                email: "cursor@example.com".to_string(),
                provider_account_id: None,
                organization_id: None,
                organization_name: None,
                tokens: ProviderAccountTokens {
                    access_token: "access".to_string(),
                    refresh_token: "refresh".to_string(),
                    expires_at: now,
                    scope: Vec::new(),
                    token_id: Some("cursor-user".to_string()),
                },
                snapshot: None,
            },
        )
        .unwrap();

    let config = Config {
        cursor_managed_accounts: vec![ManagedCursorAccountConfig {
            id: "cursor-1".to_string(),
            email: "cursor@example.com".to_string(),
            label: "Cursor account".to_string(),
            account_root: paths().cursor_accounts_dir.join("cursor-1"),
            display_name: None,
            plan: None,
            created_at: now,
            updated_at: now,
            last_authenticated_at: None,
        }],
        ..Config::default()
    };
    let mut state = AppState::empty();
    let mut account = ProviderAccountRuntimeState::empty(
        ProviderId::Cursor,
        "cursor-managed:cursor-1",
        "Cursor account",
    );
    account.health = ProviderHealth::Error;
    account.auth_state = AuthState::ActionRequired;
    state.upsert_account(account);

    assert_eq!(
        account_add_action(ProviderId::Cursor),
        ProviderAccountAddAction::Scan
    );
    let facts = prepare_account_facts(ProviderId::Cursor, &config, &state);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].label, "cursor@example.com");
    assert_eq!(
        facts[0].actions,
        vec![ProviderAccountAction::Delete, ProviderAccountAction::Rescan]
    );
    let status = facts[0].status.as_ref().unwrap();
    assert_eq!(status.kind, ProviderAccountStatusKind::Neutral);
    assert_eq!(status.badge_text, "Re-auth needed");
    assert_eq!(status.tooltip_text, "This account needs re-authentication");
    assert!(status.reauth_eligible);
    assert!(status.style_as_action_required);
    assert_eq!(facts[0].reauthenticate_tooltip, "Rescan Cursor account");
}

#[test]
fn prepared_account_facts_keep_claude_label_and_status_policy() {
    use crate::config::ManagedClaudeAccountConfig;
    use crate::model::{AppState, AuthState, ProviderAccountRuntimeState, ProviderHealth};
    use chrono::Utc;
    use std::path::PathBuf;

    let _env = crate::test_support::test_env();
    let now = Utc::now();
    let config = Config {
        claude_managed_accounts: vec![ManagedClaudeAccountConfig {
            id: "claude-1".to_string(),
            label: "Claude account".to_string(),
            config_dir: PathBuf::from("/tmp/claude-1"),
            email: Some("config@example.com".to_string()),
            organization: None,
            subscription_type: None,
            created_at: now,
            updated_at: now,
            last_authenticated_at: None,
        }],
        ..Config::default()
    };
    let mut state = AppState::empty();
    let mut account =
        ProviderAccountRuntimeState::empty(ProviderId::Claude, "claude-1", "Claude account");
    account.health = ProviderHealth::Error;
    state.upsert_account(account.clone());

    let facts = prepare_account_facts(ProviderId::Claude, &config, &state);
    assert_eq!(facts[0].label, "config@example.com");
    assert_eq!(
        facts[0].status.as_ref().unwrap().kind,
        ProviderAccountStatusKind::Warning
    );
    assert!(facts[0].status.as_ref().unwrap().reauth_eligible);

    account.auth_state = AuthState::Ready;
    account.snapshot = Some(crate::model::UsageSnapshot {
        provider: ProviderId::Claude,
        source: "test".to_string(),
        updated_at: now,
        headline: crate::model::UsageHeadline(0),
        windows: Vec::new(),
        provider_cost: None,
        extra_usage: None,
        identity: crate::model::ProviderIdentity {
            email: Some("snapshot@example.com".to_string()),
            ..crate::model::ProviderIdentity::default()
        },
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    });
    state.upsert_account(account);

    let facts = prepare_account_facts(ProviderId::Claude, &config, &state);
    assert_eq!(facts[0].label, "snapshot@example.com");
    assert_eq!(
        facts[0].status.as_ref().unwrap().kind,
        ProviderAccountStatusKind::Warning
    );
    assert!(!facts[0].status.as_ref().unwrap().reauth_eligible);
}
