use super::super::provider_assets::app_icon_handle;
use super::{
    Message, PROVIDER_CARD_SPACING, PROVIDER_GROUP_PADDING, PROVIDER_GROUP_SPACING, PopupRoute,
    account_action_button, account_label_text, apply_alpha, badge_destructive, badge_neutral,
    badge_success, badge_warning, badge_with_tooltip, card, clamp_account_page,
    component_card_background, component_container_style, component_divider_color,
    component_hover_color, component_on_color, detected_without_accounts, info_block,
    pager_account_label, plan_badge, provider_icon_handle, provider_icon_variant, provider_summary,
};
use crate::app::PagerDirection;
use crate::config::{Config, UsageAmountFormat};
use crate::currency_format;
use crate::fl;
use crate::model::{
    AccountSelectionStatus, AppState, AuthState, ExtraUsageState, KeyCreditUsage,
    ProviderAccountRuntimeState, ProviderCost, ProviderHealth, ProviderId, ProviderRuntimeState,
    STALE_THRESHOLD, UsageSnapshot, UsageWindow,
};
use crate::usage_display;
use cosmic::Element;
use cosmic::iced::widget::{column, container, row};
use cosmic::iced::{Alignment, Background, Color, Length};
use cosmic::widget;

pub(super) fn selected_provider_view<'a>(
    provider: Option<&'a ProviderRuntimeState>,
    state: &'a AppState,
    config: &'a Config,
    detection: &'a crate::detection::DetectionSnapshot,
    account_page: usize,
) -> Element<'a, Message> {
    let Some(provider) = provider else {
        return empty_state_view();
    };
    let accounts = state.accounts_for(provider.provider);
    let detected_without_accounts = detected_without_accounts(state, detection, provider.provider);
    let active_account = if !accounts.is_empty() {
        let active = clamp_account_page(account_page, accounts.len());
        Some(accounts[active])
    } else {
        None
    };

    let plan = active_account
        .and_then(|a| a.snapshot.as_ref())
        .and_then(|s| s.identity.plan.as_deref());

    let ring = elevenlabs_ring(provider.provider, state, config);
    let summary = provider_summary(provider, detected_without_accounts, plan, ring);

    let body = account_body_items(active_account, provider, state, config, detection);

    column![summary, usage_card(body),]
        .spacing(14)
        .width(Length::Fill)
        .into()
}

#[allow(dead_code)]
fn account_pager(
    account: &ProviderAccountRuntimeState,
    active: usize,
    total: usize,
) -> Element<'static, Message> {
    let name = pager_account_label(active, &account.label);
    let previous = widget::tooltip::tooltip(
        widget::button::icon(widget::icon::from_name("go-previous-symbolic"))
            .extra_small()
            .padding(6)
            .class(account_pager_button_class())
            .on_press(Message::PageProviderAccount(PagerDirection::Previous)),
        widget::text(fl!("account-pager-previous")).size(12),
        widget::tooltip::Position::Top,
    );
    let next = widget::tooltip::tooltip(
        widget::button::icon(widget::icon::from_name("go-next-symbolic"))
            .extra_small()
            .padding(6)
            .class(account_pager_button_class())
            .on_press(Message::PageProviderAccount(PagerDirection::Next)),
        widget::text(fl!("account-pager-next")).size(12),
        widget::tooltip::Position::Top,
    );
    let dots = (0..total).fold(
        row![].spacing(10).align_y(Alignment::Center),
        |dots, page| dots.push(account_page_dot(page == active)),
    );
    let center = widget::tooltip::tooltip(
        container(dots),
        widget::text(name).size(12),
        widget::tooltip::Position::Top,
    );

    container(
        row![previous, center, next]
            .spacing(24)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn account_pager_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, theme| account_pager_button_style(theme, false)),
        disabled: Box::new(|theme| account_pager_button_style(theme, false)),
        hovered: Box::new(|_focused, theme| account_pager_button_style(theme, true)),
        pressed: Box::new(|_focused, theme| account_pager_button_style(theme, true)),
    }
}

fn account_pager_button_style(theme: &cosmic::Theme, hovered: bool) -> widget::button::Style {
    let _cosmic = theme.cosmic();
    let mut style = widget::button::Style::new();
    style.background = Some(Background::Color(if hovered {
        component_hover_color(theme)
    } else {
        component_divider_color(theme)
    }));
    style.border_radius = 2.0.into();
    style.icon_color = Some(component_on_color(theme));
    style.text_color = Some(component_on_color(theme));
    style
}

fn account_page_dot(active: bool) -> Element<'static, Message> {
    container(
        cosmic::iced::widget::Space::new()
            .width(Length::Fixed(12.0))
            .height(Length::Fixed(3.0)),
    )
    .style(move |_theme: &cosmic::Theme| {
        let color = if active {
            Color::from_rgb(0.95, 0.95, 0.95)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.20)
        };
        widget::container::Style {
            text_color: None,
            background: Some(Background::Color(color)),
            border: cosmic::iced::Border {
                radius: 1.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: cosmic::iced::Shadow::default(),
            icon_color: None,
            snap: true,
        }
    })
    .into()
}

fn account_body_items<'a>(
    account: Option<&'a ProviderAccountRuntimeState>,
    provider: &'a ProviderRuntimeState,
    state: &'a AppState,
    _config: &'a Config,
    detection: &'a crate::detection::DetectionSnapshot,
) -> Vec<Element<'a, Message>> {
    let mut items = Vec::new();

    // Warning banner ONLY if there is an actual error or reauth needed
    if let Some(banner) = provider_warning_banner(provider, state, account, detection) {
        items.push(banner);
    }

    let snapshot = active_snapshot_for_account(account, provider);
    if let Some(snapshot) = snapshot {
        // 1. Session & Weekly Limits ONLY
        items.push(limits_section(snapshot));

        // 2. TOKENS BY DAY (only for providers that report daily token data)
        if !snapshot.tokens_by_day.is_empty() {
            let days = ensure_seven_days(&snapshot.tokens_by_day);
            items.push(tokens_by_day_section(&days));
        }

        // 3. TOKENS BY MODEL
        if !snapshot.tokens_by_model.is_empty() {
            items.push(tokens_by_model_section(&snapshot.tokens_by_model));
        }

        // 4. CREDITS BY KEY (ElevenLabs)
        if let Some(ProviderCost {
            limit: Some(limit), ..
        }) = &snapshot.provider_cost
            && *limit > 0.0
            && !snapshot.credits_by_key.is_empty()
        {
            items.push(credits_by_key_section(&snapshot.credits_by_key, *limit));
        }
    } else if state.accounts_for(provider.provider).is_empty() {
        // When no account is configured yet, show a clean empty state helper below the banner
        let no_account_notice = container(
            column![
                widget::text("No active usage data").size(14),
                container(
                    widget::text("Click the gear icon above to configure credentials in Settings.")
                        .size(12)
                )
                .style(|_| widget::container::Style {
                    text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                    ..Default::default()
                }),
            ]
            .spacing(4)
            .width(Length::Fill),
        )
        .padding([12, 12])
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

        items.push(no_account_notice.into());
    }

    items
}

