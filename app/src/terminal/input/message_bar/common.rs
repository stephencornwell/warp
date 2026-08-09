use warp_core::ui::appearance::Appearance;
use warp_core::ui::theme::Fill;
use warp_core::ui::Icon;
use warpui::elements::{
    Border, CacheOption, Container, CornerRadius, Element, Hoverable, Image,
    ParentElement, Radius,
};
use warpui::platform::Cursor;
use warpui::prelude::{ConstrainedBox, CrossAxisAlignment, Flex, Text};
use warpui::ui_components::keyboard_shortcut::keystroke_to_keys;
use warpui::{AppContext, SingletonEntity};

use crate::terminal::input::message_bar::{Message, MessageItem};
use crate::ui_components::blended_colors;

pub fn standard_message_bar_height(app: &AppContext) -> f32 {
    let appearance = Appearance::as_ref(app);
    app.font_cache()
        .line_height(styles::font_size(app), appearance.line_height_ratio())
        + styles::VERTICAL_PADDING * 2.
}

/// Renders a sequence of message items into a flex row.
/// Currently used for agent message bar items and zero state message bar items.
fn render_message_bar_items(items: &[MessageItem], app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let default_font_color = styles::default_font_color(app);

    let mut row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

    for (i, item) in items.iter().enumerate() {
        let mut child: Box<dyn Element> = match item {
            MessageItem::Keystroke {
                keystroke,
                color,
                background_color,
            } => render_terminal_message_items(
                &[MessageItem::Keystroke {
                    keystroke: keystroke.clone(),
                    color: *color,
                    background_color: *background_color,
                }],
                app,
            ),
            MessageItem::Text { content, color } => {
                let font_color = color.unwrap_or(default_font_color);
                Text::new(
                    content.clone(),
                    appearance.ui_font_family(),
                    styles::font_size(app),
                )
                .with_color(font_color)
                .soft_wrap(false)
                .finish()
            }
            MessageItem::Hyperlink {
                content,
                url,
                color,
                mouse_state,
            } => {
                let link_color = color.unwrap_or_else(|| appearance.theme().accent().into());
                let content = content.clone();
                let url = url.clone();
                Hoverable::new(mouse_state.clone(), |_| {
                    Text::new(
                        content.clone(),
                        appearance.ui_font_family(),
                        styles::font_size(app),
                    )
                    .with_color(link_color)
                    .soft_wrap(false)
                    .finish()
                })
                .on_click(move |_, app, _| {
                    app.open_url(&url);
                })
                .with_cursor(Cursor::PointingHand)
                .finish()
            }
            MessageItem::Icon { icon, color } => {
                let icon_color = color.unwrap_or(default_font_color);
                ConstrainedBox::new(icon.to_warpui_icon(Fill::Solid(icon_color)).finish())
                    .with_height(styles::font_size(app))
                    .with_width(styles::font_size(app))
                    .finish()
            }
            MessageItem::Clickable {
                items: clickable_items,
                action,
                mouse_state,
                disabled,
            } => {
                let message_items = render_message_bar_items(clickable_items, app);

                if !disabled {
                    let action = action.clone();
                    Hoverable::new(mouse_state.clone(), move |_| message_items)
                        .on_click(move |ctx, _app, _pos| {
                            action(ctx);
                        })
                        .with_cursor(Cursor::PointingHand)
                        .finish()
                } else {
                    message_items
                }
            }
            MessageItem::Chip {
                items: chip_items,
                action,
                mouse_state,
                disabled,
                ..
            } => {
                let chip_items = chip_items.clone();
                let action = action.clone();
                if !disabled {
                    Hoverable::new(mouse_state.clone(), move |state| {
                        render_message_chip_container(
                            render_message_bar_items(&chip_items, app),
                            state.is_hovered(),
                            app,
                        )
                    })
                    .on_click(move |ctx, _app, _pos| {
                        action(ctx);
                    })
                    .with_cursor(Cursor::PointingHand)
                    .finish()
                } else {
                    render_message_chip_container(
                        render_message_bar_items(chip_items.as_slice(), app),
                        false,
                        app,
                    )
                }
            }
            MessageItem::Image {
                source,
                width,
                height,
            } => ConstrainedBox::new(Image::new(source.clone(), CacheOption::BySize).finish())
                .with_width(*width)
                .with_height(*height)
                .finish(),
        };
        if i < items.len() - 1 {
            let next_item = &items[i + 1];
            let should_space_inline = matches!(item, MessageItem::Text { content, .. } | MessageItem::Hyperlink { content, .. } if content.ends_with(' '))
                || matches!(next_item, MessageItem::Text { content, .. } | MessageItem::Hyperlink { content, .. } if content.starts_with(' '));

            if !should_space_inline {
                let spacing = match (item, next_item) {
                    (
                        MessageItem::Keystroke { .. }
                        | MessageItem::Icon { .. }
                        | MessageItem::Image { .. },
                        MessageItem::Text { .. } | MessageItem::Hyperlink { .. },
                    )
                    | (
                        MessageItem::Keystroke { .. }
                        | MessageItem::Icon { .. }
                        | MessageItem::Image { .. },
                        MessageItem::Keystroke { .. }
                        | MessageItem::Icon { .. }
                        | MessageItem::Image { .. },
                    ) => Some(4.0),
                    (
                        MessageItem::Text { .. } | MessageItem::Hyperlink { .. },
                        MessageItem::Keystroke { .. }
                        | MessageItem::Icon { .. }
                        | MessageItem::Image { .. },
                    )
                    | (_, MessageItem::Clickable { .. } | MessageItem::Chip { .. })
                    | (MessageItem::Clickable { .. } | MessageItem::Chip { .. }, _) => Some(8.0),
                    _ => None,
                };
                if let Some(spacing) = spacing {
                    child = Container::new(child).with_margin_right(spacing).finish();
                }
            }
        }

        row.add_child(child);
    }

    row.finish()
}

