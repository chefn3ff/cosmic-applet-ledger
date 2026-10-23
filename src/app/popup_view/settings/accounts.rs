mod empty;
mod login_controls;
mod rows;

use self::empty::empty_accounts_state;
use self::login_controls::{
    antigravity_login_controls, claude_login_controls, codex_login_controls,
    copilot_login_controls, cursor_scan_controls, elevenlabs_login_controls, gemini_login_controls,
    grok_login_controls, kimi_login_controls, minimax_login_controls, opencode_go_login_controls,
    openrouter_login_controls, zai_login_controls,
};
use self::rows::{
    AccountRowPosition, account_action_container, account_selector_list, account_settings_row,
};
use super::super::{
    Alignment, AppState, Background, Color, Config, DetectionSnapshot, Element, Length, Message,
    ProviderId, ProviderLoginStates, container, detected_without_accounts, fl,
    provider_icon_handle, provider_icon_variant, row, widget,
};
use crate::providers::cursor::CursorScanState;
use crate::providers::interface::{ProviderAccountFacts, ProviderLoginKind};
use crate::providers::registry;

pub(super) fn provider_settings_view<'a>(
    state: &'a AppState,
    config: &'a Config,
    detection: &'a DetectionSnapshot,
    logins: ProviderLoginStates<'a>,
    provider_id: ProviderId,
) -> Element<'a, Message> {
    let account_facts = registry::prepare_account_facts(provider_id, config, state);
    let enabled = state
        .provider(provider_id)
        .is_some_and(|provider| provider.enabled);

    let on_btn = widget::button::custom(container(widget::text("ON").size(12)).padding([3, 10]))
        .padding(0)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(move |_focused, _theme| widget::button::Style {
                background: if enabled {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15)))
                } else {
                    None
                },
                text_color: Some(if enabled {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                }),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: if enabled {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                },
                ..Default::default()
            }),
            disabled: Box::new(|_| widget::button::Style::new()),
            hovered: Box::new(move |_focused, _theme| widget::button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.10))),
                text_color: Some(Color::WHITE),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
                ..Default::default()
            }),
            pressed: Box::new(move |_focused, _theme| widget::button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.20))),
                text_color: Some(Color::WHITE),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: Color::WHITE,
                ..Default::default()
            }),
        })
        .on_press(Message::SetProviderEnabled(provider_id, true));

    let off_btn = widget::button::custom(container(widget::text("OFF").size(12)).padding([3, 10]))
        .padding(0)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(move |_focused, _theme| widget::button::Style {
                background: if !enabled {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15)))
                } else {
                    None
                },
                text_color: Some(if !enabled {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                }),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: if !enabled {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                },
                ..Default::default()
            }),
            disabled: Box::new(|_| widget::button::Style::new()),
            hovered: Box::new(move |_focused, _theme| widget::button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.10))),
                text_color: Some(Color::WHITE),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
                ..Default::default()
            }),
            pressed: Box::new(move |_focused, _theme| widget::button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.20))),
                text_color: Some(Color::WHITE),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: Color::WHITE,
                ..Default::default()
            }),
        })
        .on_press(Message::SetProviderEnabled(provider_id, false));

    let provider_header = container(
        row![
            row![
                widget::icon::icon(provider_icon_handle(provider_id, provider_icon_variant()))
                    .size(20),
                widget::text(provider_id.label()).size(16),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            row![off_btn, on_btn].spacing(4),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([0, 4])
    .width(Length::Fill);

    let login_kind = registry::login_kind(provider_id);
    let (login_controls, login_active) = match login_kind {
        ProviderLoginKind::Codex => (
            codex_login_controls(
                logins.codex,
                registry::supports_opencode_import(provider_id),
                enabled,
            ),
            logins.codex.is_some(),
        ),
        ProviderLoginKind::Claude => (
            claude_login_controls(logins.claude, enabled),
            logins.claude.is_some(),
        ),
        ProviderLoginKind::Cursor => (
            cursor_scan_controls(logins.cursor_scan, enabled),
            !matches!(logins.cursor_scan, CursorScanState::Idle),
        ),
        ProviderLoginKind::Gemini => (
            gemini_login_controls(logins.gemini, enabled),
            logins.gemini.is_some(),
        ),
        ProviderLoginKind::Copilot => (
            copilot_login_controls(
                logins.copilot,
                registry::supports_opencode_import(provider_id),
                enabled,
            ),
            logins.copilot.is_some(),
        ),
        ProviderLoginKind::Minimax => (
            minimax_login_controls(logins.minimax, enabled),
            logins.minimax.is_some(),
        ),
        ProviderLoginKind::Zai => (
            zai_login_controls(logins.zai, enabled),
            logins.zai.is_some(),
        ),
        ProviderLoginKind::Kimi => (
            kimi_login_controls(logins.kimi, enabled),
            logins.kimi.is_some(),
        ),
        ProviderLoginKind::Antigravity => (
            antigravity_login_controls(logins.antigravity, enabled),
            logins.antigravity.is_some(),
        ),
        ProviderLoginKind::OpenCodeGo => (
            opencode_go_login_controls(logins.opencode_go, enabled),
            logins.opencode_go.is_some(),
        ),
        ProviderLoginKind::Grok => {
            let host_import_available = crate::providers::grok::account::host_auth_file_path()
                .and_then(|path| crate::providers::grok::account::read_host_credentials(&path))
                .is_some();
            (
                grok_login_controls(logins.grok, host_import_available, enabled),
                logins.grok.is_some(),
            )
        }
        ProviderLoginKind::OpenRouter => (
            openrouter_login_controls(logins.openrouter, enabled),
            logins.openrouter.is_some(),
        ),
        ProviderLoginKind::ElevenLabs => (
            elevenlabs_login_controls(logins.elevenlabs, enabled),
            logins.elevenlabs.is_some(),
        ),
    };
    let selection_warning = registry::selection_required_message(provider_id);
    let accounts_section = account_settings_section(AccountSettingsContext {
        state,
        provider_id,
        account_facts: &account_facts,
        enabled,
        login_active,
        login_controls,
        selection_warning,
        login_hint: provider_id == ProviderId::Claude
            && logins.claude.is_some_and(|login| {
                login.status == crate::providers::claude::ClaudeLoginStatus::Running
            }),
    });

    let mut sections = cosmic::iced::widget::column![provider_header].spacing(16);
    if detected_without_accounts(state, detection, provider_id) {
        sections = sections.push(widget::text(fl!("provider-detected-caption")).size(13));
    }
    Element::from(sections.push(accounts_section).width(Length::Fill))
}

struct AccountSettingsContext<'a, 'd> {
    state: &'a AppState,
    provider_id: ProviderId,
    account_facts: &'d [ProviderAccountFacts],
    enabled: bool,
    login_active: bool,
    login_controls: Element<'a, Message>,
    selection_warning: Option<String>,
    login_hint: bool,
}

fn account_settings_section<'a, 'd>(
    context: AccountSettingsContext<'a, 'd>,
) -> Element<'a, Message> {
    let AccountSettingsContext {
        state,
        provider_id,
        account_facts,
        enabled,
        login_active,
        login_controls,
        selection_warning,
        login_hint,
    } = context;
    let provider = state.provider(provider_id);
    let selected_ids: Vec<&str> = provider
        .map(|provider| {
            provider
                .selected_account_ids
                .iter()
                .map(String::as_str)
                .collect()
        })
        .unwrap_or_default();
    let accounts = account_facts;
    let active_id = provider.and_then(|provider| provider.system_active_account_id.as_deref());
    let empty_state = accounts.is_empty() && !login_active;
    let mut rows = cosmic::iced::widget::column![]
        .spacing(8)
        .width(Length::Fill);
    if accounts.is_empty() {
        rows = if empty_state {
            rows.push(empty_accounts_state(
                provider_id,
                registry::account_add_action(provider_id),
                registry::supports_opencode_import(provider_id),
                enabled,
            ))
        } else {
            rows.push(widget::text(fl!("accounts-empty-title")).size(13))
        };
    } else {
        let mut account_rows = cosmic::iced::widget::column![]
            .spacing(0)
            .width(Length::Fill);
        for (index, account) in accounts.iter().enumerate() {
            account_rows = account_rows.push(account_settings_row(
                provider_id,
                account,
                &selected_ids,
                active_id,
                enabled,
                AccountRowPosition {
                    first: index == 0,
                    last: index + 1 == accounts.len(),
                },
            ));
        }
        rows = rows.push(account_selector_list(account_rows));
    }
    if let Some(caption) = selection_warning
        && provider.is_some_and(|provider| {
            provider.account_status == crate::model::AccountSelectionStatus::SelectionRequired
        })
    {
        rows = rows.push(widget::text(caption).size(13));
    }
    if login_hint {
        rows = rows.push(widget::text(fl!("account-browser-login-hint")).size(12));
    }
    if !empty_state {
        rows = rows.push(account_action_container(login_controls));
    }

    let accounts_title =
        container(widget::text("ACCOUNTS").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let accounts_card = container(rows)
        .padding([4, 4])
        .width(Length::Fill)
        .style(|_| widget::container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: cosmic::iced::Border {
                radius: 2.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            },
            ..Default::default()
        });

    cosmic::iced::widget::column![accounts_title, accounts_card,]
        .spacing(8)
        .width(Length::Fill)
        .into()
}
