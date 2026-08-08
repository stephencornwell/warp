use std::path::PathBuf;

use markdown_parser::{
    FormattedText, FormattedTextFragment, FormattedTextLine, FormattedTextStyles, Hyperlink,
};
use warpui::elements::{
    Align, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex,
    FormattedTextElement, MainAxisAlignment, MainAxisSize, MouseStateHandle, ParentElement, Radius,
    Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::geometry::vector::Vector2F;
use warpui::keymap::{FixedBinding, Keystroke};
use warpui::platform::file_picker::{FilePickerConfiguration, FilePickerError};
use warpui::{
    AppContext, Element, Entity, EventContext, SingletonEntity, TypedActionView, View, ViewContext,
    ViewHandle,
};

use pathfinder_color::ColorU;
use warp_core::ui::theme::{phenomenon::PhenomenonStyle, Fill};

use crate::appearance::Appearance;
use crate::settings::AISettings;
use crate::tab_configs::session_config::{is_git_repo, SessionConfigSelection, SessionType};
use crate::tab_configs::session_config_rendering;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{
    ActionButton, ActionButtonTheme, ButtonSize, KeystrokeSource,
};
use crate::view_components::callout_bubble::{
    callout_body_color, callout_title_color, render_callout_bubble, CalloutArrowDirection,
    CalloutArrowPosition, CalloutBubbleConfig,
};

use super::tab_config_step;
use super::welcome_banner;

const CALLOUT_WIDTH: f32 = 480.;

struct HoaPrimaryButtonTheme;

impl ActionButtonTheme for HoaPrimaryButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        Some(PhenomenonStyle::primary_button_background(hovered))
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::primary_button_text()
    }
}
struct HoaWelcomeModalButtonTheme;
struct HoaWelcomeModalCloseButtonTheme;

impl ActionButtonTheme for HoaWelcomeModalButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        Some(PhenomenonStyle::modal_button_background_fill(hovered))
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_button_text()
    }
}

impl ActionButtonTheme for HoaWelcomeModalCloseButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        if hovered {
            Some(Fill::Solid(PhenomenonStyle::modal_close_button_hover()))
        } else {
            None
        }
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_close_button_text()
    }
}

/// The sequential steps in the HOA onboarding flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoaOnboardingStep {
    WelcomeBanner,
    AgentInboxCallout,
    TabConfig,
}

impl HoaOnboardingStep {
    fn index(&self) -> usize {
        match self {
            HoaOnboardingStep::WelcomeBanner => 0,
            HoaOnboardingStep::AgentInboxCallout => 1,
            HoaOnboardingStep::TabConfig => 2,
        }
    }

    fn total_dots() -> usize {
        3
    }
}

pub fn init(app: &mut warpui::AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "enter",
        HoaOnboardingAction::EnterPressed,
        id!(HoaOnboardingFlow::ui_name()),
    )]);
}

#[derive(Clone, Debug)]
pub enum HoaOnboardingAction {
    EnterPressed,
    AdvanceFromWelcome,
    AdvanceFromInbox,
    SelectSessionType(usize),
    OpenDirectoryPicker,
    DirectorySelected(Result<String, FilePickerError>),
    ToggleWorktree,
    ToggleAutogenerateWorktreeBranchName,
    Finish,
    Dismiss,
}

pub enum HoaOnboardingFlowEvent {
    Completed(Option<SessionConfigSelection>),
    Dismissed,
    StepChanged,
}

pub struct HoaOnboardingFlow {
    step: HoaOnboardingStep,
    // Step 1 state
    close_button: ViewHandle<ActionButton>,
    cta_button: ViewHandle<ActionButton>,

    // Step 2 state
    next_inbox_button: ViewHandle<ActionButton>,

    // Step 3 state
    finish_button: ViewHandle<ActionButton>,
    session_types: Vec<SessionType>,
    selected_session_type_index: usize,
    selected_directory: PathBuf,
    is_git_repo: bool,
    enable_worktree: bool,
    autogenerate_worktree_branch_name: bool,
    session_pill_mouse_states: Vec<MouseStateHandle>,
    directory_button_mouse_state: MouseStateHandle,
    worktree_checkbox_mouse_state: MouseStateHandle,
    worktree_tooltip_mouse_state: MouseStateHandle,
    autogenerate_checkbox_mouse_state: MouseStateHandle,
    autogenerate_tooltip_mouse_state: MouseStateHandle,
}

