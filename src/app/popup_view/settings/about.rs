use super::super::super::provider_assets::app_icon_handle;
use super::super::{
    Alignment, Background, Color, Element, Length, Message, UpdateStatus, apply_alpha,
    component_container_style, component_divider_color, component_hover_color, component_on_color,
    component_surface_color, container, fl, widget,
};

const REPOSITORY_URL: &str = "https://github.com/chefn3ff/cosmic-applet-ledger";

pub(super) fn about_view(update_status: &UpdateStatus) -> Element<'static, Message> {
    let version = env!("CARGO_PKG_VERSION");
    let mut content = cosmic::iced::widget::column![about_identity(version)]
        .spacing(12)
        .width(Length::Fill);

    if let Some(update) = update_section(update_status) {
        content = content.push(update);
    }

    content.into()
}

fn about_identity(version: &str) -> Element<'static, Message> {
    let identity_col = cosmic::iced::widget::column![
        widget::icon::icon(app_icon_handle()).size(48),
        widget::text(fl!("app-title")).size(20),
        container(widget::text(format!("v{version}")).size(12)).style(|_| {
            widget::container::Style {
                text_color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                ..Default::default()
            }
        }),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .width(Length::Fill);

    let app_btn = widget::button::custom(identity_col)
        .padding([8, 0])
        .width(Length::Fill)
        .class(cosmic::theme::Button::Custom {
            active: Box::new(|_focused, _theme| widget::button::Style {
                background: None,
                border_radius: 0.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                text_color: Some(Color::WHITE),
                icon_color: Some(Color::WHITE),
                ..Default::default()
            }),
            disabled: Box::new(|_| widget::button::Style::new()),
            hovered: Box::new(|_focused, _theme| widget::button::Style {
                background: None,
                border_radius: 0.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                text_color: Some(Color::WHITE),
                icon_color: Some(Color::WHITE),
                ..Default::default()
            }),
            pressed: Box::new(|_focused, _theme| widget::button::Style {
                background: None,
                border_radius: 0.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                text_color: Some(Color::WHITE),
                icon_color: Some(Color::WHITE),
                ..Default::default()
            }),
        })
        .on_press(Message::OpenUrl(REPOSITORY_URL.to_string()));

    app_btn.into()
}

#[allow(dead_code)]
fn version_badge(version: String) -> Element<'static, Message> {
    container(widget::text(version).size(12))
        .padding([3, 8])
        .style(|theme| {
            let mut style = component_container_style(theme);
            style.background = Some(Background::Color(apply_alpha(
                component_surface_color(theme),
                0.82,
            )));
            style.border.width = 0.0;
            style.border.color = Color::TRANSPARENT;
            style.icon_color = None;
            style.text_color = Some(component_on_color(theme));
            style
        })
        .into()
}

fn update_section(update_status: &UpdateStatus) -> Option<Element<'static, Message>> {
    match update_status {
        UpdateStatus::UpdateAvailable { version, url } => Some(
            widget::button::custom(
                cosmic::iced::widget::column![
                    widget::text(fl!("update-available-title")).size(14),
                    widget::text(fl!("update-available-detail", version = version)).size(13),
                    widget::text(fl!("update-open-release", version = version))
                        .size(13)
                        .class(cosmic::theme::Text::Accent),
                ]
                .spacing(6)
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding(12)
            .class(update_available_button_class())
            .on_press(Message::OpenUrl(url.clone()))
            .into(),
        ),
        UpdateStatus::Unchecked => Some(update_status_text(fl!("update-checking"))),
        UpdateStatus::NoUpdate => None,
        UpdateStatus::Error(reason) => Some(
            cosmic::iced::widget::column![
                widget::text(fl!("update-failed", reason = reason)).size(13),
                widget::button::text(fl!("update-check-again")).on_press(Message::CheckUpdates),
            ]
            .spacing(6)
            .into(),
        ),
    }
}

fn update_status_text(label: String) -> Element<'static, Message> {
    container(widget::text(label).size(13))
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
}

fn update_available_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, theme| update_available_button_style(theme, false)),
        disabled: Box::new(|theme| update_available_button_style(theme, false)),
        hovered: Box::new(|_focused, theme| update_available_button_style(theme, true)),
        pressed: Box::new(|_focused, theme| update_available_button_style(theme, true)),
    }
}

fn update_available_button_style(
    theme: &cosmic::Theme,
    hovered: bool,
) -> cosmic::widget::button::Style {
    let cosmic = theme.cosmic();
    let color: Color = cosmic.destructive.base.into();
    let mut style = cosmic::widget::button::Style::new();
    style.text_color = Some(cosmic.background(theme.transparent).on.into());
    style.icon_color = Some(color);
    style.background = Some(Background::Color(apply_alpha(
        color,
        if hovered { 0.36 } else { 0.28 },
    )));
    style.border_radius = 2.0.into();
    style.border_width = 1.0;
    style.border_color = apply_alpha(color, 0.75);
    style
}

#[allow(dead_code)]
fn link_section<const N: usize>(
    title: String,
    links: [(String, &str); N],
) -> Element<'static, Message> {
    let mut rows = cosmic::iced::widget::column![].width(Length::Fill);
    for (index, (label, url)) in links.into_iter().enumerate() {
        if index > 0 {
            rows = rows.push(link_divider());
        }
        rows = rows.push(link_button(label, url));
    }

    let group: Element<'static, Message> = container(rows)
        .width(Length::Fill)
        .style(link_group_style)
        .into();
    cosmic::iced::widget::column![widget::text(title).size(13), group]
        .spacing(5)
        .width(Length::Fill)
        .into()
}

#[allow(dead_code)]
fn link_button(label: String, url: &str) -> Element<'static, Message> {
    widget::button::custom(
        cosmic::iced::widget::row![
            widget::text(label).size(13),
            cosmic::iced::widget::Space::new().width(Length::Fill),
            widget::icon::from_name("link-symbolic").icon().size(16),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([11, 12])
    .class(link_button_class())
    .on_press(Message::OpenUrl(url.to_string()))
    .into()
}

#[allow(dead_code)]
fn link_divider() -> Element<'static, Message> {
    container(cosmic::iced::widget::Space::new().height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(|theme: &cosmic::Theme| cosmic::widget::container::Style {
            text_color: None,
            background: Some(Background::Color(component_divider_color(theme))),
            border: cosmic::iced::Border::default(),
            shadow: cosmic::iced::Shadow::default(),
            icon_color: None,
            snap: true,
        })
        .into()
}

#[allow(dead_code)]
fn link_group_style(theme: &cosmic::Theme) -> cosmic::widget::container::Style {
    let mut style = component_container_style(theme);
    style.border.width = 0.0;
    style.border.color = Color::TRANSPARENT;
    style.text_color = Some(component_on_color(theme));
    style.icon_color = Some(component_on_color(theme));
    style
}

#[allow(dead_code)]
fn link_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, theme| link_button_style(theme, false)),
        disabled: Box::new(|theme| link_button_style(theme, false)),
        hovered: Box::new(|_focused, theme| link_button_style(theme, true)),
        pressed: Box::new(|_focused, theme| link_button_style(theme, true)),
    }
}

#[allow(dead_code)]
fn link_button_style(theme: &cosmic::Theme, hovered: bool) -> cosmic::widget::button::Style {
    let _cosmic = theme.cosmic();
    let mut style = cosmic::widget::button::Style::new();
    style.background = hovered.then(|| Background::Color(component_hover_color(theme)));
    style.text_color = Some(component_on_color(theme));
    style.icon_color = Some(component_on_color(theme));
    style.border_radius = 2.0.into();
    style
}
