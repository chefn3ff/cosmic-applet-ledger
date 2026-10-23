// SPDX-License-Identifier: MPL-2.0

//! A wrapper that takes up a fixed footprint in layout but lets its content
//! be larger, centred on that footprint and spilling past it. iced's own
//! `float` would do this, but libcosmic's theme doesn't implement its style
//! catalog.

use cosmic::iced::advanced::layout::{self, Layout};
use cosmic::iced::advanced::renderer;
use cosmic::iced::advanced::widget::{self, Tree, Widget};
use cosmic::iced::advanced::{Clipboard, Shell, mouse, overlay};
use cosmic::iced::{Element, Event, Length, Rectangle, Size, Vector};

pub struct Overhang<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    footprint: Size,
    nudge: Vector,
}

/// Lays `content` out at its own size, but reports only `footprint` to the
/// parent, so neighbours are placed as if the content were that small.
pub fn overhang<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    footprint: Size,
) -> Overhang<'a, Message, Theme, Renderer> {
    Overhang {
        content: content.into(),
        footprint,
        nudge: Vector::ZERO,
    }
}

impl<Message, Theme, Renderer> Overhang<'_, Message, Theme, Renderer> {
    /// Shifts the content off centre, e.g. down so it isn't clipped by the
    /// edge of whatever sits above it.
    #[must_use]
    pub fn nudge(mut self, nudge: Vector) -> Self {
        self.nudge = nudge;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Overhang<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.content.as_widget_mut().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.footprint.width),
            Length::Fixed(self.footprint.height),
        )
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        let unbounded = layout::Limits::new(Size::ZERO, Size::INFINITE);
        let content = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &unbounded);
        let offset = (self.footprint - content.size()) * 0.5;
        layout::Node::with_children(
            self.footprint,
            vec![content.move_to((offset.width + self.nudge.x, offset.height + self.nudge.y))],
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(content) = layout.children().next() {
            self.content.as_widget_mut().update(
                tree, event, content, cursor, renderer, clipboard, shell, viewport,
            );
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(content) = layout.children().next() {
            self.content
                .as_widget()
                .draw(tree, renderer, theme, style, content, cursor, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map_or_else(mouse::Interaction::default, |content| {
                self.content
                    .as_widget()
                    .mouse_interaction(tree, content, cursor, viewport, renderer)
            })
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(content) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, content, renderer, operation);
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let content = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, content, renderer, viewport, offset)
    }
}

impl<'a, Message, Theme, Renderer> From<Overhang<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(overhang: Overhang<'a, Message, Theme, Renderer>) -> Self {
        Element::new(overhang)
    }
}