impl HoaOnboardingFlow {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let show_oz = AISettings::as_ref(ctx).is_any_ai_enabled(ctx);
        let session_types = session_config_rendering::visible_session_types(show_oz);
        let pill_mouse_states: Vec<_> = session_types
            .iter()
            .map(|_| MouseStateHandle::default())
            .collect();

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let is_git = is_git_repo(&home);

        let close_button = ctx.add_view(|_ctx| {
            ActionButton::new("", HoaWelcomeModalCloseButtonTheme)
                .with_icon(Icon::X)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| ctx.dispatch_typed_action(HoaOnboardingAction::Dismiss))
        });

        let cta_button = ctx.add_view(|_ctx| {
            ActionButton::new("See what's new", HoaWelcomeModalButtonTheme)
                .with_full_width(true)
                .on_click(|ctx| ctx.dispatch_typed_action(HoaOnboardingAction::AdvanceFromWelcome))
        });

        let enter = Keystroke::parse("enter").unwrap_or_default();

        let next_inbox_button = ctx.add_view(|ctx| {
            ActionButton::new("Next", HoaPrimaryButtonTheme)
                .with_keybinding(KeystrokeSource::Fixed(enter.clone()), ctx)
                .on_click(|ctx| ctx.dispatch_typed_action(HoaOnboardingAction::AdvanceFromInbox))
        });

        let finish_button = ctx.add_view(|ctx| {
            ActionButton::new("Finish", HoaPrimaryButtonTheme)
                .with_keybinding(KeystrokeSource::Fixed(enter), ctx)
                .on_click(|ctx| ctx.dispatch_typed_action(HoaOnboardingAction::Finish))
        });

        Self {
            step: HoaOnboardingStep::WelcomeBanner,
            close_button,
            cta_button,
            next_inbox_button,
            finish_button,
            session_types,
            selected_session_type_index: 0,
            selected_directory: home,
            is_git_repo: is_git,
            enable_worktree: false,
            autogenerate_worktree_branch_name: false,
            session_pill_mouse_states: pill_mouse_states,
            directory_button_mouse_state: MouseStateHandle::default(),
            worktree_checkbox_mouse_state: MouseStateHandle::default(),
            worktree_tooltip_mouse_state: MouseStateHandle::default(),
            autogenerate_checkbox_mouse_state: MouseStateHandle::default(),
            autogenerate_tooltip_mouse_state: MouseStateHandle::default(),
        }
    }

    pub fn step(&self) -> HoaOnboardingStep {
        self.step
    }

    fn selected_session_type(&self) -> SessionType {
        self.session_types[self.selected_session_type_index]
    }

    fn advance(&mut self, ctx: &mut ViewContext<Self>) {
        self.step = match self.step {
            HoaOnboardingStep::WelcomeBanner => HoaOnboardingStep::AgentInboxCallout,
            HoaOnboardingStep::AgentInboxCallout => HoaOnboardingStep::TabConfig,
            HoaOnboardingStep::TabConfig => {
                self.finish(ctx);
                return;
            }
        };
        // Emit StepChanged so the workspace re-renders and switches
        // from add_child to add_positioned_child for the new step.
        ctx.emit(HoaOnboardingFlowEvent::StepChanged);
        ctx.notify();
    }

    fn finish(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(HoaOnboardingFlowEvent::Completed(Some(
            SessionConfigSelection {
                session_type: self.selected_session_type(),
                directory: self.selected_directory.clone(),
                enable_worktree: self.enable_worktree,
                autogenerate_worktree_branch_name: self.autogenerate_worktree_branch_name,
            },
        )));
    }

    fn dismiss(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.emit(HoaOnboardingFlowEvent::Dismissed);
    }

    // ── Rendering helpers ──

    fn render_progress_dots(&self, appearance: &Appearance) -> Box<dyn Element> {
        let _ = appearance;
        let active_index = self.step.index();

        let mut row = Flex::row().with_spacing(4.);
        for i in 0..HoaOnboardingStep::total_dots() {
            let fill = if i == active_index {
                Fill::Solid(PhenomenonStyle::blue())
            } else {
                Fill::Solid(PhenomenonStyle::subtle_border())
            };
            row.add_child(
                ConstrainedBox::new(
                    Container::new(Empty::new().finish())
                        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
                        .with_background(fill)
                        .finish(),
                )
                .with_width(8.)
                .with_height(8.)
                .finish(),
            );
        }
        row.finish()
    }

    fn render_callout_footer(
        &self,
        button: &ViewHandle<ActionButton>,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let mut row = Flex::row()
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_main_axis_size(MainAxisSize::Max);

        row = row.with_main_axis_alignment(MainAxisAlignment::SpaceBetween);
        row.add_child(self.render_progress_dots(appearance));
        row.add_child(ChildView::new(button).finish());

        Container::new(row.finish())
            .with_horizontal_padding(16.)
            .with_vertical_padding(16.)
            .finish()
    }

    fn render_inbox_callout(&self, appearance: &Appearance) -> Box<dyn Element> {
        let title = Text::new(
            "Meet your new agent inbox",
            appearance.ui_font_family(),
            16.,
        )
        .with_color(callout_title_color(appearance))
        .with_style(Properties::default().weight(Weight::Bold))
        .finish();

        // Build the description with an inline "Learn more" hyperlink.
        let learn_more_fragment = FormattedTextFragment {
            text: "Learn more".into(),
            styles: FormattedTextStyles {
                underline: true,
                hyperlink: Some(Hyperlink::Url(
                    "https://docs.warp.dev/agent-platform/warp-agents/agent-notifications".into(),
                )),
                ..Default::default()
            },
        };

        let formatted = FormattedText::new([FormattedTextLine::Line(vec![
            FormattedTextFragment::plain_text(
                "Warp pipes through notifications from any CLI coding agent into a unified notification center that works across all coding agents and harnesses. ",
            ),
            learn_more_fragment,
        ])]);

        let description = FormattedTextElement::new(
            formatted,
            14.,
            appearance.ui_font_family(),
            appearance.ui_font_family(),
            callout_body_color(appearance),
            Default::default(),
        )
        .with_line_height_ratio(1.2)
        .register_default_click_handlers(|link, _ctx, app| {
            app.open_url(&link.url);
        })
        .finish();

        let body_content = Flex::column()
            .with_cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(title)
            .with_child(Container::new(description).with_margin_top(8.).finish())
            .finish();

        let body = Container::new(body_content)
            .with_horizontal_padding(16.)
            .with_padding_top(16.)
            .with_padding_bottom(12.)
            .finish();
        let footer = self.render_callout_footer(&self.next_inbox_button, appearance);

        Flex::column().with_child(body).with_child(footer).finish()
    }

    fn render_tab_config_step(&self, appearance: &Appearance) -> Box<dyn Element> {
        let form = tab_config_step::render_tab_config_form(
            tab_config_step::TabConfigFormState {
                session_types: &self.session_types,
                selected_session_type_index: self.selected_session_type_index,
                session_pill_mouse_states: &self.session_pill_mouse_states,
                selected_directory: &self.selected_directory,
                directory_button_mouse_state: self.directory_button_mouse_state.clone(),
                enable_worktree: self.enable_worktree,
                is_git_repo: self.is_git_repo,
                worktree_checkbox_mouse_state: self.worktree_checkbox_mouse_state.clone(),
                worktree_tooltip_mouse_state: self.worktree_tooltip_mouse_state.clone(),
                autogenerate_worktree_branch_name: self.autogenerate_worktree_branch_name,
                autogenerate_checkbox_mouse_state: self.autogenerate_checkbox_mouse_state.clone(),
                autogenerate_tooltip_mouse_state: self.autogenerate_tooltip_mouse_state.clone(),
            },
            tab_config_step::TabConfigFormHandlers {
                on_select_session_type: |i: usize, ctx: &mut EventContext, _: Vector2F| {
                    ctx.dispatch_typed_action(HoaOnboardingAction::SelectSessionType(i));
                },
                on_open_directory_picker: |ctx: &mut EventContext, _: Vector2F| {
                    ctx.dispatch_typed_action(HoaOnboardingAction::OpenDirectoryPicker);
                },
                on_toggle_worktree: |ctx: &mut EventContext, _: Vector2F| {
                    ctx.dispatch_typed_action(HoaOnboardingAction::ToggleWorktree);
                },
                on_toggle_autogenerate: |ctx: &mut EventContext, _: Vector2F| {
                    ctx.dispatch_typed_action(
                        HoaOnboardingAction::ToggleAutogenerateWorktreeBranchName,
                    );
                },
            },
            appearance,
        );

        let footer = self.render_callout_footer(&self.finish_button, appearance);

        let body = Container::new(form)
            .with_horizontal_padding(16.)
            .with_vertical_padding(16.)
            .finish();

        Flex::column().with_child(body).with_child(footer).finish()
    }
}

