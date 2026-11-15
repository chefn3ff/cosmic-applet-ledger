// SPDX-License-Identifier: MPL-2.0

//! `Debug` output that never prints a credential.
//!
//! Types that hold tokens or API keys implement `Debug` through
//! [`redacted_debug!`] instead of `#[derive(Debug)]`. The macro destructures
//! the type exhaustively, so adding a field without saying whether it is
//! secret is a compile error, and `debug_derive_never_reaches_a_secret_field`
//! fails if a new credential type derives `Debug` again.

use std::fmt;

/// Shows whether a secret is present, never its value.
pub struct Redacted<'a, T: ?Sized>(pub &'a T);

const MASK: &str = "<redacted>";

fn mask(value: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if value.is_empty() {
        f.write_str("\"\"")
    } else {
        f.write_str(MASK)
    }
}

impl fmt::Debug for Redacted<'_, String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        mask(self.0, f)
    }
}

impl fmt::Debug for Redacted<'_, std::borrow::Cow<'_, str>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        mask(self.0, f)
    }
}

impl fmt::Debug for Redacted<'_, Option<String>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => f.debug_tuple("Some").field(&Redacted(value)).finish(),
            None => f.write_str("None"),
        }
    }
}

/// `redacted_debug!(Type { plain, fields } secret { token, fields });`
///
/// Implements `Debug` for a struct, printing `secret` fields as `<redacted>`.
/// Every field must be listed in one group or the other.
macro_rules! redacted_debug {
    ($ty:ident { $($field:ident),* $(,)? } secret { $($secret:ident),+ $(,)? }) => {
        impl ::std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let Self { $($field,)* $($secret,)+ } = self;
                f.debug_struct(stringify!($ty))
                    $(.field(stringify!($field), $field))*
                    $(.field(stringify!($secret), &$crate::redact::Redacted($secret)))+
                    .finish()
            }
        }
    };
}
pub(crate) use redacted_debug;

#[cfg(test)]
mod tests {
    use std::path::Path;

    struct Sample {
        email: Option<String>,
        access_token: String,
        refresh_token: Option<String>,
        empty_key: String,
    }
    redacted_debug!(Sample { email } secret { access_token, refresh_token, empty_key });

    #[test]
    fn secrets_are_masked_and_plain_fields_shown() {
        let sample = Sample {
            email: Some("me@example.com".into()),
            access_token: "sk-live-abc".into(),
            refresh_token: Some("rt-xyz".into()),
            empty_key: String::new(),
        };
        let out = format!("{sample:?}");
        assert!(
            !out.contains("sk-live-abc") && !out.contains("rt-xyz"),
            "{out}"
        );
        assert!(out.contains("me@example.com"), "{out}");
        assert!(out.contains("access_token: <redacted>"), "{out}");
        assert!(out.contains("refresh_token: Some(<redacted>)"), "{out}");
        assert!(out.contains("empty_key: \"\""), "{out}");
        let pretty = format!("{sample:#?}");
        assert!(!pretty.contains("sk-live-abc"), "{pretty}");
    }

    /// Field names that always hold a credential.
    const SECRET_FIELDS: &[&str] = &[
        "access_token",
        "refresh_token",
        "id_token",
        "api_key",
        "client_secret",
        "code_verifier",
        "code_input",
        "state_token",
    ];

    /// Guards the whole crate: no struct that derives `Debug` may hold a field
    /// named like a credential. Such types use `redacted_debug!` instead.
    #[test]
    fn debug_derive_never_reaches_a_secret_field() {
        let mut offenders = Vec::new();
        scan(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut offenders,
        );
        assert!(
            offenders.is_empty(),
            "these types derive Debug but hold a credential; use redacted_debug!:\n{}",
            offenders.join("\n")
        );
    }

    fn scan(dir: &Path, offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, offenders);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                offenders.extend(
                    debug_types_with_secrets(&text)
                        .into_iter()
                        .map(|name| format!("{}: {name}", path.display())),
                );
            }
        }
    }

    /// Names of structs whose attributes derive `Debug` and whose body declares
    /// a field from `SECRET_FIELDS` (directly, not in a nested type).
    fn debug_types_with_secrets(text: &str) -> Vec<String> {
        let lines: Vec<&str> = text.lines().collect();
        let mut found = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed
                .split_once("struct ")
                .filter(|(before, _)| before.trim().is_empty() || before.trim().starts_with("pub"))
                .map(|(_, rest)| rest)
            else {
                continue;
            };
            if trimmed.starts_with("//") || !rest.trim_end().ends_with('{') {
                continue;
            }
            let derives_debug = lines[..index]
                .iter()
                .rev()
                .take_while(|attr| {
                    let attr = attr.trim_start();
                    attr.starts_with("#[") || attr.starts_with("//")
                })
                .any(|attr| attr.contains("derive(") && attr.contains("Debug"));
            if !derives_debug {
                continue;
            }
            let has_secret = lines[index + 1..]
                .iter()
                .take_while(|field| field.trim_start() != "}" && !field.starts_with('}'))
                .any(|field| {
                    let field = field.trim_start().trim_start_matches("pub(crate) ");
                    let field = field
                        .trim_start_matches("pub(super) ")
                        .trim_start_matches("pub ");
                    SECRET_FIELDS
                        .iter()
                        .any(|name| field.starts_with(&format!("{name}:")))
                });
            if has_secret {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                found.push(name);
            }
        }
        found
    }

    #[test]
    fn the_guard_sees_a_derived_secret() {
        let text = "#[derive(Debug, Clone)]\npub struct Leaky {\n    pub access_token: String,\n}\n\n#[derive(Debug)]\nstruct Fine {\n    email: String,\n}\n";
        assert_eq!(debug_types_with_secrets(text), vec!["Leaky".to_string()]);
    }
}