fn limits_section(snapshot: &UsageSnapshot) -> Element<'static, Message> {
    let now = chrono::Utc::now();
    let section_label =
        container(widget::text("LIMITS").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let mut meters_col = column![].spacing(16).width(Length::Fill);

    let (session_window, weekly_window) = limit_windows(&snapshot.windows);

    if let Some(session) = session_window {
        let pct = session.used_percent.clamp(0.0, 100.0);
        let right_label = window_value_label(snapshot, session);
        let row_title = session_row_title(snapshot.provider, session);
        let header_row = row![
            widget::text(row_title.clone()).size(14),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            widget::text(right_label.clone()).size(13),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        let bar = usage_progress_bar(usage_display::UsageMeter {
            fill_percent: pct,
            marker_percent: None,
            tooltip: format!("{row_title}: {right_label}"),
        });

        let mut session_col = column![header_row, bar].spacing(6).width(Length::Fill);

        // Only the ElevenLabs credits row carries a reset time here; other
        // providers' session windows have none.
        if snapshot.provider == ProviderId::ElevenLabs
            && let Some(reset) = session.reset_at
        {
            session_col = session_col.push(reset_caption(reset, now));
        }

        meters_col = meters_col.push(session_col);
    }

    if let Some(weekly) = weekly_window {
        let pct = weekly.used_percent.clamp(0.0, 100.0);
        let right_label = window_value_label(snapshot, weekly);
        let header_row = row![
            widget::text("Weekly").size(14),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            widget::text(right_label.clone()).size(13),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        let bar = usage_progress_bar(usage_display::UsageMeter {
            fill_percent: pct,
            marker_percent: None,
            tooltip: format!("Weekly: {right_label}"),
        });

        let mut weekly_col = column![header_row, bar].spacing(6).width(Length::Fill);

        if let Some(reset) = weekly.reset_at {
            weekly_col = weekly_col.push(reset_caption(reset, now));
        }

        meters_col = meters_col.push(weekly_col);
    }

    if session_window.is_none()
        && weekly_window.is_none()
        && let Some(cost) = &snapshot.provider_cost
    {
        let balance_str = if cost.used.fract() == 0.0 {
            format!("{:.0}", cost.used)
        } else {
            format!("{:.2}", cost.used)
        };
        let cost_header = row![
            widget::text("Prepaid Credits").size(14),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            widget::text(format!("{balance_str} {}", cost.units)).size(13),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        meters_col = meters_col.push(cost_header);
    }

    column![
        section_label,
        container(meters_col)
            .padding([12, 12])
            .width(Length::Fill)
            .style(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                border: cosmic::iced::Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                ..Default::default()
            })
    ]
    .spacing(8)
    .width(Length::Fill)
    .into()
}

/// "Resets in 2d 4h" caption shown under a limits meter.
fn reset_caption(
    reset: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Element<'static, Message> {
    container(widget::text(reset_text(reset, now)).size(11))
        .style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        })
        .into()
}

fn reset_text(reset: chrono::DateTime<chrono::Utc>, now: chrono::DateTime<chrono::Utc>) -> String {
    let diff = reset - now;
    if diff.num_seconds() <= 0 {
        "Resets now".to_string()
    } else if diff.num_days() > 0 {
        format!("Resets in {}d {}h", diff.num_days(), diff.num_hours() % 24)
    } else if diff.num_hours() > 0 {
        format!(
            "Resets in {}h {}m",
            diff.num_hours(),
            diff.num_minutes() % 60
        )
    } else {
        format!("Resets in {}m", diff.num_minutes().max(1))
    }
}

/// The ElevenLabs credits ring for every other provider's header, when the
/// ring is switched on, ElevenLabs is enabled and has a snapshot.
fn elevenlabs_ring(
    provider: ProviderId,
    state: &AppState,
    config: &Config,
) -> Option<Element<'static, Message>> {
    let elevenlabs_enabled = state
        .provider(ProviderId::ElevenLabs)
        .is_some_and(|elevenlabs| elevenlabs.enabled);
    if provider == ProviderId::ElevenLabs || !config.elevenlabs_ring_enabled || !elevenlabs_enabled
    {
        return None;
    }
    let snapshot = state
        .accounts_for(ProviderId::ElevenLabs)
        .into_iter()
        .find_map(|account| account.snapshot.as_ref())?;
    let window = snapshot.windows.first()?;
    let pct = window.used_percent.clamp(0.0, 100.0);

    let mut tooltip = format!("ElevenLabs: {}", window_value_label(snapshot, window));
    if snapshot.provider_cost.is_some() {
        tooltip.push_str(" credits");
    }
    if let Some(reset) = window.reset_at {
        tooltip.push_str(" · ");
        tooltip.push_str(&reset_text(reset, chrono::Utc::now()));
    }

    let ring = widget::Svg::new(widget::svg::Handle::from_memory(credit_ring_svg(pct)))
        .width(Length::Fixed(super::CREDIT_RING_SIZE))
        .height(Length::Fixed(super::CREDIT_RING_SIZE));
    let ring = super::overhang::overhang(
        ring,
        cosmic::iced::Size::new(super::CREDIT_RING_FOOTPRINT, super::CREDIT_RING_FOOTPRINT),
    )
    .nudge(cosmic::iced::Vector::new(0.0, super::CREDIT_RING_NUDGE_Y));
    Some(
        widget::tooltip::tooltip(
            ring,
            widget::text(tooltip).size(12),
            widget::tooltip::Position::Bottom,
        )
        .into(),
    )
}

/// A 32×32 gauge: faint track, an arc for the share used (clockwise from the
/// top), and the ElevenLabs two-bar mark in the middle.
fn credit_ring_svg(used_percent: f32) -> Vec<u8> {
    const RADIUS: f32 = 13.5;
    let circumference = 2.0 * std::f32::consts::PI * RADIUS;
    let arc_length = circumference * used_percent.clamp(0.0, 100.0) / 100.0;
    let arc = if arc_length > 0.0 {
        format!(
            r##"<circle cx="16" cy="16" r="{RADIUS}" fill="none" stroke="#ffffff" stroke-opacity="0.9" stroke-width="3" stroke-linecap="round" stroke-dasharray="{arc_length:.2} {circumference:.2}" transform="rotate(-90 16 16)"/>"##
        )
    } else {
        String::new()
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="16" cy="16" r="{RADIUS}" fill="none" stroke="#ffffff" stroke-opacity="0.14" stroke-width="3"/>{arc}<rect x="12.6" y="9.5" width="2.4" height="13" rx="0.8" fill="#ffffff"/><rect x="17" y="9.5" width="2.4" height="13" rx="0.8" fill="#ffffff"/></svg>"##
    )
    .into_bytes()
}

/// Right-hand value for a limits row. Spend-based providers put a formatted
/// amount in `reset_description`; everyone else uses it for reset metadata
/// (often an RFC3339 timestamp), so they get the percentage. ElevenLabs shows
/// raw credits ("8,200 / 30,000") from `provider_cost`.
fn window_value_label(snapshot: &UsageSnapshot, window: &UsageWindow) -> String {
    if snapshot.provider == ProviderId::OpenRouter
        && let Some(amount) = &window.reset_description
    {
        return amount.clone();
    }
    if snapshot.provider == ProviderId::ElevenLabs
        && let Some(ProviderCost {
            used,
            limit: Some(limit),
            ..
        }) = &snapshot.provider_cost
        && *limit > 0.0
    {
        return format!("{} / {}", group_thousands(*used), group_thousands(*limit));
    }
    format!("{:.0}%", window.used_percent.clamp(0.0, 100.0))
}

/// ElevenLabs has a single credits window, so the row carries its own label
/// instead of the generic "Session".
fn session_row_title(provider: ProviderId, window: &UsageWindow) -> String {
    if provider == ProviderId::ElevenLabs {
        window.label.clone()
    } else {
        "Session".to_string()
    }
}