pub fn render_terminal_message(message: Message, app: &AppContext) -> Box<dyn Element> {
    render_terminal_message_items(&message.items, app)
}

/// Renders a sequence of message items into a flex row with terminal-specific styling.
/// Currently used for terminal message bar items and use agent footer message items.
fn render_terminal_message_items(items: &[MessageItem], app: &AppContext) -> Box<dyn Element> {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    let default_text_color = theme.disabled_text_color(theme.background()).into_solid();
    let font_family = appearance.monospace_font_family();
    let font_size = appearance.monospace_font_size() - 2.;
    let icon_size = font_size;

    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_constrain_horizontal_bounds_to_parent(true);

    for item in items {
        let child = match item {
            MessageItem::Keystroke {
                keystroke, color, ..
            } => {
                let keystroke_color = color.unwrap_or(default_text_color);
                let keys = keystroke_to_keys(keystroke);
                let mut key_row = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

                for (i, key) in keys.iter().enumerate() {
                    let mut key_text = key.text(true);
                    let rendered_key = if key_text == "⏎" {
                        ConstrainedBox::new(
                            Icon::CornerDownLeft
                                .to_warpui_icon(Fill::Solid(keystroke_color))
                                .finish(),
                        )
                        .with_height(icon_size)
                        .with_width(icon_size)
                        .finish()
                    } else if key_text == "⇧" {
                        ConstrainedBox::new(
                            Icon::ArrowBlockUp
                                .to_warpui_icon(Fill::Solid(keystroke_color))
                                .finish(),
                        )
                        .with_height(icon_size)
                        .with_width(icon_size)
                        .finish()
                    } else {
                        if !cfg!(target_os = "macos") && i < (keys.len() - 1) {
                            key_text = format!("{key_text}-").into();
                        }
                        Text::new(key_text, font_family, icon_size)
                            .with_color(keystroke_color)
                            .soft_wrap(false)
                            .finish()
                    };

                    if i == 0 {
                        key_row.add_child(rendered_key);
                    } else {
                        key_row
                            .add_child(Container::new(rendered_key).with_margin_left(2.).finish());
                    }
                }

                key_row.finish()
            }
            MessageItem::Text { content, color } => {
                let text_color = color.unwrap_or(default_text_color);
                Text::new(content.clone(), font_family, font_size)
                    .with_color(text_color)
                    .soft_wrap(false)
                    .finish()
            }
            MessageItem::Hyperlink {
                content,
                url,
                color,
                mouse_state,
            } => {
                let link_color = color.unwrap_or_else(|| theme.accent().into());
                let content = content.clone();
                let url = url.clone();
                Hoverable::new(mouse_state.clone(), |_| {
                    Text::new(content.clone(), font_family, font_size)
                        .with_color(link_color)
                        .soft_wrap(false)
                        .finish()
                })
                .on_click(move |_, app, _| {
                    app.open_url(&url);
                })
                .with_cursor(Cursor::PointingHand)
                .finish()
            }
            MessageItem::Icon { icon, color } => {
                let icon_color = color.unwrap_or(default_text_color);
                ConstrainedBox::new(icon.to_warpui_icon(Fill::Solid(icon_color)).finish())
                    .with_height(icon_size)
                    .with_width(icon_size)
                    .finish()
            }
            MessageItem::Clickable {
                items: clickable_items,
                action,
                mouse_state,
                disabled,
            } => {
                let message_items = render_terminal_message_items(clickable_items, app);
                if !disabled {
                    let action = action.clone();
                    Hoverable::new(mouse_state.clone(), move |_| message_items)
                        .on_click(move |ctx, _app, _pos| {
                            action(ctx);
                        })
                        .with_cursor(Cursor::PointingHand)
                        .finish()
                } else {
                    message_items
                }
            }
            MessageItem::Chip {
                items: chip_items,
                action,
                mouse_state,
                disabled,
                ..
            } => {
                let chip_items = chip_items.clone();
                if !disabled {
                    let action = action.clone();
                    Hoverable::new(mouse_state.clone(), move |state| {
                        render_message_chip_container(
                            render_terminal_message_items(&chip_items, app),
                            state.is_hovered(),
                            app,
                        )
                    })
                    .on_click(move |ctx, _app, _pos| {
                        action(ctx);
                    })
                    .with_cursor(Cursor::PointingHand)
                    .finish()
                } else {
                    render_message_chip_container(
                        render_terminal_message_items(chip_items.as_slice(), app),
                        false,
                        app,
                    )
                }
            }
            MessageItem::Image {
                source,
                width,
                height,
            } => ConstrainedBox::new(Image::new(source.clone(), CacheOption::BySize).finish())
                .with_width(*width)
                .with_height(*height)
                .finish(),
        };

        row.add_child(child);
    }

    row.finish()
}

