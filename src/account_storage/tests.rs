// SPDX-License-Identifier: MPL-2.0

use super::*;
use crate::model::{ProviderIdentity, UsageHeadline};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("yapcap-account-storage-{name}-{nanos}"))
}

fn tokens() -> ProviderAccountTokens {
    ProviderAccountTokens {
        access_token: "access".to_string(),
        refresh_token: "refresh".to_string(),
        expires_at: Utc::now(),
        scope: vec!["user:profile".to_string()],
        token_id: Some("token-1".to_string()),
    }
}

fn snapshot() -> UsageSnapshot {
    UsageSnapshot {
        provider: ProviderId::Claude,
        source: "claude".to_string(),
        updated_at: Utc::now(),
        headline: UsageHeadline(0),
        windows: Vec::new(),
        provider_cost: None,
        extra_usage: None,
        identity: ProviderIdentity {
            email: Some("person@example.com".to_string()),
            account_id: Some("acct-1".to_string()),
            plan: None,
            display_name: None,
        },
        tokens_by_day: Vec::new(),
        tokens_by_model: Vec::new(),
        credits_by_key: Vec::new(),
    }
}

#[test]
fn creates_account_owned_files_without_using_email_in_directory_name() {
    let storage = ProviderAccountStorage::new(test_dir("create"));
    let token_payload = tokens();
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: Some("acct-1".to_string()),
            organization_id: Some("org-1".to_string()),
            organization_name: Some("Example Org".to_string()),
            tokens: token_payload.clone(),
            snapshot: Some(snapshot()),
        })
        .unwrap();

    assert!(stored.account_dir.ends_with(&stored.account_ref.account_id));
    assert!(!stored.account_ref.account_id.contains("person"));
    assert!(!stored.account_ref.account_id.contains("example"));
    assert!(stored.account_dir.join(METADATA_FILE).exists());
    assert!(stored.account_dir.join(TOKENS_FILE).exists());
    assert!(stored.account_dir.join(SNAPSHOT_FILE).exists());
    assert_eq!(
        storage
            .load_metadata(&stored.account_ref.account_id)
            .unwrap(),
        stored.metadata
    );
    assert_eq!(
        storage.load_tokens(&stored.account_ref.account_id).unwrap(),
        token_payload
    );
    assert_eq!(
        storage
            .load_snapshot(&stored.account_ref.account_id)
            .unwrap()
            .unwrap()
            .identity
            .email
            .as_deref(),
        Some("person@example.com")
    );
}

