use crate::{
    appearance::Appearance,
    features::FeatureFlag,
    settings::{AppEditorSettings, InputModeSettings},
    terminal::{
        block_list_settings::BlockListSettings,
        block_list_viewport::InputMode,
        input::{
            common::{
                add_command_xray_overlay, add_input_suggestions_overlays, add_vim_status_to_stack,
                should_show_terminal_input_message_bar,
                wrap_input_with_terminal_padding_and_focus_handler,
            },
            get_input_box_top_border_width, InputDropTargetData,
        },
        settings::{SpacingMode, TerminalSettings},
        view::TerminalAction,
    },
};
use settings::Setting;
use warpui::{
    elements::{
        Border, ChildAnchor, Container, DropTarget, Element, Empty, Flex,
        Hoverable, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
        SavePosition, Stack,
    },
    AppContext, SingletonEntity,
};

use super::{should_render_prompt_on_same_line, Input};

impl Input {
    /// Renders the classic input. This is used when the user has 'Honor PS1' enabled in settings,
    /// OR if `FeatureFlag::AgentView` is disabled and the user has 'Classic' input type selected
    /// in settings.
    pub(super) fn render_classic_input(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let menu_positioning = self.menu_positioning(app);

        let model = self.model.lock();
        let should_render_prompt_using_editor_decorator_elements =
            should_render_prompt_on_same_line(false, &model, app);

        // We should likely rework this stack to not need to use `with_constrain_absolute_children`,
        // by reworking the positioning of the children to not depend on this.
        let mut stack = Stack::new().with_constrain_absolute_children();

        let mut prompt_row = Stack::new();

        let (lprompt_top_area_option, rprompt_area_option);
        let mut prompt_top_padding_row = Stack::new();
        if should_render_prompt_using_editor_decorator_elements {
            // These are rendered as sections/notches in the EditorElement.
            lprompt_top_area_option = None;
            rprompt_area_option = None;

            let terminal_spacing = TerminalSettings::as_ref(app)
                .terminal_input_spacing(appearance.line_height_ratio(), app);
            let default_prompt_top_padding = terminal_spacing.block_padding.padding_top
                * self.size_info(app).cell_height_px().as_f32()
                - get_input_box_top_border_width();
            let prompt_top_padding_element = Container::new(Empty::new().finish())
                .with_padding_top(default_prompt_top_padding)
                .finish();
            prompt_top_padding_row.add_child(prompt_top_padding_element);
        } else {
            let prompt_elements = self
                .prompt_render_helper
                .render_prompt_areas(&model, appearance, app);
            lprompt_top_area_option = prompt_elements.lprompt;
            rprompt_area_option = prompt_elements.rprompt;
        }

        if !should_render_prompt_using_editor_decorator_elements {
            if let Some(lprompt_top_area) = lprompt_top_area_option {
                prompt_row.add_child(lprompt_top_area);
            }
            if let Some(rprompt_area) = rprompt_area_option {
                let block = &model.block_list().active_block();
                prompt_row.add_positioned_child(
                    rprompt_area,
                    OffsetPositioning::offset_from_parent(
                        block.rprompt_render_offset(&self.size_info(app)),
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        let vim_state = self.editor.as_ref(app).vim_state(app);
        let app_editor_settings = AppEditorSettings::as_ref(app);
        let show_vim_status = vim_state.is_some() && *app_editor_settings.vim_status_bar.value();
        let input_mode = *InputModeSettings::as_ref(app).input_mode.value();

        let is_compact_mode = matches!(
            TerminalSettings::as_ref(app).spacing_mode.value(),
            SpacingMode::Compact
        );
        let mut column = Flex::column();

        if matches!(input_mode, InputMode::PinnedToBottom | InputMode::Waterfall) {
            if let Some(banner) =
                self.render_input_banner(appearance, app, input_mode, is_compact_mode)
            {
                column.add_child(banner);
            }
        }

        column.add_children([prompt_top_padding_row.finish(), prompt_row.finish()]);

        column.add_child(self.render_input_box(show_vim_status, appearance, app));

        let _ = should_show_terminal_input_message_bar;
        column.add_child(
            Container::new(Flex::row().finish())
                .with_margin_bottom(4.)
                .finish(),
        );

        if matches!(input_mode, InputMode::PinnedToTop) {
            if let Some(banner) =
                self.render_input_banner(appearance, app, input_mode, is_compact_mode)
            {
                column.add_child(banner);
            }
        }

        if !FeatureFlag::AgentView.is_enabled() {
            if let Some(vim_state) = vim_state.as_ref() {
                if show_vim_status {
                    add_vim_status_to_stack(
                        &mut stack, vim_state, appearance,
                        false, // legacy doesn't use adjusted padding for vim status
                    );
                }
            }
        }

        stack.add_child(wrap_input_with_terminal_padding_and_focus_handler(
            self.is_active_session(app),
            column.finish(),
            false, // legacy uses full padding
        ));

        if self.is_pane_focused(app) {
            add_input_suggestions_overlays(self, &mut stack, appearance, menu_positioning, app);
        }

        if let Some(token_description) = &self.command_x_ray_description {
            add_command_xray_overlay(
                self,
                &mut stack,
                token_description,
                appearance,
                menu_positioning,
                app,
            );
        }

        let input_mode = *InputModeSettings::as_ref(app).input_mode.value();

        // When AgentView is enabled, match terminal-mode input behavior and only render the
        // divider adjacent to the status/message line when block dividers are enabled.
        let show_block_dividers = *BlockListSettings::as_ref(app).show_block_dividers.value();
        let should_render_divider = !FeatureFlag::AgentView.is_enabled() || show_block_dividers;

        let border = match input_mode {
            InputMode::PinnedToBottom => Border::top(if should_render_divider {
                get_input_box_top_border_width()
            } else {
                0.
            })
            .with_border_fill(theme.outline()),
            InputMode::PinnedToTop => Border::bottom(if should_render_divider {
                get_input_box_top_border_width()
            } else {
                0.
            })
            .with_border_fill(theme.outline()),
            InputMode::Waterfall => Border::new(get_input_box_top_border_width())
                .with_sides(true, false, true, false)
                .with_border_fill(theme.outline()),
        };

        let drop_target = DropTarget::new(
            Container::new(stack.finish()).with_border(border).finish(),
            InputDropTargetData::new(self.weak_view_handle.clone()),
        )
        .finish();

        let input = SavePosition::new(
            Hoverable::new(self.hoverable_handle.clone(), |_| drop_target)
                .on_middle_click(|ctx, _app, _position| {
                    ctx.dispatch_typed_action(TerminalAction::MiddleClickOnInput)
                })
                .finish(),
            &self.status_free_input_save_position_id(),
        )
        .finish();

        SavePosition::new(input, &self.save_position_id()).finish()
    }
}
