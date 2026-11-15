// SPDX-License-Identifier: MPL-2.0

mod account_selection;
pub mod account_storage;
mod app;
mod auth;
mod browser;
mod config;
mod currency_format;
#[cfg(debug_assertions)]
mod debug_env;
mod demo_env;
mod detection;
mod error;
mod i18n;
mod key_authentication;
mod logging;
mod model;
mod provider_enablement;
mod providers;
mod redact;
mod refresh_owner;
mod runtime;
mod shared_state;
#[cfg(test)]
mod test_support;
mod updates;
mod usage_display;

fn main() -> cosmic::iced::Result {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let default_level = if cfg!(debug_assertions) {
        "warn,cosmic::theme=off,cosmic_applet_ledger=debug"
    } else {
        "warn,cosmic::theme=off,cosmic_applet_ledger=info"
    };
    let _log_guard = logging::init(default_level).ok();
    config::migrate_legacy_config();

    if running_in_cosmic_panel() {
        cosmic::applet::run::<app::AppModel>(app::LaunchMode::Panel)
    } else {
        cosmic::app::run::<app::AppModel>(app::applet_settings(), app::LaunchMode::Standalone)
    }
}

fn running_in_cosmic_panel() -> bool {
    std::env::vars().any(|(key, _)| key.starts_with("COSMIC_PANEL_"))
}