impl Entity for HoaOnboardingFlow {
    type Event = HoaOnboardingFlowEvent;
}

impl View for HoaOnboardingFlow {
    fn ui_name() -> &'static str {
        "HoaOnboardingFlow"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        match self.step {
            HoaOnboardingStep::WelcomeBanner => {
                // Full-window scrim with centered banner
                let banner = welcome_banner::render_welcome_banner(
                    &self.close_button,
                    &self.cta_button,
                    appearance,
                );

                Container::new(Align::new(banner).finish())
                    .with_background_color(ColorU::new(18, 18, 18, 128))
                    .finish()
            }
            HoaOnboardingStep::AgentInboxCallout => {
                let content = self.render_inbox_callout(appearance);
                render_callout_bubble(
                    content,
                    &CalloutBubbleConfig {
                        width: CALLOUT_WIDTH,
                        arrow_direction: CalloutArrowDirection::Up,
                        arrow_position: CalloutArrowPosition::End(24.),
                    },
                    appearance,
                )
            }
            HoaOnboardingStep::TabConfig => {
                let tab_content = self.render_tab_config_step(appearance);
                render_callout_bubble(
                    tab_content,
                    &CalloutBubbleConfig {
                        width: CALLOUT_WIDTH,
                        arrow_direction: CalloutArrowDirection::Up,
                        arrow_position: CalloutArrowPosition::Center,
                    },
                    appearance,
                )
            }
        }
    }
}

