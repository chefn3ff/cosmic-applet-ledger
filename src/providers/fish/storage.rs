// SPDX-License-Identifier: MPL-2.0

use crate::account_storage::ProviderAccountStorage;
use std::path::Path;

pub const API_KEY_FILE: &str = "api_key.txt";

pub fn write_api_key(account_id: &str, api_key: &str) -> Result<(), String> {
    write_api_key_at(
        &crate::config::paths().fish_accounts_dir,
        account_id,
        api_key,
    )
}

pub fn load_api_key(account_id: &str) -> Result<String, String> {
    load_api_key_at(&crate::config::paths().fish_accounts_dir, account_id)
}

pub(crate) fn delete_account(account_id: &str) -> Result<(), String> {
    ProviderAccountStorage::new(&crate::config::paths().fish_accounts_dir)
        .delete_account(account_id)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(crate) fn write_api_key_at(root: &Path, account_id: &str, api_key: &str) -> Result<(), String> {
    let api_key = normalize_api_key(api_key)?;
    ProviderAccountStorage::new(root)
        .write_text_file(account_id, API_KEY_FILE, &api_key)
        .map_err(|error| error.to_string())
}

pub(crate) fn load_api_key_at(root: &Path, account_id: &str) -> Result<String, String> {
    ProviderAccountStorage::new(root)
        .read_text_file(account_id, API_KEY_FILE)
        .map_err(|error| error.to_string())
}

pub(crate) fn normalize_api_key(api_key: &str) -> Result<String, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("API key is required".to_string());
    }
    if api_key.chars().any(char::is_control) {
        return Err("API key contains invalid characters".to_string());
    }
    // Fish Audio keys have no documented prefix, so the only checks are the
    // ones that make the key unusable as a header value.
    if api_key.chars().any(char::is_whitespace) {
        return Err("API key must not contain spaces".to_string());
    }
    Ok(api_key.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_api_key;

    #[test]
    fn trims_and_accepts_a_plain_key() {
        assert_eq!(normalize_api_key("  abc123def  ").unwrap(), "abc123def");
        assert_eq!(
            normalize_api_key(&"a1".repeat(16)).unwrap(),
            "a1".repeat(16)
        );
    }

    #[test]
    fn rejects_empty_control_and_inner_whitespace() {
        assert_eq!(normalize_api_key("").unwrap_err(), "API key is required");
        assert_eq!(
            normalize_api_key("   \t ").unwrap_err(),
            "API key is required"
        );
        assert!(normalize_api_key("abc\u{7}def").is_err());
        assert!(normalize_api_key("abc\ndef").is_err());
        assert!(normalize_api_key("abc def").is_err());
        assert!(normalize_api_key("abc\u{a0}def").is_err());
    }
}
