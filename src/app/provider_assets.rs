// SPDX-License-Identifier: MPL-2.0

use crate::model::ProviderId;
use cosmic::widget::icon::{self, Handle};
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether provider logos keep their brand colours. The view sets it from
/// the config before drawing, so every icon call site follows the setting
/// without passing the config around.
static COLOR_PROVIDER_ICONS: AtomicBool = AtomicBool::new(false);

pub fn set_color_provider_icons(enabled: bool) {
    COLOR_PROVIDER_ICONS.store(enabled, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderIconVariant {
    Default,
    Reversed,
}

pub fn provider_icon_handle(provider: ProviderId, variant: ProviderIconVariant) -> Handle {
    let bytes: &[u8] = match (provider, variant) {
        (ProviderId::Codex, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/codex.svg")
        }
        (ProviderId::Codex, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/codex-reversed.svg")
        }
        (ProviderId::Claude, _) => {
            include_bytes!("../../resources/providers/claude-color.svg")
        }
        (ProviderId::Cursor, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/cursor.svg")
        }
        (ProviderId::Cursor, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/cursor-reversed.svg")
        }
        (ProviderId::Gemini, _) => {
            include_bytes!("../../resources/providers/gemini-color.svg")
        }
        (ProviderId::Copilot, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/copilot.svg")
        }
        (ProviderId::Copilot, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/copilot-reversed.svg")
        }
        (ProviderId::Minimax, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/minimax.svg")
        }
        (ProviderId::Minimax, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/minimax-reversed.svg")
        }
        (ProviderId::Zai, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/zai.svg")
        }
        (ProviderId::Zai, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/zai-reversed.svg")
        }
        (ProviderId::Kimi, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/kimi.svg")
        }
        (ProviderId::Kimi, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/kimi-reversed.svg")
        }
        (ProviderId::Antigravity, _) => {
            include_bytes!("../../resources/providers/antigravity-color.svg")
        }
        (ProviderId::OpenCodeGo, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/opencode-go.svg")
        }
        (ProviderId::OpenCodeGo, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/opencode-go-reversed.svg")
        }
        (ProviderId::Grok, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/grok.svg")
        }
        (ProviderId::Grok, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/grok-reversed.svg")
        }
        (ProviderId::OpenRouter, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/openrouter.svg")
        }
        (ProviderId::OpenRouter, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/openrouter-reversed.svg")
        }
        (ProviderId::ElevenLabs, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/elevenlabs.svg")
        }
        (ProviderId::ElevenLabs, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/elevenlabs-reversed.svg")
        }
        (ProviderId::Fish, ProviderIconVariant::Default) => {
            include_bytes!("../../resources/providers/fish.svg")
        }
        (ProviderId::Fish, ProviderIconVariant::Reversed) => {
            include_bytes!("../../resources/providers/fish-reversed.svg")
        }
    };

    let mut handle = icon::from_svg_bytes(bytes);
    // Symbolic icons are tinted with the theme's text colour, which turns
    // every logo, colour ones included, into a black or white silhouette.
    handle.symbolic = !COLOR_PROVIDER_ICONS.load(Ordering::Relaxed);
    handle
}

/// Two stacked arrows pointing opposite ways: the Switch Provider button.
/// Symbolic, so it takes the theme colour like the stock header icons.
pub fn switch_provider_icon() -> Handle {
    let mut handle = icon::from_svg_bytes(
        include_bytes!("../../resources/icons/switch-provider-symbolic.svg").as_slice(),
    );
    handle.symbolic = true;
    handle
}

pub fn app_icon_handle() -> Handle {
    if cosmic::theme::is_dark() {
        icon::from_svg_bytes(include_bytes!(
            "../../resources/ledger-icon/svg/ledger-app-dark.svg"
        ))
    } else {
        icon::from_svg_bytes(include_bytes!(
            "../../resources/ledger-icon/svg/ledger-app-light.svg"
        ))
    }
}

pub fn app_symbolic_icon_handle() -> Handle {
    if cosmic::theme::is_dark() {
        icon::from_svg_bytes(include_bytes!(
            "../../resources/ledger-icon/svg/ledger-mark-white.svg"
        ))
    } else {
        icon::from_svg_bytes(include_bytes!(
            "../../resources/ledger-icon/svg/ledger-mark-black.svg"
        ))
    }
}

pub fn provider_icon_variant() -> ProviderIconVariant {
    if cosmic::theme::is_dark() {
        ProviderIconVariant::Reversed
    } else {
        ProviderIconVariant::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_icons_are_black_and_white_unless_color_is_on() {
        set_color_provider_icons(false);
        assert!(provider_icon_handle(ProviderId::Claude, ProviderIconVariant::Default).symbolic);
        set_color_provider_icons(true);
        assert!(!provider_icon_handle(ProviderId::Claude, ProviderIconVariant::Default).symbolic);
        set_color_provider_icons(false);
    }
}