/// 8200.0 -> "8,200".
fn group_thousands(value: f64) -> String {
    let digits = format!("{:.0}", value.max(0.0));
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Picks the Session and Weekly rows. The finders fall back loosely, so with a
/// single window (e.g. Grok's weekly credit) both can land on the same one;
/// it is then shown only in the row it actually describes.
fn limit_windows(windows: &[UsageWindow]) -> (Option<&UsageWindow>, Option<&UsageWindow>) {
    let session = find_session_window(windows);
    let weekly = find_weekly_window(windows);
    match (session, weekly) {
        (Some(s), Some(w)) if std::ptr::eq(s, w) => {
            if is_session_like(s) {
                (Some(s), None)
            } else {
                (None, Some(w))
            }
        }
        _ => (session, weekly),
    }
}

fn is_session_like(w: &UsageWindow) -> bool {
    let label = w.label.to_lowercase();
    label.contains("session")
        || label.contains("5h")
        || label.contains("5 hour")
        || label.contains("five hour")
        || label == "chat"
        || w.window_seconds == Some(5 * 3600)
}

fn find_session_window(windows: &[UsageWindow]) -> Option<&UsageWindow> {
    windows
        .iter()
        .find(|w| is_session_like(w))
        .or_else(|| {
            windows
                .iter()
                .find(|w| w.window_seconds.is_some_and(|s| s < 24 * 3600))
        })
        .or_else(|| windows.first())
}

fn find_weekly_window(windows: &[UsageWindow]) -> Option<&UsageWindow> {
    windows
        .iter()
        .find(|w| {
            let label = w.label.to_lowercase();
            label.contains("week")
                || label.contains("7-day")
                || label.contains("7d")
                || label.contains("weekly")
                || w.window_seconds == Some(7 * 24 * 3600)
        })
        .or_else(|| {
            windows.iter().find(|w| {
                let label = w.label.to_lowercase();
                !label.contains("session")
                    && !label.contains("5h")
                    && !label.contains("5 hour")
                    && w.window_seconds.is_some_and(|s| s >= 24 * 3600)
            })
        })
        .or_else(|| {
            if windows.len() > 1 {
                windows.get(1)
            } else {
                None
            }
        })
}

fn ensure_seven_days(days: &[crate::model::DayTokenUsage]) -> Vec<crate::model::DayTokenUsage> {
    use chrono::{Duration, Local};
    use std::collections::HashMap;

    let today = Local::now().date_naive();
    let mut day_map: HashMap<String, u64> = HashMap::new();
    for d in days {
        if !d.date.is_empty() {
            day_map.insert(d.date.clone(), d.token_count);
        }
    }

    let mut result = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let date = today - Duration::days(i);
        let date_str = date.format("%Y-%m-%d").to_string();
        let is_today = i == 0;
        let day_label = if is_today {
            "Today".to_string()
        } else {
            date.format("%a").to_string()
        };
        let token_count = day_map.get(&date_str).copied().unwrap_or(0);
        result.push(crate::model::DayTokenUsage {
            date: date_str,
            day_label,
            token_count,
            is_today,
        });
    }

    result
}

/// Fixed token tiers the charts scale against, so bar lengths stay
/// comparable between refreshes instead of always filling to the peak.
fn token_scale_max(raw_max: u64) -> u64 {
    match raw_max {
        0..1_000_000 => 1_000_000,
        1_000_000..100_000_000 => 100_000_000,
        100_000_000..500_000_000 => 500_000_000,
        _ => 1_000_000_000,
    }
}

fn tokens_by_day_section(days: &[crate::model::DayTokenUsage]) -> Element<'static, Message> {
    let scale_max = token_scale_max(days.iter().map(|d| d.token_count).max().unwrap_or(0));

    let section_label =
        container(widget::text("TOKENS BY DAY").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let mut day_items = column![].spacing(8).width(Length::Fill);

    for day in days {
        let pct = (day.token_count as f64 / scale_max as f64 * 100.0).clamp(0.0, 100.0) as f32;
        let is_today = day.is_today;

        let label_color = if is_today {
            Color::from_rgb(0.95, 0.95, 0.95)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.45)
        };

        let label = container(widget::text(day.day_label.clone()).size(12))
            .width(Length::Fixed(44.0))
            .style(move |_| widget::container::Style {
                text_color: Some(label_color),
                ..Default::default()
            });

        let bar = usage_progress_bar(usage_display::UsageMeter {
            fill_percent: pct,
            marker_percent: None,
            tooltip: format!("{}: {} tokens", day.day_label, day.token_count),
        });

        let value = container(widget::text(format_token_metric(day.token_count)).size(12))
            .width(Length::Fixed(48.0))
            .align_x(cosmic::iced::alignment::Horizontal::Right)
            .style(move |_| widget::container::Style {
                text_color: Some(label_color),
                ..Default::default()
            });

        day_items = day_items.push(
            row![label, bar, value]
                .spacing(8)
                .align_y(Alignment::Center)
                .width(Length::Fill),
        );
    }

    column![
        section_label,
        container(day_items)
            .padding([12, 12])
            .width(Length::Fill)
            .style(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                border: cosmic::iced::Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                ..Default::default()
            })
    ]
    .spacing(8)
    .width(Length::Fill)
    .into()
}

/// One row of a ranked-bar card: name on the left, value on the right, the
/// bar filled to `pct` behind both.
struct RankedBar {
    name: String,
    value: String,
    tooltip: String,
    pct: f32,
}

fn tokens_by_model_section(models: &[crate::model::ModelTokenUsage]) -> Element<'static, Message> {
    let scale_max = token_scale_max(models.iter().map(|m| m.token_count).max().unwrap_or(0));
    let rows = models
        .iter()
        .map(|model| RankedBar {
            name: model.model_name.clone(),
            value: format_token_metric(model.token_count),
            tooltip: format!("{}: {} tokens", model.model_name, model.token_count),
            pct: (model.token_count as f64 / scale_max as f64 * 100.0) as f32,
        })
        .collect();
    ranked_bars_section("TOKENS BY MODEL", rows)
}

/// Bars are scaled to the period's credit limit, not the busiest key, so
/// together they read as shares of the whole allowance.
fn credits_by_key_section(keys: &[KeyCreditUsage], limit: f64) -> Element<'static, Message> {
    let rows = keys
        .iter()
        .map(|key| {
            let name = key.key_name.clone().unwrap_or_else(|| "No key".to_string());
            let value = group_thousands(key.credits as f64);
            RankedBar {
                tooltip: format!("{name}: {value} of {} credits", group_thousands(limit)),
                pct: (key.credits as f64 / limit * 100.0) as f32,
                name,
                value,
            }
        })
        .collect();
    ranked_bars_section("CREDITS BY KEY", rows)
}

fn ranked_bars_section(title: &'static str, rows: Vec<RankedBar>) -> Element<'static, Message> {
    let section_label =
        container(widget::text(title).size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let mut model_items = column![].spacing(8).width(Length::Fill);

    for bar in rows {
        let pct = bar.pct.clamp(0.0, 100.0);
        let fill = (pct * 10.0).round() as u16;
        let empty = 1000 - fill;

        let fill_segment = container(cosmic::iced::widget::Space::new())
            .width(Length::FillPortion(fill))
            .height(Length::Fill)
            .style(|_| widget::container::Style {
                text_color: None,
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.14))),
                border: cosmic::iced::Border::default(),
                shadow: cosmic::iced::Shadow::default(),
                icon_color: None,
                snap: true,
            });

        let empty_segment = cosmic::iced::widget::Space::new().width(Length::FillPortion(empty));

        let track_row = if fill == 0 {
            row![empty_segment]
        } else if fill >= 1000 {
            row![fill_segment]
        } else {
            row![fill_segment, empty_segment]
        };

        let slider_track = container(track_row)
            .width(Length::Fill)
            .height(Length::Fixed(28.0))
            .style(|_| widget::container::Style {
                text_color: None,
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: cosmic::iced::Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                shadow: cosmic::iced::Shadow::default(),
                icon_color: None,
                snap: true,
            });

        let text_overlay = container(
            row![
                widget::text(bar.name).size(12),
                cosmic::iced::widget::Space::new().width(Length::Fill),
                widget::text(bar.value).size(12),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fixed(28.0))
        .padding([0, 10])
        .align_y(cosmic::iced::alignment::Vertical::Center);

        let slider = cosmic::iced::widget::Stack::new()
            .push(slider_track)
            .push(text_overlay)
            .width(Length::Fill)
            .height(Length::Fixed(28.0));

        let slider_with_tip = widget::tooltip::tooltip(
            slider,
            widget::text(bar.tooltip).size(12),
            widget::tooltip::Position::Top,
        );

        model_items = model_items.push(slider_with_tip);
    }

    column![
        section_label,
        container(model_items)
            .padding([12, 12])
            .width(Length::Fill)
            .style(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                border: cosmic::iced::Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                ..Default::default()
            })
    ]
    .spacing(8)
    .width(Length::Fill)
    .into()
}

