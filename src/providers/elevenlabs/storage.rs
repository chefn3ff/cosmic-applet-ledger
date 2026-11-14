// SPDX-License-Identifier: MPL-2.0

use crate::account_storage::ProviderAccountStorage;
use std::path::Path;

pub const API_KEY_FILE: &str = "api_key.txt";

pub fn write_api_key(account_id: &str, api_key: &str) -> Result<(), String> {
    write_api_key_at(
        &crate::config::paths().elevenlabs_accounts_dir,
        account_id,
        api_key,
    )
}

pub fn load_api_key(account_id: &str) -> Result<String, String> {
    load_api_key_at(&crate::config::paths().elevenlabs_accounts_dir, account_id)
}

pub(crate) fn delete_account(account_id: &str) -> Result<(), String> {
    ProviderAccountStorage::new(&crate::config::paths().elevenlabs_accounts_dir)
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
    // ElevenLabs lists a key ID next to each key; only the key itself starts
    // with `sk_`, and an ID sent as a key gets HTTP 400 on every request.
    if !api_key.starts_with("sk_") {
        return Err("That looks like a key ID. ElevenLabs API keys start with sk_".to_string());
    }
    Ok(api_key.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_api_key;

    #[test]
    fn rejects_values_that_are_not_api_keys() {
        assert!(normalize_api_key("").is_err());
        assert!(normalize_api_key(&"a1".repeat(32)).is_err());
        assert_eq!(normalize_api_key("  sk_abc123 ").unwrap(), "sk_abc123");
    }
}
