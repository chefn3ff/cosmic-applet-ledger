// SPDX-License-Identifier: MPL-2.0

//! The one place Ledger hands a URL to `xdg-open`.
//!
//! Some of these URLs come from the network (Copilot's device-flow
//! verification URI, release links), and `xdg-open` will launch whatever
//! handler a scheme maps to (`file:`, `smb:`, custom app schemes). Only plain
//! web pages are ever opened.

/// Opens `url` in the default browser if it is an `http` or `https` URL.
/// `what` names the link in the log line when it is refused or fails to open.
pub fn open(url: &str, what: &str) {
    if !is_web_url(url) {
        tracing::warn!(url = %url, what, "refused to open a non-http(s) URL");
        return;
    }
    if let Err(error) = std::process::Command::new("xdg-open").arg(url).spawn() {
        tracing::warn!(url = %url, what, error = %error, "failed to open URL");
    }
}

/// True for `http://` and `https://` URLs with a host, scheme matched without case.
fn is_web_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    (scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
        && !rest.is_empty()
        && !rest.starts_with('/')
        && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::is_web_url;

    #[test]
    fn only_http_and_https_urls_are_opened() {
        for ok in [
            "https://github.com/login/device",
            "http://localhost:1455/auth/callback",
            "HTTPS://claude.ai/oauth/authorize?state=x",
        ] {
            assert!(is_web_url(ok), "{ok} should open");
        }
        for refused in [
            "file:///etc/passwd",
            "smb://host/share",
            "javascript:alert(1)",
            "steam://run/1",
            "https:///no-host",
            "https://",
            "httpsx://example.com",
            "/home/user/file.html",
            "https://example.com/\nfile:///etc/passwd",
            "",
        ] {
            assert!(!is_web_url(refused), "{refused:?} should be refused");
        }
    }
}