#[allow(dead_code)]
fn account_header_content<'a>(
    account: &'a ProviderAccountRuntimeState,
    provider: &'a ProviderRuntimeState,
) -> Element<'a, Message> {
    let snapshot = account.snapshot.as_ref();
    let account_label = snapshot
        .and_then(|snapshot| snapshot.identity.email.as_deref())
        .filter(|email| !email.is_empty())
        .unwrap_or(account.label.as_str());
    let plan_label = snapshot.and_then(|snapshot| snapshot.identity.plan.as_deref());

    let mut label_row = row![account_label_text(account_label, 14)]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    label_row = label_row.push(cosmic::iced::widget::Space::new().width(Length::Fill));
    if let Some(plan) = plan_label.filter(|plan| !plan.trim().is_empty()) {
        label_row = label_row.push(plan_badge(plan));
    }

    let status = account_status_badge(account, provider);
    let mut status_row = row![status].spacing(8).align_y(Alignment::Center);
    let active_id = provider.system_active_account_id.as_deref();
    if active_id == Some(account.account_id.as_str()) {
        status_row = status_row.push(badge_with_tooltip(
            badge_success(fl!("badge-active")),
            fl!("badge-active-tooltip"),
        ));
    }
    if let Some(updated) = account.last_success_at.map(format_updated_label) {
        status_row = status_row.push(cosmic::iced::widget::Space::new().width(Length::Fill));
        status_row = status_row.push(widget::text(updated).size(12));
    }

    column![
        label_row,
        status_row,
        account_card_divider(),
        account_action_button(
            fl!("manage-accounts"),
            Some(Message::NavigateTo(PopupRoute::ManageAccounts(
                provider.provider,
            ))),
        ),
    ]
    .spacing(8)
    .width(Length::Fill)
    .into()
}

#[allow(dead_code)]
fn account_view<'a>(
    account: &'a ProviderAccountRuntimeState,
    provider: &'a ProviderRuntimeState,
    state: &'a AppState,
    config: &'a Config,
    detection: &'a crate::detection::DetectionSnapshot,
    pager: Option<(usize, usize)>,
) -> Element<'a, Message> {
    let header = account_header_content(account, provider);
    let body = account_body_items(Some(account), provider, state, config, detection);
    let mut account_content = column![header].spacing(8).width(Length::Fill);
    if let Some((active, total)) = pager {
        account_content = account_content
            .push(account_card_divider())
            .push(account_pager(account, active, total));
    }
    let account_card: Element<'a, Message> = container(account_content)
        .width(Length::Fill)
        .padding(8)
        .style(component_card_style)
        .into();

    column![
        section_title(fl!("account-label")),
        account_card,
        section_title(fl!("usage-label")),
        usage_card(body),
    ]
    .spacing(PROVIDER_CARD_SPACING)
    .width(Length::Fill)
    .into()
}

#[allow(dead_code)]
fn section_title(label: String) -> Element<'static, Message> {
    widget::text(label).size(15).into()
}

fn account_card_divider() -> Element<'static, Message> {
    container(cosmic::iced::widget::Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|theme: &cosmic::Theme| widget::container::Style {
            text_color: None,
            background: Some(Background::Color(component_divider_color(theme))),
            border: cosmic::iced::Border::default(),
            shadow: cosmic::iced::Shadow::default(),
            icon_color: None,
            snap: true,
        })
        .into()
}

fn usage_card<'a>(items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut content = column![].spacing(14).width(Length::Fill);
    for item in items {
        content = content.push(item);
    }
    container(content).width(Length::Fill).padding(0).into()
}

#[allow(dead_code)]
fn component_card_style(theme: &cosmic::Theme) -> widget::container::Style {
    let mut style = component_container_style(theme);
    style.background = Some(component_card_background(theme));
    style.border.radius = 2.0.into();
    style.border.width = 1.0;
    style.border.color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
    style.text_color = None;
    style.icon_color = None;
    style.snap = false;
    style
}

pub(super) fn empty_state_view<'a>() -> Element<'a, Message> {
    let logos = row![
        widget::icon::icon(provider_icon_handle(
            ProviderId::Claude,
            provider_icon_variant()
        ))
        .size(22),
        widget::icon::icon(app_icon_handle()).size(48),
        widget::icon::icon(provider_icon_handle(
            ProviderId::Codex,
            provider_icon_variant()
        ))
        .size(22),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let content = column![
        logos,
        widget::text(fl!("no-providers")).size(22),
        widget::text(fl!("no-providers-detail")).size(13),
        widget::button::suggested(fl!("no-providers-open-settings"))
            .on_press(Message::NavigateTo(PopupRoute::ManageProviders)),
    ]
    .spacing(12)
    .align_x(Alignment::Center)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(32)
        .into()
}

fn provider_warning_banner(
    provider: &ProviderRuntimeState,
    state: &AppState,
    active_account: Option<&ProviderAccountRuntimeState>,
    detection: &crate::detection::DetectionSnapshot,
) -> Option<Element<'static, Message>> {
    let cursor_reauth_needed = active_account.is_some_and(|a| {
        a.provider == ProviderId::Cursor && a.auth_state == AuthState::ActionRequired
    });

    let accounts = state.accounts_for(provider.provider);
    let warning_text = if cursor_reauth_needed {
        Some(fl!("cursor-account-reauth-detail"))
    } else if let Some(account) = active_account
        && account.health == ProviderHealth::Error
    {
        if account.auth_state == AuthState::ActionRequired {
            Some(fl!("account-reauth-summary"))
        } else if let Some(error) = &account.error {
            Some(error.clone())
        } else {
            Some("Account error or expired".to_string())
        }
    } else if accounts.is_empty() {
        if detected_without_accounts(state, detection, provider.provider) {
            Some(format!(
                "{} detected on machine. Add account in Settings.",
                provider.provider.label()
            ))
        } else {
            Some(format!(
                "No account configured for {}. Add in Settings.",
                provider.provider.label()
            ))
        }
    } else if provider.account_status == AccountSelectionStatus::LoginRequired {
        Some(format!(
            "Login required for {}. Configure in Settings.",
            provider.provider.label()
        ))
    } else {
        None
    };

    let msg = warning_text?;

    Some(
        container(
            row![
                widget::icon::icon(widget::icon::from_name("dialog-warning-symbolic").into())
                    .size(16),
                widget::text(msg).size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(8)
        .style(|_| widget::container::Style {
            text_color: Some(Color::from_rgb(0.95, 0.45, 0.45)),
            background: Some(Background::Color(Color::from_rgba(1.0, 0.35, 0.35, 0.08))),
            border: cosmic::iced::Border {
                radius: 2.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 0.35, 0.35, 0.30),
            },
            ..Default::default()
        })
        .into(),
    )
}

#[allow(dead_code)]
fn provider_status_info(
    provider: &ProviderRuntimeState,
    state: &AppState,
    active_account: Option<&ProviderAccountRuntimeState>,
    detection: &crate::detection::DetectionSnapshot,
) -> Option<Element<'static, Message>> {
    if detected_without_accounts(state, detection, provider.provider) {
        return Some(detected_provider_cta(provider.provider));
    }
    let message = provider_status_message(provider, state, active_account);
    if message.is_empty() {
        return None;
    }
    Some(info_block(
        fl!("status-label"),
        message,
        None,
        login_required_settings_action(provider, state, active_account),
    ))
}

