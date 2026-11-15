// SPDX-License-Identifier: MPL-2.0

use crate::account_storage::ProviderAccountStorage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

const TOKENS_FILE: &str = "tokens.json";
const METADATA_FILE: &str = "metadata.json";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopilotTokens {
    pub access_token: String,
}
crate::redact::redacted_debug!(CopilotTokens {  } secret { access_token });

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CopilotMetadata {
    pub github_user_id: u64,
    pub login: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_authenticated_at: Option<DateTime<Utc>>,
}

pub fn account_id_for_github_user(github_user_id: u64) -> String {
    format!("copilot-{github_user_id}")
}

pub fn write_account(
    account_id: &str,
    tokens: &CopilotTokens,
    metadata: &CopilotMetadata,
) -> Result<(), String> {
    write_account_at(
        &crate::config::paths().copilot_accounts_dir,
        account_id,
        tokens,
        metadata,
    )
}

pub fn load_tokens(account_id: &str) -> Result<CopilotTokens, String> {
    load_tokens_at(&crate::config::paths().copilot_accounts_dir, account_id)
}

pub(crate) fn load_tokens_at(root: &Path, account_id: &str) -> Result<CopilotTokens, String> {
    ProviderAccountStorage::new(root)
        .read_json_file(account_id, TOKENS_FILE)
        .map_err(stringify)
}

pub(crate) fn write_account_at(
    root: &Path,
    account_id: &str,
    tokens: &CopilotTokens,
    metadata: &CopilotMetadata,
) -> Result<(), String> {
    let storage = ProviderAccountStorage::new(root);
    storage
        .validate_account_files_for_write(account_id, &[TOKENS_FILE, METADATA_FILE])
        .map_err(stringify)?;
    storage
        .write_json_file(account_id, TOKENS_FILE, tokens)
        .map_err(stringify)?;
    storage
        .write_json_file(account_id, METADATA_FILE, metadata)
        .map_err(stringify)
}

fn stringify(error: crate::account_storage::AccountStorageError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn account() -> (CopilotTokens, CopilotMetadata) {
        let now = Utc::now();
        (
            CopilotTokens {
                access_token: "test-token".to_string(),
            },
            CopilotMetadata {
                github_user_id: 42,
                login: "octocat".to_string(),
                created_at: now,
                updated_at: now,
                last_authenticated_at: Some(now),
            },
        )
    }

    #[test]
    fn account_id_uses_github_user_id() {
        assert_eq!(account_id_for_github_user(42), "copilot-42");
    }

    #[test]
    fn write_account_creates_private_files_under_the_managed_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("copilot-accounts");
        let (tokens, metadata) = account();

        write_account_at(&root, "copilot-42", &tokens, &metadata).unwrap();

        let storage = ProviderAccountStorage::new(&root);
        assert_eq!(
            storage
                .read_json_file::<CopilotTokens>("copilot-42", TOKENS_FILE)
                .unwrap(),
            tokens
        );
        assert_eq!(
            storage
                .read_json_file::<CopilotMetadata>("copilot-42", METADATA_FILE)
                .unwrap(),
            metadata
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_account_directories_and_files() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().unwrap();
        let root = temp.path().join("copilot-accounts");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let target = outside.join("tokens.json");
        fs::write(&target, "unchanged").unwrap();
        fs::create_dir_all(&root).unwrap();
        symlink(&outside, root.join("copilot-42")).unwrap();
        let (tokens, metadata) = account();

        assert!(write_account_at(&root, "copilot-42", &tokens, &metadata).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "unchanged");

        fs::remove_file(root.join("copilot-42")).unwrap();
        fs::create_dir(root.join("copilot-42")).unwrap();
        symlink(&target, root.join("copilot-42").join(TOKENS_FILE)).unwrap();

        assert!(write_account_at(&root, "copilot-42", &tokens, &metadata).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "unchanged");
    }
}