fn render_message_chip_container(
    content: Box<dyn Element>,
    is_hovered: bool,
    app: &AppContext,
) -> Box<dyn Element> {
    let theme = Appearance::as_ref(app).theme();
    let background = if is_hovered {
        blended_colors::fg_overlay_2(theme)
    } else {
        blended_colors::fg_overlay_1(theme)
    };

    Container::new(content)
        .with_background(background)
        .with_border(
            Border::all(styles::CHIP_BORDER_WIDTH)
                .with_border_color(blended_colors::neutral_3(theme)),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(
            styles::CHIP_CORNER_RADIUS,
        )))
        .with_vertical_padding(styles::CHIP_VERTICAL_PADDING)
        .with_horizontal_padding(styles::CHIP_HORIZONTAL_PADDING)
        .with_vertical_margin(-(styles::CHIP_VERTICAL_PADDING + styles::CHIP_BORDER_WIDTH))
        .finish()
}

pub mod styles {
    use pathfinder_color::ColorU;
    use warp_core::ui::appearance::Appearance;
    use warpui::{AppContext, SingletonEntity};

    use crate::ui_components::blended_colors;

    pub fn font_size(app: &AppContext) -> f32 {
        let appearance = Appearance::as_ref(app);
        appearance.monospace_font_size() - 2.
    }

    pub fn default_font_color(app: &AppContext) -> ColorU {
        let theme = Appearance::as_ref(app).theme();
        theme
            .sub_text_color(blended_colors::neutral_1(theme).into())
            .into_solid()
    }

    pub const VERTICAL_PADDING: f32 = 8.;
    pub const CHIP_CORNER_RADIUS: f32 = 2.;
    pub const CHIP_BORDER_WIDTH: f32 = 1.;
    pub const CHIP_VERTICAL_PADDING: f32 = 2.;
    pub const CHIP_HORIZONTAL_PADDING: f32 = 4.;
}