fn detected_provider_cta(provider: ProviderId) -> Element<'static, Message> {
    info_block(
        fl!("provider-detected-chip"),
        fl!("provider-detected-cta", provider = provider.label()),
        None,
        Some(
            widget::button::suggested(fl!("provider-detected-add-account"))
                .on_press(Message::NavigateTo(PopupRoute::ManageAccounts(provider)))
                .into(),
        ),
    )
}

fn login_required_settings_action(
    provider: &ProviderRuntimeState,
    state: &AppState,
    active_account: Option<&ProviderAccountRuntimeState>,
) -> Option<Element<'static, Message>> {
    if !should_show_login_required_settings_action(provider, state, active_account) {
        return None;
    }

    Some(
        widget::button::standard(fl!(
            "open-provider-settings",
            provider = provider.provider.label()
        ))
        .on_press(Message::NavigateTo(PopupRoute::ManageAccounts(
            provider.provider,
        )))
        .into(),
    )
}

fn should_show_login_required_settings_action(
    provider: &ProviderRuntimeState,
    state: &AppState,
    active_account: Option<&ProviderAccountRuntimeState>,
) -> bool {
    active_account.is_none()
        && state.accounts_for(provider.provider).is_empty()
        && provider.account_status == AccountSelectionStatus::LoginRequired
}

#[allow(dead_code)]
fn provider_status_message(
    provider: &ProviderRuntimeState,
    _state: &AppState,
    active_account: Option<&ProviderAccountRuntimeState>,
) -> String {
    let mut messages = Vec::new();

    let cursor_reauth_needed = active_account.is_some_and(|a| {
        a.provider == ProviderId::Cursor && a.auth_state == AuthState::ActionRequired
    });

    if cursor_reauth_needed {
        messages.push(fl!("cursor-account-reauth-detail"));
    } else if let Some(account) = active_account
        && account.health == ProviderHealth::Error
    {
        if account.auth_state == AuthState::ActionRequired {
            messages.push(fl!("account-reauth-summary"));
        } else if let Some(error) = &account.error {
            messages.push(error.clone());
        }
    } else {
        messages.push(provider.status_line(active_account));
    }

    dedup_status_messages(messages).join(" ")
}

#[allow(dead_code)]
fn dedup_status_messages(messages: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for message in messages {
        if !message.is_empty() && !deduped.contains(&message) {
            deduped.push(message);
        }
    }
    deduped
}

fn active_snapshot_for_account<'a>(
    account: Option<&'a ProviderAccountRuntimeState>,
    provider: &'a ProviderRuntimeState,
) -> Option<&'a UsageSnapshot> {
    account
        .and_then(|account| account.snapshot.as_ref())
        .or(provider.legacy_display_snapshot.as_ref())
}

#[allow(dead_code)]
fn window_sections<'a>(
    snapshot: &'a UsageSnapshot,
    config: &'a Config,
) -> Vec<Element<'static, Message>> {
    let mut items = Vec::new();
    if zai_coding_plan_absent(snapshot.provider, &snapshot.windows) {
        items.push(info_block(
            fl!("zai-coding-plan-unavailable-title"),
            fl!("zai-coding-plan-unavailable-detail"),
            None,
            None,
        ));
    }
    let mut windows = snapshot.windows.iter().peekable();
    while let Some(window) = windows.next() {
        let Some(group) = window.group.as_deref() else {
            items.push(card(usage_section_content(
                window,
                snapshot.provider,
                config,
            )));
            continue;
        };
        let mut sections = vec![usage_section_content(window, snapshot.provider, config)];
        while let Some(next) = windows.next_if(|next| next.group.as_deref() == Some(group)) {
            sections.push(usage_section_content(next, snapshot.provider, config));
        }
        items.push(usage_group_card(group, sections));
    }
    items
}

fn zai_coding_plan_absent(provider: ProviderId, windows: &[UsageWindow]) -> bool {
    provider == ProviderId::Zai
        && !windows.is_empty()
        && windows.iter().all(|window| window.label == "MCP")
}

fn usage_group_card(
    group: &str,
    sections: Vec<Element<'static, Message>>,
) -> Element<'static, Message> {
    let mut content = column![widget::text(group.to_string()).size(18)]
        .spacing(PROVIDER_GROUP_SPACING)
        .width(Length::Fill);
    for section in sections {
        content = content.push(section);
    }
    container(content)
        .width(Length::Fill)
        .padding(PROVIDER_GROUP_PADDING / 2.0)
        .style(group_card_style)
        .into()
}

fn group_card_style(theme: &cosmic::Theme) -> widget::container::Style {
    let cosmic = theme.cosmic();
    let mut style = component_card_style(theme);
    style.border.width = 1.0;
    style.border.color = cosmic
        .background(theme.transparent)
        .component
        .divider
        .into();
    style
}

fn usage_section_content(
    window: &UsageWindow,
    provider: ProviderId,
    config: &Config,
) -> Element<'static, Message> {
    let now = chrono::Utc::now();
    usage_block_content(
        window_display_label(provider, &window.label),
        usage_display::usage_amount_label(window, now, config.usage_amount_format),
        UsageBlockDetails {
            secondary: usage_display::reset_label(window, now, config.reset_time_format),
            secondary_tooltip: None,
            meter: usage_display::usage_meter(window, now, config.usage_amount_format),
            overage: overage_text(window),
        },
    )
}

fn window_display_label(provider: ProviderId, label: &str) -> String {
    if provider == ProviderId::Copilot {
        match label {
            "chat" => return fl!("copilot-window-chat"),
            "completions" => return fl!("copilot-window-completions"),
            "premium_interactions" => return fl!("copilot-window-premium"),
            "credits" => return fl!("copilot-window-credits"),
            _ => {}
        }
    }
    label.to_string()
}

fn overage_text(window: &UsageWindow) -> Option<String> {
    if window.label == "premium_interactions" || window.label == "credits" {
        return window.reset_description.clone();
    }
    None
}

#[allow(dead_code)]
fn extra_usage_detail_section(
    state: &ExtraUsageState,
    usage_amount_format: UsageAmountFormat,
) -> Element<'static, Message> {
    match state {
        ExtraUsageState::Disabled => info_block(
            fl!("extra-usage-label"),
            fl!("extra-usage-disabled"),
            None,
            None,
        ),
        ExtraUsageState::Active { used_percent, cost } => {
            extra_usage_cost_bar(cost, Some(*used_percent), usage_amount_format)
        }
    }
}

#[allow(dead_code)]
fn extra_usage_pct_from_cost(cost: &ProviderCost) -> f32 {
    cost.limit
        .filter(|l| *l > f64::EPSILON)
        .map_or(0.0_f32, |l| usage_display::portion_percent(cost.used, l))
}

#[allow(dead_code)]
fn extra_usage_cost_bar(
    cost: &ProviderCost,
    used_percent: Option<f32>,
    usage_amount_format: UsageAmountFormat,
) -> Element<'static, Message> {
    let now = chrono::Utc::now();
    let used_percent = used_percent
        .unwrap_or_else(|| extra_usage_pct_from_cost(cost))
        .clamp(0.0, 100.0);
    let window = UsageWindow {
        label: String::new(),
        used_percent,
        reset_at: None,
        window_seconds: None,
        reset_description: None,
        group: None,
    };
    let (cost_line, cost_tip) = currency_format::format_provider_cost(cost);
    usage_block(
        fl!("extra-usage-label"),
        usage_display::usage_amount_label(&window, now, usage_amount_format),
        UsageBlockDetails {
            secondary: Some(cost_line),
            secondary_tooltip: Some(cost_tip),
            meter: usage_display::usage_meter(&window, now, usage_amount_format),
            overage: None,
        },
    )
}