#[test]
fn save_snapshot_keeps_snapshot_in_account_directory() {
    let storage = ProviderAccountStorage::new(test_dir("snapshot"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();
    let snapshot = snapshot();

    storage
        .save_snapshot(&stored.account_ref.account_id, &snapshot)
        .unwrap();

    assert_eq!(
        storage
            .load_snapshot(&stored.account_ref.account_id)
            .unwrap(),
        Some(snapshot)
    );
}

#[test]
fn replace_account_updates_tokens_and_metadata_in_existing_directory() {
    let storage = ProviderAccountStorage::new(test_dir("replace"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: Some("acct-1".to_string()),
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();
    let mut replacement_tokens = tokens();
    replacement_tokens.access_token = "new-access".to_string();

    let replaced = storage
        .replace_account(
            stored.account_ref.account_id.clone(),
            NewProviderAccount {
                provider: ProviderId::Claude,
                email: "person@example.com".to_string(),
                provider_account_id: Some("acct-2".to_string()),
                organization_id: Some("org-2".to_string()),
                organization_name: Some("New Org".to_string()),
                tokens: replacement_tokens.clone(),
                snapshot: None,
            },
        )
        .unwrap();

    assert_eq!(replaced.account_dir, stored.account_dir);
    assert_eq!(replaced.metadata.created_at, stored.metadata.created_at);
    assert_eq!(
        replaced.metadata.provider_account_id.as_deref(),
        Some("acct-2")
    );
    assert_eq!(
        storage
            .load_tokens(&stored.account_ref.account_id)
            .unwrap()
            .access_token,
        "new-access"
    );
}

#[test]
fn account_config_ref_contains_only_provider_and_account_id() {
    let account_ref = ProviderAccountRef {
        provider: ProviderId::Claude,
        account_id: "claude-1".to_string(),
    };

    let serialized = serde_json::to_value(&account_ref).unwrap();

    assert_eq!(
        serialized,
        serde_json::json!({
            "provider": "claude",
            "account_id": "claude-1"
        })
    );
}

#[test]
fn create_account_creates_missing_provider_root() {
    let root = test_dir("missing-root");
    assert!(!root.exists());
    let storage = ProviderAccountStorage::new(&root);

    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();

    assert!(root.is_dir());
    assert!(stored.account_dir.is_dir());
    assert!(stored.account_dir.join(METADATA_FILE).exists());
    assert!(stored.account_dir.join(TOKENS_FILE).exists());
}

#[test]
fn creates_zai_account_ids_with_zai_prefix() {
    let storage = ProviderAccountStorage::new(test_dir("zai-prefix"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Zai,
            email: String::new(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();

    assert!(stored.account_ref.account_id.starts_with("zai-"));
}

#[cfg(unix)]
#[test]
fn managed_account_storage_repairs_directory_and_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let storage = ProviderAccountStorage::new(test_dir("managed-permissions"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: Some(snapshot()),
        })
        .unwrap();

    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&stored.account_dir), 0o700);
    for file_name in [METADATA_FILE, TOKENS_FILE, SNAPSHOT_FILE] {
        assert_eq!(mode(&stored.account_dir.join(file_name)), 0o600);
    }

    fs::set_permissions(&stored.account_dir, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(
        stored.account_dir.join(TOKENS_FILE),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    storage
        .save_tokens(&stored.account_ref.account_id, &tokens())
        .unwrap();

    assert_eq!(mode(&stored.account_dir), 0o700);
    assert_eq!(mode(&stored.account_dir.join(TOKENS_FILE)), 0o600);
}

#[test]
fn save_snapshot_recreates_missing_account_directory() {
    let storage = ProviderAccountStorage::new(test_dir("save-missing-dir"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();

    fs::remove_dir_all(&stored.account_dir).unwrap();
    assert!(!stored.account_dir.exists());

    storage
        .save_snapshot(&stored.account_ref.account_id, &snapshot())
        .unwrap();
    storage
        .save_tokens(&stored.account_ref.account_id, &tokens())
        .unwrap();
    storage
        .save_metadata(&stored.account_ref.account_id, &stored.metadata)
        .unwrap();

    assert!(stored.account_dir.join(SNAPSHOT_FILE).exists());
    assert!(stored.account_dir.join(TOKENS_FILE).exists());
    assert!(stored.account_dir.join(METADATA_FILE).exists());
}

#[test]
fn create_account_returns_clear_error_when_path_component_is_a_file() {
    let parent = test_dir("file-in-path");
    fs::create_dir_all(&parent).unwrap();
    let blocker = parent.join("claude-accounts");
    fs::write(&blocker, "not a dir").unwrap();
    let storage = ProviderAccountStorage::new(&blocker);

    let err = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap_err();

    assert!(matches!(err, AccountStorageError::NotDirectory { path } if path == blocker));
}

#[test]
fn delete_account_removes_account_directory() {
    let storage = ProviderAccountStorage::new(test_dir("delete"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: None,
        })
        .unwrap();

    assert!(
        storage
            .delete_account(&stored.account_ref.account_id)
            .unwrap()
    );

    assert!(!stored.account_dir.exists());
    assert!(
        !storage
            .delete_account(&stored.account_ref.account_id)
            .unwrap()
    );
}

#[test]
fn rejects_invalid_account_ids_without_touching_outside_paths() {
    let root = test_dir("invalid-id-root");
    let outside = root
        .parent()
        .unwrap()
        .join("yapcap-account-storage-outside");
    fs::write(&outside, "unchanged").unwrap();
    let storage = ProviderAccountStorage::new(&root);

    for account_id in [
        "",
        "../yapcap-account-storage-outside",
        "/tmp/account",
        "a/b",
        "a\\b",
        ".",
        "..",
    ] {
        assert!(matches!(
            storage.account_dir(account_id),
            Err(AccountStorageError::InvalidAccountId)
        ));
        assert!(matches!(
            storage.load_tokens(account_id),
            Err(AccountStorageError::InvalidAccountId)
        ));
        assert!(matches!(
            storage.delete_account(account_id),
            Err(AccountStorageError::InvalidAccountId)
        ));
        assert!(matches!(
            storage.replace_account(
                account_id.to_string(),
                NewProviderAccount {
                    provider: ProviderId::Claude,
                    email: "person@example.com".to_string(),
                    provider_account_id: None,
                    organization_id: None,
                    organization_name: None,
                    tokens: tokens(),
                    snapshot: None,
                },
            ),
            Err(AccountStorageError::InvalidAccountId)
        ));
    }

    assert_eq!(fs::read_to_string(outside).unwrap(), "unchanged");
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_account_directories_without_modifying_the_target() {
    use std::os::unix::fs::symlink;

    let root = test_dir("symlink-root");
    let outside = test_dir("symlink-outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("marker"), "unchanged").unwrap();
    fs::create_dir_all(&root).unwrap();
    symlink(&outside, root.join("account")).unwrap();
    let storage = ProviderAccountStorage::new(&root);

    assert!(matches!(
        storage.load_metadata("account"),
        Err(AccountStorageError::RefuseSymlink { .. })
    ));
    assert!(matches!(
        storage.save_tokens("account", &tokens()),
        Err(AccountStorageError::RefuseSymlink { .. })
    ));
    assert!(matches!(
        storage.delete_account("account"),
        Err(AccountStorageError::RefuseSymlink { .. })
    ));
    assert_eq!(
        fs::read_to_string(outside.join("marker")).unwrap(),
        "unchanged"
    );
}

#[cfg(unix)]
#[test]
fn refuses_symlinked_credential_files_without_following_them() {
    use std::os::unix::fs::symlink;

    let storage = ProviderAccountStorage::new(test_dir("symlinked-files-root"));
    let stored = storage
        .create_account(NewProviderAccount {
            provider: ProviderId::Claude,
            email: "person@example.com".to_string(),
            provider_account_id: None,
            organization_id: None,
            organization_name: None,
            tokens: tokens(),
            snapshot: Some(snapshot()),
        })
        .unwrap();
    let outside = test_dir("symlinked-files-outside");
    fs::create_dir_all(&outside).unwrap();

    for file_name in [TOKENS_FILE, METADATA_FILE, SNAPSHOT_FILE] {
        let target = outside.join(file_name);
        fs::write(&target, "unchanged").unwrap();
        let managed = stored.account_dir.join(file_name);
        fs::remove_file(&managed).unwrap();
        symlink(&target, &managed).unwrap();

        assert!(
            storage
                .read_text_file(&stored.account_ref.account_id, file_name)
                .is_err()
        );
        assert!(
            storage
                .write_text_file(&stored.account_ref.account_id, file_name, "replacement")
                .is_err()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "unchanged");

        fs::remove_file(managed).unwrap();
        fs::write(stored.account_dir.join(file_name), "{}").unwrap();
    }
}

#[cfg(unix)]
#[test]
fn create_private_dir_sets_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = test_dir("private-dir");
    create_private_dir(&dir).unwrap();

    let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[cfg(unix)]
#[test]
fn set_private_file_permissions_restricts_to_owner_read_write() {
    use std::os::unix::fs::PermissionsExt;

    let dir = test_dir("private-file");
    create_private_dir(&dir).unwrap();
    let file = dir.join("secret.json");
    write_json(&file, &"secret").unwrap();

    set_private_file_permissions(&file).unwrap();

    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
