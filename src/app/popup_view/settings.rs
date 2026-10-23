mod about;
mod accounts;
mod general;

use super::{
    Alignment, AppState, Background, Color, Config, DetectionSnapshot, Element, Length, Message,
    PanelIconStyle, PopupRoute, ProviderId, ProviderLoginStates, UpdateStatus,
    component_container_style, component_divider_color, container, fl, provider_icon_handle,
    provider_icon_variant, row, widget,
};

pub(super) fn settings_view(
    state: &AppState,
    config: &Config,
    update_status: &UpdateStatus,
) -> Element<'static, Message> {
    let providers_label =
        container(widget::text("PROVIDERS").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let providers = ProviderId::ALL;
    let mut provider_rows = cosmic::iced::widget::column![]
        .spacing(6)
        .width(Length::Fill);

    for provider_id in providers {
        let enabled = state.provider(provider_id).is_some_and(|p| p.enabled);

        let icon =
            widget::icon::icon(provider_icon_handle(provider_id, provider_icon_variant())).size(18);

        let name = widget::text(provider_id.label()).size(14);

        let off_btn =
            widget::button::custom(container(widget::text("OFF").size(11)).padding([2, 8]))
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

        let on_btn = widget::button::custom(container(widget::text("ON").size(11)).padding([2, 8]))
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

        let left_button_content = row![icon, name].spacing(10).align_y(Alignment::Center);

        let provider_nav_btn = widget::button::custom(left_button_content)
            .padding([8, 10])
            .width(Length::Fill)
            .class(cosmic::theme::Button::Custom {
                active: Box::new(|_focused, _theme| widget::button::Style {
                    text_color: Some(Color::from_rgb(0.95, 0.95, 0.95)),
                    icon_color: Some(Color::from_rgb(0.95, 0.95, 0.95)),
                    ..Default::default()
                }),
                disabled: Box::new(|_| widget::button::Style::new()),
                hovered: Box::new(|_focused, _theme| widget::button::Style {
                    text_color: Some(Color::WHITE),
                    icon_color: Some(Color::WHITE),
                    ..Default::default()
                }),
                pressed: Box::new(|_focused, _theme| widget::button::Style {
                    text_color: Some(Color::WHITE),
                    icon_color: Some(Color::WHITE),
                    ..Default::default()
                }),
            })
            .on_press(Message::NavigateTo(PopupRoute::ManageAccounts(provider_id)));

        let row_container = container(
            row![
                provider_nav_btn,
                container(row![off_btn, on_btn].spacing(4)).padding([0, 10]),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
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

        provider_rows = provider_rows.push(row_container);
    }

    let divider = || {
        container(cosmic::iced::widget::Space::new().height(Length::Fixed(1.0)))
            .width(Length::Fill)
            .style(|_| widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                ..Default::default()
            })
    };

    let preferences_label =
        container(widget::text("PREFERENCES").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let bars_enabled = config.panel_icon_style != PanelIconStyle::IconOnly;
    let off_btn = widget::button::custom(container(widget::text("OFF").size(12)).padding([3, 10]))
        .padding(0)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(move |_focused, _theme| widget::button::Style {
                background: if !bars_enabled {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15)))
                } else {
                    None
                },
                text_color: Some(if !bars_enabled {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                }),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: if !bars_enabled {
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
        .on_press(Message::SetPanelIconStyle(PanelIconStyle::IconOnly));

    let on_btn = widget::button::custom(container(widget::text("ON").size(12)).padding([3, 10]))
        .padding(0)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(move |_focused, _theme| widget::button::Style {
                background: if bars_enabled {
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15)))
                } else {
                    None
                },
                text_color: Some(if bars_enabled {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                }),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: if bars_enabled {
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
        .on_press(Message::SetPanelIconStyle(PanelIconStyle::LogoAndBars));

    let panel_bars_row = row![
        cosmic::iced::widget::column![
            widget::text("Panel Status Bars").size(14),
            container(widget::text("Show usage meters in panel when minimized").size(11)).style(
                |_| widget::container::Style {
                    text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                    ..Default::default()
                }
            )
        ]
        .spacing(2),
        cosmic::iced::widget::Space::new().width(Length::Fill),
        row![off_btn, on_btn].spacing(4),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    // Provider usage endpoints rate-limit aggressive polling, so the shortest
    // interval offered is one minute.
    let refresh_options: &[(u64, &str)] = &[(60, "1m"), (300, "5m"), (900, "15m"), (1800, "30m")];
    let refresh_row = row![
        widget::text("Refresh Interval").size(14),
        cosmic::iced::widget::Space::new().width(Length::Fill),
        refresh_options
            .iter()
            .fold(row![].spacing(4), |r, (secs, label)| {
                let is_selected = *secs == config.refresh_interval_seconds;
                let btn = widget::button::custom(
                    container(widget::text(*label).size(12)).padding([3, 8]),
                )
                .padding(0)
                .class(cosmic::theme::Button::Custom {
                    active: Box::new(move |_focused, _theme| widget::button::Style {
                        background: if is_selected {
                            Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15)))
                        } else {
                            None
                        },
                        text_color: Some(if is_selected {
                            Color::WHITE
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                        }),
                        border_radius: 2.0.into(),
                        border_width: 1.0,
                        border_color: if is_selected {
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
                .on_press(Message::SetRefreshInterval(*secs));

                r.push(btn)
            })
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut preference_rows = cosmic::iced::widget::column![panel_bars_row, refresh_row]
        .spacing(12)
        .width(Length::Fill);
    if state
        .provider(ProviderId::ElevenLabs)
        .is_some_and(|elevenlabs| elevenlabs.enabled)
    {
        preference_rows = preference_rows.push(elevenlabs_ring_row(config.elevenlabs_ring_enabled));
    }

    let preferences_container = container(preference_rows)
        .padding([10, 12])
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

    let about_label =
        container(widget::text("ABOUT").size(11)).style(|_| widget::container::Style {
            text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
            ..Default::default()
        });

    let about_content = about::about_view(update_status);

    cosmic::iced::widget::column![
        providers_label,
        provider_rows,
        divider(),
        preferences_label,
        preferences_container,
        divider(),
        about_label,
        about_content,
    ]
    .spacing(14)
    .width(Length::Fill)
    .into()
}

/// PREFERENCES row for the ElevenLabs credits ring in other providers' headers.
fn elevenlabs_ring_row(enabled: bool) -> Element<'static, Message> {
    row![
        cosmic::iced::widget::column![
            widget::text("ElevenLabs Credits Ring").size(14),
            container(widget::text("Show ElevenLabs credits on every provider").size(11)).style(
                |_| widget::container::Style {
                    text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                    ..Default::default()
                }
            )
        ]
        .spacing(2),
        cosmic::iced::widget::Space::new().width(Length::Fill),
        row![
            preference_choice_button("OFF", !enabled, Message::SetElevenLabsRingEnabled(false)),
            preference_choice_button("ON", enabled, Message::SetElevenLabsRingEnabled(true)),
        ]
        .spacing(4),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// One half of a PREFERENCES OFF/ON pair, styled like Panel Status Bars.
fn preference_choice_button(
    label: &'static str,
    selected: bool,
    message: Message,
) -> Element<'static, Message> {
    widget::button::custom(container(widget::text(label).size(12)).padding([3, 10]))
        .padding(0)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(move |_focused, _theme| widget::button::Style {
                background: selected
                    .then(|| Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.15))),
                text_color: Some(if selected {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.45)
                }),
                border_radius: 2.0.into(),
                border_width: 1.0,
                border_color: if selected {
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
        .on_press(message)
        .into()
}

#[allow(dead_code)]
pub(super) fn general_settings_view<'a>(config: &'a Config) -> Element<'a, Message> {
    general::general_settings_view(config)
}

pub(super) fn manage_providers_view(state: &AppState) -> Element<'static, Message> {
    let providers = ProviderId::ALL;
    let mut rows = cosmic::iced::widget::column![].width(Length::Fill);
    for (index, provider_id) in providers.into_iter().enumerate() {
        let enabled = state
            .provider(provider_id)
            .is_some_and(|provider| provider.enabled);
        rows = rows.push(manage_provider_row(provider_id, enabled));
        if index + 1 < providers.len() {
            rows = rows.push(manage_provider_divider());
        }
    }
    Element::from(
        cosmic::iced::widget::column![
            cosmic::widget::text(fl!("manage-providers")).size(20),
            container(rows)
                .width(Length::Fill)
                .style(manage_provider_list_style),
        ]
        .spacing(16)
        .width(Length::Fill),
    )
}

fn manage_provider_row(provider: ProviderId, enabled: bool) -> Element<'static, Message> {
    container(
        cosmic::iced::widget::row![
            widget::icon::icon(provider_icon_handle(provider, provider_icon_variant())).size(20),
            cosmic::widget::text(provider.label()).size(16),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            cosmic::widget::toggler(enabled)
                .on_toggle(move |enabled| Message::SetProviderEnabled(provider, enabled)),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([12, 12])
    .width(Length::Fill)
    .into()
}

fn manage_provider_divider() -> Element<'static, Message> {
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

fn manage_provider_list_style(theme: &cosmic::Theme) -> widget::container::Style {
    component_container_style(theme)
}

pub(super) fn about_view(update_status: &UpdateStatus) -> Element<'static, Message> {
    about::about_view(update_status)
}

pub(super) fn provider_settings_view<'a>(
    state: &'a AppState,
    config: &'a Config,
    detection: &'a DetectionSnapshot,
    logins: ProviderLoginStates<'a>,
    provider_id: ProviderId,
) -> Element<'a, Message> {
    accounts::provider_settings_view(state, config, detection, logins, provider_id)
}