#[allow(dead_code)]
fn cost_section(provider: ProviderId, cost: &ProviderCost) -> Element<'static, Message> {
    if provider == ProviderId::Codex || provider == ProviderId::Grok {
        return credit_section(cost);
    }
    let (primary, iso_tip) = currency_format::format_provider_cost(cost);
    let body = widget::tooltip::tooltip(
        widget::text(primary).size(14),
        widget::text(iso_tip).size(12),
        widget::tooltip::Position::Top,
    );
    card(column![widget::text(fl!("extra-usage-label")).size(15), body,].spacing(6))
}

fn credit_section(cost: &ProviderCost) -> Element<'static, Message> {
    let balance = if cost.used.fract() == 0.0 {
        format!("{:.0}", cost.used)
    } else {
        format!("{:.2}", cost.used)
    };

    card(
        column![
            widget::text(fl!("credits-label")).size(15),
            widget::text(fl!("credits-available", balance = balance.as_str())).size(14),
        ]
        .spacing(6),
    )
}

fn format_token_metric(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        format!("{tokens}")
    }
}

#[allow(dead_code)]
fn tokens_by_day_card(days: &[crate::model::DayTokenUsage]) -> Element<'static, Message> {
    let peak = days.iter().map(|d| d.token_count).max().unwrap_or(1).max(1);
    let mut rows = column![widget::text("TOKENS BY DAY").size(12)]
        .spacing(8)
        .width(Length::Fill);

    for day in days {
        let pct = (day.token_count as f32 / peak as f32) * 100.0;
        let is_today = day.is_today;

        let label_color = if is_today {
            Color::from_rgb(0.95, 0.95, 0.95)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.50)
        };

        let label = container(widget::text(day.day_label.clone()).size(12))
            .width(Length::Fixed(44.0))
            .style(move |_| widget::container::Style {
                text_color: Some(label_color),
                ..Default::default()
            });

        let bar = usage_progress_bar(usage_display::UsageMeter {
            fill_percent: pct,
            marker_percent: None,
            tooltip: format!("{}: {} tokens", day.day_label, day.token_count),
        });

        let value = container(widget::text(format_token_metric(day.token_count)).size(12))
            .width(Length::Fixed(48.0))
            .align_x(cosmic::iced::alignment::Horizontal::Right)
            .style(move |_| widget::container::Style {
                text_color: Some(label_color),
                ..Default::default()
            });

        rows = rows.push(
            row![label, bar, value]
                .spacing(8)
                .align_y(Alignment::Center)
                .width(Length::Fill),
        );
    }

    card(rows)
}

#[allow(dead_code)]
fn tokens_by_model_card(models: &[crate::model::ModelTokenUsage]) -> Element<'static, Message> {
    let peak = models
        .iter()
        .map(|m| m.token_count)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut rows = column![widget::text("TOKENS BY MODEL").size(12)]
        .spacing(8)
        .width(Length::Fill);

    for model in models {
        let pct = (model.token_count as f32 / peak as f32).clamp(0.0, 100.0);
        let fill = (pct * 10.0).round() as u16;
        let empty = 1000 - fill;

        let fill_segment = container(cosmic::iced::widget::Space::new())
            .width(Length::FillPortion(fill))
            .height(Length::Fill)
            .style(|_| widget::container::Style {
                text_color: None,
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.14))),
                border: cosmic::iced::Border::default(),
                shadow: cosmic::iced::Shadow::default(),
                icon_color: None,
                snap: true,
            });

        let empty_segment = cosmic::iced::widget::Space::new().width(Length::FillPortion(empty));

        let track_row = if fill == 0 {
            row![empty_segment]
        } else if fill >= 1000 {
            row![fill_segment]
        } else {
            row![fill_segment, empty_segment]
        };

        let slider_track = container(track_row)
            .width(Length::Fill)
            .height(Length::Fixed(28.0))
            .style(|_| widget::container::Style {
                text_color: None,
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: cosmic::iced::Border {
                    radius: 2.0.into(),
                    width: 1.0,
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                },
                shadow: cosmic::iced::Shadow::default(),
                icon_color: None,
                snap: true,
            });

        let text_overlay = container(
            row![
                widget::text(model.model_name.clone()).size(12),
                cosmic::iced::widget::Space::new().width(Length::Fill),
                widget::text(format_token_metric(model.token_count)).size(12),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fixed(28.0))
        .padding([0, 10])
        .align_y(cosmic::iced::alignment::Vertical::Center);

        let slider = cosmic::iced::widget::Stack::new()
            .push(slider_track)
            .push(text_overlay)
            .width(Length::Fill)
            .height(Length::Fixed(28.0));

        let slider_with_tip = widget::tooltip::tooltip(
            slider,
            widget::text(format!(
                "{}: {} tokens",
                model.model_name, model.token_count
            ))
            .size(12),
            widget::tooltip::Position::Top,
        );

        rows = rows.push(slider_with_tip);
    }

    card(rows)
}

#[allow(dead_code)]
fn usage_block(
    title: String,
    primary: String,
    details: UsageBlockDetails,
) -> Element<'static, Message> {
    card(usage_block_content(title, primary, details))
}

#[allow(dead_code)]
fn usage_block_content(
    title: String,
    primary: String,
    details: UsageBlockDetails,
) -> Element<'static, Message> {
    let header_row = row![
        widget::text(title).size(14),
        cosmic::iced::widget::Space::new().width(Length::Fill),
        widget::text(primary).size(13),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let bar = usage_progress_bar(details.meter);

    let mut content = column![header_row, bar,].spacing(6).width(Length::Fill);

    if let Some(secondary) = details.secondary
        && !secondary.is_empty()
    {
        let sec_text =
            container(widget::text(secondary).size(11)).style(|_| widget::container::Style {
                text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                ..Default::default()
            });
        content = content.push(sec_text);
    }

    if let Some(overage) = details.overage {
        content = content.push(overage_line(overage));
    }

    content.into()
}

#[allow(dead_code)]
struct UsageBlockDetails {
    secondary: Option<String>,
    #[allow(dead_code)]
    secondary_tooltip: Option<String>,
    meter: usage_display::UsageMeter,
    overage: Option<String>,
}

#[allow(dead_code)]
fn overage_line(text: String) -> Element<'static, Message> {
    container(widget::text(text).size(13))
        .style(|theme: &cosmic::Theme| {
            let color = apply_alpha(theme.cosmic().warning.base.into(), 0.92);
            widget::container::Style {
                text_color: Some(color),
                background: None,
                border: cosmic::iced::Border::default(),
                shadow: cosmic::iced::Shadow::default(),
                icon_color: Some(color),
                snap: true,
            }
        })
        .into()
}

#[allow(dead_code)]
fn secondary_cost_text(text: String, tooltip: Option<String>) -> Element<'static, Message> {
    if text.is_empty() {
        cosmic::iced::widget::Space::new()
            .width(Length::Shrink)
            .into()
    } else if let Some(tip) = tooltip {
        widget::tooltip::tooltip(
            widget::text(text).size(13),
            widget::text(tip).size(12),
            widget::tooltip::Position::Top,
        )
        .into()
    } else {
        widget::text(text).size(13).into()
    }
}