impl TypedActionView for HoaOnboardingFlow {
    type Action = HoaOnboardingAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            HoaOnboardingAction::EnterPressed => {
                self.advance(ctx);
            }
            HoaOnboardingAction::AdvanceFromWelcome | HoaOnboardingAction::AdvanceFromInbox => {
                self.advance(ctx);
            }
            HoaOnboardingAction::SelectSessionType(index) => {
                self.selected_session_type_index = *index;
                ctx.notify();
            }
            HoaOnboardingAction::OpenDirectoryPicker => {
                ctx.open_file_picker(
                    |result, ctx| {
                        if let Some(path_result) =
                            result.map(|paths| paths.into_iter().next()).transpose()
                        {
                            ctx.dispatch_typed_action(&HoaOnboardingAction::DirectorySelected(
                                path_result,
                            ));
                        }
                    },
                    FilePickerConfiguration::new().folders_only(),
                );
            }
            HoaOnboardingAction::DirectorySelected(result) => match result {
                Ok(path) => {
                    let path = PathBuf::from(path);
                    self.is_git_repo = is_git_repo(&path);
                    if !self.is_git_repo {
                        self.enable_worktree = false;
                        self.autogenerate_worktree_branch_name = false;
                    }
                    self.selected_directory = path;
                    ctx.notify();
                }
                Err(err) => {
                    log::warn!("File picker error in HOA onboarding: {err}");
                }
            },
            HoaOnboardingAction::ToggleWorktree => {
                if self.is_git_repo {
                    self.enable_worktree = !self.enable_worktree;
                    if !self.enable_worktree {
                        self.autogenerate_worktree_branch_name = false;
                    }
                    ctx.notify();
                }
            }
            HoaOnboardingAction::ToggleAutogenerateWorktreeBranchName => {
                if self.enable_worktree {
                    self.autogenerate_worktree_branch_name =
                        !self.autogenerate_worktree_branch_name;
                    ctx.notify();
                }
            }
            HoaOnboardingAction::Finish => {
                self.finish(ctx);
            }
            HoaOnboardingAction::Dismiss => {
                self.dismiss(ctx);
            }
        }
    }
}