fn usage_progress_bar(meter: usage_display::UsageMeter) -> Element<'static, Message> {
    let fill = (meter.fill_percent.clamp(0.0, 100.0) * 10.0).round() as u16;
    let empty = 1000 - fill;

    let fill_segment = container(cosmic::iced::widget::Space::new())
        .width(Length::FillPortion(fill))
        .height(Length::Fixed(6.0))
        .style(|_theme: &cosmic::Theme| widget::container::Style {
            text_color: None,
            background: Some(Background::Color(Color::from_rgb(0.92, 0.92, 0.92))),
            border: cosmic::iced::Border {
                radius: 1.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: cosmic::iced::Shadow::default(),
            icon_color: None,
            snap: true,
        });

    let empty_segment = cosmic::iced::widget::Space::new().width(Length::FillPortion(empty));

    let bar_row = if fill == 0 {
        row![empty_segment]
    } else if fill >= 1000 {
        row![fill_segment]
    } else {
        row![fill_segment, empty_segment]
    };

    let track = container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(8.0))
        .padding([1, 1])
        .style(|_theme: &cosmic::Theme| widget::container::Style {
            text_color: None,
            background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.14))),
            border: cosmic::iced::Border {
                radius: 2.0.into(),
                width: 1.0,
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
            },
            shadow: cosmic::iced::Shadow::default(),
            icon_color: None,
            snap: true,
        });

    let bar: Element<'static, Message> = if let Some(marker_percent) = meter.marker_percent {
        cosmic::iced::widget::Stack::new()
            .push(track)
            .push(pace_marker(marker_percent))
            .width(Length::Fill)
            .height(Length::Fixed(8.0))
            .into()
    } else {
        track.into()
    };

    widget::tooltip::tooltip(
        bar,
        widget::text(meter.tooltip).size(12),
        widget::tooltip::Position::Top,
    )
    .into()
}

fn pace_marker(expected_percent: f32) -> Element<'static, Message> {
    let left = pace_marker_portion(expected_percent);
    let right = 1000 - left;
    row![
        cosmic::iced::widget::Space::new().width(Length::FillPortion(left)),
        container(cosmic::iced::widget::Space::new())
            .width(Length::Fixed(2.0))
            .height(Length::Fixed(8.0))
            .style(|_theme: &cosmic::Theme| {
                widget::container::Style {
                    text_color: None,
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.6))),
                    border: cosmic::iced::Border::default(),
                    shadow: cosmic::iced::Shadow::default(),
                    icon_color: None,
                    snap: true,
                }
            }),
        cosmic::iced::widget::Space::new().width(Length::FillPortion(right)),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(8.0))
    .into()
}

fn pace_marker_portion(expected_percent: f32) -> u16 {
    let scaled = (expected_percent * 10.0).clamp(1.0, 999.0);
    let mut portion = 1u16;
    while portion < 999 && f32::from(portion) + 0.5 <= scaled {
        portion += 1;
    }
    portion
}

fn account_status_badge(
    account: &ProviderAccountRuntimeState,
    provider: &ProviderRuntimeState,
) -> Element<'static, Message> {
    if provider.is_refreshing {
        return badge_with_tooltip(
            badge_neutral(fl!("badge-refreshing")),
            fl!("badge-refreshing-tooltip"),
        );
    }
    if account.auth_state == AuthState::ActionRequired {
        if account.provider == ProviderId::Cursor {
            return badge_with_tooltip(
                badge_neutral(fl!("badge-cursor-reauth-needed")),
                fl!("badge-cursor-reauth-needed-tooltip"),
            );
        }
        return badge_with_tooltip(
            badge_warning(fl!("badge-login-required")),
            fl!("badge-login-required-tooltip"),
        );
    }
    if account.health == ProviderHealth::Error {
        return badge_with_tooltip(
            badge_destructive(fl!("badge-error")),
            fl!("badge-error-tooltip"),
        );
    }
    let now = chrono::Utc::now();
    if account.health == ProviderHealth::Ok
        && account.snapshot.is_some()
        && account
            .last_success_at
            .is_some_and(|updated| now - updated < STALE_THRESHOLD)
    {
        return badge_with_tooltip(badge_success(fl!("badge-live")), fl!("badge-live-tooltip"));
    }
    if account.snapshot.is_some() {
        return badge_with_tooltip(
            badge_warning(fl!("badge-stale")),
            fl!("badge-stale-tooltip"),
        );
    }
    badge_with_tooltip(
        badge_neutral(fl!("badge-loading")),
        fl!("badge-loading-tooltip"),
    )
}

fn format_updated_label(last_success_at: chrono::DateTime<chrono::Utc>) -> String {
    let age = chrono::Utc::now() - last_success_at;
    if age.num_seconds() < 10 {
        fl!("updated-just-now")
    } else if age.num_minutes() < 1 {
        fl!("updated-seconds-ago", n = age.num_seconds())
    } else if age.num_hours() < 1 {
        fl!("updated-minutes-ago", n = age.num_minutes())
    } else if age.num_days() < 1 {
        fl!("updated-hours-ago", n = age.num_hours())
    } else {
        let date = last_success_at.format("%Y-%m-%d %H:%M").to_string();
        fl!("updated-at", date = date.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountSelectionStatus, AuthState, ProviderHealth};

    fn state_with_elevenlabs_snapshot() -> AppState {
        let mut account =
            ProviderAccountRuntimeState::empty(ProviderId::ElevenLabs, "el-1", "ElevenLabs");
        account.snapshot = Some(
            crate::providers::elevenlabs::quota::parse(
                r#"{"tier":"starter","character_count":13903,"character_limit":30010}"#,
                chrono::Utc::now(),
            )
            .expect("parses"),
        );
        let mut state = AppState::empty();
        state.upsert_provider(ProviderRuntimeState::empty(ProviderId::ElevenLabs));
        state.provider_accounts.push(account);
        state
    }

    #[test]
    fn elevenlabs_ring_shows_on_every_other_provider_when_on() {
        let mut state = state_with_elevenlabs_snapshot();
        let mut config = Config::default();
        assert!(elevenlabs_ring(ProviderId::Claude, &state, &config).is_none());

        config.elevenlabs_ring_enabled = true;
        assert!(elevenlabs_ring(ProviderId::Claude, &state, &config).is_some());
        assert!(elevenlabs_ring(ProviderId::Codex, &state, &config).is_some());
        assert!(elevenlabs_ring(ProviderId::ElevenLabs, &state, &config).is_none());

        state.upsert_provider(ProviderRuntimeState {
            enabled: false,
            ..ProviderRuntimeState::empty(ProviderId::ElevenLabs)
        });
        assert!(elevenlabs_ring(ProviderId::Claude, &state, &config).is_none());
    }

    #[test]
    fn credit_ring_arc_tracks_used_share() {
        let empty = String::from_utf8(credit_ring_svg(0.0)).expect("utf8");
        assert!(!empty.contains("stroke-dasharray"));

        let half = String::from_utf8(credit_ring_svg(50.0)).expect("utf8");
        // Half of 2π·13.5 ≈ 84.82.
        assert!(half.contains(r#"stroke-dasharray="42.41 84.82""#));

        let over = String::from_utf8(credit_ring_svg(140.0)).expect("utf8");
        assert!(over.contains(r#"stroke-dasharray="84.82 84.82""#));
    }

    #[test]
    fn elevenlabs_credits_row_shows_grouped_raw_counts() {
        let snapshot = crate::providers::elevenlabs::quota::parse(
            r#"{"tier":"creator","character_count":8200,"character_limit":30000}"#,
            chrono::Utc::now(),
        )
        .expect("parses");
        let window = &snapshot.windows[0];
        assert_eq!(window_value_label(&snapshot, window), "8,200 / 30,000");
        assert_eq!(session_row_title(ProviderId::ElevenLabs, window), "Credits");
        assert_eq!(session_row_title(ProviderId::Codex, window), "Session");
        assert_eq!(group_thousands(999.0), "999");
        assert_eq!(group_thousands(1_234_567.0), "1,234,567");
    }

    fn limit_window(label: &str, window_seconds: Option<i64>) -> UsageWindow {
        UsageWindow {
            label: label.to_string(),
            used_percent: 40.0,
            reset_at: None,
            window_seconds,
            reset_description: None,
            group: None,
        }
    }

    #[test]
    fn single_weekly_window_is_not_repeated_as_session() {
        let windows = vec![limit_window("Weekly", Some(7 * 24 * 3600))];
        let (session, weekly) = limit_windows(&windows);
        assert!(session.is_none());
        assert_eq!(weekly.map(|w| w.label.as_str()), Some("Weekly"));
    }

    #[test]
    fn single_session_window_is_not_repeated_as_weekly() {
        let windows = vec![limit_window("Session", Some(5 * 3600))];
        let (session, weekly) = limit_windows(&windows);
        assert_eq!(session.map(|w| w.label.as_str()), Some("Session"));
        assert!(weekly.is_none());
    }

    #[test]
    fn session_and_weekly_windows_both_shown() {
        let windows = vec![
            limit_window("Session", Some(5 * 3600)),
            limit_window("Weekly", Some(7 * 24 * 3600)),
        ];
        let (session, weekly) = limit_windows(&windows);
        assert_eq!(session.map(|w| w.label.as_str()), Some("Session"));
        assert_eq!(weekly.map(|w| w.label.as_str()), Some("Weekly"));
    }

    #[test]
    fn zai_coding_plan_notice_only_for_mcp_only_usage() {
        let mut windows = vec![UsageWindow {
            label: "MCP".to_string(),
            used_percent: 35.0,
            reset_at: None,
            window_seconds: None,
            reset_description: None,
            group: None,
        }];
        assert!(zai_coding_plan_absent(ProviderId::Zai, &windows));
        assert_eq!(
            fl!("zai-coding-plan-unavailable-detail"),
            "Coding Plan quotas were not reported by Z.AI."
        );
        assert!(!zai_coding_plan_absent(ProviderId::Minimax, &windows));
        assert!(!zai_coding_plan_absent(ProviderId::Zai, &[]));

        let mut coding_plan = windows[0].clone();
        coding_plan.label = "Weekly".to_string();
        windows.push(coding_plan);
        assert!(!zai_coding_plan_absent(ProviderId::Zai, &windows));
        windows[1].label = "5 Hour".to_string();
        assert!(!zai_coding_plan_absent(ProviderId::Zai, &windows));
        windows.remove(0);
        assert!(!zai_coding_plan_absent(ProviderId::Zai, &windows));
    }

    #[test]
    fn action_required_account_reports_reauth_message() {
        let provider = ProviderRuntimeState {
            provider: ProviderId::Claude,
            enabled: true,
            selected_account_ids: vec!["claude-1".to_string()],
            active_account_id: Some("claude-1".to_string()),
            system_active_account_id: None,
            account_status: AccountSelectionStatus::Ready,
            is_refreshing: false,
            refresh_started_at: None,
            legacy_display_snapshot: None,
            error: None,
        };
        let mut account =
            ProviderAccountRuntimeState::empty(ProviderId::Claude, "claude-1", "Claude account");
        account.health = ProviderHealth::Error;
        account.auth_state = AuthState::ActionRequired;
        account.error = Some("claude token refresh returned http 400".to_string());
        let state = AppState::empty();

        let message = provider_status_message(&provider, &state, Some(&account));
        assert!(
            !message.contains("http 400"),
            "status message must not contain raw error: {message}"
        );
        assert!(
            message.contains("Re-authenticate") || message.contains("Settings"),
            "status message should be action-oriented: {message}"
        );
    }

    #[test]
    fn cursor_action_required_account_reports_reauth_needed_message() {
        let provider = ProviderRuntimeState {
            provider: ProviderId::Cursor,
            enabled: true,
            selected_account_ids: vec!["cursor-1".to_string()],
            active_account_id: Some("cursor-1".to_string()),
            system_active_account_id: Some("cursor-1".to_string()),
            account_status: AccountSelectionStatus::Ready,
            is_refreshing: false,
            refresh_started_at: None,
            legacy_display_snapshot: None,
            error: None,
        };
        let mut account =
            ProviderAccountRuntimeState::empty(ProviderId::Cursor, "cursor-1", "user@example.com");
        account.health = ProviderHealth::Error;
        account.auth_state = AuthState::ActionRequired;
        account.error = Some("Unauthorized".to_string());
        let state = AppState::empty();

        let message = provider_status_message(&provider, &state, Some(&account));

        assert!(
            message.contains("Re-authenticate") || message.contains("rescan"),
            "status message should explain the Cursor recovery path: {message}"
        );
        assert!(
            !message.contains("Inactive") && !message.contains("inactive"),
            "status message must not describe auth failures as inactive: {message}"
        );
    }

    #[test]
    fn cursor_action_required_badge_copy_uses_reauth_needed() {
        assert_eq!(fl!("badge-cursor-reauth-needed"), "Re-auth needed");
        assert!(!fl!("badge-cursor-reauth-needed-tooltip").contains("inactive"));
    }

    #[test]
    fn codex_without_accounts_reports_login_required() {
        let provider = ProviderRuntimeState {
            provider: ProviderId::Codex,
            enabled: true,
            selected_account_ids: Vec::new(),
            active_account_id: None,
            system_active_account_id: None,
            account_status: AccountSelectionStatus::LoginRequired,
            is_refreshing: false,
            refresh_started_at: None,
            legacy_display_snapshot: None,
            error: Some("Login required".to_string()),
        };
        let state = AppState::empty();

        assert_eq!(
            provider_status_message(&provider, &state, None),
            "Login required"
        );
    }

    #[test]
    fn login_required_empty_state_links_to_each_provider_settings_page() {
        for provider_id in ProviderId::ALL {
            let provider = ProviderRuntimeState {
                provider: provider_id,
                enabled: true,
                selected_account_ids: Vec::new(),
                active_account_id: None,
                system_active_account_id: None,
                account_status: AccountSelectionStatus::LoginRequired,
                is_refreshing: false,
                refresh_started_at: None,
                legacy_display_snapshot: None,
                error: Some("Login required".to_string()),
            };
            let state = AppState::empty();

            assert!(should_show_login_required_settings_action(
                &provider, &state, None
            ));
        }
    }

    #[test]
    fn login_required_settings_action_hides_when_account_exists() {
        let provider = ProviderRuntimeState {
            provider: ProviderId::Codex,
            enabled: true,
            selected_account_ids: Vec::new(),
            active_account_id: None,
            system_active_account_id: None,
            account_status: AccountSelectionStatus::LoginRequired,
            is_refreshing: false,
            refresh_started_at: None,
            legacy_display_snapshot: None,
            error: Some("Login required".to_string()),
        };
        let mut state = AppState::empty();
        state
            .provider_accounts
            .push(ProviderAccountRuntimeState::empty(
                ProviderId::Codex,
                "codex-test",
                "test@example.com",
            ));

        assert!(!should_show_login_required_settings_action(
            &provider, &state, None
        ));
    }

    #[test]
    fn detected_provider_without_accounts_shows_detected_cta() {
        let home = tempfile::tempdir().expect("create temporary home");
        std::fs::create_dir(home.path().join(".codex")).expect("create Codex marker");
        let detection = crate::detection::detect(home.path());
        let state = AppState::empty();

        assert!(detected_without_accounts(
            &state,
            &detection,
            ProviderId::Codex
        ));
        assert!(!detected_without_accounts(
            &state,
            &detection,
            ProviderId::Claude
        ));

        let mut with_account = state.clone();
        with_account
            .provider_accounts
            .push(ProviderAccountRuntimeState::empty(
                ProviderId::Codex,
                "codex-test",
                "test@example.com",
            ));
        assert!(!detected_without_accounts(
            &with_account,
            &detection,
            ProviderId::Codex
        ));
    }
}
