use crate::report_if_error;
#[cfg(enable_crash_recovery)]
mod crash_recovery;
pub mod global_search;
pub(crate) mod launch_modal;
pub(crate) mod left_panel;
pub(crate) mod openwarp_launch_modal;
mod startup_directory;
#[cfg(test)]
#[path = "view_test.rs"]
mod tests;
#[cfg(target_family = "wasm")]
mod wasm_view;

use crate::app_state::{
    LeafContents, LeafSnapshot, LeftPanelDisplayedTab, LeftPanelSnapshot, NotebookPaneSnapshot,
    PaneNodeSnapshot, PaneUuid, SettingsPaneSnapshot, TabSnapshot, TerminalPaneSnapshot,
    WindowSnapshot, WorkflowPaneSnapshot,
};
use crate::coding_panel_enablement_state::CodingPanelEnablementState;
use crate::default_terminal::DefaultTerminal;
use crate::notification::NotificationContext;
use crate::pane_group::pane::ActionOrigin;
use crate::projects::ProjectManagementModel;
use crate::terminal::model::terminal_model::ConversationTranscriptViewerStatus;
use crate::terminal::session_settings::SessionSettings;
use crate::ui_components::red_notification_dot::RedNotificationDot;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::settings::OpenConversationPreference;
use crate::workspace::toast_stack::ToastStack;
use crate::workspace::view::global_search::view::GlobalSearchEntryFocus;
use crate::workspace::view::left_panel::{
    LeftPanelAction, LeftPanelEvent, LeftPanelView, ToolPanelView,
};

use crate::ui_components::window_focus_dimming::WindowFocusDimming;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::Editor;
#[cfg(feature = "local_fs")]
use crate::util::file::external_editor::EditorSettings;
use crate::util::openable_file_type::FileTarget;
#[cfg(feature = "local_fs")]
use crate::util::openable_file_type::{resolve_file_target_with_editor_choice, EditorLayout};

use crate::workspace::header_toolbar_item::HeaderToolbarItemKind;
use crate::workspace::tab_settings::TabCloseButtonPosition;
use crate::workspace::view::launch_modal::LaunchModal;
use crate::workspace::view::openwarp_launch_modal::{
    OpenWarpLaunchModal, OpenWarpLaunchModalEvent,
};
#[cfg(all(target_os = "macos", feature = "crash_reporting"))]
use sentry::protocol::{Attachment, AttachmentType};
use serde_json;
use warpui::notification::NotificationSendError;

use super::lightbox_view::{LightboxParams, LightboxView, LightboxViewEvent};
use super::util;
use super::WorkspaceRegistry;
#[cfg(feature = "local_fs")]
use crate::code::editor_management::CodeSource;
use crate::launch_configs::launch_config::WindowTemplate;
use crate::pane_group::{Direction as PaneGroupDirection, PaneGroup, PaneId, TerminalPaneId};
use crate::quit_warning::UnsavedStateSummary;
use crate::search::command_palette::view::NavigationMode;
use crate::search::slash_command_menu::static_commands::commands;
use crate::settings::{CodeSettings, CodeSettingsChangedEvent, CtrlTabBehavior, InputModeSettings};
use crate::settings_view::pane_manager::SettingsPaneManager;
use crate::settings_view::{SettingsSection, SettingsView, SettingsViewEvent};
#[cfg(all(target_os = "windows", feature = "local_tty"))]
use crate::shell_indicator::ShellIndicatorType;
use crate::terminal::available_shells::AvailableShell;
#[cfg(target_os = "windows")]
use crate::terminal::available_shells::AvailableShells;
use crate::terminal::block_list_viewport::InputMode;
use crate::terminal::ligature_settings::should_use_ligature_rendering;
use crate::ui_components::avatar::{Avatar, AvatarContent, StatusElementTypes};

#[cfg(target_family = "wasm")]
use crate::ai::agent_conversations_model::AgentConversationsModelEvent;
#[cfg(target_family = "wasm")]
use crate::ai::conversation_details_panel::ConversationDetailsPanel;
#[cfg(target_family = "wasm")]
use crate::uri::browser_url_handler::{parse_current_url, update_browser_url};
#[cfg(feature = "local_fs")]
use repo_metadata::RemoteRepositoryIdentifier;
#[cfg(target_family = "wasm")]
use url::Url;

#[cfg(target_family = "wasm")]
use crate::wasm_nux_dialog::WasmNUXDialog;

use crate::appearance::{Appearance, AppearanceManager};
use crate::banner::BannerState;
use crate::channel::Channel;
use crate::context_chips::ChipRuntimeCapabilities;
use crate::menu::{
    Event as MenuEvent, Menu, MenuItem, MenuItemFields, MenuSelectionSource,
    DEFAULT_WIDTH as MENU_DEFAULT_WIDTH,
};
use crate::modal::{Modal, ModalEvent, ModalViewState};
use crate::network::{NetworkStatus, NetworkStatusEvent};
use crate::pane_group::{
    self, AnyPaneContent, Direction, NewTerminalOptions, PanesLayout, TabBarHoverIndex,
};
use crate::terminal::keys_settings::KeysSettings;
use crate::GlobalResourceHandles;

use crate::referral_theme_status::ReferralThemeEvent;
use crate::resource_center::{
    mark_feature_used_and_write_to_user_defaults, skip_tips_and_write_to_user_defaults,
    ResourceCenterEvent, ResourceCenterPage, ResourceCenterView, Tip, TipAction, TipsCompleted,
};
use crate::reward_view::{RewardEvent, RewardKind, RewardView};
use crate::root_view::{quake_mode_window_id, NewWorkspaceSource, OpenLaunchConfigArg};
use crate::search::command_search::searcher::{AcceptedHistoryItem, CommandSearchItemAction};
use crate::search::command_search::view::{CommandSearchEvent, CommandSearchView};
use crate::server::ids::{ObjectUid, ServerId, SyncId};
use crate::session_management::{SessionNavigationData, SessionSource};
use crate::settings::{
    active_theme_kind, respect_system_theme, AccessibilitySettings, AliasExpansionSettings,
    AppEditorSettings, BlockVisibilitySettings, ChangelogSettings, CursorBlink, DebugSettings,
    FontSettings, GPUSettings, InputSettings, MonospaceFontSize, PaneSettings, PrivacySettings,
    SelectionSettings, Settings, SshSettings, ThemeSettings,
};
use crate::settings_view::flags;
use crate::settings_view::keybindings::{KeybindingChangedEvent, KeybindingChangedNotifier};
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::terminal::general_settings::GeneralSettings;
use crate::terminal::input::{Input, MenuPositioning};
#[cfg(feature = "local_tty")]
use crate::terminal::local_tty::docker_sandbox::resolve_sbx_path_from_user_shell;
use crate::terminal::model::blockgrid::BlockGrid;
#[cfg(feature = "local_fs")]
use crate::terminal::model::session::Session;
use crate::terminal::model::session::SessionId;
use crate::terminal::resizable_data::{
    ModalSizes, ModalType, ResizableData, DEFAULT_LEFT_PANEL_WIDTH, DEFAULT_RIGHT_PANEL_WIDTH,
};
use crate::terminal::safe_mode_settings::SafeModeSettings;
use crate::terminal::session_settings::{
    NewSessionSource, NotificationsMode, NotificationsSettings, SessionSettingsChangedEvent,
    WorkingDirectoryMode,
};
use crate::terminal::settings::{SpacingMode, TerminalSettings};
use crate::terminal::shell::ShellType;
use crate::terminal::{self, SizeInfo, TerminalView};
#[cfg(target_os = "macos")]
use crate::workspace::cli_install;
use ::settings::{Setting, ToggleableSetting};
use warp_core::features::FeatureFlag;

use crate::search::{self, QueryFilter};
use crate::terminal::view::{
    SyncEvent, SyncInputType, TerminalAction, NOTIFICATIONS_TROUBLESHOOT_URL,
};
use crate::terminal::{BlockListSettings, TerminalModel};
use crate::themes::theme::{AnsiColorIdentifier, RespectSystemTheme, ThemeKind};
use crate::themes::theme_chooser::{ThemeChooser, ThemeChooserEvent, ThemeChooserMode};
use crate::themes::theme_creator_modal::{ThemeCreatorModal, ThemeCreatorModalEvent};
use crate::themes::theme_deletion_modal::{ThemeDeletionModal, ThemeDeletionModalEvent};
use crate::tips::{TipsEvent, TipsView};
use crate::ui_components::blended_colors;
use crate::ui_components::buttons::{combo_inner_button, icon_button_with_color};
use crate::undo_close::UndoCloseStack;
#[cfg(feature = "local_fs")]
use crate::user_config::{
    ensure_default_worktree_config, find_unused_tab_config_path, find_unused_toml_path,
    find_unused_worktree_config_path, materialize_default_worktree_config, sanitize_toml_base_name,
    tab_configs_dir,
};
use crate::user_config::{WarpConfig, WarpConfigUpdateEvent};
use crate::util::bindings::{
    keybinding_name_to_display_string, keybinding_name_to_keystroke, trigger_to_keystroke,
};
use crate::util::links;
use crate::util::traffic_lights::{traffic_light_data, TrafficLightMouseStates, TrafficLightSide};
use crate::util::truncation::truncate_from_end;
#[cfg(target_family = "wasm")]
use crate::view_components::action_button::ActionButton;
use crate::view_components::callout_bubble::{
    render_callout_bubble, CalloutArrowDirection, CalloutArrowPosition, CalloutBubbleConfig,
};
use crate::view_components::{
    AgentToast, AgentToastStack, DismissibleToast, DismissibleToastStack, ToastLink,
};
use crate::window_settings::{WindowSettings, WindowSettingsChangedEvent, ZoomLevel};
use crate::workspace::action::{AddTabWithShellSource, CommandSearchOptions, PaletteSource};
use crate::workspace::one_time_modal_model::OneTimeModalModel;
use crate::workspace::sync_inputs::SyncedInputState;
use crate::workspace::toast_stack::{
    ToastStack as WorkspaceToastStack, ToastStackEvent as WorkspaceToastStackEvent,
};

use futures::Future;
use itertools::Itertools;
use parking_lot::FairMutex;
use pathfinder_geometry::rect::RectF;
use repo_metadata::repositories::DetectedRepositories;
use session_sharing_protocol::common::SessionId as SharedSessionId;
use std::collections::{HashMap, HashSet};
#[cfg(feature = "local_fs")]
use std::convert::TryFrom;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::{SystemTime, UNIX_EPOCH};
use warp_core::context_flag::ContextFlag;
use warp_core::execution_mode::AppExecutionMode;
use warp_core::semantic_selection::SemanticSelection;
use warp_util::path::{user_friendly_path, LineAndColumnArg};
use warpui::fonts::Weight;
use warpui::modals::{AlertDialogWithCallbacks, AppModalCallback};
use warpui::windowing::{StateEvent, WindowManager};

use warp_core::user_preferences::GetUserPreferences as _;
use warpui::clipboard::ClipboardContent;
#[cfg(target_family = "wasm")]
use warpui::elements::Percentage;
use warpui::elements::{CacheOption, DispatchEventResult, DropTarget, EventHandler, Image, Rect};
use warpui::ui_components::button::{Button, ButtonVariant};
use warpui::{elements::MouseStateHandle, fonts::Properties};

use crate::channel::ChannelState;

use crate::editor::{
    EditorView, Event as EditorEvent, PropagateAndNoOpNavigationKeys, SingleLineEditorOptions,
    TextOptions,
};
use crate::persistence::ModelEvent;

use super::action::{
    InitContent, RestoreConversationLayout, TabContextMenuAnchor, WorkspaceAction,
};
use super::close_session_confirmation_dialog::{
    CloseSessionConfirmationDialog, CloseSessionConfirmationEvent, OpenDialogSource,
};
use super::native_modal::{NativeModal, NativeModalEvent};
use super::one_time_modal_model::OneTimeModalEvent;
use super::{ActiveSession, TabBarDropTargetData, TabBarLocation};

use super::tab_settings::{
    HeaderToolbarChipSelection, NewTabPlacement, TabSettings, TabSettingsChangedEvent,
    WorkspaceDecorationVisibility,
};
use super::util::{
    PaneViewLocator, TabMovement, TerminalSessionFallbackBehavior, WelcomeTipsViewState,
    WorkspaceMouseStates, WorkspaceState,
};
use crate::launch_configs::save_modal::{LaunchConfigModalEvent, LaunchConfigSaveModal};
use crate::tab_configs::action_sidecar::SidecarItemKind;
use crate::tab_configs::remove_confirmation_dialog::{
    RemoveTabConfigConfirmationDialog, RemoveTabConfigConfirmationEvent,
};
use crate::tab_configs::session_config_modal::{SessionConfigModal, SessionConfigModalEvent};
use crate::tab_configs::{TabConfigParamsModal, TabConfigParamsModalEvent};

use crate::code::editor::{add_color, remove_color};
use crate::palette::PaletteMode;
use crate::search::command_palette::view::{Event as CommandPaletteEvent, View as CommandPalette};
use crate::tab::{
    tab_position_id, NewSessionMenuItem, SelectedTabColor, TabBarState, TabComponent, TabData,
    TabTelemetryAction, TAB_BAR_BORDER_HEIGHT,
};
use crate::ui_components::icons;
#[cfg(target_os = "macos")]
use command::blocking::Command;
use lazy_static::lazy_static;
use pathfinder_color::ColorU;
#[cfg(target_os = "macos")]
use std::env;
use std::fmt::Write;
#[cfg(all(target_os = "macos", feature = "crash_reporting"))]
use std::fs;
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process;
use std::sync::{mpsc, Mutex};
use std::{cmp::Ordering, sync::Arc};
use warp_core::ui::theme::{color::internal_colors, phenomenon::PhenomenonStyle, Fill};
use warp_core::ui::{color::coloru_with_opacity, Icon};
use warp_editor::editor::NavigationKey;
use warpui::keymap::Context;
use warpui::notification::{RequestPermissionsOutcome, UserNotification};
use warpui::platform::{
    Cursor, FilePickerConfiguration, FullscreenState, SystemTheme, TerminationMode,
};
use warpui::text_layout::ClipConfig;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{
    accessibility::{
        AccessibilityContent, AccessibilityVerbosity, ActionAccessibilityContent, WarpA11yRole,
    },
    elements::{
        Align, Border, ChildAnchor, ChildView, Clipped, ConstrainedBox, Container, CornerRadius,
        CrossAxisAlignment, Dismiss, Element, Empty, Expanded, Fill as ElementFill, Flex,
        Highlight, Hoverable, Icon as WarpUiIcon, MainAxisAlignment, MainAxisSize,
        OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds,
        PositionedElementAnchor, PositionedElementOffsetBounds, Radius, SavePosition, Shrinkable,
        Stack, Text,
    },
    geometry::vector::{vec2f, Vector2F},
    AppContext, Entity, TypedActionView, UpdateView, View, ViewContext, ViewHandle,
};
use warpui::{
    EntityId, FocusContext, ModelHandle, SingletonEntity, UpdateModel, ViewAsRef, WeakViewHandle,
    WindowId,
};

use crate::terminal::view::LeftPanelTargetView;

/// The padding that should be applied to the workspace as a whole.
pub const WORKSPACE_PADDING: f32 = 1.0;

/// The minimum font size at which terminal text will be rendered.
const MIN_FONT_SIZE: f32 = 5.0;

/// The maximum font size at which terminal text will be rendered.
const MAX_FONT_SIZE: f32 = 25.0;

/// The increment for increasing/decreasing the font size.
const FONT_SIZE_INCREMENT: f32 = 1.0;

pub const TAB_BAR_HEIGHT: f32 = 34.;
/// Height for all panel headers (tab bar, warp drive, resource center, theme chooser, etc.).
/// This ensures consistent header heights across all UI panels.
pub const PANEL_HEADER_HEIGHT: f32 = TAB_BAR_HEIGHT;
/// The hover area height for states where the tab bar is revealed on hover.
const TAB_BAR_HOVER_HEIGHT: f32 = 12.;
const TAB_BAR_PADDING_LEFT: f32 = 4.;
const TAB_BAR_PADDING_RIGHT: f32 = 8.;
const TITLE_BAR_SEARCH_BAR_MAX_WIDTH: f32 = 320.;
#[allow(dead_code)]
const TITLE_BAR_SEARCH_BAR_SLOT_PADDING: f32 = 8.;

// The total height taken up by the tab bar, including its bottom border.
pub const TOTAL_TAB_BAR_HEIGHT: f32 = TAB_BAR_HEIGHT + TAB_BAR_BORDER_HEIGHT;

const TAB_BAR_ICON_PADDING: f32 = 4.;

const TAB_BAR_PILL_WIDTH: f32 = 100.;
const PILL_FONT_SIZE: f32 = 12.;
// We use the word "Warp" in the Update Ready button to make it obvious that the terminal is Warp.
// This can lead to free advertising when users screen-share Warp when an update is available.
const UPDATE_READY_TEXT: &str = "Update Warp";

const TAB_BAR_OVERFLOW_MENU_WIDTH: f32 = 300.;

#[cfg(not(target_family = "wasm"))]
const RESOURCE_CENTER_WIDTH: f32 = 361.;

// Ratio of terminal : theme chooser when theme chooser is active
const THEME_CHOOSER_RATIO: f32 = 3.5;

/// Save position for the tab bar.
const TAB_BAR_POSITION_ID: &str = "workspace_view:tab_bar";

/// The main content area in a workspace. This is directly below the tab bar.
const TAB_CONTENT_POSITION_ID: &str = "workspace_view:tab_content";

const WELCOME_TIPS_POSITION_ID: &str = "welcome_tips_pill";
const ELLIPSE_SVG_PATH: &str = "bundled/svg/ellipse.svg";

const AI_ASSISTANT_BUTTON_ID: &str = "workspace_view:ai_assistant_button";

const VERSION_DEPRECATION_BANNER_TEXT: &str = "Your app is out of date and some features may not work as expected. Please update immediately.";

const VERSION_DEPRECATION_WITHOUT_PERMISSIONS_BANNER_TEXT: &str = "Some Warp features may not work as expected without updating immediately, but Warp is unable to perform the update.";

const ASK_AI_ASSISTANT_KEYBINDING_NAME: &str = "workspace:toggle_ai_assistant";
const TOGGLE_RESOURCE_CENTER_KEYBINDING_NAME: &str = "workspace:toggle_resource_center";

/// Shared position ID for the new-session sidecar overlay. Used for both the
/// `SavePosition` wrapper and the safe-zone rect lookup.
const NEW_SESSION_SIDECAR_POSITION_ID: &str = "new_session_sidecar";
const NEW_SESSION_SIDECAR_WIDTH: f32 = 300.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_HEIGHT: f32 = 32.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_HORIZONTAL_PADDING: f32 = 12.;
const NEW_SESSION_SIDECAR_SEARCH_BOX_VERTICAL_PADDING: f32 = 6.;
const NEW_SESSION_SIDECAR_FOOTER_HORIZONTAL_PADDING: f32 = 16.;
const NEW_SESSION_SIDECAR_FOOTER_VERTICAL_PADDING: f32 = 8.;
const SESSION_CONFIG_TAB_CONFIG_CHIP_TEXT: &str = "Access your tab configs here.";
const SESSION_CONFIG_TAB_CONFIG_CHIP_WIDTH: f32 = 206.;
const SHOW_SETTINGS_KEYBINDING_NAME: &str = "workspace:show_settings";
pub const TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME: &str = "workspace:toggle_command_palette";

const USER_AVATAR_BUTTON_POSITION_ID: &str = "workspace:user_avatar_button";
const NOTIFICATIONS_MAILBOX_POSITION_ID: &str = "workspace:notifications_mailbox";
pub(crate) const JUMP_TO_LATEST_TOAST_BINDING_NAME: &str = "workspace:jump_to_latest_toast";
pub(crate) const TOGGLE_NOTIFICATION_MAILBOX_BINDING_NAME: &str =
    "workspace:toggle_notification_mailbox";

// these won't have to be public after we deprecate the code mode v1 project explorer which is defined in terminal
pub(crate) const TOGGLE_PROJECT_EXPLORER_BINDING_NAME: &str = "workspace:toggle_project_explorer";
pub(crate) const TOGGLE_WARP_DRIVE_BINDING_NAME: &str = "workspace:toggle_warp_drive";
pub(crate) const TOGGLE_RIGHT_PANEL_BINDING_NAME: &str = "workspace:toggle_right_panel";
pub(crate) const OPEN_GLOBAL_SEARCH_BINDING_NAME: &str = "workspace:open_global_search";
pub(crate) const NEW_TAB_BINDING_NAME: &str = "workspace:new_tab";
pub(crate) const NEW_TERMINAL_TAB_BINDING_NAME: &str = "workspace:new_terminal_tab";
pub(crate) const TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME: &str = "workspace:toggle_tab_configs_menu";

// Editable left panel toolbelt keybindings.
pub(crate) const LEFT_PANEL_PROJECT_EXPLORER_BINDING_NAME: &str =
    "workspace:left_panel_project_explorer";
pub(crate) const LEFT_PANEL_GLOBAL_SEARCH_BINDING_NAME: &str = "workspace:left_panel_global_search";
pub(crate) const LEFT_PANEL_WARP_DRIVE_BINDING_NAME: &str = "workspace:left_panel_warp_drive";

const KEYBINDINGS_TO_CACHE: [&str; 4] = [
    ASK_AI_ASSISTANT_KEYBINDING_NAME,
    TOGGLE_RESOURCE_CENTER_KEYBINDING_NAME,
    SHOW_SETTINGS_KEYBINDING_NAME,
    TOGGLE_COMMAND_PALETTE_KEYBINDING_NAME,
];

const WORKFLOW_AND_ENV_VAR_SPLIT_RATIO: f32 = 0.56;
const NOTEBOOK_SMART_SPLIT_RATIO: f32 = 0.42;

#[cfg(target_family = "wasm")]
const MOBILE_OVERLAY_PANEL_WIDTH_RATIO: f32 = 0.9;
#[cfg(target_family = "wasm")]
const MOBILE_OVERLAY_SCRIM_ALPHA: u8 = 128;

pub const NEW_TAB_BUTTON_POSITION_ID: &str = "new_tab_button";
pub const NEW_SESSION_MENU_BUTTON_POSITION_ID: &str = "new_session_menu_button";

// The max length of the title of a fork toast (after which we truncate it).
const MAX_FORK_TOAST_TITLE_LENGTH: usize = 100;

// The max length of the window title (matching conversation title truncation).
const MAX_WINDOW_TITLE_LENGTH: usize = 80;

/// The default display name used for the user if they have no associated display name.
pub const DEFAULT_USER_DISPLAY_NAME: &str = "User";

lazy_static! {
    static ref OPENING_WARP_DRIVE_ON_START_UP: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    static ref PANEL_CORNER_RADIUS: CornerRadius = CornerRadius::with_all(Radius::Pixels(8.));
    static ref PANEL_HEADER_CORNER_RADIUS: CornerRadius =
        CornerRadius::with_top(Radius::Pixels(8.));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TabConfigsMenuOpenSource {
    KeyboardShortcut,
    Pointer,
}

/// This enumerates the different kinds of banners we show to the user.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceBanner {
    /// to display the banner when we are in AutoupdateStage::UpdateReady and the
    /// user's current version has been deprecated
    VersionDeprecated,
    /// to display the AutoupdateStage::UnableToUpdateToNewVersion
    UnableToUpdateToNewVersion,
    /// to display the AutoupdateStage::UnableToLaunchNewVersion
    UnableToLaunchNewVersion,
    /// to display when the user needs to reauthenticate
    Reauth,
    // to display an anonymous user has X days left to sign in
    AnonymousUserAuth,
    /// to display when recovering from a crash that may have been due to use
    /// of Wayland
    #[cfg(target_os = "linux")]
    WaylandCrashRecovery,
    /// to display when settings.toml has errors (parse failure or invalid values)
    InvalidSettings,
}

impl WorkspaceBanner {
    /// We want some banners to have a close button and not others, e.g. if they are running a very
    /// outdated version and we want to nag them to update, AutoupdateBanner::VersionDeprecated should
    /// not be dismissible
    fn is_dismissible(&self) -> bool {
        match self {
            Self::UnableToUpdateToNewVersion => true,
            Self::UnableToLaunchNewVersion => true,
            Self::VersionDeprecated => false,
            Self::AnonymousUserAuth => false,
            Self::Reauth => true,
            #[cfg(target_os = "linux")]
            Self::WaylandCrashRecovery => true,
            Self::InvalidSettings => true,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum SessionCycleDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanePanelDirection {
    Prev,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusRegion {
    LeftPanel,
    PaneGroup,
    Other,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PanelPosition {
    Left,
    Right,
}

pub struct TabPaneGroupIdentifiers {
    pub tab_idx: usize,
    pub pane_group_id: EntityId,
    pub terminal_ids: Vec<EntityId>,
}

/// Categorization of how the tab bar should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ShowTabBar {
    /// Show the tab bar stacked on top of the pane group area.
    #[default]
    Stacked,
    /// Hide the tab bar.
    Hidden,
}

impl ShowTabBar {
    fn has_tab_bar(self) -> bool {
        matches!(self, ShowTabBar::Stacked)
    }
}

/// The type of content being displayed when the simplified WASM tab bar is shown.
/// Used to determine which elements to render (e.g., icon, info button).
#[cfg(target_family = "wasm")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimplifiedWasmTabBarContent {
    /// Viewing a Warp Drive object (notebook, workflow, env vars, AI facts, MCP servers)
    WarpDriveObject,
    /// Participating in a shared session (viewer or writer). Contains the optional ambient agent task ID.
    SharedSession { task_id: Option<AmbientAgentTaskId> },
    /// Viewing a conversation transcript. Contains the optional ambient agent task ID.
    ConversationTranscript { task_id: Option<AmbientAgentTaskId> },
}

type WorkspaceMenuHandles = (
    ViewHandle<Menu<WorkspaceAction>>,
    ViewHandle<Menu<WorkspaceAction>>,
    ViewHandle<Menu<NewSessionSidecarSelection>>,
);
type SerializedBlockListItem = crate::terminal::model::block::SerializedBlock;

#[derive(Clone, Debug, PartialEq, Eq)]
enum NewSessionSidecarSelection {
    OpenWorktreeRepo { repo_path: String },
}

/// Controls the color palette used for a workspace banner.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BannerSeverity {
    /// Warning banners use an ansi-blended yellow background.
    Warning,
    /// Error banners use an ansi-blended red background.
    Error,
}

/// Visual style for an individual banner action button.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BannerButtonVariant {
    /// No fill, no border, just text (and optional icon). Used for the primary
    /// action in the Figma design (e.g. "Fix with Oz").
    Naked,
    /// Border-only, no fill (e.g. "Open file").
    Outlined,
}

struct WorkspaceBannerButtonDetails {
    text: String,
    action: WorkspaceAction,
    variant: BannerButtonVariant,
    /// Optional leading icon shown before the label.
    icon: Option<Icon>,
    /// If set, renders an adjacent "More info" pill that dispatches this action.
    more_info_button_action: Option<WorkspaceAction>,
}

struct WorkspaceBannerFields {
    banner_type: WorkspaceBanner,
    severity: BannerSeverity,
    /// Optional bold heading rendered inline before the description.
    heading: Option<String>,
    /// Main description text (regular weight).
    description: String,
    secondary_button: Option<WorkspaceBannerButtonDetails>,
    button: Option<WorkspaceBannerButtonDetails>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DefaultSessionModeBehavior {
    /// Respect the user's default-session-mode setting and auto-enter agent view when applicable.
    Apply,
    /// Skip default-session-mode auto-entry because the caller is explicitly specifying the mode for the new session.
    Ignore,
}

/// Groups a modal view handle with the ID of the tab that was created to host
/// it, so the custom tab title can be cleared on close regardless of which tab
/// is active at that point.
struct ModalWithTab<V> {
    view: ViewHandle<V>,
    /// Set when the modal opens a new tab; consumed (taken) when the modal
    /// closes so we can clear the custom tab title.
    tab_pane_group_id: Option<EntityId>,
}

/// Context saved when the session config modal triggers `open_tab_config` and
/// the tab config has params (worktree). The params modal opens asynchronously,
/// so we store what we need to finish the tab replacement when it completes.
struct PendingSessionConfigReplacement {
    old_pane_group_id: EntityId,
}
pub struct TransferredTab {
    pub pane_group: ViewHandle<PaneGroup>,
    pub color: Option<AnsiColorIdentifier>,
    pub custom_title: Option<String>,
    pub left_panel_open: bool,
}

pub struct Workspace {
    window_id: WindowId,
    tabs: Vec<TabData>,
    active_tab_index: usize,
    hovered_tab_index: Option<TabBarHoverIndex>,
    tab_bar_hover_state: MouseStateHandle,
    tab_fixed_width: Option<f32>,
    traffic_light_mouse_states: TrafficLightMouseStates,
    tab_rename_editor: ViewHandle<EditorView>,
    pane_rename_editor: ViewHandle<EditorView>,
    tips_completed: ModelHandle<TipsCompleted>,
    user_default_shell_unsupported_banner_model_handle: ModelHandle<BannerState>,
    tab_bar_overflow_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_tab_bar_overflow_menu: bool,
    tab_right_click_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_tab_right_click_menu: Option<(usize, TabContextMenuAnchor)>,
    // TODO(CORE-2300): this used to be add_tab_dropdown_menu.
    // Because we are rolling out the change behind a feature flag,
    // keep this comment here until the feature flag is removed.
    // Otherwise people might be confused as to why there is a right click
    // menu in the "new_session_dropdown_menu"
    // Same applies to "show_new_session_dropdown_menu"
    new_session_dropdown_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_new_session_dropdown_menu: Option<Vector2F>,
    palette: ViewHandle<CommandPalette>,
    ctrl_tab_palette: ViewHandle<CommandPalette>,
    mouse_states: WorkspaceMouseStates,
    settings_pane: ViewHandle<SettingsView>,
    theme_chooser_view: ViewHandle<ThemeChooser>,
    previous_theme: Option<ThemeKind>,
    current_workspace_state: WorkspaceState,
    previous_workspace_state: Option<WorkspaceState>,
    model_event_sender: Option<mpsc::SyncSender<ModelEvent>>,
    launch_config_save_modal: ModalViewState<LaunchConfigSaveModal>,
    tab_config_params_modal: ModalViewState<Modal<TabConfigParamsModal>>,
    session_config_modal: ModalViewState<Modal<SessionConfigModal>>,
    pending_session_config_replacement: Option<PendingSessionConfigReplacement>,
    /// When set, the guided onboarding tutorial will start after the session
    /// config modal is closed (submitted or dismissed).
    pending_session_config_tab_config_chip: bool,
    show_session_config_tab_config_chip: bool,
    close_session_confirmation_dialog: ViewHandle<CloseSessionConfirmationDialog>,
    command_search_view: ViewHandle<CommandSearchView>,
    autoupdate_unable_to_update_banner_dismissed: bool,
    autoupdate_unable_to_launch_new_version: bool,
    reauth_banner_dismissed: bool,
    settings_file_error: Option<crate::settings::SettingsFileError>,
    settings_error_banner_dismissed: bool,
    header_toolbar_context_menu: ViewHandle<Menu<WorkspaceAction>>,
    show_header_toolbar_context_menu: Option<Vector2F>,
    theme_creator_modal: ViewHandle<ThemeCreatorModal>,
    theme_deletion_modal: ViewHandle<ThemeDeletionModal>,
    openwarp_launch_modal: ViewHandle<OpenWarpLaunchModal>,
    toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
    agent_toast_stack: ViewHandle<AgentToastStack>,
    update_toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
    /// We need to render some dynamic keybindings for our tooltips. These cannot be looked up in the
    /// render method, so look them up when the view is constructed and cache them here. Note that they
    /// need to be kept in sync as the keybindings change.
    cached_keybindings: HashMap<String, Option<String>>,
    is_user_menu_open: bool,
    tab_bar_pinned_by_popup: bool,
    user_menu: ViewHandle<Menu<WorkspaceAction>>,
    native_modal: ViewHandle<NativeModal>,
    shown_staging_banner_count: u32,

    // When user's open WEB for the first time, we ask them to select a preference of
    // always opening in web or opening in native app.
    #[cfg(target_family = "wasm")]
    show_wasm_nux_dialog: bool,
    #[cfg(target_family = "wasm")]
    wasm_nux_dialog: ViewHandle<WasmNUXDialog>,
    #[cfg(target_family = "wasm")]
    open_in_warp_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    view_cloud_runs_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    transcript_info_button: ViewHandle<ActionButton>,
    #[cfg(target_family = "wasm")]
    transcript_details_panel: ViewHandle<ConversationDetailsPanel>,

    left_panel_open: bool,
    left_panel_view: ViewHandle<LeftPanelView>,
    left_panel_views: Vec<ToolPanelView>,
    working_directories_model: ModelHandle<pane_group::WorkingDirectoriesModel>,
    lightbox_view: Option<ViewHandle<LightboxView>>,
    /// When true, this workspace was created to receive a transferred PaneGroup.
    /// The placeholder tab will be replaced when adopt_transferred_pane_group is called.
    pending_pane_group_transfer: bool,
    is_drag_preview_workspace: bool,
    /// Sidecar menu for submenu-parent items (Terminal, New worktree config) in the
    /// new-session dropdown. Shown as a positioned overlay next to the hovered
    /// parent item, following the model picker sidecar pattern.
    new_session_sidecar_menu: ViewHandle<Menu<NewSessionSidecarSelection>>,
    show_new_session_sidecar: bool,
    worktree_sidecar_active: bool,
    worktree_sidecar_search_editor: ViewHandle<EditorView>,
    worktree_sidecar_search_query: String,
    new_session_sidecar_add_repo_mouse_state: MouseStateHandle,
    tab_config_action_sidecar_item: Option<SidecarItemKind>,
    tab_config_action_sidecar_mouse_states: crate::tab_configs::action_sidecar::SidecarMouseStates,
    remove_tab_config_confirmation_dialog: ViewHandle<RemoveTabConfigConfirmationDialog>,
}

impl Workspace {
    pub fn is_drag_preview_workspace(&self) -> bool {
        self.is_drag_preview_workspace
    }

    fn tab_rename_editor_font_size(_ctx: &AppContext, appearance: &Appearance) -> f32 {
        appearance.ui_font_size()
    }

    /// Clears the worktree sidecar state and hides the sidecar.
    fn clear_worktree_sidecar_state(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_new_session_sidecar = false;
        self.worktree_sidecar_active = false;
        self.worktree_sidecar_search_query.clear();
        self.worktree_sidecar_search_editor
            .update(ctx, |editor, ctx| {
                editor.clear_buffer(ctx);
            });
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            menu.clear_pinned_header_builder();
            menu.clear_pinned_footer_builder();
            menu.set_content_padding_overrides(None, None);
            menu.reset_selection(view_ctx);
        });
    }

    fn close_new_session_dropdown_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_new_session_dropdown_menu = None;
        self.tab_config_action_sidecar_item = None;
        self.clear_worktree_sidecar_state(ctx);
        self.new_session_dropdown_menu.update(ctx, |menu, _| {
            menu.set_safe_zone_target(None);
            menu.set_submenu_being_shown_for_item_index(None);
        });
        ctx.notify();
    }

    fn select_first_worktree_sidecar_repo(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            if menu.items_len() > 1 {
                menu.set_selected_by_index(1, view_ctx);
            } else {
                menu.reset_selection(view_ctx);
            }
        });
    }

    fn reset_worktree_sidecar_repo_selection(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            menu.reset_selection(view_ctx);
        });
    }

    fn navigate_worktree_sidecar_selection(
        &mut self,
        select_next: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            let items_len = menu.items_len();
            if items_len <= 1 {
                return;
            }

            match menu.selected_index() {
                Some(_) if select_next => menu.select_next(view_ctx),
                Some(_) => menu.select_previous(view_ctx),
                None if select_next => menu.set_selected_by_index(1, view_ctx),
                None => menu.set_selected_by_index(items_len.saturating_sub(1), view_ctx),
            }
        });
    }

    fn confirm_worktree_sidecar_selection(&mut self, ctx: &mut ViewContext<Self>) {
        let selected_selection = self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            if menu.items_len() <= 1 {
                return None;
            }

            if menu.selected_index().is_none() {
                menu.set_selected_by_index(1, view_ctx);
            }

            menu.selected_item().and_then(|item| match item {
                MenuItem::Item(fields) => fields.on_select_action().cloned(),
                _ => None,
            })
        });

        if let Some(selection) = selected_selection {
            self.execute_new_session_sidecar_selection(selection, ctx);
            self.close_new_session_dropdown_menu(ctx);
        }
    }

    fn sync_new_session_sidecar_selection_to_hover(&mut self, ctx: &mut ViewContext<Self>) {
        self.new_session_sidecar_menu.update(ctx, |menu, view_ctx| {
            let Some(hovered_index) = menu.hovered_index() else {
                return;
            };
            let hovered_item_has_action = menu
                .items()
                .get(hovered_index)
                .and_then(MenuItem::item_on_select_action)
                .is_some();

            if hovered_item_has_action && menu.selected_index() != Some(hovered_index) {
                menu.set_selected_by_index(hovered_index, view_ctx);
            }
        });
    }

    fn build_worktree_sidecar_search_input(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let mut editor = EditorView::single_line(
                SingleLineEditorOptions {
                    text: TextOptions::ui_text(Some(appearance.ui_font_size()), appearance),
                    select_all_on_focus: true,
                    clear_selections_on_blur: true,
                    propagate_and_no_op_vertical_navigation_keys:
                        PropagateAndNoOpNavigationKeys::Always,
                    ..Default::default()
                },
                ctx,
            );
            editor.set_placeholder_text("Search repos", ctx);
            editor
        });
        ctx.subscribe_to_view(&editor, |me, editor_view, event, ctx| match event {
            EditorEvent::Edited(_) => {
                me.worktree_sidecar_search_query = editor_view.as_ref(ctx).buffer_text(ctx);
                ctx.notify();
            }
            EditorEvent::Escape => {
                me.close_new_session_dropdown_menu(ctx);
            }
            EditorEvent::Navigate(NavigationKey::Up) => {
                me.navigate_worktree_sidecar_selection(false, ctx);
            }
            EditorEvent::Navigate(NavigationKey::Down) => {
                me.navigate_worktree_sidecar_selection(true, ctx);
            }
            EditorEvent::Enter => {
                me.confirm_worktree_sidecar_selection(ctx);
            }
            _ => {}
        });
        editor
    }

    fn tab_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = {
            ctx.add_typed_action_view(|ctx| {
                let appearance = Appearance::as_ref(ctx);
                let options = SingleLineEditorOptions {
                    text: TextOptions::ui_text(
                        Some(Self::tab_rename_editor_font_size(ctx, appearance)),
                        appearance,
                    ),
                    ..Default::default()
                };
                EditorView::single_line(options, ctx)
            })
        };
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_tab_rename_editor_event(event, ctx);
        });
        editor
    }

    fn pane_rename_editor(ctx: &mut ViewContext<Self>) -> ViewHandle<EditorView> {
        let editor = ctx.add_typed_action_view(|ctx| {
            let appearance = Appearance::as_ref(ctx);
            let options = SingleLineEditorOptions {
                text: TextOptions::ui_text(Some(12.), appearance),
                ..Default::default()
            };
            EditorView::single_line(options, ctx)
        });
        ctx.subscribe_to_view(&editor, move |me, _, event, ctx| {
            me.handle_pane_rename_editor_event(event, ctx);
        });
        editor
    }

    pub fn handle_tab_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.current_workspace_state.is_tab_being_renamed() {
            match event {
                EditorEvent::Blurred | EditorEvent::Enter => {
                    self.finish_tab_rename(ctx);
                }
                EditorEvent::Escape => {
                    self.cancel_tab_rename(ctx);
                }
                _ => {}
            }
        }
    }

    pub fn handle_pane_rename_editor_event(
        &mut self,
        event: &EditorEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.current_workspace_state.is_any_pane_being_renamed() {
            match event {
                EditorEvent::Blurred | EditorEvent::Enter => {
                    self.finish_pane_rename(ctx);
                }
                EditorEvent::Escape => {
                    self.cancel_pane_rename(ctx);
                }
                _ => {}
            }
        }
    }

    fn finish_tab_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(tab_index) = self.current_workspace_state.tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            let title = self.tab_rename_editor.as_ref(ctx).buffer_text(ctx);
            let tab = &self.tabs[tab_index];
            tab.pane_group.update(ctx, |view, ctx| {
                // Only update the title if it was actually changed. Otherwise, lets assume
                // user's intend was to cancel the operation.
                if view.display_title(ctx) != title {
                    view.set_title(&title, ctx);
                }
            });
            self.clear_tab_name_editor(ctx);
            self.update_window_title(ctx);
            ctx.notify();
        }
    }

    fn finish_pane_rename(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(locator) = self.current_workspace_state.pane_being_renamed() else {
            return;
        };

        self.current_workspace_state.clear_pane_being_renamed();
        let title = self.pane_rename_editor.as_ref(ctx).buffer_text(ctx);
        self.set_custom_pane_name(locator, title, ctx);
        self.clear_pane_name_editor(ctx);
        self.focus_pane(locator, ctx);
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn cancel_tab_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if self.current_workspace_state.is_tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            self.clear_tab_name_editor(ctx);
            self.focus_active_tab(ctx);
            ctx.notify();
        }
    }

    fn cancel_pane_rename(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(locator) = self.current_workspace_state.pane_being_renamed() {
            self.current_workspace_state.clear_pane_being_renamed();
            self.clear_pane_name_editor(ctx);
            self.focus_pane(locator, ctx);
            ctx.notify();
        }
    }

    fn build_settings_views(
        global_resource_handles: GlobalResourceHandles,
        tips_completed: ModelHandle<TipsCompleted>,
        ctx: &mut ViewContext<Self>,
    ) -> (ViewHandle<SettingsView>, ViewHandle<ThemeChooser>) {
        let theme_chooser_view = ctx.add_typed_action_view(|ctx| {
            ThemeChooser::new(
                global_resource_handles.referral_theme_status,
                ctx,
                tips_completed,
            )
        });

        ctx.subscribe_to_view(&theme_chooser_view, |me, _, event, ctx| {
            me.handle_theme_chooser_event(event, ctx);
        });

        let settings_pane = ctx.add_typed_action_view(move |ctx| SettingsView::new(None, ctx));
        let window_id = ctx.window_id();
        SettingsPaneManager::handle(ctx).update(ctx, |manager, _| {
            manager.register_view(window_id, settings_pane.clone());
        });

        (settings_pane, theme_chooser_view)
    }

    fn build_theme_creator_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<ThemeCreatorModal> {
        let theme_creator_modal = ctx.add_typed_action_view(ThemeCreatorModal::new);
        ctx.subscribe_to_view(&theme_creator_modal, move |me, _, event, ctx| {
            me.handle_theme_creator_modal_event(event, ctx);
        });

        theme_creator_modal
    }

    fn build_theme_deletion_modal(ctx: &mut ViewContext<Self>) -> ViewHandle<ThemeDeletionModal> {
        let theme_deletion_modal = ctx.add_typed_action_view(ThemeDeletionModal::new);
        ctx.subscribe_to_view(&theme_deletion_modal, move |me, _, event, ctx| {
            me.handle_theme_deletion_modal_event(event, ctx);
        });

        theme_deletion_modal
    }

    fn build_close_session_confirmation_dialog(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<CloseSessionConfirmationDialog> {
        let close_session_confirmation_dialog =
            ctx.add_typed_action_view(|_| CloseSessionConfirmationDialog::new());
        ctx.subscribe_to_view(
            &close_session_confirmation_dialog,
            move |me, _, event, ctx| {
                me.handle_close_session_confirmation_dialog_event(event, ctx);
            },
        );

        close_session_confirmation_dialog
    }

    fn build_native_modal_view(ctx: &mut ViewContext<Self>) -> ViewHandle<NativeModal> {
        let native_modal = ctx.add_typed_action_view(NativeModal::new);
        ctx.subscribe_to_view(&native_modal, move |me, _, event, ctx| {
            me.handle_native_modal_event(event, ctx);
        });
        native_modal
    }

    fn build_tab_bar_overflow_menu(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<Menu<WorkspaceAction>> {
        let tab_bar_overflow_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_width(TAB_BAR_OVERFLOW_MENU_WIDTH)
                .with_drop_shadow()
        });
        ctx.subscribe_to_view(&tab_bar_overflow_menu, move |me, _, event, ctx| {
            me.handle_tab_bar_overflow_menu_event(event, ctx);
        });
        tab_bar_overflow_menu
    }

    fn build_menus(ctx: &mut ViewContext<Self>) -> WorkspaceMenuHandles {
        let tab_right_click_menu = ctx.add_typed_action_view(|_| Menu::new());
        ctx.subscribe_to_view(&tab_right_click_menu, move |me, _, event, ctx| {
            me.handle_tab_right_click_menu_event(event, ctx);
        });

        // Currently setting the width to 300 px as a middle ground that looks
        // ok when the shells show the path to the executables, and when they
        // don't. Going forward we may want to enhance the menu to allow for a
        // `max_width` and `min_width` instead, so we can allow the menu to
        // grow as needed.
        const NEW_SESSION_MENU_WIDTH: f32 = 300.;
        let new_session_menu = ctx.add_typed_action_view(|ctx| {
            if FeatureFlag::ShellSelector.is_enabled() {
                let theme = Appearance::as_ref(ctx).theme();
                Menu::new()
                    .with_width(NEW_SESSION_MENU_WIDTH)
                    .with_border(Border::all(1.).with_border_color(theme.outline().into()))
                    .with_drop_shadow()
                    .with_safe_triangle()
                    .with_ignore_hover_when_covered()
                    .prevent_interaction_with_other_elements()
            } else {
                Menu::new()
                    .with_safe_triangle()
                    .with_ignore_hover_when_covered()
            }
        });
        ctx.subscribe_to_view(&new_session_menu, move |me, _, event, ctx| {
            me.handle_new_session_menu_event(event, ctx);
        });

        let new_session_sidecar = ctx.add_typed_action_view(|_ctx| {
            let mut menu = Menu::new()
                .without_item_action_dispatch()
                .with_width(NEW_SESSION_SIDECAR_WIDTH)
                .with_menu_variant(crate::menu::MenuVariant::scrollable());
            menu.set_height(400.);
            menu
        });
        ctx.subscribe_to_view(&new_session_sidecar, move |me, _, event, ctx| {
            me.handle_new_session_sidecar_event(event, ctx);
        });

        (tab_right_click_menu, new_session_menu, new_session_sidecar)
    }

    fn build_launch_config_save_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<LaunchConfigSaveModal> {
        let launch_config_save_modal = ctx.add_typed_action_view(LaunchConfigSaveModal::new);
        ctx.subscribe_to_view(&launch_config_save_modal, move |me, _, event, ctx| {
            me.handle_launch_config_save_modal_event(event, ctx);
        });

        ModalViewState::new(launch_config_save_modal)
    }

    fn build_tab_config_params_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<Modal<TabConfigParamsModal>> {
        let body = ctx.add_typed_action_view(TabConfigParamsModal::new);
        // Subscribe to body events before moving `body` into the Modal closure.
        ctx.subscribe_to_view(&body, |me, _, event, ctx| {
            me.handle_tab_config_params_modal_body_event(event, ctx);
        });
        let modal = ctx.add_typed_action_view(|ctx| {
            Modal::new(None, body, ctx)
                .with_modal_style(UiComponentStyles {
                    width: Some(460.),
                    height: Some(480.),
                    ..Default::default()
                })
                .with_body_style(UiComponentStyles {
                    padding: Some(Coords::uniform(0.)),
                    height: Some(480.),
                    background: Some(ElementFill::None),
                    ..Default::default()
                })
                .with_dismiss_on_click()
        });
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            me.handle_tab_config_params_modal_event(event, ctx);
        });
        ModalViewState::new(modal)
    }

    fn build_session_config_modal(
        ctx: &mut ViewContext<Self>,
    ) -> ModalViewState<Modal<SessionConfigModal>> {
        let body = ctx.add_typed_action_view(SessionConfigModal::new);
        ctx.subscribe_to_view(&body, |me, _, event, ctx| {
            me.handle_session_config_modal_event(event, ctx);
        });
        let modal = ctx.add_typed_action_view(|ctx| {
            Modal::new(None, body, ctx)
                .close_modal_button_disabled()
                .with_modal_style(UiComponentStyles {
                    width: Some(424.),
                    ..Default::default()
                })
                .with_background_opacity(0)
                .with_body_style(UiComponentStyles {
                    padding: Some(Coords::uniform(0.)),
                    ..Default::default()
                })
                .with_header_style(UiComponentStyles {
                    height: Some(0.),
                    padding: Some(Coords::uniform(0.)),
                    ..Default::default()
                })
        });
        ctx.subscribe_to_view(&modal, |me, _, event, ctx| {
            if matches!(event, ModalEvent::Close) {
                me.close_session_config_modal(ctx);
            }
        });
        ModalViewState::new(modal)
    }

    fn build_remove_tab_config_confirmation_dialog(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<RemoveTabConfigConfirmationDialog> {
        let dialog = ctx.add_typed_action_view(RemoveTabConfigConfirmationDialog::new);
        dialog
    }

    fn handle_session_config_modal_event(
        &mut self,
        event: &SessionConfigModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            SessionConfigModalEvent::Completed(selection) => {
                self.close_session_config_modal(ctx);
                self.handle_session_config_completed(selection, ctx);

                // Show the chip only when no params modal followed.
                if !self.current_workspace_state.is_tab_config_params_modal_open {
                    self.promote_session_config_tab_config_chip(ctx);
                }
            }
            SessionConfigModalEvent::Dismissed => {
                // No tab config was created, so don't show the chip.
                self.pending_session_config_tab_config_chip = false;
                self.close_session_config_modal(ctx);
            }
        }
    }

    /// Stores an onboarding intention so the guided tutorial starts after the
    /// session config modal is closed.
    #[cfg(feature = "local_fs")]
    fn handle_session_config_completed(
        &mut self,
        selection: &crate::tab_configs::session_config::SessionConfigSelection,
        ctx: &mut ViewContext<Self>,
    ) {
        use crate::tab_configs::session_config::{build_tab_config, write_tab_config};

        // Build a TabConfig.
        let config = build_tab_config(
            &selection.session_type,
            &selection.directory,
            selection.enable_worktree,
            selection.autogenerate_worktree_branch_name,
        );

        let old_pane_group_id = self.active_tab_pane_group().id();
        let has_params = !config.params.is_empty();

        // Save and open the tab config. The user's `default_session_mode`
        // is intentionally left untouched: creating a tab config should not
        // change the global default for new tabs.
        // Agent view entry for Oz is handled by PaneMode::Agent in the tab config,
        // so no manual enter_agent_view call is needed.
        let dir = crate::user_config::tab_configs_dir();
        if let Err(e) = write_tab_config(&config, &dir, "startup_config") {
            log::warn!("Failed to write startup tab config: {e:?}");
        }

        if has_params {
            // When the config has params (worktree), open_tab_config shows the
            // params modal instead of creating the tab immediately.
            // Store the replacement context so we can finish when the modal completes.
            self.pending_session_config_replacement =
                Some(PendingSessionConfigReplacement { old_pane_group_id });
            self.open_tab_config(config, ctx);
        } else {
            let worktree_branch_name = self.maybe_generate_worktree_name(&config);
            let param_values = config.default_param_values();
            self.open_tab_config_with_params(
                config,
                param_values,
                worktree_branch_name.as_deref(),
                ctx,
            );
            self.remove_tab_by_pane_group_id(old_pane_group_id, ctx);
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn handle_session_config_completed(
        &mut self,
        _selection: &crate::tab_configs::session_config::SessionConfigSelection,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    pub(crate) fn show_session_config_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.session_config_modal.view.update(ctx, |modal, ctx| {
            modal.body().update(ctx, |body, ctx| {
                body.configure(false);
                ctx.notify();
            });
        });

        self.session_config_modal.open();
        self.current_workspace_state.is_session_config_modal_open = true;
        self.pending_session_config_tab_config_chip = false;
        self.show_session_config_tab_config_chip = false;
        ctx.focus(&self.session_config_modal.view);
        ctx.notify();
    }

    fn close_session_config_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.session_config_modal.close();
        self.current_workspace_state.is_session_config_modal_open = false;
        // Don't promote pending → show here. The caller is responsible for
        // calling `promote_session_config_tab_config_chip` once all
        // intermediate modals (e.g. params modal) have closed.
        self.focus_active_tab(ctx);
        ctx.notify();
    }

    /// Promotes the pending tab-config chip to visible. This must be called
    /// only after **all** intermediate modals (session config modal, params
    /// modal) are closed. The chip is non-blocking: the user can still
    /// interact with the terminal and must click the chip's close button or
    /// press Escape/Enter to dismiss it.
    fn promote_session_config_tab_config_chip(&mut self, ctx: &mut ViewContext<Self>) {
        if self.pending_session_config_tab_config_chip {
            self.show_session_config_tab_config_chip = true;
            self.pending_session_config_tab_config_chip = false;
            ctx.notify();
        }
    }

    fn should_show_session_config_tab_config_chip(&self) -> bool {
        self.show_session_config_tab_config_chip
            && !self.current_workspace_state.is_session_config_modal_open
            && !self.current_workspace_state.is_tab_config_params_modal_open
    }

    fn dismiss_session_config_tab_config_chip(&mut self, ctx: &mut ViewContext<Self>) {
        self.pending_session_config_tab_config_chip = false;
        self.show_session_config_tab_config_chip = false;
        ctx.notify();
    }

    /// Subscribe to the [`ServerApiProvider`] model to report status changes.
    fn subscribe_to_workspace_toast_stack(
        toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        let workspace_toast_stack = WorkspaceToastStack::handle(ctx);
        ctx.subscribe_to_model(
            &workspace_toast_stack,
            move |_me, _, event, ctx| match event {
                WorkspaceToastStackEvent::AddEphemeralToast { window_id, toast }
                    if *window_id == ctx.window_id() =>
                {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.add_ephemeral_toast(toast.clone(), ctx)
                    });
                }
                WorkspaceToastStackEvent::AddPersistentToast { window_id, toast }
                    if *window_id == ctx.window_id() =>
                {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.add_persistent_toast(toast.clone(), ctx)
                    });
                }
                WorkspaceToastStackEvent::RemoveToast {
                    window_id,
                    identifier,
                } if *window_id == ctx.window_id() => {
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.dismiss_older_toasts(identifier, ctx)
                    });
                }
                _ => {}
            },
        );
    }

    /// Subscribes to `WarpConfigUpdateEvent::TabConfigErrors` and shows a persistent
    /// error toast for each tab config file that failed to parse.  Uses `object_id`
    /// keyed by file path so that re-saving the same file auto-dismisses the stale
    /// toast.
    fn subscribe_to_tab_config_errors(
        toast_stack: ViewHandle<DismissibleToastStack<WorkspaceAction>>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.subscribe_to_model(&WarpConfig::handle(ctx), move |_me, _, event, ctx| {
            match event {
                WarpConfigUpdateEvent::TabConfigs => {
                    // On every tab config reload, dismiss error toasts for
                    // files that now parse successfully.  The model has already
                    // been updated with the current error set before this event
                    // fires, so we just need to clear stale toasts.
                    //
                    // `TabConfigErrors` is only emitted when errors exist, so
                    // when all files are fixed we only get `TabConfigs` — this
                    // branch handles that case by prefix-dismissing all
                    // tab-config-error toasts and letting `TabConfigErrors`
                    // re-add any that still apply.
                    toast_stack.update(ctx, |toast_stack, ctx| {
                        toast_stack.dismiss_toasts_by_prefix("tab_config_error:", ctx);
                    });
                }
                WarpConfigUpdateEvent::TabConfigErrors(errors) => {
                    let home_dir = dirs::home_dir();
                    for error in errors {
                        let object_id = format!("tab_config_error:{}", error.file_path.display());
                        let raw_path = error.file_path.display().to_string();
                        let friendly_path = user_friendly_path(
                            &raw_path,
                            home_dir.as_ref().and_then(|h| h.to_str()),
                        );
                        let message = format!(
                            "Failed to load tab config {friendly_path}: {}",
                            error.error_message
                        );
                        let path = error.file_path.clone();
                        let toast = DismissibleToast::error(message)
                            .with_object_id(object_id.clone())
                            .with_link(
                                ToastLink::new("Open file".to_string()).with_onclick_action(
                                    WorkspaceAction::OpenTabConfigErrorFile {
                                        path,
                                        toast_object_id: object_id,
                                    },
                                ),
                            );
                        toast_stack.update(ctx, |toast_stack, ctx| {
                            toast_stack.add_persistent_toast(toast, ctx);
                        });
                    }
                }
                _ => {}
            }
        });
    }

    /// Subscribes to `WarpConfigUpdateEvent::SettingsErrors` and
    /// `SettingsErrorsCleared` to update the workspace settings-error banner
    /// and mirror the state into the settings pane for its nav-rail footer.
    fn subscribe_to_settings_errors(ctx: &mut ViewContext<Self>) {
        ctx.subscribe_to_model(&WarpConfig::handle(ctx), |me, _, event, ctx| match event {
            WarpConfigUpdateEvent::SettingsErrors(error) => {
                me.settings_file_error = Some(error.clone());
                me.sync_settings_error_state_into_settings_pane(ctx);
                ctx.notify();
            }
            WarpConfigUpdateEvent::SettingsErrorsCleared => {
                me.settings_file_error = None;
                me.sync_settings_error_state_into_settings_pane(ctx);
                ctx.notify();
            }
            _ => {}
        });
    }

    /// Pushes the current settings-file error + banner-dismissal state into
    /// the settings pane so its nav-rail footer ("Open settings file" button
    /// or inline error alert) stays in sync with the workspace banner.
    fn sync_settings_error_state_into_settings_pane(&mut self, ctx: &mut ViewContext<Self>) {
        let error = self.settings_file_error.clone();
        let dismissed = self.settings_error_banner_dismissed;
        self.settings_pane.update(ctx, |view, ctx| {
            view.set_settings_error_state(error, dismissed, ctx);
        });
    }

    pub fn dismiss_older_toasts(&mut self, object_id: &str, ctx: &mut ViewContext<Self>) {
        self.toast_stack.update(ctx, |toast_stack, ctx| {
            toast_stack.dismiss_older_toasts(object_id, ctx);
        });
    }

    fn on_tips_model_changed(
        &mut self,
        _: ModelHandle<TipsCompleted>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.notify();
    }

    pub fn new(
        global_resource_handles: GlobalResourceHandles,
        workspace_setting: NewWorkspaceSource,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let GlobalResourceHandles {
            model_event_sender,
            tips_completed,
            user_default_shell_unsupported_banner_model_handle,
            referral_theme_status,
            settings_file_error,
        } = global_resource_handles.clone();

        // Inserting a (window, ModalSizes) pair to the ResizableData singleton. A restored window
        // reads the sizes from the window snapshot. A new window initializes with all default sizes.
        let resizable_data = ResizableData::handle(ctx);
        let window_id = ctx.window_id();
        let has_horizontal_split = workspace_setting.has_horizontal_split();

        let (left_panel_size, right_panel_size) =
            compute_default_panel_widths(ctx, window_id, has_horizontal_split);
        let new_resizable_modal_sizes = match workspace_setting.clone() {
            NewWorkspaceSource::Restored {
                window_snapshot, ..
            } => ModalSizes::from_restored(&window_snapshot, left_panel_size, right_panel_size),
            _ => ModalSizes::default_with_panel_defaults(left_panel_size, right_panel_size),
        };
        resizable_data.update(ctx, |model, _| {
            model.insert(window_id, new_resizable_modal_sizes)
        });

        terminal::platform::init().expect("Terminal platform initialized");

        let tab_bar_overflow_menu = Self::build_tab_bar_overflow_menu(ctx);
        let (tab_right_click_menu, new_session_dropdown_menu, new_session_sidecar_menu) =
            Self::build_menus(ctx);

        // Subscribe to network changes
        ctx.subscribe_to_model(
            &NetworkStatus::handle(ctx),
            Self::handle_network_status_event,
        );

        let palette =
            ctx.add_typed_action_view(|ctx| CommandPalette::new(NavigationMode::Normal, ctx));
        ctx.subscribe_to_view(&palette, |me, _, event, ctx| {
            me.handle_palette_event(event, ctx);
        });

        let ctrl_tab_palette =
            ctx.add_typed_action_view(|ctx| CommandPalette::new(NavigationMode::CtrlTab, ctx));
        ctx.subscribe_to_view(&ctrl_tab_palette, |me, _, event, ctx| {
            me.handle_palette_event(event, ctx);
        });

        // Handle theme updates when there is a cloud update to themes while the picker is open.
        ctx.subscribe_to_model(&ThemeSettings::handle(ctx), |me, _, _, ctx| {
            if me.is_theme_chooser_open() {
                me.theme_chooser_view.update(ctx, |view, ctx| {
                    view.handle_theme_change(ctx);
                });
            }
        });

        let bindings_notifier = KeybindingChangedNotifier::handle(ctx);
        ctx.subscribe_to_model(&bindings_notifier, |me, _, event, ctx| {
            me.handle_keybinding_changed(event, ctx);
        });

        let state_handle = WindowManager::handle(ctx);
        ctx.subscribe_to_model(&state_handle, |me, _, event, ctx| {
            me.handle_window_state_change(event, ctx);
        });

        let (settings_pane, theme_chooser_view) =
            Self::build_settings_views(global_resource_handles, tips_completed.clone(), ctx);

        let theme_creator_modal = Self::build_theme_creator_modal(ctx);

        let theme_deletion_modal = Self::build_theme_deletion_modal(ctx);

        let openwarp_launch_view = ctx.add_typed_action_view(OpenWarpLaunchModal::new);
        let launch_config_save_modal = Self::build_launch_config_save_modal(ctx);

        let tab_config_params_modal = Self::build_tab_config_params_modal(ctx);

        let session_config_modal = Self::build_session_config_modal(ctx);

        let close_session_confirmation_dialog = Self::build_close_session_confirmation_dialog(ctx);
        let command_search_view = ctx.add_typed_action_view(CommandSearchView::new);
        ctx.subscribe_to_view(&command_search_view, |me, _, event, ctx| {
            me.handle_command_search_event(event, ctx);
        });

        let working_directories_model =
            ctx.add_model(|_| pane_group::WorkingDirectoriesModel::new());

        let left_panel_views = Self::compute_left_panel_views(ctx);

        let left_panel_view = ctx.add_typed_action_view(|ctx| {
            LeftPanelView::new(
                working_directories_model.clone(),
                left_panel_views.clone(),
                ctx,
            )
        });

        ctx.subscribe_to_view(&left_panel_view, |me, _, event, ctx| {
            me.handle_left_panel_event(event, ctx);
        });

        ctx.observe(&tips_completed, Workspace::on_tips_model_changed);

        ctx.subscribe_to_model(
            &SessionSettings::handle(ctx),
            Self::handle_session_settings_event,
        );

        let tab_settings_handle = TabSettings::handle(ctx);
        ctx.subscribe_to_model(&tab_settings_handle, |me, _, event, ctx| {
            me.handle_tab_settings_change(event, ctx)
        });

        ctx.subscribe_to_model(&CodeSettings::handle(ctx), |me, _, event, ctx| {
            if matches!(
                event,
                CodeSettingsChangedEvent::ShowProjectExplorer { .. }
                    | CodeSettingsChangedEvent::ShowGlobalSearch { .. }
            ) {
                me.update_left_panel_available_views(ctx);
                ctx.notify();
            }
        });

        let toast_stack =
            ctx.add_typed_action_view(|_| DismissibleToastStack::new(Duration::from_secs(4)));

        let agent_toast_stack =
            ctx.add_typed_action_view(|ctx| AgentToastStack::new(Duration::from_secs(4), ctx));

        let update_toast_stack =
            ctx.add_typed_action_view(|_| DismissibleToastStack::new(Duration::from_secs(4)));

        #[cfg(target_family = "wasm")]
        let wasm_nux_dialog = Self::build_wasm_nux_dialog(ctx);

        #[cfg(target_family = "wasm")]
        let open_in_warp_button = Self::build_open_in_warp_button(ctx);

        #[cfg(target_family = "wasm")]
        let transcript_info_button = Self::build_transcript_info_button(ctx);

        #[cfg(target_family = "wasm")]
        let view_cloud_runs_button = Self::build_view_cloud_runs_button(ctx);

        #[cfg(target_family = "wasm")]
        let transcript_details_panel = Self::build_transcript_details_panel(ctx);

        // Subscribe to task updates so the transcript details panel can refresh when task data arrives
        #[cfg(target_family = "wasm")]
        ctx.subscribe_to_model(
            &AgentConversationsModel::handle(ctx),
            |me, _, event, ctx| match event {
                // Update transcript details if task or conversation data is updated
                AgentConversationsModelEvent::NewTasksReceived
                | AgentConversationsModelEvent::TasksUpdated
                | AgentConversationsModelEvent::ConversationUpdated
                | AgentConversationsModelEvent::ConversationArtifactsUpdated { .. } => {
                    me.update_transcript_details_panel_data(ctx);
                }
                _ => {}
            },
        );

        let cached_keybindings = KEYBINDINGS_TO_CACHE
            .iter()
            .map(|name| {
                (
                    String::from(*name),
                    keybinding_name_to_display_string(name, ctx),
                )
            })
            .collect();

        Self::subscribe_to_workspace_toast_stack(toast_stack.clone(), ctx);
        Self::subscribe_to_tab_config_errors(toast_stack.clone(), ctx);
        Self::subscribe_to_settings_errors(ctx);
        Self::subscribe_to_shared_session_manager(ctx);

        let user_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&user_menu, |me, _, event, ctx| {
            if let MenuEvent::Close { .. } = event {
                me.is_user_menu_open = false;
                ctx.notify();
            }
        });

        let native_modal = Self::build_native_modal_view(ctx);

        let mut ws = Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            hovered_tab_index: None,
            tab_bar_hover_state: Default::default(),
            traffic_light_mouse_states: Default::default(),
            tab_rename_editor: Self::tab_rename_editor(ctx),
            pane_rename_editor: Self::pane_rename_editor(ctx),
            tips_completed,
            user_default_shell_unsupported_banner_model_handle,
            tab_bar_overflow_menu,
            show_tab_bar_overflow_menu: false,
            tab_right_click_menu,
            show_tab_right_click_menu: None,
            new_session_dropdown_menu,
            show_new_session_dropdown_menu: None,
            palette,
            ctrl_tab_palette,
            mouse_states: Default::default(),
            previous_theme: None,
            settings_pane,
            theme_chooser_view,
            current_workspace_state: Default::default(),
            previous_workspace_state: None,
            model_event_sender,
            launch_config_save_modal,
            tab_config_params_modal,
            session_config_modal,
            pending_session_config_replacement: None,
            pending_session_config_tab_config_chip: false,
            show_session_config_tab_config_chip: false,
            close_session_confirmation_dialog,
            command_search_view,
            autoupdate_unable_to_update_banner_dismissed: false,
            autoupdate_unable_to_launch_new_version: false,
            reauth_banner_dismissed: false,
            settings_file_error,
            settings_error_banner_dismissed: false,
            theme_creator_modal,
            theme_deletion_modal,
            window_id: ctx.window_id(),
            toast_stack,
            agent_toast_stack,
            update_toast_stack,
            cached_keybindings,
            header_toolbar_context_menu: Self::build_header_toolbar_context_menu(ctx),
            show_header_toolbar_context_menu: None,
            is_user_menu_open: false,
            tab_bar_pinned_by_popup: false,
            user_menu,
            native_modal,
            left_panel_open: false,
            left_panel_view,
            left_panel_views,
            working_directories_model,
            shown_staging_banner_count: 0,

            #[cfg(target_family = "wasm")]
            show_wasm_nux_dialog: WasmNUXDialog::should_display(ctx),
            #[cfg(target_family = "wasm")]
            wasm_nux_dialog,
            #[cfg(target_family = "wasm")]
            open_in_warp_button,
            #[cfg(target_family = "wasm")]
            transcript_info_button,
            #[cfg(target_family = "wasm")]
            view_cloud_runs_button,
            #[cfg(target_family = "wasm")]
            transcript_details_panel,
            tab_fixed_width: None,
            openwarp_launch_modal: openwarp_launch_view,
            lightbox_view: None,
            pending_pane_group_transfer: false,
            is_drag_preview_workspace: false,
            new_session_sidecar_menu,
            show_new_session_sidecar: false,
            worktree_sidecar_active: false,
            worktree_sidecar_search_editor: Self::build_worktree_sidecar_search_input(ctx),
            worktree_sidecar_search_query: String::new(),
            new_session_sidecar_add_repo_mouse_state: Default::default(),
            tab_config_action_sidecar_item: None,
            tab_config_action_sidecar_mouse_states: Default::default(),
            remove_tab_config_confirmation_dialog:
                Self::build_remove_tab_config_confirmation_dialog(ctx),
        };

        ws.configure_new_workspace(workspace_setting, ctx);
        ws.sync_panel_positions_from_config(ctx);
        ws.sync_window_button_visibility(ctx);
        ws.update_titlebar_height(ctx);
        // Seed the settings pane with the initial settings-file error (if
        // any) read from `GlobalResourceHandles`. Subsequent updates are
        // pushed by `subscribe_to_settings_errors` and `dismiss_workspace_banner`.
        ws.sync_settings_error_state_into_settings_pane(ctx);

        let weak_handle = ctx.handle();
        WorkspaceRegistry::handle(ctx).update(ctx, |registry, _| {
            registry.register(window_id, weak_handle);
        });

        ws
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn command_palette_view(&self) -> ViewHandle<crate::search::command_palette::View> {
        self.palette.clone()
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn ai_fact_view(&self) -> ViewHandle<AIFactView> {
        self.ai_fact_view.clone()
    }

    /// Handles updating the tab status when an agent task status changes.
    fn workspace_contains_terminal_view(
        &self,
        terminal_view_id: EntityId,
        ctx: &AppContext,
    ) -> bool {
        self.tabs.iter().any(|tab| {
            tab.pane_group
                .as_ref(ctx)
                .contains_terminal_view(terminal_view_id, ctx)
        })
    }

    /// Handle session settings changes.
    fn handle_session_settings_event(
        &mut self,
        session_settings: ModelHandle<SessionSettings>,
        event: &SessionSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let SessionSettingsChangedEvent::HonorPS1 { .. } = event {
            let honor_ps1 = *session_settings.as_ref(ctx).honor_ps1;
            for tab in &self.tabs {
                // Each tab has a pane group.
                tab.pane_group.update(ctx, |pane_group, ctx| {
                    pane_group.send_prompt_change_bindkey_to_all_sessions(honor_ps1, ctx);
                });
            }
        }

        // When Notifications settings change, request system notification permissions if needed.
        if let SessionSettingsChangedEvent::Notifications { .. } = event {
            self.request_notification_permissions_if_needed(ctx);
        }
    }

    /// Handle a change to the tab settings.
    fn handle_tab_settings_change(
        &mut self,
        event: &TabSettingsChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            TabSettingsChangedEvent::WorkspaceDecorationVisibility { .. } => {
                self.sync_window_button_visibility(ctx);
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowIndicatorsButton { .. }
            | TabSettingsChangedEvent::NewTabPlacement { .. }
            | TabSettingsChangedEvent::TabCloseButtonPosition { .. }
            | TabSettingsChangedEvent::PreserveActiveTabColor { .. } => {
                self.sync_window_button_visibility(ctx);
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowCodeReviewButton { .. } => {
                ctx.notify();
            }
            TabSettingsChangedEvent::ShowCodeReviewDiffStats { .. } => {
                ctx.notify();
            }
            TabSettingsChangedEvent::DirectoryTabColors { .. } => {
                if FeatureFlag::DirectoryTabColors.is_enabled() {
                    for tab in &mut self.tabs {
                        Self::sync_codebase_tab_color(tab, ctx);
                    }
                }
                ctx.notify();
            }
            TabSettingsChangedEvent::HeaderToolbarChipSelection { .. } => {
                self.sync_panel_positions_from_config(ctx);
                ctx.notify();
            }
        }
    }

    /// Opens a launch config window into the workspace.
    pub fn open_launch_config_window(
        &mut self,
        window: WindowTemplate,
        ctx: &mut ViewContext<Self>,
    ) {
        let start_index = self.tabs.len();

        window
            .tabs
            .iter()
            .enumerate()
            .for_each(|(tab_index, tab_template)| {
                self.add_tab_with_pane_layout(
                    PanesLayout::Template(tab_template.layout.clone()),
                    Arc::new(HashMap::new()),
                    tab_template.title.clone(),
                    ctx,
                );
                self.tabs[start_index + tab_index].selected_color = tab_template
                    .color
                    .map_or(SelectedTabColor::Unset, SelectedTabColor::Color);
            });

        if !window.tabs.is_empty() {
            // Focus the active tab from the launch config.

            let mut index = start_index + window.active_tab_index.unwrap_or_default();

            if index >= self.tab_count() {
                index = start_index;
            }

            self.activate_tab_internal(index, ctx);
        }
    }

    fn configure_new_workspace(
        &mut self,
        workspace_setting: NewWorkspaceSource,
        ctx: &mut ViewContext<Self>,
    ) {
        match workspace_setting {
            NewWorkspaceSource::Empty {
                previous_active_window,
                shell,
            } => {
                self.configure_empty_workspace(previous_active_window, shell, ctx);
            }
            NewWorkspaceSource::Restored { window_snapshot } => {
                let active_tab_index = window_snapshot.active_tab_index;
                let restored_left_panel_open = window_snapshot.left_panel_open;

                window_snapshot
                    .tabs
                    .iter()
                    .enumerate()
                    .for_each(|(tab_index, saved_tab)| {
                        let custom_title = saved_tab.custom_title.clone();
                        self.add_tab_with_pane_layout(
                            PanesLayout::Snapshot(Box::new(saved_tab.root.clone())),
                            Arc::new(HashMap::new()),
                            custom_title,
                            ctx,
                        );
                        self.tabs[tab_index].default_directory_color =
                            saved_tab.default_directory_color;
                        self.tabs[tab_index].selected_color = saved_tab.selected_color;

                        let pane_group = self.tabs[tab_index].pane_group.clone();

                        if let Some(left_panel_snapshot) = &saved_tab.left_panel {
                            self.restore_left_panel_for_tab(&pane_group, left_panel_snapshot, ctx);
                        }
                    });

                if self.tab_count() == 0 {
                    // If we still haven't created any tabs after attempting to restore, create a new tab
                    // with sensible defaults.
                    self.add_new_session_tab_with_default_mode(
                        NewSessionSource::Window,
                        None,  /* previous_active_window */
                        None,  /* chosen_shell */
                        false, /* hide_homepage */
                        ctx,
                    );
                } else if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = restored_left_panel_open;
                }

                self.activate_tab_internal(active_tab_index, ctx);
            }
            NewWorkspaceSource::FromTemplate { window_template } => {
                self.open_launch_config_window(window_template, ctx);
            }
            NewWorkspaceSource::Session { options } => {
                self.add_tab_with_pane_layout(
                    PanesLayout::SingleTerminal(options),
                    Arc::new(HashMap::new()),
                    None,
                    ctx,
                );
            }
            #[cfg(feature = "local_fs")]
            NewWorkspaceSource::TransferredTab {
                tab_color,
                custom_title,
                left_panel_open,
                for_drag_preview,
                ..
            } => {
                self.is_drag_preview_workspace = for_drag_preview;
                self.add_tab_with_pane_layout(
                    Default::default(),
                    Arc::new(HashMap::new()),
                    custom_title,
                    ctx,
                );
                if let (Some(color), Some(tab)) = (tab_color, self.tabs.last_mut()) {
                    tab.selected_color = SelectedTabColor::Color(color);
                }
                if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = left_panel_open;
                }
                self.pending_pane_group_transfer = true;
            }
            #[cfg(not(feature = "local_fs"))]
            NewWorkspaceSource::TransferredTab {
                tab_color,
                custom_title,
                left_panel_open,
                for_drag_preview,
                ..
            } => {
                self.is_drag_preview_workspace = for_drag_preview;
                self.add_tab_with_pane_layout(
                    Default::default(),
                    Arc::new(HashMap::new()),
                    custom_title,
                    ctx,
                );
                if let (Some(color), Some(tab)) = (tab_color, self.tabs.last_mut()) {
                    tab.selected_color = SelectedTabColor::Color(color);
                }
                if self.left_panel_visibility_across_tabs_enabled(ctx) {
                    self.left_panel_open = left_panel_open;
                }
                self.pending_pane_group_transfer = true;
            }
            _ => {}
        };

        debug_assert!(
            self.tab_count() > 0,
            "Workspace should have at least one tab upon configuration"
        );

        if self.left_panel_visibility_across_tabs_enabled(ctx) {
            self.reconcile_left_panel_open_for_active_tab(ctx);
        }

        let active_pane_group = self.active_tab_pane_group().clone();
        let working_directories_model = self.working_directories_model.clone();
        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.set_active_pane_group(active_pane_group, &working_directories_model, ctx);
        });
    }

    fn restore_left_panel_for_tab(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        left_panel_snapshot: &LeftPanelSnapshot,
        ctx: &mut ViewContext<Self>,
    ) {
        pane_group.update(ctx, |pg, ctx| {
            pg.set_left_panel_open(true, ctx);
        });

        let resizable = ResizableData::handle(ctx);
        if let Some(modal_sizes) = resizable.as_ref(ctx).get_all_handles(self.window_id) {
            if let Ok(mut handle) = modal_sizes.left_panel_width.lock() {
                handle.set_size(left_panel_snapshot.width as f32);
            }
        }

        self.left_panel_view.update(ctx, |lp, ctx| {
            // Restore which panel tab was active
            let active_view = match left_panel_snapshot.left_panel_displayed_tab {
                LeftPanelDisplayedTab::FileTree => ToolPanelView::ProjectExplorer,
                LeftPanelDisplayedTab::GlobalSearch => ToolPanelView::GlobalSearch {
                    entry_focus: GlobalSearchEntryFocus::Results,
                },
            };
            lp.restore_active_view_from_snapshot(active_view, ctx);
            lp.set_active_pane_group(pane_group.clone(), &self.working_directories_model, ctx);
        });

        ctx.notify();
    }

    // Configure an empty workspace. The behavior here is platform-specific.
    fn configure_empty_workspace(
        &mut self,
        previous_active_window: Option<WindowId>,
        shell: Option<AvailableShell>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Window,
            previous_active_window,
            shell,
            false,
            ctx,
        );
    }

    /// Opens a cloud conversation by server token.
    /// If the current user owns or created it, navigate to its open pane or restore it
    /// into a new tab. Otherwise, open the read-only transcript viewer.
    /// Load the conversation into a transcript viewer in a new tab (with no input/backing shell)
    fn stop_sharing_all_panes_in_tab(
        &mut self,
        pane_group: &WeakViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        let _ = (pane_group, ctx);
    }

    fn copy_shared_session_link_from_tab(&mut self, tab_index: usize, ctx: &mut ViewContext<Self>) {
        // Get the pane group for the specified tab
        let Some(pane_group) = self.tabs.get(tab_index).map(|tab| tab.pane_group.clone()) else {
            return;
        };

        // Get the focused terminal view in that tab
        let Some(terminal_view) = pane_group.as_ref(ctx).focused_session_view(ctx) else {
            return;
        };

        let _ = (terminal_view, ctx);
    }

    fn subscribe_to_shared_session_manager(_ctx: &mut ViewContext<Self>) {
        /*
                ManagerEvent::StartedShare {
                    window_id,
                    session_id,
                } => {
                    if *window_id == ctx.window_id() {
                        me.copy_shared_session_link(session_id, ctx);
                    }
                }
                #[cfg(target_family = "wasm")]
                ManagerEvent::JoinedSession { view_id, .. } => {
                    // Check if this session is in the current window and has an ambient agent task
                    let manager = Manager::as_ref(ctx);
                    if let Some(terminal_view) = manager.joined_view_by_id(view_id, ctx) {
                        let task_id = terminal_view
                            .as_ref(ctx)
                            .model
                            .lock()
                            .ambient_agent_task_id();
                        if task_id.is_some() {
                            // Open the details panel for shared ambient agent sessions (unless on mobile)
                            if !warpui::platform::wasm::is_mobile_device() {
                                me.current_workspace_state.is_transcript_details_panel_open = true;
                                me.transcript_info_button.update(ctx, |button, ctx| {
                                    button.set_active(true, ctx);
                                });
                            }
                            me.update_transcript_details_panel_data(ctx);
                        }
                    }
                }
                #[cfg(not(target_family = "wasm"))]
                ManagerEvent::JoinedSession { .. } => {}
                _ => {}
            }
            ctx.notify();
        });
        */
    }

    /*
    fn copy_shared_session_link(
        &mut self,
        session_id: &SharedSessionId,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.clipboard().write(ClipboardContent::plain_text(
            terminal::shared_session::join_link(session_id),
        ));

        self.toast_stack.update(ctx, |toast_stack, ctx| {
            let toast = DismissibleToast::default("Remote control link copied.".to_string());
            toast_stack.add_ephemeral_toast(toast, ctx);
        });
    }
    */

    // Returns true if the focused pane is the viewer of a shared session
    /// Returns the type of simplified WASM tab bar content to display, if any.
    /// Used to determine whether to show the simplified tab bar layout on WASM.
    #[cfg(target_family = "wasm")]
    /// Add and focus a new terminal pane in AI mode in a new tab.
    /// Add and focus a new terminal pane in AI mode. Add the terminal pane to the right of
    /// all other panes, as a split on the root node.
    /// Add a new terminal tab and enter the agent view with a new conversation.
    /// Sets focused to the index of either the selected object or the first item in WD
    /// Check if Warp Drive view is focused within.
    /// Routes to the appropriate Warp Drive panel.
    fn current_focus_region(&self, ctx: &mut ViewContext<Self>) -> FocusRegion {
        let app = ctx;
        if self.active_tab_pane_group().is_self_or_child_focused(app) {
            return FocusRegion::PaneGroup;
        }

        if self.left_panel_view.is_self_or_child_focused(app) {
            return FocusRegion::LeftPanel;
        }
        FocusRegion::Other
    }

    fn has_left_region(&self, app: &AppContext) -> bool {
        self.active_tab_pane_group().as_ref(app).left_panel_open
    }

    fn has_right_region(&self, app: &AppContext) -> bool {
        let _ = app;
        false
    }

    fn focus_next_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.try_navigate_next(ctx))
    }

    fn focus_prev_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.try_navigate_prev(ctx))
    }

    fn focus_first_visible_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.focus_first_pane(ctx))
    }

    fn focus_last_visible_pane_in_group(&mut self, ctx: &mut ViewContext<Self>) -> bool {
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| pane_group.focus_last_pane(ctx))
    }

    fn focus_left_region_entry(&mut self, ctx: &mut ViewContext<Self>) {
        if self.has_left_region(ctx) {
            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.focus_active_view_on_entry(ctx);
            });
        }
    }

    fn focus_right_region_entry(&mut self, ctx: &mut ViewContext<Self>) {
        self.focus_active_tab(ctx);
    }

    fn navigate_pane_or_panel(
        &mut self,
        direction: PanePanelDirection,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_region = FocusRegion::PaneGroup;
        let has_left_panel = self.has_left_region(ctx);
        let has_right_panel = self.has_right_region(ctx);

        let target_region = self.compute_target_focus_region(
            current_region,
            direction,
            has_left_panel,
            has_right_panel,
            ctx,
        );

        self.set_pane_dimming_for_region(target_region, ctx);

        ctx.notify();
    }

    fn compute_target_focus_region(
        &mut self,
        region: FocusRegion,
        direction: PanePanelDirection,
        has_left_panel: bool,
        has_right_panel: bool,
        ctx: &mut ViewContext<Self>,
    ) -> FocusRegion {
        match (region, direction) {
            // NEXT: Left panel to first pane
            (FocusRegion::LeftPanel, PanePanelDirection::Next) => {
                // Always attempt to focus the first pane in the group and ensure the pane group
                // regains application focus.
                self.focus_first_visible_pane_in_group(ctx);
                self.focus_active_tab(ctx);
                FocusRegion::PaneGroup
            }
            // NEXT: Right panel to left panel if open, else first pane
            // NEXT: Pane group to next pane, or at end to right panel, left panel, first pane
            // Included Other here for cases like the command palette action "Activate next Pane"
            (FocusRegion::PaneGroup, PanePanelDirection::Next)
            | (FocusRegion::Other, PanePanelDirection::Next) => {
                let moved = self.focus_next_pane_in_group(ctx);
                if moved {
                    FocusRegion::PaneGroup
                } else if has_left_panel {
                    self.focus_left_region_entry(ctx);
                    FocusRegion::LeftPanel
                } else {
                    // No panels, wrap within panes.
                    self.focus_first_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }

            // PREV: Right panel to last pane
            // PREV: Left panel to right panel if open, else last pane
            (FocusRegion::LeftPanel, PanePanelDirection::Prev) => {
                self.focus_last_visible_pane_in_group(ctx);
                FocusRegion::PaneGroup
            }
            // PREV: Pane group to prev pane, or at beginning to left panel to right panel to last pane
            // Included Other here for cases like the command palette action "Activate next Pane"
            (FocusRegion::PaneGroup, PanePanelDirection::Prev)
            | (FocusRegion::Other, PanePanelDirection::Prev) => {
                let did_move = self.focus_prev_pane_in_group(ctx);
                if did_move {
                    FocusRegion::PaneGroup
                } else if has_left_panel {
                    self.focus_left_region_entry(ctx);
                    FocusRegion::LeftPanel
                } else {
                    // No panels, wrap within panes.
                    self.focus_last_visible_pane_in_group(ctx);
                    FocusRegion::PaneGroup
                }
            }
        }
    }

    fn update_pane_dimming_for_current_focus_region(&mut self, ctx: &mut ViewContext<Self>) {
        self.set_pane_dimming_for_region(FocusRegion::PaneGroup, ctx);
    }

    fn set_pane_dimming_for_region(&mut self, region: FocusRegion, ctx: &mut ViewContext<Self>) {
        let dim_even_if_focused = matches!(region, FocusRegion::LeftPanel);
        let handle = self.active_tab_pane_group().clone();
        handle.update(ctx, |pane_group, ctx| {
            pane_group.set_dim_even_if_focused_for_all_panes(dim_even_if_focused, ctx);
        });
    }

    /// This function shifts focus to the panel on the left.
    fn focus_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        if self.active_tab_pane_group().is_self_or_child_focused(ctx) {
            if self.is_theme_chooser_open() {
                ctx.focus(&self.theme_chooser_view);
            }
        } else if self.theme_chooser_view.is_self_or_child_focused(ctx) {
            self.focus_active_tab(ctx);
        }

        self.update_pane_dimming_for_current_focus_region(ctx);

        ctx.notify();
    }

    /// This function shifts focus to the panel on the right.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    pub fn is_overflow_menu_showing(&self) -> bool {
        self.show_tab_bar_overflow_menu
    }

    pub fn is_resource_center_showing(&self) -> bool {
        self.current_workspace_state.is_resource_center_open
    }

    #[cfg(feature = "integration_tests")]
    pub fn is_command_search_open(&self) -> bool {
        self.current_workspace_state.is_command_search_open
    }

    /// Retrieves the Pane Group view for the passed tab index.
    pub fn get_pane_group_view(&self, index: usize) -> Option<&ViewHandle<PaneGroup>> {
        self.tabs.get(index).map(|s| &s.pane_group)
    }

    /// Retrieves the Pane Group view for the passed tab index. Unlike the other
    /// method, this does not check for out of bounds.
    pub fn get_pane_group_view_unchecked(&self, index: usize) -> &ViewHandle<PaneGroup> {
        &self.tabs[index].pane_group
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn tab_views(&self) -> impl Iterator<Item = &ViewHandle<PaneGroup>> {
        self.tabs.iter().map(|s| &s.pane_group)
    }

    /// Get the tab color for a given tab index.
    pub fn get_tab_color(&self, index: usize) -> Option<AnsiColorIdentifier> {
        self.tabs.get(index).and_then(|tab| tab.color())
    }

    /// Get information needed for transferring a tab to another window.
    /// Returns None if the index is invalid or if this is the last tab.
    pub fn get_tab_transfer_info(&self, index: usize, ctx: &AppContext) -> Option<TransferredTab> {
        if self.tabs.len() <= 1 {
            return None;
        }
        let tab = self.tabs.get(index)?;
        let pane_group = tab.pane_group.clone();
        let color = tab.color();
        let custom_title = pane_group.read(ctx, |pg, ctx| pg.custom_title(ctx));
        let left_panel_open = pane_group.read(ctx, |pg, _| pg.left_panel_open);
        Some(TransferredTab {
            pane_group,
            color,
            custom_title,
            left_panel_open,
        })
    }

    /// Gets all sessions in the current workspace.
    pub fn workspace_sessions<'a>(
        &'a self,
        window_id: WindowId,
        app: &'a AppContext,
    ) -> impl Iterator<Item = SessionNavigationData> + 'a {
        self.tabs.iter().flat_map(move |tab| {
            // Each tab has a pane group
            let pane_group_id = tab.pane_group.id();
            let view = tab.pane_group.as_ref(app);

            view.pane_sessions(pane_group_id, window_id, app)
        })
    }

    /// Returns the PaneGroup view handle for the currently active tab.
    pub fn active_tab_pane_group(&self) -> &ViewHandle<PaneGroup> {
        self.get_pane_group_view(self.active_tab_index)
            .expect("Active tab index entry should exist")
    }

    /// Attempts to get selected text from the focused pane.
    /// Returns None if there is no selection, multiple selections, or an empty selection.
    /// Supports code, notebook, AI document, and terminal panes.
    fn get_selected_text_from_focused_view(&self, ctx: &AppContext) -> Option<String> {
        self.active_tab_pane_group()
            .as_ref(ctx)
            .selected_text_from_focused_pane(ctx)
    }

    /// This is meant to be dispatched directly by actions.
    pub fn activate_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        self.activate_tab_internal(index, ctx);
        ctx.notify();
    }

    /// This function is meant to be used by other actions to perform the logic to update the
    /// view's state. It's not meant to be invoked directly by an action.
    pub fn activate_tab_internal(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if index < self.tab_count() {
            // If the command palette is open when the tab is switched using a keybinding,
            // we want to close the palette so that we don't get into a state where the palette
            // is open but doesn't have focus.
            if self.is_palette_open() {
                self.close_palette(false, None, ctx);
            }

            self.set_active_tab_index(index, ctx);
            self.focus_active_tab(ctx);
            self.update_window_title(ctx);
        }
    }

    fn left_panel_visibility_across_tabs_enabled(&self, ctx: &AppContext) -> bool {
        *WindowSettings::as_ref(ctx)
            .left_panel_visibility_across_tabs
            .value()
    }

    /// Reconciles the active tab's tools panel open/closed state to match the window-scoped desired state
    /// (syncing left panel open/closed state across tabs).
    fn reconcile_left_panel_open_for_active_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let pane_group = self.active_tab_pane_group().clone();
        let pane_group_supports_tools_panel = pane_group.read(ctx, |pane_group, _| {
            Self::should_enable_file_tree_and_global_search_for_pane_group(pane_group)
        });

        if !pane_group_supports_tools_panel {
            return;
        }

        let desired_open = self.left_panel_open;
        pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(desired_open, ctx);
        });
    }

    /// Notifies the agent views model and notifications model that a terminal view gained focus.
    /// Change the active tab index. This must be used instead of setting `self.active_tab_index`
    /// directly, as it updates related state.
    fn set_active_tab_index(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let index = if index >= self.tab_count() {
            log::warn!(
                "Attempted to set active tab index {index} but only {} tabs exist, clamping",
                self.tab_count()
            );
            self.tab_count().saturating_sub(1)
        } else {
            index
        };

        self.active_tab_index = index;

        if self.left_panel_visibility_across_tabs_enabled(ctx) {
            self.reconcile_left_panel_open_for_active_tab(ctx);
        }

        let left_active_pane_group = self.active_tab_pane_group().clone();
        let right_active_pane_group = self.active_tab_pane_group().clone();
        let working_directories_model = self.working_directories_model.clone();

        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.set_active_pane_group(
                left_active_pane_group,
                &working_directories_model,
                ctx,
            );
        });

        let pane_group = self.active_tab_pane_group();
        let focused_terminal_view_id = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .terminal_view_from_pane_id(pane_group.as_ref(ctx).focused_pane_id(ctx), ctx)
            .map(|tv| tv.id());
        let _ = focused_terminal_view_id;

        self.update_active_session(ctx);
    }

    fn update_window_title(&self, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            log::warn!(
                "Tried to update window title but active tab index ({}) was out of range 0..{}",
                self.active_tab_index,
                self.tabs.len()
            );
            return;
        };
        let tab_title = tab.pane_group.as_ref(ctx).display_title(ctx);

        let window_title = truncate_from_end(&tab_title, MAX_WINDOW_TITLE_LENGTH);

        let window_id = ctx.window_id();
        ctx.windows().set_window_title(window_id, &window_title);
    }

    fn rename_tab_internal(&mut self, index: usize, title: &str, ctx: &mut ViewContext<Self>) {
        // Focusing on the clicked tab
        if index >= self.tab_count() {
            return;
        }

        self.set_active_tab_index(index, ctx);

        self.current_workspace_state.set_tab_being_renamed(index);

        // Clear the tab name editor to handle the case when another tab is already being renamed
        self.clear_tab_name_editor(ctx);
        let font_size = Self::tab_rename_editor_font_size(ctx, Appearance::as_ref(ctx));

        self.tab_rename_editor.update(ctx, move |editor, ctx| {
            editor.set_font_size(font_size, ctx);
            editor.insert_selected_text(title, ctx);
        });

        ctx.focus(&self.tab_rename_editor);
        ctx.notify();
    }

    pub fn rename_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let tab = &self.tabs[index];
        let title = tab.pane_group.as_ref(ctx).display_title(ctx);

        self.rename_tab_internal(index, &title, ctx);
    }

    fn set_active_tab_name(&mut self, title: &str, ctx: &mut ViewContext<Self>) {
        let Some(pane_group) = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.pane_group.clone())
        else {
            log::warn!(
                "Tried to set active tab name but active tab index ({}) was out of range 0..{}",
                self.active_tab_index,
                self.tabs.len()
            );
            return;
        };

        if self.current_workspace_state.is_tab_being_renamed() {
            self.current_workspace_state.clear_tab_being_renamed();
            self.clear_tab_name_editor(ctx);
        }

        let title = title.trim();
        if title.is_empty() {
            ctx.notify();
            return;
        }
        pane_group.update(ctx, |pane_group, ctx| {
            if pane_group.display_title(ctx) != title {
                pane_group.set_title(title, ctx);
            }
        });
        ctx.notify();
    }

    /// Programmatically sets the manual color override for a tab.
    ///
    /// - `Color(_)` applies that color.
    /// - `Cleared` explicitly clears the color (also suppresses any directory default).
    /// - `Unset` removes the manual override, letting the directory default apply.
    pub fn set_tab_color(
        &mut self,
        index: usize,
        color: SelectedTabColor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.get(index).is_none() {
            log::warn!(
                "Not setting tab color: index was {index} but len is {}",
                self.tabs.len()
            );
            return;
        }
        if self.tabs[index].selected_color == color {
            return;
        }
        self.tabs[index].selected_color = color;
        ctx.notify();
    }

    pub fn toggle_tab_color(
        &mut self,
        index: usize,
        color: AnsiColorIdentifier,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.get(index).is_none() {
            log::warn!(
                "Not toggling tab color: index was {index} but len is {}",
                self.tabs.len()
            );
            return;
        }
        let next = if self.tabs[index].color() == Some(color) {
            if FeatureFlag::DirectoryTabColors.is_enabled() {
                SelectedTabColor::Cleared
            } else {
                SelectedTabColor::Unset
            }
        } else {
            SelectedTabColor::Color(color)
        };
        self.set_tab_color(index, next, ctx);
    }

    /// Syncs the tab color for the given tab based on the active terminal's CWD.
    /// If the CWD is within a directory that has a configured color, applies it.
    /// If the CWD moves outside all configured directories, the directory color is cleared.
    fn sync_codebase_tab_color(tab: &mut TabData, ctx: &mut ViewContext<Self>) {
        let cwd = tab
            .pane_group
            .as_ref(ctx)
            .active_session_view(ctx)
            .and_then(|tv| tv.as_ref(ctx).pwd_if_local(ctx));

        let Some(cwd) = cwd else {
            return;
        };

        let cwd_path = Path::new(&cwd);
        let color = TabSettings::as_ref(ctx)
            .directory_tab_colors
            .value()
            .color_for_directory(cwd_path)
            .and_then(|c| c.ansi_color());

        tab.default_directory_color = color;
        ctx.notify();
    }

    fn clear_tab_name_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.tab_rename_editor.update(ctx, move |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    fn clear_pane_name_editor(&mut self, ctx: &mut ViewContext<Self>) {
        self.pane_rename_editor.update(ctx, move |editor, ctx| {
            editor.clear_buffer_and_reset_undo_stack(ctx);
        });
    }

    pub fn clear_tab_name(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let tab = &self.tabs[index];
        tab.pane_group.update(ctx, |view, ctx| {
            view.clear_title(ctx);
        });
        self.update_window_title(ctx);
        ctx.notify();
    }

    fn set_custom_pane_name(
        &mut self,
        locator: PaneViewLocator,
        title: String,
        ctx: &mut ViewContext<Self>,
    ) {
        let title = title.trim().to_owned();
        let Some(pane_group_view) = self.get_pane_group_view_with_id(locator.pane_group_id) else {
            log::warn!("Tried to rename pane in a missing pane group");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            let Some(pane) = pane_group.pane_by_id(locator.pane_id) else {
                log::warn!("Tried to rename a missing pane");
                return;
            };
            pane.pane_configuration().update(ctx, |configuration, ctx| {
                configuration.set_title(title.clone(), ctx);
                configuration.set_custom_title(title, ctx);
            });
            ctx.emit(pane_group::Event::AppStateChanged);
        });
    }

    pub fn clear_pane_name(&mut self, locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        let Some(pane_group_view) = self.get_pane_group_view_with_id(locator.pane_group_id) else {
            log::warn!("Tried to clear pane name in a missing pane group");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            let Some(pane) = pane_group.pane_by_id(locator.pane_id) else {
                log::warn!("Tried to clear a missing pane name");
                return;
            };
            pane.pane_configuration().update(ctx, |configuration, ctx| {
                configuration.set_title("", ctx);
                configuration.clear_custom_title(ctx);
            });
            ctx.emit(pane_group::Event::AppStateChanged);
        });
        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    pub fn rename_pane(&mut self, locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        let Some((index, tab)) = self
            .tabs
            .iter()
            .enumerate()
            .find(|(_, tab_data)| tab_data.pane_group.id() == locator.pane_group_id)
        else {
            log::warn!("Tried to rename pane in a missing tab");
            return;
        };

        let Some(title) = tab
            .pane_group
            .as_ref(ctx)
            .pane_by_id(locator.pane_id)
            .map(|pane| {
                let configuration = pane.pane_configuration();
                let configuration = configuration.as_ref(ctx);
                {
                    let title = configuration.title().trim();
                    if title.is_empty() {
                        "Untitled pane".to_string()
                    } else {
                        title.to_string()
                    }
                }
            })
        else {
            log::warn!("Tried to rename a missing pane");
            return;
        };

        tab.pane_group.update(ctx, |pane_group, ctx| {
            pane_group.focus_pane_by_id(locator.pane_id, ctx);
        });
        self.set_active_tab_index(index, ctx);
        self.current_workspace_state.set_pane_being_renamed(locator);
        self.clear_pane_name_editor(ctx);
        self.pane_rename_editor.update(ctx, move |editor, ctx| {
            editor.insert_selected_text(&title, ctx);
        });
        ctx.focus(&self.pane_rename_editor);
        ctx.notify();
    }

    pub fn list_tab_pane_groups(&self, app: &AppContext) -> Vec<TabPaneGroupIdentifiers> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(tab_idx, tab)| {
                let pane_group_id = tab.pane_group.id();
                let pane_group = tab.pane_group.as_ref(app);

                let pane_ids = pane_group.terminal_pane_ids();
                let terminal_ids = pane_ids
                    .into_iter()
                    .filter_map(|pane_id| {
                        let terminal_view = pane_group.terminal_view_from_pane_id(pane_id, app)?;
                        Some(terminal_view.id())
                    })
                    .collect::<Vec<_>>();

                TabPaneGroupIdentifiers {
                    tab_idx,
                    pane_group_id,
                    terminal_ids,
                }
            })
            .collect::<Vec<_>>()
    }

    /// Focuses the given pane within the pane group.
    pub fn focus_pane(&mut self, pane_view_locator: PaneViewLocator, ctx: &mut ViewContext<Self>) {
        if let Some((index, tab)) = self
            .tabs
            .iter()
            .enumerate()
            .find(|(_, tab_data)| tab_data.pane_group.id() == pane_view_locator.pane_group_id)
        {
            // Update the pane group to focus the active pane,
            // and then focus the pane group (tab). The order is important
            // because if we otherwise focus the tab first and another pane
            // was focused in the mean time, that pane will be the one that will
            // remain focused (as opposed to the pane with pane_id) since its
            // input would remain focused.
            tab.pane_group.update(ctx, |view, ctx| {
                view.focus_pane_by_id(pane_view_locator.pane_id, ctx);
            });
            self.activate_tab_internal(index, ctx);
            ctx.notify();
        }
    }

    /// Searches this workspace's tabs for the given terminal view and focuses it.
    /// Returns true if the terminal view was found and focused.
    fn focus_terminal_view_locally(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        for tab in self.tabs.iter() {
            let pane_group_handle = &tab.pane_group;
            let pane_group = pane_group_handle.as_ref(ctx);
            if let Some(pane_id) = pane_group.find_pane_id_for_terminal_view(terminal_view_id, ctx)
            {
                self.focus_pane(
                    PaneViewLocator {
                        pane_group_id: pane_group_handle.id(),
                        pane_id,
                    },
                    ctx,
                );
                return true;
            }
        }
        false
    }

    /// Searches other windows for the given terminal view and focuses it there.
    /// (Uses the same cross-window dispatch pattern as open_notebook/open_workflow.)
    fn focus_terminal_view_in_other_window(
        &self,
        terminal_view_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_window = ctx.window_id();
        let result = WorkspaceRegistry::as_ref(ctx)
            .all_workspaces(ctx)
            .iter()
            .filter(|(win_id, _)| *win_id != current_window)
            .find_map(|(win_id, workspace)| {
                workspace.as_ref(ctx).tab_views().find_map(|pane_group| {
                    let pane_id = pane_group
                        .as_ref(ctx)
                        .find_pane_id_for_terminal_view(terminal_view_id, ctx)?;
                    Some((
                        *win_id,
                        PaneViewLocator {
                            pane_group_id: pane_group.id(),
                            pane_id,
                        },
                    ))
                })
            });

        if let Some((window_id, locator)) = result {
            ctx.windows().show_window_and_focus_app(window_id);
            if let Some(root_view_id) = ctx.root_view_id(window_id) {
                ctx.dispatch_action_for_view(
                    window_id,
                    root_view_id,
                    "root_view:handle_pane_navigation_event",
                    &locator,
                );
            }
        }
    }

    /// Shows the notification error in the specific pane.
    pub fn show_notification_error(
        &mut self,
        notification_error: NotificationSendError,
        pane_group_id: EntityId,
        pane_id: PaneId,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab_data| tab_data.pane_group.id() == pane_group_id)
        {
            tab.pane_group.update(ctx, |view, ctx| {
                view.show_notification_error(notification_error, pane_id, ctx);
            });

            ctx.notify();
        }
    }

    fn sync_panel_positions_from_config(&mut self, ctx: &mut ViewContext<Self>) {
        let _ = ctx;
    }

    fn build_header_toolbar_context_menu(
        ctx: &mut ViewContext<Self>,
    ) -> ViewHandle<Menu<WorkspaceAction>> {
        let menu = ctx.add_typed_action_view(|_| Menu::new().with_drop_shadow());
        ctx.subscribe_to_view(&menu, |me, _, event, ctx| {
            if let MenuEvent::Close { .. } = event {
                me.show_header_toolbar_context_menu = None;
                ctx.notify();
            }
        });
        menu
    }

    fn show_header_toolbar_context_menu(
        &mut self,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        if !FeatureFlag::ConfigurableToolbar.is_enabled() {
            return;
        }
        let items = Vec::new();
        self.header_toolbar_context_menu
            .update(ctx, |menu, ctx| menu.set_items(items, ctx));
        self.show_header_toolbar_context_menu = Some(position);
        ctx.focus(&self.header_toolbar_context_menu);
        ctx.notify();
    }

    #[cfg(feature = "local_fs")]
    fn get_active_session(&self, ctx: &mut ViewContext<Self>) -> Option<Arc<Session>> {
        let pane_group = self.active_tab_pane_group();
        pane_group
            .as_ref(ctx)
            .active_session_id(ctx)
            .and_then(|session_id| {
                pane_group
                    .as_ref(ctx)
                    .terminal_view_from_pane_id(session_id, ctx)
            })
            .and_then(|tv| {
                let tv_ref = tv.as_ref(ctx);
                let session_id = tv_ref.active_block_session_id()?;
                tv_ref.sessions_model().as_ref(ctx).get(session_id)
            })
    }

    #[cfg(not(feature = "local_fs"))]
    pub fn open_file_with_target(
        &mut self,
        _path: PathBuf,
        _target: FileTarget,
        _line_col: Option<LineAndColumnArg>,
        _code_source: CodeSource,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    #[cfg(feature = "local_fs")]
    pub fn open_file_with_target(
        &mut self,
        path: PathBuf,
        target: FileTarget,
        line_col: Option<LineAndColumnArg>,
        code_source: CodeSource,
        ctx: &mut ViewContext<Self>,
    ) {
        // Handle directories for CodeEditor(NewTab) target by opening a new terminal tab
        if path.is_dir() && matches!(target, FileTarget::CodeEditor(EditorLayout::NewTab)) {
            self.add_tab_with_pane_layout(
                PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                    initial_directory: Some(path.clone()),
                    hide_homepage: true,
                    ..Default::default()
                })),
                Arc::new(HashMap::new()),
                None,
                ctx,
            );
            return;
        }

        match target {
            FileTarget::CodeEditor(_) => {}
            FileTarget::MarkdownViewer(layout) => {
                let _ = (path, layout);
            }
            FileTarget::EnvEditor => {
                let editor_value: Option<String> = self
                    .get_active_session(ctx)
                    .and_then(|session| session.editor().map(|s| s.to_string()));

                if let Some(ref editor_env) = editor_value {
                    if let Ok(editor) = Editor::try_from(editor_env.as_str()) {
                        crate::util::file::open_file_path_with_editor(
                            line_col,
                            path.clone(),
                            Some(editor),
                            ctx,
                        );
                        return;
                    }

                    // If we have an editor string but it's not a known Editor, we try to run it in a new pane
                    let new_pane_id =
                        self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
                            pane_group.add_terminal_pane(
                                Direction::Right,
                                None, /*chosen_shell*/
                                ctx,
                            )
                        });

                    if let Some(terminal_view_handle) = self
                        .active_tab_pane_group()
                        .as_ref(ctx)
                        .terminal_view_from_pane_id(new_pane_id, ctx)
                    {
                        let editor_ref = Some(editor_env.as_str());
                        let path_clone = path.clone();
                        terminal_view_handle.update(ctx, |terminal, ctx| {
                            let editor_command =
                                crate::util::file::external_editor::generate_editor_command(
                                    &path_clone,
                                    line_col,
                                    editor_ref,
                                );
                            terminal.set_pending_command(&editor_command, ctx);
                        });
                        return;
                    } else {
                        log::error!(
                            "Could not get terminal view handle for new pane when attempting to open file with $EDITOR."
                        );
                    }
                }

                crate::util::file::open_file_path_in_external_editor(line_col, path.clone(), ctx);
            }
            FileTarget::ExternalEditor(editor) => {
                crate::util::file::open_file_path_with_editor(
                    line_col,
                    path.clone(),
                    Some(editor),
                    ctx,
                );
            }
            FileTarget::SystemDefault => {
                crate::util::file::open_file_path_with_editor(line_col, path.clone(), None, ctx);
            }
            FileTarget::SystemGeneric => {
                ctx.open_file_path(&path);
            }
        }
    }

    fn handle_left_panel_event(&mut self, event: &LeftPanelEvent, ctx: &mut ViewContext<Self>) {
        match event {
            LeftPanelEvent::FileTree(pane_group_event) => {
                let pane_group = self.active_tab_pane_group().clone();
                self.handle_file_tree_event(pane_group, pane_group_event, ctx);
            }
            LeftPanelEvent::OpenFileWithTarget {
                path,
                target,
                line_col,
            } => {
                self.open_file_with_target(
                    path.clone(),
                    target.clone(),
                    *line_col,
                    CodeSource::FileTree { path: path.clone() },
                    ctx,
                );
            }
        }
    }

    /// Show the referral reward modal page, informing the user they have earned a theme reward
    fn join_slack(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::SLACK_URL);
    }

    fn view_user_docs(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::USER_DOCS_URL);
    }

    fn view_latest_changelog(&mut self, ctx: &mut ViewContext<Self>) {
        self.update_toast_stack.update(ctx, |stack, ctx| {
            stack.clear_toasts(ctx);
        });
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::Changelog),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });
    }

    fn view_privacy_policy(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.open_url(links::PRIVACY_POLICY_URL);
    }

    #[cfg(not(target_family = "wasm"))]
    fn view_logs(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async { tokio::task::spawn_blocking(warp_logging::create_log_bundle_zip).await },
            |me, result, ctx| match result {
                Ok(Ok(path)) => {
                    ctx.open_file_path_in_explorer(&path);
                }
                Ok(Err(err)) => {
                    let error_message = format!("Failed to create log bundle: {err}");
                    log::error!("{error_message}");
                    me.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
                Err(err) => {
                    let error_message = format!("Failed to create log bundle: {err}");
                    log::error!("{error_message}");
                    me.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            },
        );
    }

    fn copy_version(&mut self, version: &str, ctx: &mut ViewContext<Self>) {
        ctx.clipboard()
            .write(ClipboardContent::plain_text(version.to_string()));
    }

    /// Builds the unified new-session menu items
    /// tab bar chevron and the vertical tab bar `+` button.
    ///
    /// Order: Agent → Terminal (sidecar) → Cloud Oz → [tab configs] → separator → New worktree config (sidecar) → New tab config → separator → Reopen closed session.
    fn unified_new_session_menu_items(
        &self,
        ctx: &mut ViewContext<Self>,
    ) -> Vec<MenuItem<WorkspaceAction>> {
        vec![
            MenuItemFields::new("Terminal")
                .with_on_select_action(WorkspaceAction::AddTerminalTab {
                    hide_homepage: false,
                })
                .with_icon(icons::Icon::LayoutAlt01)
                .with_key_shortcut_label(keybinding_name_to_display_string(
                    NEW_TAB_BINDING_NAME,
                    ctx,
                ))
                .into_item(),
            MenuItem::Separator,
            MenuItemFields::new("Reopen closed session")
                .with_on_select_action(WorkspaceAction::ReopenClosedSession)
                .with_key_shortcut_label(keybinding_name_to_display_string(
                    "app:reopen_closed_session",
                    ctx,
                ))
                .with_disabled(UndoCloseStack::handle(ctx).as_ref(ctx).is_empty())
                .into_item(),
        ]
    }

    fn open_tab_configs_menu(
        &mut self,
        position: Vector2F,
        open_source: TabConfigsMenuOpenSource,
        ctx: &mut ViewContext<Self>,
    ) {
        let menu_items = self.unified_new_session_menu_items(ctx);
        ctx.update_view(&self.new_session_dropdown_menu, |context_menu, view_ctx| {
            context_menu.set_width(MENU_DEFAULT_WIDTH);
            context_menu.set_items(menu_items, view_ctx);
            match open_source {
                TabConfigsMenuOpenSource::KeyboardShortcut => {
                    context_menu.set_selected_by_index(0, view_ctx);
                }
                TabConfigsMenuOpenSource::Pointer => {
                    context_menu.reset_selection(view_ctx);
                }
            }
        });
        self.show_new_session_dropdown_menu = Some(position);
        ctx.focus(&self.new_session_dropdown_menu);
        ctx.notify();
    }

    pub fn open_new_session_dropdown_menu(
        &mut self,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        self.open_tab_configs_menu(position, TabConfigsMenuOpenSource::Pointer, ctx);
    }

    fn toggle_tab_configs_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if self.show_new_session_dropdown_menu.is_some() {
            self.close_new_session_dropdown_menu(ctx);
            return;
        }

        let position = ctx
            .element_position_by_id_at_last_frame(self.window_id, NEW_TAB_BUTTON_POSITION_ID)
            .map(|position| position.lower_left())
            .unwrap_or_else(Vector2F::zero);
        self.open_tab_configs_menu(position, TabConfigsMenuOpenSource::KeyboardShortcut, ctx);
    }

    pub fn toggle_new_session_dropdown_menu(
        &mut self,
        position: Vector2F,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_new_session_dropdown_menu.is_some() {
            self.close_new_session_dropdown_menu(ctx);
            return;
        }

        self.open_tab_configs_menu(position, TabConfigsMenuOpenSource::Pointer, ctx);
    }

    /// Opens a tab config after the user has filled in (or confirmed) param values.
    fn open_tab_config_with_params(
        &mut self,
        tab_config: crate::tab_configs::TabConfig,
        param_values: HashMap<String, String>,
        worktree_branch_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        let tab_color = tab_config.color;
        let (rendered_title, pane_template) =
            crate::tab_configs::render_tab_config(&tab_config, &param_values, worktree_branch_name);
        self.add_tab_with_pane_layout(
            PanesLayout::Template(pane_template),
            Arc::new(HashMap::new()),
            rendered_title,
            ctx,
        );
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            // Apply tab color if specified, matching the launch config pattern.
            if let Some(color) = tab_color {
                tab.selected_color = SelectedTabColor::Color(color);
            }
        }
    }

    /// Opens a tab config, showing the param-fill modal when the config has parameters,
    /// or opening the tab directly when there are no parameters.
    fn open_tab_config(
        &mut self,
        tab_config: crate::tab_configs::TabConfig,
        ctx: &mut ViewContext<Self>,
    ) {
        if tab_config.params.is_empty() {
            let is_worktree_config = tab_config.is_worktree();
            let worktree_branch_name = self.maybe_generate_worktree_name(&tab_config);
            let param_values = tab_config.default_param_values();
            self.open_tab_config_with_params(
                tab_config,
                param_values,
                worktree_branch_name.as_deref(),
                ctx,
            );
        } else {
            let modal_title = format!("Open: {}", tab_config.name);
            self.tab_config_params_modal.view.update(ctx, |modal, ctx| {
                modal.body().update(ctx, |body, ctx| {
                    body.set_title(modal_title);
                    body.on_open(tab_config, ctx);
                });
            });
            self.tab_config_params_modal.open();
            self.current_workspace_state.is_tab_config_params_modal_open = true;
            ctx.notify();
        }
    }

    /// Writes the default tab config template to an unused path in `~/.warp/tab_configs/`
    /// and opens it respecting the user's configured editor setting.
    #[cfg(feature = "local_fs")]
    fn create_and_open_new_tab_config(&mut self, ctx: &mut ViewContext<Self>) {
        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }
        let path = find_unused_tab_config_path(&dir);
        const TEMPLATE: &str =
            include_str!("../../resources/tab_configs/new_tab_config_template.toml");
        if let Err(e) = std::fs::write(&path, TEMPLATE) {
            log::warn!("Failed to write new tab config template: {e:?}");
            return;
        }
        let settings = EditorSettings::as_ref(ctx);
        let target = resolve_file_target_with_editor_choice(
            &path,
            *settings.open_code_panels_file_editor,
            *settings.prefer_markdown_viewer,
            *settings.open_file_layout,
            None,
        );
        self.open_file_with_target(
            path.clone(),
            target,
            None,
            CodeSource::Link {
                path,
                range_start: None,
                range_end: None,
            },
            ctx,
        );
    }

    /// Snapshots the given tab's pane layout and writes it as a new tab config
    /// TOML to `~/.warp/tab_configs/`, then opens the file in the user's editor.
    #[cfg(feature = "local_fs")]
    fn save_current_tab_as_new_config(&mut self, tab_index: usize, ctx: &mut ViewContext<Self>) {
        use crate::tab_configs::session_config::{tab_config_from_pane_snapshot, write_tab_config};

        let tab = &self.tabs[tab_index];
        let snapshot = tab.pane_group.as_ref(ctx).snapshot(ctx);
        let custom_title = tab.pane_group.as_ref(ctx).custom_title(ctx);
        let color = tab.color();
        let config = tab_config_from_pane_snapshot(&snapshot, custom_title, color);

        let dir = tab_configs_dir();
        match write_tab_config(&config, &dir, "my_tab_config") {
            Ok(path) => {
                let settings = EditorSettings::as_ref(ctx);
                let target = resolve_file_target_with_editor_choice(
                    &path,
                    *settings.open_code_panels_file_editor,
                    *settings.prefer_markdown_viewer,
                    *settings.open_file_layout,
                    None,
                );
                self.open_file_with_target(
                    path.clone(),
                    target,
                    None,
                    CodeSource::Link {
                        path,
                        range_start: None,
                        range_end: None,
                    },
                    ctx,
                );
            }
            Err(e) => log::warn!("Failed to save tab config: {e:?}"),
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn save_current_tab_as_new_config(&mut self, _tab_index: usize, _ctx: &mut ViewContext<Self>) {}

    pub fn toggle_tab_right_click_menu(
        &mut self,
        tab_index: usize,
        anchor: TabContextMenuAnchor,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.show_tab_right_click_menu.is_some() {
            self.show_tab_right_click_menu = None;
            ctx.notify();
            return;
        }

        let tab = &self.tabs[tab_index];
        let menu_items = tab.menu_items(tab_index, self.tabs.len(), ctx);
        ctx.update_view(&self.tab_right_click_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_right_click_menu = Some((tab_index, anchor));
        ctx.focus(&self.tab_right_click_menu);
        ctx.notify();
    }

    /// The tab bar overflow menu is the context menu that appears when
    /// a user clicks "Update Warp" in the top right of the tab bar.
    pub fn toggle_tab_bar_overflow_menu(&mut self, ctx: &mut ViewContext<Self>) {
        if self.show_tab_bar_overflow_menu {
            self.close_tab_bar_overflow_menu(ctx);
            return;
        }

        let mut menu_items = vec![];
        ctx.update_view(&self.tab_bar_overflow_menu, |context_menu, view_ctx| {
            context_menu.set_items(menu_items, view_ctx);
        });
        self.show_tab_bar_overflow_menu = true;
        ctx.focus(&self.tab_bar_overflow_menu);
        ctx.notify();
    }

    fn read_from_active_terminal_view<T>(
        &self,
        ctx: &AppContext,
        accessor: impl FnOnce(&TerminalView) -> T,
    ) -> Option<T> {
        self.get_pane_group_view(self.active_tab_index)
            .and_then(|view| {
                view.read(ctx, |pane_group, ctx| {
                    pane_group
                        .active_session_view(ctx)
                        .map(|terminal_view_handle| {
                            terminal_view_handle.read(ctx, |terminal, _| accessor(terminal))
                        })
                })
            })
    }

    pub fn active_terminal_id(&self, app: &AppContext) -> Option<EntityId> {
        self.read_from_active_terminal_view(app, |terminal| terminal.id())
    }

    /// Retrieves the entity id of the active current active input. This is needed
    /// by the Welcome Tip View in order to know where to dispatch the actions
    /// directly from the tip menu.
    fn active_input_id(&self, app: &AppContext) -> Option<EntityId> {
        self.read_from_active_terminal_view(app, |terminal| terminal.input().id())
    }

    /// Gets the ID of the active terminal session, if any.
    pub fn active_session_id(&self, ctx: &ViewContext<Self>) -> Option<SessionId> {
        self.get_pane_group_view(self.active_tab_index)
            .and_then(|view| {
                view.read(ctx, |pane_group, ctx| {
                    pane_group
                        .active_session_view(ctx)
                        .and_then(|terminal_view_handle| {
                            terminal_view_handle
                                .read(ctx, |terminal, _| terminal.active_block_session_id())
                        })
                })
            })
    }

    fn open_settings_pane(
        &mut self,
        page: Option<SettingsSection>,
        search_query: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Ensure there is only one settings pane per window
        let settings_pane_manager = SettingsPaneManager::handle(ctx);
        if let Some(locator) = settings_pane_manager.as_ref(ctx).find_pane(ctx.window_id()) {
            // Update to new page if specified
            if let Some(page) = page {
                self.settings_pane.update(ctx, |settings_pane, ctx| {
                    settings_pane.set_and_refresh_current_page(page, ctx);
                    if let Some(search_query) = search_query {
                        settings_pane.set_search_query(search_query, ctx);
                    }
                });
            }
            // Navigate to and focus existing pane
            self.focus_pane(locator, ctx);
            return;
        }

        let ps1_grid_info = self.active_session_ps1_grid_info(ctx);
        // Open new tab and update current page
        self.settings_pane.update(ctx, move |settings_pane, ctx| {
            // TODO: This check shouldn't be necessary, but `active_session_ps1_grid_info` returns
            // None when the active tab has no running terminal sessions, e.g. if it contains only
            // notebooks/workflow panes.
            if ps1_grid_info.is_some() {
                settings_pane.set_ps1_info(ps1_grid_info, ctx);
            }
        });

        let panes_layout = PanesLayout::Snapshot(Box::new(PaneNodeSnapshot::Leaf(LeafSnapshot {
            is_focused: true,
            contents: LeafContents::Settings(SettingsPaneSnapshot::Local {
                current_page: page.unwrap_or_default(),
                search_query: search_query.map(|s| s.to_owned()),
            }),
        })));
        self.add_tab_with_pane_layout(
            panes_layout,
            Arc::new(HashMap::new()),
            Some("Settings".to_owned()),
            ctx,
        );
    }

    fn cd_to_directory(&mut self, path: PathBuf, ctx: &mut ViewContext<Self>) {
        let Some(input_handle) = self.get_active_input_view_handle(ctx) else {
            log::warn!("No active input view when trying to cd to directory");
            return;
        };

        let Some(path_str) = path.to_str() else {
            log::warn!("Could not convert path to string for cd command");
            return;
        };

        let cd_command = format!("cd {}", shell_words::quote(path_str));
        input_handle.update(ctx, |input_view, ctx| {
            input_view.replace_buffer_content(&cd_command, ctx);
        });
    }

    fn open_directory_in_new_tab(&mut self, path: PathBuf, ctx: &mut ViewContext<Self>) {
        let options = NewTerminalOptions::default().with_initial_directory(path);
        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(options)),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
    }

    pub(super) fn active_session_view(
        &self,
        ctx: &mut ViewContext<Self>,
    ) -> Option<ViewHandle<TerminalView>> {
        self.active_tab_pane_group()
            .read(ctx, |pane_group, ctx| pane_group.active_session_view(ctx))
    }

    pub fn close_tab_bar_overflow_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_tab_bar_overflow_menu = false;
        ctx.notify();
    }

    /// Find an active session and pre-fill the input editor the Warp executable with the
    /// [`warp_cli::Command::DumpDebugInfo`] subcommand.
    fn dump_debug_info(&mut self, ctx: &mut ViewContext<Self>) {
        if let Some(exec) = std::env::current_exe()
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
        {
            let command = format!("{exec} {}", warp_cli::dump_debug_info_flag());
            // Get the active session for this tab if it exists.
            let mut active_session_handle = self
                .active_tab_pane_group()
                .read(ctx, |pane_group_view, ctx| {
                    pane_group_view.active_session_view(ctx)
                });
            // A tab may not have any active session, say if it only contains notebook(s). If
            // that's the case, create a new tab.
            if active_session_handle.is_none() {
                self.add_new_session_tab_with_default_mode(
                    NewSessionSource::Tab,
                    None,
                    None,
                    false,
                    ctx,
                );
            }
            active_session_handle = self
                .active_tab_pane_group()
                .read(ctx, |pane_group_view, ctx| {
                    pane_group_view.active_session_view(ctx)
                });
            if let Some(terminal_view_handle) = active_session_handle {
                terminal_view_handle.update(ctx, |terminal_view, ctx| {
                    terminal_view.set_pending_command(&command, ctx);
                });
            }
        }
    }

    /// Install the Warp CLI by creating a symlink in /usr/local/bin
    #[cfg(target_os = "macos")]
    fn install_cli(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(async { cli_install::install_cli() }, |view, result, ctx| {
            match result {
                Ok(_) => {
                    let command_name = ChannelState::channel().cli_command_name();
                    let message = format!("Successfully installed the Oz CLI! You can now run '{command_name}' from the command line.");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::success(message.to_string())
                            .with_link(
                                ToastLink::new("Learn more".to_string()).with_href(
                                    "https://docs.warp.dev/reference/cli".to_string(),
                                ),
                            );
                        toast_stack.add_ephemeral_toast(toast, ctx);
                    });
                }
                Err(error) => {
                    let error_message = format!("Failed to install Oz command: {error}");
                    log::error!("{error_message}");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            }
        });
    }

    /// Uninstall the Warp CLI by removing the symlink from /usr/local/bin
    #[cfg(target_os = "macos")]
    fn uninstall_cli(&mut self, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async { cli_install::uninstall_cli() },
            |view, result, ctx| match result {
                Ok(_) => {
                    let message = "Successfully uninstalled the Oz command.";
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::success(message.to_string());
                        toast_stack.add_ephemeral_toast(toast, ctx);
                    });
                }
                Err(error) => {
                    let error_message = format!("Failed to uninstall Oz command: {error}");
                    log::error!("{error_message}");
                    view.toast_stack.update(ctx, |toast_stack, ctx| {
                        let toast = DismissibleToast::error(error_message);
                        toast_stack.add_persistent_toast(toast, ctx);
                    });
                }
            },
        );
    }

    fn toggle_recording_mode(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, settings_ctx| {
            report_if_error!(debug_settings
                .recording_mode
                .toggle_and_save_value(settings_ctx));
        });
    }

    fn toggle_in_band_generators(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, settings_ctx| {
            report_if_error!(debug_settings
                .are_in_band_generators_for_all_sessions_enabled
                .toggle_and_save_value(settings_ctx));
        });
    }

    fn toggle_debug_network_status(&self, ctx: &mut ViewContext<Self>) {
        NetworkStatus::handle(ctx).update(ctx, |network_status, network_ctx| {
            let is_reachable = network_status.is_online();
            let new_is_reachable = !is_reachable;
            if new_is_reachable {
                log::info!("Manually toggled network status to be reachable");
            } else {
                log::info!("Manually toggled network status to be not reachable");
            }
            network_status.reachability_changed(new_is_reachable, network_ctx);
        });
    }

    fn toggle_show_memory_stats(&self, ctx: &mut ViewContext<Self>) {
        DebugSettings::handle(ctx).update(ctx, |debug_settings, ctx| {
            report_if_error!(debug_settings.show_memory_stats.toggle_and_save_value(ctx));
        })
    }

    pub fn toggle_resource_center(&mut self, ctx: &mut ViewContext<Self>) {
        self.open_settings_pane(None, None, ctx);
    }

    fn open_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.left_panel_open = true;

        let active_pane_group = self.active_tab_pane_group().clone();
        active_pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(true, ctx);
        });

        ctx.notify();
    }

    fn close_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        self.left_panel_open = false;

        let active_pane_group = self.active_tab_pane_group().clone();
        active_pane_group.update(ctx, |pane_group, ctx| {
            pane_group.set_left_panel_open(false, ctx);
        });

        ctx.notify();
    }

    /// Sets the visibility state of the agent management view
    /// and updates the AgentConversationsModel to reflect the new state.
    fn toggle_left_panel(&mut self, ctx: &mut ViewContext<Self>) {
        let active_pane_group = self.active_tab_pane_group().clone();

        let was_open = active_pane_group.read(ctx, |pane_group, _| pane_group.left_panel_open);
        let new_state = !was_open;

        if new_state {
            self.open_left_panel(ctx);
        } else {
            self.close_left_panel(ctx);
        }

        // If we are opening the panel, set width based on the most recent tab's width if available,
        // otherwise compute default width from current window size. Also auto-expand the project
        // explorer if it's the active left panel view.
        if new_state {
            let window_id = ctx.window_id();
            let resizable_data = ResizableData::handle(ctx);
            if let Some(handle) = resizable_data
                .as_ref(ctx)
                .get_handle(window_id, ModalType::LeftPanelWidth)
            {
                if let Ok(mut state) = handle.lock() {
                    // Get the current width from ResizableData - this reflects the most recent tab's width
                    let current_width = state.size();

                    // Only recompute default if the current width is at the default value
                    // This preserves the width from the most recent tab
                    if current_width == DEFAULT_LEFT_PANEL_WIDTH {
                        let has_horizontal_split = active_pane_group
                            .read(ctx, |pane_group, _| pane_group.has_horizontal_split());
                        let (left_width, _right_width) =
                            compute_default_panel_widths(ctx, window_id, has_horizontal_split);
                        state.set_size(left_width);
                    }
                    // If current_width is not the default, it means we have a width from a previous tab,
                    // so we don't need to do anything - the width is already preserved
                }
            }
        }

        if !new_state {
            self.focus_active_tab(ctx);
        }

        ctx.notify();
    }

    fn user_menu_items(&self, app: &AppContext) -> Vec<MenuItem<WorkspaceAction>> {
        let mut items = Vec::new();
        let appearance = Appearance::as_ref(app);

        items.extend([
            MenuItemFields::new("What's new")
                .with_on_select_action(WorkspaceAction::ViewLatestChangelog)
                .into_item(),
            MenuItemFields::new("Settings")
                .with_on_select_action(WorkspaceAction::ShowSettings)
                .into_item(),
            MenuItemFields::new("Keyboard shortcuts")
                .with_on_select_action(WorkspaceAction::ToggleKeybindingsPage)
                .into_item(),
            MenuItem::Separator,
            MenuItemFields::new("Documentation")
                .with_on_select_action(WorkspaceAction::ViewUserDocs)
                .into_item(),
            MenuItemFields::new("Feedback")
                .with_on_select_action(WorkspaceAction::SendFeedback)
                .into_item(),
        ]);

        #[cfg(not(target_family = "wasm"))]
        items.push(
            MenuItemFields::new("View Warp logs")
                .with_on_select_action(WorkspaceAction::ViewLogs)
                .into_item(),
        );

        items.extend([
            MenuItemFields::new("Slack")
                .with_on_select_action(WorkspaceAction::JoinSlack)
                .into_item(),
            MenuItem::Separator,
        ]);

        items.push(
            MenuItemFields::new("Invite a friend")
                .with_on_select_action(WorkspaceAction::ShowReferralSettingsPage)
                .into_item(),
        );

        items
    }

    fn selected_new_session_sidecar_selection(
        &self,
        ctx: &AppContext,
    ) -> Option<NewSessionSidecarSelection> {
        self.new_session_sidecar_menu.read(ctx, |menu, _| {
            menu.selected_item().and_then(|item| match item {
                MenuItem::Item(fields) => fields.on_select_action().cloned(),
                _ => None,
            })
        })
    }

    fn execute_new_session_sidecar_selection(
        &mut self,
        selection: NewSessionSidecarSelection,
        ctx: &mut ViewContext<Self>,
    ) {
        match selection {
            NewSessionSidecarSelection::OpenWorktreeRepo { repo_path } => {
                self.open_worktree_in_repo(repo_path, ctx);
            }
        }
    }

    fn toggle_user_menu(&mut self, ctx: &mut ViewContext<Self>) {
        self.is_user_menu_open = !self.is_user_menu_open;
        if self.is_user_menu_open {
            let items = self.user_menu_items(ctx);
            self.user_menu.update(ctx, |menu, ctx| {
                menu.set_items(items, ctx);
            });
        }
        ctx.focus(&self.user_menu);
        ctx.notify();
    }

    pub fn toggle_keybindings_page(&mut self, ctx: &mut ViewContext<Self>) {
        self.open_settings_pane(None, None, ctx);
    }

    fn handle_tab_right_click_menu_event(
        &mut self,
        event: &MenuEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let MenuEvent::Close { via_select_item: _ } = event {
            self.show_tab_right_click_menu = None;
            ctx.notify();
        }
    }

    fn handle_new_session_menu_event(&mut self, event: &MenuEvent, ctx: &mut ViewContext<Self>) {
        match event {
            MenuEvent::Close { .. } => {
                self.close_new_session_dropdown_menu(ctx);
            }
            MenuEvent::ItemHovered | MenuEvent::ItemSelected => {}
        }
    }

    fn handle_new_session_sidecar_event(&mut self, event: &MenuEvent, ctx: &mut ViewContext<Self>) {
        match event {
            MenuEvent::Close { via_select_item } => {
                let selection = if *via_select_item {
                    self.selected_new_session_sidecar_selection(ctx)
                } else {
                    None
                };
                log::info!(
                    "New-session sidecar closed: worktree_active={}, via_select_item={via_select_item}",
                    self.worktree_sidecar_active
                );
                if let Some(selection) = selection {
                    self.execute_new_session_sidecar_selection(selection, ctx);
                }
                if *via_select_item {
                    // Item clicked in sidecar — also close the main menu.
                    self.show_new_session_dropdown_menu = None;
                }
                self.clear_worktree_sidecar_state(ctx);
                self.new_session_dropdown_menu.update(ctx, |menu, _| {
                    menu.set_safe_zone_target(None);
                    menu.set_submenu_being_shown_for_item_index(None);
                });
                ctx.notify();
            }
            MenuEvent::ItemSelected => {}
            MenuEvent::ItemHovered => {
                self.sync_new_session_sidecar_selection_to_hover(ctx);
            }
        }
    }

    fn should_include_worktree_sidecar_repo(repo_path: &Path, ctx: &AppContext) -> bool {
        // This performs one repo-metadata lookup per persisted workspace while the
        // sidecar items are rebuilt. That's acceptable for now given the expected
        // repo counts here, and it keeps linked-worktree filtering scoped to the
        // only UI that currently needs it.
        let Some(repository) =
            DetectedRepositories::as_ref(ctx).get_watched_repo_for_path(repo_path, ctx)
        else {
            return true;
        };
        // Linked worktrees (and submodules) have an external gitdir; exclude
        // them so only primary repository checkouts appear in the list.

        repository.as_ref(ctx).external_git_directory().is_none()
    }

    fn handle_tab_bar_overflow_menu_event(
        &mut self,
        event: &MenuEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if let MenuEvent::Close { via_select_item: _ } = event {
            self.close_tab_bar_overflow_menu(ctx)
        }
    }

    fn handle_launch_config_save_modal_event(
        &mut self,
        event: &LaunchConfigModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            LaunchConfigModalEvent::Close => {
                self.current_workspace_state
                    .is_launch_config_save_modal_open = false;
                self.launch_config_save_modal.close();
                ctx.notify();
            }
            LaunchConfigModalEvent::SuccessfullySavedConfig(launch_config) => {
                ctx.update_model(&WarpConfig::handle(ctx), move |warp_config, ctx| {
                    warp_config.append_launch_config(launch_config, ctx);
                });
                ctx.notify();
            }
            #[cfg(feature = "local_fs")]
            LaunchConfigModalEvent::OpenFileWithTarget {
                path,
                target,
                line_col,
            } => {
                self.open_file_with_target(
                    path.clone(),
                    target.clone(),
                    *line_col,
                    CodeSource::Link {
                        path: path.clone(),
                        range_start: None,
                        range_end: None,
                    },
                    ctx,
                );
            }
        }
    }

    fn handle_tab_config_params_modal_event(
        &mut self,
        event: &ModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ModalEvent::Close => {
                self.cancel_tab_config_params_modal(ctx);
            }
        }
    }

    /// Cleans up pending state and closes the tab-config params modal without
    /// creating a tab config. Used when the modal is dismissed or cancelled.
    fn cancel_tab_config_params_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.pending_session_config_replacement = None;
        self.pending_session_config_tab_config_chip = false;
        self.close_tab_config_params_modal(ctx);
    }

    fn handle_tab_config_params_modal_body_event(
        &mut self,
        event: &TabConfigParamsModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            TabConfigParamsModalEvent::Submit { config, params } => {
                let should_track_existing_config_open =
                    self.pending_session_config_replacement.is_none();
                let worktree_name = self.maybe_generate_worktree_name(config);
                self.open_tab_config_with_params(
                    config.as_ref().clone(),
                    params.clone(),
                    worktree_name.as_deref(),
                    ctx,
                );
                if should_track_existing_config_open {}
                self.close_tab_config_params_modal(ctx);
                self.complete_pending_session_config_replacement(ctx);

                // The new tab has setup commands (worktree creation); wait for
                // them to finish before starting the onboarding tutorial, but
                // only after the tab-config chip is dismissed.
                // Params modal is now closed; show the chip if it was pending.
                self.promote_session_config_tab_config_chip(ctx);
            }
            TabConfigParamsModalEvent::Close => {
                self.cancel_tab_config_params_modal(ctx);
            }
        }
    }

    /// Finishes the tab replacement that was deferred while the params modal
    /// was open (worktree flow from the session config modal).
    fn complete_pending_session_config_replacement(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(pending) = self.pending_session_config_replacement.take() else {
            return;
        };

        self.remove_tab_by_pane_group_id(pending.old_pane_group_id, ctx);
    }

    /// Removes the tab whose pane group matches `pane_group_id`, if it exists
    /// and there is more than one tab.
    fn remove_tab_by_pane_group_id(
        &mut self,
        pane_group_id: EntityId,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.pane_group.id() == pane_group_id)
        {
            self.remove_tab(index, false, true, ctx);
        }
    }

    fn close_tab_config_params_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_tab_config_params_modal_open = false;
        self.tab_config_params_modal.close();
        self.tab_config_params_modal.view.update(ctx, |modal, ctx| {
            modal.body().update(ctx, |body, ctx| {
                body.on_close(ctx);
            });
        });
        ctx.notify();
    }

    /// Checks whether the tab config references the special-cased
    /// `autogenerated_branch_name` template var. If so, fetches existing
    /// branches and generates a unique themed name.
    fn maybe_generate_worktree_name(
        &self,
        config: &crate::tab_configs::TabConfig,
    ) -> Option<String> {
        if !config.uses_autogenerated_branch_name() {
            return None;
        }
        let pane = config
            .panes
            .iter()
            .find(|pane| pane.directory.is_some())
            .or_else(|| config.panes.first())?;

        let repo_path = pane.directory.as_deref().map(Path::new);
        let branches = repo_path
            .map(crate::util::git::list_local_branches_sync)
            .unwrap_or_default();
        let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
        Some(warp_util::worktree_names::generate_worktree_branch_name(
            &branch_refs,
        ))
    }

    /// Generates a worktree tab config TOML, writes it to `~/.warp/tab_configs/`,
    /// and opens the resulting config as a new tab.
    ///
    /// When `worktree_branch_name` is `None` (autogenerate), the TOML stores
    /// commands with `{autogenerated_branch_name}` template variables that get
    /// substituted with a fresh name on every open.
    /// When `Some(name)` (manual naming), the commands are baked in and a
    /// `worktree_branch_name` param is added so re-opens show the params modal.
    #[cfg(feature = "local_fs")]
    fn handle_new_worktree_submit(
        &mut self,
        repo: &str,
        base_branch: &str,
        worktree_branch_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        let repo_display_name = Path::new(repo)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| repo.to_string());
        let config_name = match worktree_branch_name {
            Some(name) if !name.is_empty() => {
                format!("New worktree: {repo_display_name}, {name}")
            }
            _ if !base_branch.is_empty() => {
                format!("New worktree: {repo_display_name}, {base_branch}")
            }
            _ => format!("New worktree: {repo_display_name}"),
        };

        let filename_hint = if let Some(name) = worktree_branch_name {
            name.to_string()
        } else {
            let branches = crate::util::git::list_local_branches_sync(Path::new(repo));
            let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
            warp_util::worktree_names::generate_worktree_branch_name(&branch_refs)
        };

        let toml_content = crate::tab_configs::build_worktree_config_toml(
            &config_name,
            repo,
            base_branch,
            worktree_branch_name,
        );

        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }

        let path = find_unused_worktree_config_path(&dir, &filename_hint);
        if let Err(e) = std::fs::write(&path, &toml_content) {
            log::warn!("Failed to write worktree tab config: {e:?}");
            return;
        }

        match toml::from_str::<crate::tab_configs::TabConfig>(&toml_content) {
            Ok(tab_config) => {
                if let Some(name) = worktree_branch_name {
                    // First open with manual name — bypass the params modal.
                    let mut param_values = HashMap::new();
                    param_values.insert("worktree_branch_name".to_string(), name.to_string());
                    self.open_tab_config_with_params(tab_config, param_values, None, ctx);
                } else {
                    // Autogenerate — open with the name we just generated.
                    let param_values = tab_config.default_param_values();
                    self.open_tab_config_with_params(
                        tab_config,
                        param_values,
                        Some(&filename_hint),
                        ctx,
                    );
                }
            }
            Err(e) => {
                log::warn!("Failed to parse generated worktree config: {e:?}");
            }
        }
    }

    #[cfg(not(feature = "local_fs"))]
    fn handle_new_worktree_submit(
        &mut self,
        _repo: &str,
        _base_branch: &str,
        _worktree_branch_name: Option<&str>,
        _ctx: &mut ViewContext<Self>,
    ) {
    }

    /// Opens a worktree in the given repo using the default worktree tab config,
    /// saving the materialized config to `~/.warp/tab_configs/` first.
    /// The branch name is auto-generated.
    #[cfg(feature = "local_fs")]
    fn open_worktree_in_repo(&mut self, repo_path: String, ctx: &mut ViewContext<Self>) {
        log::info!("open_worktree_in_repo requested: repo_path={repo_path:?}");
        let config_path = ensure_default_worktree_config();
        log::info!("Reading default worktree config from {config_path:?}");
        let template_toml = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to read default worktree config from {config_path:?}: {e:?}");
                return;
            }
        };
        let branches = crate::util::git::list_local_branches_sync(Path::new(&repo_path));
        let branch_refs: HashSet<&str> = branches.iter().map(|s| s.as_str()).collect();
        let branch_name = warp_util::worktree_names::generate_worktree_branch_name(&branch_refs);
        let repo_display_name = Path::new(&repo_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| repo_path.clone());
        let config_name = format!("Worktree: {repo_display_name}");
        let pane_type = "terminal";
        log::info!(
            "Materializing default worktree config: repo_path={repo_path:?}, branch_name={branch_name:?}, pane_type={pane_type}"
        );

        let (toml_content, tab_config) = match materialize_default_worktree_config(
            &template_toml,
            &config_name,
            &repo_path,
            pane_type,
        ) {
            Ok(materialized) => materialized,
            Err(e) => {
                log::warn!(
                    "Failed to materialize default worktree config from {config_path:?}: {e}"
                );
                return;
            }
        };

        let dir = tab_configs_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create tab_configs dir: {e:?}");
            return;
        }

        let saved_config_path =
            find_unused_toml_path(&dir, &sanitize_toml_base_name(&repo_display_name));
        if let Err(e) = std::fs::write(&saved_config_path, &toml_content) {
            log::warn!("Failed to write worktree tab config to {saved_config_path:?}: {e:?}");
            return;
        }

        log::info!(
            "Saved default worktree config to {saved_config_path:?}: config_name={:?}",
            tab_config.name
        );

        let param_values = tab_config.default_param_values();
        log::info!("Opening tab from saved worktree config");
        self.open_tab_config_with_params(tab_config, param_values, Some(&branch_name), ctx);
    }

    #[cfg(not(feature = "local_fs"))]
    fn open_worktree_in_repo(&mut self, _repo_path: String, _ctx: &mut ViewContext<Self>) {}

    /// Opens a native folder picker to add a new repo to PersistedWorkspace,
    /// triggered from the "+ Add new repo..." item in the New worktree config submenu.
    fn is_input_box_visible(&self, app: &AppContext) -> bool {
        if let (Some(terminal_model), Some(terminal_view)) = (
            self.get_active_session_terminal_model(app),
            self.active_tab_pane_group()
                .as_ref(app)
                .active_session_view(app),
        ) {
            terminal_view.read(app, |view, ctx| {
                view.is_input_box_visible(&terminal_model.lock(), ctx)
            })
        } else {
            false
        }
    }

    fn handle_theme_creator_modal_event(
        &mut self,
        event: &ThemeCreatorModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeCreatorModalEvent::Close => {
                self.current_workspace_state.is_theme_creator_modal_open = false;
                ctx.notify();
            }
            ThemeCreatorModalEvent::SetCustomTheme { theme } => {
                self.theme_chooser_view
                    .update(ctx, |theme_chooser_view, ctx| {
                        theme_chooser_view.reload_and_set_custom_theme(theme.clone(), ctx);
                    });
            }
            ThemeCreatorModalEvent::ShowErrorToast { message } => {
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::error(message.clone());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
            }
        }
    }

    fn handle_theme_deletion_modal_event(
        &mut self,
        event: &ThemeDeletionModalEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeDeletionModalEvent::Close => {
                self.current_workspace_state.is_theme_deletion_modal_open = false;
                ctx.notify();
            }
            ThemeDeletionModalEvent::ShowErrorToast { message } => {
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::error(message.clone());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
            }
            ThemeDeletionModalEvent::DeleteCurrentTheme => {
                self.theme_chooser_view
                    .update(ctx, |theme_chooser_view, ctx| {
                        // Reset theme to Dark if we are deleting the current theme
                        theme_chooser_view.select_and_save_theme(&ThemeKind::Dark, ctx);
                    });
            }
        }
    }

    /// Returns the pane group with the matching EntityId, or None if it doesn't exist.
    fn get_pane_group_view_with_id(&self, id: EntityId) -> Option<&ViewHandle<PaneGroup>> {
        self.tab_views().find(|view| view.id() == id)
    }

    // The workspace manages the close confirmation dialog, so it may need to close a pane after the user confirms in the dialog.
    // The flow is:
    // - User closes pane in pane group, which emits event to workspace
    // - Workspace shows confirmation dialog, and calls back into pane group to close pane here if user confirms
    fn close_pane(
        &mut self,
        pane_group_id: EntityId,
        pane_id: PaneId,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(pane_group_view) = self.get_pane_group_view_with_id(pane_group_id) else {
            log::error!("Could not close pane because pane group doesn't exist");
            return;
        };
        pane_group_view.update(ctx, |pane_group, ctx| {
            pane_group.close_pane(pane_id, ctx);
        });
    }

    fn handle_close_session_confirmation_dialog_event(
        &mut self,
        event: &CloseSessionConfirmationEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            CloseSessionConfirmationEvent::Cancel => {
                self.current_workspace_state
                    .is_close_session_confirmation_dialog_open = false;
                ctx.notify();
            }
            CloseSessionConfirmationEvent::CloseSession {
                dont_show_again,
                open_confirmation_source,
            } => {
                if *dont_show_again {
                    if let Err(e) = SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
                        settings.should_confirm_close_session.set_value(false, ctx)
                    }) {
                        log::error!(
                            "Failed to set should_confirm_close_session setting to false: {e}"
                        );
                    };
                }
                match *open_confirmation_source {
                    OpenDialogSource::CloseTab { tab_index } => {
                        self.remove_tab(tab_index, true, true, ctx);
                    }
                    OpenDialogSource::ClosePane {
                        pane_group_id,
                        pane_id,
                    } => {
                        self.close_pane(pane_group_id, pane_id, ctx);
                    }
                    OpenDialogSource::CloseTabsDirection {
                        tab_index,
                        direction,
                    } => {
                        self.close_tabs_direction(tab_index, direction, true, ctx);
                    }
                    OpenDialogSource::CloseOtherTabs { tab_index } => {
                        self.close_other_tabs(tab_index, true, ctx);
                    }
                }
                self.current_workspace_state
                    .is_close_session_confirmation_dialog_open = false;
                ctx.notify();
            }
        }
    }

    pub fn handle_network_status_event(
        &mut self,
        _handle: ModelHandle<NetworkStatus>,
        _event: &NetworkStatusEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.notify();
    }

    pub fn toggle_block_snackbar(&mut self, ctx: &mut ViewContext<Self>) {
        BlockListSettings::handle(ctx).update(ctx, |blocklist_settings, ctx| {
            report_if_error!(blocklist_settings
                .snackbar_enabled
                .toggle_and_save_value(ctx));
        });
    }

    pub fn toggle_error_underlining(&mut self, ctx: &mut ViewContext<Self>) {
        InputSettings::handle(ctx).update(ctx, |input_settings, ctx| {
            report_if_error!(input_settings.error_underlining.toggle_and_save_value(ctx));
        });
    }

    pub fn toggle_syntax_highlighting(&mut self, ctx: &mut ViewContext<Self>) {
        InputSettings::handle(ctx).update(ctx, |input_settings, ctx| {
            report_if_error!(input_settings
                .syntax_highlighting
                .toggle_and_save_value(ctx));
        });
    }

    pub fn change_cursor(&mut self, cursor_shape: Cursor, ctx: &mut ViewContext<Self>) {
        ctx.set_cursor_shape(cursor_shape);
        ctx.notify();
    }

    pub fn set_a11y_verbosity(
        &mut self,
        verbosity: AccessibilityVerbosity,
        ctx: &mut ViewContext<Self>,
    ) {
        AccessibilitySettings::handle(ctx).update(ctx, |accessibility_settings, ctx| {
            report_if_error!(accessibility_settings
                .a11y_verbosity
                .set_value(verbosity, ctx));
        });
    }

    pub fn snapshot(
        &self,
        window_id: WindowId,
        quake_mode: bool,
        app: &AppContext,
    ) -> WindowSnapshot {
        let window_bounds = app.window_bounds(&window_id);
        let window_fullscreen_state = app
            .windows()
            .platform_window(window_id)
            .map(|window| window.fullscreen_state())
            .unwrap_or_default();
        let active_tab_index = self.active_tab_index();
        let tabs = self
            .tab_views()
            .enumerate()
            .map(|(tab_index, pane_group_view)| {
                let resizable_data = ResizableData::handle(app);
                let modal_sizes = resizable_data.as_ref(app).get_all_handles(window_id);

                let left_panel_width = modal_sizes.map(|ms| {
                    ms.left_panel_width
                        .lock()
                        .expect("should be able to lock left panel handle")
                        .size()
                });

                let pane_group = pane_group_view.as_ref(app);
                let root = pane_group.snapshot(app);
                let left_panel =
                    self.compute_left_panel_snapshot(pane_group_view, left_panel_width, app);
                TabSnapshot {
                    root,
                    custom_title: pane_group.custom_title(app),
                    default_directory_color: self
                        .tabs
                        .get(tab_index)
                        .and_then(|tab| tab.default_directory_color),
                    selected_color: self
                        .tabs
                        .get(tab_index)
                        .map_or(SelectedTabColor::Unset, |tab| tab.selected_color),
                    left_panel,
                }
            })
            .filter(|tab| {
                // Filter out any tab that contains a single, read-only session.
                !matches!(
                    tab.root,
                    PaneNodeSnapshot::Leaf(LeafSnapshot {
                        contents: LeafContents::Terminal(TerminalPaneSnapshot {
                            is_read_only: true,
                            ..
                        }),
                        ..
                    })
                )
            })
            .collect();

        let resizable_data = ResizableData::handle(app);
        let modal_sizes = resizable_data.as_ref(app).get_all_handles(window_id);

        // Reads the current width of the universal search modal, to store with the window snapshot
        let universal_search_width = modal_sizes.map(|ms| {
            ms.universal_search_width
                .lock()
                .expect("should be able to lock universal search resizable state handle")
                .size()
        });

        let warp_ai_width = modal_sizes.map(|ms| {
            ms.warp_ai_width
                .lock()
                .expect("should be able to lock warp_ai resizable state handle")
                .size()
        });

        let voltron_width = modal_sizes.map(|ms| {
            ms.voltron_width
                .lock()
                .expect("should be able to lock voltron resizable state handle")
                .size()
        });

        let left_panel_width = modal_sizes.map(|ms| {
            ms.left_panel_width
                .lock()
                .map(|guard| guard.size())
                .unwrap_or(DEFAULT_LEFT_PANEL_WIDTH)
        });

        let right_panel_width = modal_sizes.map(|ms| {
            ms.right_panel_width
                .lock()
                .map(|guard| guard.size())
                .unwrap_or(DEFAULT_RIGHT_PANEL_WIDTH)
        });

        WindowSnapshot {
            tabs,
            active_tab_index,
            bounds: window_bounds,
            fullscreen_state: window_fullscreen_state,
            quake_mode,
            universal_search_width,
            warp_ai_width,
            voltron_width,
            left_panel_open: self.left_panel_open,
            vertical_tabs_panel_open: false,
            left_panel_width,
            right_panel_width,
        }
    }

    fn compute_left_panel_snapshot(
        &self,
        pane_group: &ViewHandle<PaneGroup>,
        left_panel_width: Option<f32>,
        app: &AppContext,
    ) -> Option<LeftPanelSnapshot> {
        let pane_group_ref = pane_group.as_ref(app);
        if !pane_group_ref.left_panel_open {
            return None;
        }

        let pane_group_id = pane_group.id();

        self.left_panel_view.read(app, |lp, _| {
            Some(LeftPanelSnapshot {
                left_panel_displayed_tab: lp.active_view().into(),
                pane_group_id: pane_group_id.to_string(),
                width: left_panel_width.unwrap_or(DEFAULT_LEFT_PANEL_WIDTH) as usize,
            })
        })
    }

    pub fn open_launch_config_save_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.close_palette(true, None, ctx); // close palettes if any are open
        self.launch_config_save_modal.open();
        self.current_workspace_state
            .is_launch_config_save_modal_open = true;

        self.launch_config_save_modal.view.update(ctx, |view, ctx| {
            view.set_snapshot_source(ctx);
            view.reset_editor(ctx); // placeholder and clear editor
            ctx.notify();
        });

        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::SaveNewLaunchConfig),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        ctx.focus(&self.launch_config_save_modal.view);
        ctx.notify();
    }

    pub fn cycle_prev_session(&mut self, ctx: &mut ViewContext<Self>) {
        self.cycle_session(SessionCycleDirection::Previous, ctx);
    }

    pub fn cycle_next_session(&mut self, ctx: &mut ViewContext<Self>) {
        self.cycle_session(SessionCycleDirection::Next, ctx);
    }

    fn cycle_session(&mut self, direction: SessionCycleDirection, ctx: &mut ViewContext<Self>) {
        let keys_settings = KeysSettings::as_ref(ctx);
        match *keys_settings.ctrl_tab_behavior {
            CtrlTabBehavior::ActivatePrevNextTab => match direction {
                SessionCycleDirection::Next => {
                    self.activate_next_tab(ctx);
                }
                SessionCycleDirection::Previous => {
                    self.activate_prev_tab(ctx);
                }
            },
            CtrlTabBehavior::CycleMostRecentSession => {
                self.current_workspace_state.is_palette_open = false;
                if !self.current_workspace_state.is_ctrl_tab_palette_open {
                    self.open_palette(
                        PaletteMode::Navigation,
                        PaletteSource::CtrlTab { query: None },
                        ctx,
                    );
                }
                self.ctrl_tab_palette
                    .update(ctx, |palette, ctx| match direction {
                        SessionCycleDirection::Next => {
                            palette.select_next_item(ctx);
                        }
                        SessionCycleDirection::Previous => {
                            palette.select_prev_item(ctx);
                        }
                    });
                ctx.notify();
            }
        }
    }

    pub fn activate_prev_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let index = if self.active_tab_index > 0 {
            self.active_tab_index - 1
        } else {
            self.tabs.len() - 1
        };
        self.activate_tab(index, ctx);
    }

    pub fn activate_next_tab(&mut self, ctx: &mut ViewContext<Self>) {
        let index = if self.active_tab_index + 1 < self.tabs.len() {
            self.active_tab_index + 1
        } else {
            0
        };
        self.activate_tab(index, ctx);
    }

    pub fn activate_last_tab(&mut self, ctx: &mut ViewContext<Self>) {
        if self.tabs.len() > 1 {
            let target_index = self.tabs.len() - 1;
            self.activate_tab(target_index, ctx);
        }
    }

    fn remove_tab(
        &mut self,
        index: usize,
        add_to_undo_stack: bool,
        detach_panes_for_close: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(tab_data) = self.tabs.get(index) else {
            debug_assert!(false, "Tried to remove a tab with an invalid index");
            return;
        };

        // If this is the last tab, close the window instead of actually removing
        // the tab.
        if self.tabs.len() == 1 {
            if ContextFlag::CloseWindow.is_enabled() {
                ctx.close_window();
            }
            return;
        }

        if detach_panes_for_close {
            let working_directories_model = self.working_directories_model.clone();
            tab_data.pane_group.update(ctx, |pane_group, ctx| {
                pane_group.for_all_terminal_panes(
                    |terminal_view, ctx| {
                        if terminal_view
                            .model
                            .lock()
                            .block_list()
                            .active_block()
                            .is_active_and_long_running()
                        {
                            terminal_view.shutdown_pty(ctx);
                        }
                    },
                    ctx,
                );

                pane_group.detach_panes_for_close(&working_directories_model, ctx);
            });
        }

        let tab_data = self.tabs.remove(index);

        if add_to_undo_stack {
            let handle = ctx.handle();
            UndoCloseStack::handle(ctx).update(ctx, |stack, ctx| {
                log::info!("storing data for closed tab");
                stack.handle_tab_closed(handle, index, tab_data, ctx);
            });
        }

        match index.cmp(&self.active_tab_index) {
            Ordering::Equal => {
                // If there's a previous tab, activate it. Otherwise, keep the active
                // tab at index 0.
                self.activate_tab_internal(index.saturating_sub(1), ctx);
            }
            Ordering::Less => {
                // If we are closing a tab before the active tab we need to adjust
                // the active tab index.
                self.active_tab_index -= 1;
            }
            _ => {}
        }

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    pub fn remove_tab_without_undo(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        self.remove_tab(index, false, false, ctx);
    }
    /// Adopts a transferred PaneGroup into the placeholder tab created during window transfer.
    /// This replaces the placeholder tab's PaneGroup with the actual transferred one.
    pub fn adopt_transferred_pane_group(
        &mut self,
        new_pane_group: ViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        if !self.pending_pane_group_transfer {
            debug_assert!(
                false,
                "adopt_transferred_pane_group called without pending transfer"
            );
            return;
        }

        if self.tabs.is_empty() {
            debug_assert!(false, "adopt_transferred_pane_group called with no tabs");
            return;
        }
        let Some(placeholder_tab) = self.tabs.last_mut() else {
            debug_assert!(
                false,
                "adopt_transferred_pane_group missing placeholder tab"
            );
            return;
        };

        let placeholder_pane_group =
            std::mem::replace(&mut placeholder_tab.pane_group, new_pane_group);
        let working_directories_model = self.working_directories_model.clone();
        placeholder_pane_group.update(ctx, |pg, ctx| {
            pg.detach_panes_for_close(&working_directories_model, ctx);
        });
        self.pending_pane_group_transfer = false;

        ctx.dispatch_global_action("workspace:save_app", ());
        ctx.notify();
    }

    fn should_confirm_close_session(&self, ctx: &mut ViewContext<Self>) -> bool {
        // If we're closing the only remaining tab, we're actually going to close the window.
        // We don't need a user confirmation here because there's already another one on window close.
        if self.tab_count() == 1 {
            return false;
        }
        // TODO: remove session sharing flag check when long-running commands are included
        FeatureFlag::CreatingSharedSessions.is_enabled()
            && ContextFlag::CreateSharedSession.is_enabled()
            && *SessionSettings::as_ref(ctx).should_confirm_close_session
    }

    /// Checks if the provided tab indices need to be confirmed before closing, unless skip_confirmation is true.
    /// If none of them need confirmation (or the confirm setting is turned off), we close all the provided tabs.
    /// Returns true iff all of the tabs were closed.
    fn close_tabs(
        &mut self,
        tab_indices: impl Iterator<Item = usize>,
        dialog_source: OpenDialogSource,
        skip_confirmation: bool,
        add_to_undo_stack: bool,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let tab_indices_vec = tab_indices.collect_vec();
        // Check if there are any tabs that can't be closed without confirmation
        if !skip_confirmation && self.should_confirm_close_session(ctx) {
            for i in tab_indices_vec.iter() {
                let is_tab_shared = self
                    .get_pane_group_view(*i)
                    .is_some_and(|view| view.as_ref(ctx).is_terminal_pane_being_shared(ctx));
                if is_tab_shared {
                    self.show_close_session_confirmation_dialog(dialog_source, ctx);
                    return false;
                }
            }
        }

        if !skip_confirmation {
            let tabs = tab_indices_vec
                .iter()
                .filter_map(|i| self.get_pane_group_view(*i))
                .map(|tab| tab.downgrade())
                .collect_vec();
            let summary = UnsavedStateSummary::for_tabs(tabs, ctx);

            if summary.should_display_warning(ctx) {
                // The quit-warning dialog uses app-scoped callbacks (ironically, because that's
                // what Self::show_native_modal expects). That means we need a handle to the
                // current workspace here.
                let confirm_self = ctx.handle();
                let navigate_self = ctx.handle();
                let confirm_tabs = tab_indices_vec.clone();
                let dialog = summary
                    .dialog()
                    .on_confirm(move |ctx| {
                        if let Some(workspace) = confirm_self.upgrade(ctx) {
                            workspace.update(ctx, |workspace, ctx| {
                                workspace.close_tabs(
                                    confirm_tabs.into_iter(),
                                    dialog_source,
                                    true,
                                    add_to_undo_stack,
                                    ctx,
                                );
                            });
                        }
                    })
                    .on_cancel(|_ctx| { /* No action needed besides dismissing the dialog. */ })
                    .on_show_processes(move |ctx| {
                        if let Some(workspace) = navigate_self.upgrade(ctx) {
                            workspace.update(ctx, |workspace, ctx| {
                                // TODO(ben): Ideally, this would filter to the relevant tabs.
                                workspace.open_palette(
                                    PaletteMode::Navigation,
                                    PaletteSource::QuitModal,
                                    ctx,
                                );
                            })
                        }
                    })
                    .build();

                if cfg!(all(not(target_family = "wasm"), target_os = "macos")) {
                    AppContext::show_native_platform_modal(ctx, dialog);
                    return false;
                } else if cfg!(all(
                    not(target_family = "wasm"),
                    any(target_os = "linux", windows)
                )) {
                    self.show_native_modal(dialog, ctx);
                    return false;
                }
            }
        }

        // If we are renaming a tab, cancel that.  Closing tabs causes the renamed tab index
        // to fall out of sync.  This can cause inconsistencies.
        self.cancel_tab_rename(ctx);

        // Remove the tabs in reverse order to avoid indexing OOB.
        for i in tab_indices_vec.into_iter().sorted().rev() {
            self.remove_tab(i, add_to_undo_stack, true, ctx);
        }
        true
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    fn close_tab(
        &mut self,
        index: usize,
        skip_confirmation: bool,
        add_to_undo_stack: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let is_last_tab = self.tabs.len() == 1;
        if !ContextFlag::CloseWindow.is_enabled() && is_last_tab {
            return;
        }

        let tabs_closed = self.close_tabs(
            vec![index].into_iter(),
            OpenDialogSource::CloseTab { tab_index: index },
            skip_confirmation || is_last_tab, // If this is the last tab, the confirmation dialog will be handled by the window close.
            add_to_undo_stack,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {
            ctx.dispatch_global_action("workspace:save_app", ());
        }
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    pub fn close_other_tabs(
        &mut self,
        index: usize,
        skip_confirmation: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        // Figure out what indices we want to delete for the "other tabs" case.
        let indices_to_remove = (0..self.tabs.len()).filter(|i| *i != index);

        let tabs_closed = self.close_tabs(
            indices_to_remove,
            OpenDialogSource::CloseOtherTabs { tab_index: index },
            skip_confirmation,
            true,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {}
    }

    /// Opens a confirmation dialog if necessary, or closes immediately if not.
    /// Always closes immediately if skip_confirmation is true.
    pub fn close_tabs_direction(
        &mut self,
        index: usize,
        direction: TabMovement,
        skip_confirmation: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let indices_to_remove = match direction {
            TabMovement::Left => 0..index,
            TabMovement::Right => (index + 1)..self.tabs.len(),
        };
        let tabs_closed = self.close_tabs(
            indices_to_remove,
            OpenDialogSource::CloseTabsDirection {
                tab_index: index,
                direction,
            },
            skip_confirmation,
            true,
            ctx,
        );

        // Telemetry whenever tabs actually closed, not when confirmation dialog comes up.
        if tabs_closed {
            match direction {
                TabMovement::Right if self.active_tab_index > index => {}
                _ => (),
            }
        }
    }

    /// Update this workspace when it is reopened after being closed.
    pub fn handle_reopen(&mut self, ctx: &mut ViewContext<Self>) {
        self.sync_window_button_visibility(ctx);
        for pane_group in self.tab_views() {
            pane_group.update(ctx, |pane_group, ctx| {
                pane_group.reattach_panes(ctx);
            })
        }
        self.update_active_session(ctx);
    }

    pub fn restore_closed_tab(
        &mut self,
        tab_index: usize,
        tab_data: TabData,
        ctx: &mut ViewContext<Self>,
    ) {
        // When restoring a closed tab, we have to reattach its panes so that they know they're
        // user-accessible again.
        tab_data.pane_group.update(ctx, |pane_group, ctx| {
            pane_group.reattach_panes(ctx);
        });

        self.tabs.insert(tab_index, tab_data);
        self.activate_tab(tab_index, ctx);

        ctx.notify();
    }

    pub fn add_terminal_tab(&mut self, hide_homepage: bool, ctx: &mut ViewContext<Self>) {
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            None,
            hide_homepage,
            ctx,
        );
        ctx.notify();
    }

    // Adds a tab with a specific shell, only meant to be dispatched directly by actions.
    fn add_tab_with_shell(
        &mut self,
        shell: AvailableShell,
        source: AddTabWithShellSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.add_new_session_tab_with_default_mode(
            NewSessionSource::Tab,
            Some(ctx.window_id()),
            Some(shell),
            false,
            ctx,
        );
        ctx.notify();
    }

    fn add_new_session_tab_with_default_mode(
        &mut self,
        new_session_source: NewSessionSource,
        previous_session_window_id: Option<WindowId>,
        chosen_shell: Option<AvailableShell>,
        hide_homepage: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.add_new_session_tab_internal_with_default_session_mode_behavior(
            new_session_source,
            previous_session_window_id,
            chosen_shell,
            hide_homepage,
            DefaultSessionModeBehavior::Apply,
            ctx,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn add_new_session_tab_internal_with_default_session_mode_behavior(
        &mut self,
        new_session_source: NewSessionSource,
        previous_session_window_id: Option<WindowId>,
        chosen_shell: Option<AvailableShell>,
        hide_homepage: bool,
        default_session_mode_behavior: DefaultSessionModeBehavior,
        ctx: &mut ViewContext<Self>,
    ) {
        // Check if we should default to agent mode (only for new sessions, not restorations)

        let startup_directory = self.get_new_tab_startup_directory(
            new_session_source,
            previous_session_window_id,
            chosen_shell.as_ref(),
            ctx,
        );

        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                shell: chosen_shell,
                initial_directory: startup_directory,
                hide_homepage,
                ..Default::default()
            })),
            Arc::new(HashMap::new()),
            None, /*custom_tab_title*/
            ctx,
        );
    }

    /// Enters agent view with a new conversation on the active tab's terminal.
    ///
    /// Used after adding a new tab when the session mode should default to agent view.
    pub fn add_tab_with_pane_layout(
        &mut self,
        panes_layout: PanesLayout,
        block_lists: Arc<HashMap<PaneUuid, Vec<SerializedBlockListItem>>>,
        custom_tab_title: Option<String>,
        ctx: &mut ViewContext<Self>,
    ) {
        // Remember whether the left panel was open on the current active pane group
        // before creating a new active pane group.
        let left_panel_was_open = if self.tabs.is_empty() {
            false
        } else {
            self.active_tab_pane_group().as_ref(ctx).left_panel_open
        };

        // Capture the active tab's colors before creating the new tab.
        let active_tab = self.tabs.get(self.active_tab_index);
        let active_tab_selected_color = active_tab.map(|tab| tab.selected_color);
        let active_tab_default_color = active_tab.and_then(|tab| tab.default_directory_color);

        let is_new_terminal = matches!(panes_layout, PanesLayout::SingleTerminal(_));
        let is_restoration = matches!(panes_layout, PanesLayout::Snapshot(_));
        let new_pane_group = ctx.add_typed_action_view(|ctx| {
            let mut pane_group = PaneGroup::new_with_panes_layout(
                self.tips_completed.clone(),
                self.user_default_shell_unsupported_banner_model_handle
                    .clone(),
                panes_layout,
                block_lists,
                self.model_event_sender.clone(),
                ctx,
            );
            if let Some(title) = custom_tab_title {
                pane_group.set_title(&title, ctx);
            }
            pane_group
        });

        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        let new_tab_placement_setting = TabSettings::as_ref(ctx).new_tab_placement;

        match new_tab_placement_setting {
            NewTabPlacement::AfterAllTabs => {
                self.tabs.push(TabData::new(new_pane_group));
                self.activate_tab_internal(self.tab_count() - 1, ctx);
            }
            // Add tab after current tab
            _ => {
                if self.tab_count() == 0 {
                    self.tabs.push(TabData::new(new_pane_group));
                    self.activate_tab_internal(self.tab_count() - 1, ctx);
                } else {
                    self.tabs
                        .insert(self.active_tab_index + 1, TabData::new(new_pane_group));
                    self.activate_tab_internal(self.active_tab_index + 1, ctx);
                }
            }
        }

        if !is_restoration {
            if *TabSettings::as_ref(ctx).preserve_active_tab_color.value() {
                if let Some(SelectedTabColor::Color(color)) = active_tab_selected_color {
                    self.tabs[self.active_tab_index].selected_color =
                        SelectedTabColor::Color(color);
                }
            }

            // preserve the current tab's default directory color when the new tab inherits the working directory
            // (otherwise the new tab's color flashes from no-color to default color during bootstrapping).
            if FeatureFlag::DirectoryTabColors.is_enabled() && is_new_terminal {
                let wd_config = &SessionSettings::as_ref(ctx).working_directory_config;
                let inherits_cwd = wd_config.config_for_source(NewSessionSource::Tab).mode
                    == WorkingDirectoryMode::PreviousDir
                    || wd_config.config_for_source(NewSessionSource::Window).mode
                        == WorkingDirectoryMode::PreviousDir;
                if inherits_cwd {
                    if let Some(color) = active_tab_default_color {
                        self.tabs[self.active_tab_index].default_directory_color = Some(color);
                    }
                }
            }
        }

        // If the previous tab's left panel was open, maintain that state with the new tab
        // (unless we're restoring the tab from a persisted snapshot).
        if !is_restoration && left_panel_was_open {
            self.active_tab_pane_group().update(ctx, |pg, ctx| {
                pg.set_left_panel_open(true, ctx);
            });
        }
    }

    pub fn add_tab_from_existing_pane(
        &mut self,
        pane: Box<dyn AnyPaneContent>,
        new_idx: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        let new_pane_group = ctx.add_typed_action_view(|ctx| {
            PaneGroup::new_from_existing_pane(
                pane,
                self.tips_completed.clone(),
                self.user_default_shell_unsupported_banner_model_handle
                    .clone(),
                self.model_event_sender.clone(),
                ctx,
            )
        });
        ctx.subscribe_to_view(&new_pane_group, move |me, pane_group, event, ctx| {
            me.handle_file_tree_event(pane_group, event, ctx)
        });

        if self.tab_count() == 0 {
            self.tabs.push(TabData::new(new_pane_group));
            self.activate_tab_internal(self.tab_count() - 1, ctx);
        } else {
            self.tabs.insert(new_idx, TabData::new(new_pane_group));
            self.activate_tab_internal(new_idx, ctx);
        }
    }

    fn open_repository(&mut self, path: Option<&str>, ctx: &mut ViewContext<Self>) {
        match path {
            Some(path) => self.handle_open_repository(path, ctx),
            None => ctx.open_file_picker(
                |result, ctx| match result {
                    Ok(paths) => {
                        let Some(path) = paths.into_iter().next() else {
                            return;
                        };

                        if let Some(handle) = ctx.handle().upgrade(ctx) {
                            handle.update(ctx, |workspace, ctx| {
                                workspace.handle_open_repository(&path, ctx);
                            });
                        }
                    }
                    Err(err) => {
                        let window_id = ctx.window_id();
                        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
                            toast_stack.add_ephemeral_toast(
                                DismissibleToast::error(format!("{err}")),
                                window_id,
                                ctx,
                            );
                        });
                    }
                },
                FilePickerConfiguration::new().folders_only(),
            ),
        }
    }

    fn handle_open_repository(&mut self, path: &str, ctx: &mut ViewContext<Self>) {
        let path_buf = PathBuf::from(path);
        ProjectManagementModel::handle(ctx).update(ctx, |projects, ctx| {
            projects.upsert_project(path_buf.clone(), ctx);
        });
        self.add_tab_with_pane_layout(
            PanesLayout::SingleTerminal(Box::new(NewTerminalOptions {
                initial_directory: Some(path_buf.clone()),
                hide_homepage: true,
                ..Default::default()
            })),
            Arc::new(HashMap::new()),
            None,
            ctx,
        );
    }

    /// Navigate to an existing AI conversation, focusing on its terminal view, if it's open anywhere.
    /// If the conversation is not in an open pane, restore it based on the provided layout override
    /// or the user's setting.
    /// Restores a conversation into the active terminal pane.
    /// Shows a full-screen loading state while fetching, then restores the conversation into the existing terminal.
    /// Falls back to new tab if we cannot restore into the active pane (has a long-running command or is invalid).
    /// Restores a conversation in a new split pane.
    /// Creates a loading pane immediately, then replaces it with the real terminal with the conversation once data loads.
    /// We have to do this instead of loading the data into the same terminal pane to avoid problems with
    /// restoring conversations while the shell is bootstrapping.
    /// Restores a conversation in a new tab.
    /// Handle a tab being dragged
    ///
    /// Will determine if the dragged tab needs to be swapped with another tab in the list and
    /// perform the swap, making sure to maintain the active tab if necessary
    fn on_tab_drag(&mut self, current_index: usize, position: RectF, ctx: &mut ViewContext<Self>) {
        let new_index = self.calculate_updated_tab_index(current_index, position, ctx);

        if new_index != current_index {
            self.tabs.swap(new_index, current_index);

            // Update the active tab index if it was impacted by the swap
            if current_index == self.active_tab_index {
                self.set_active_tab_index(new_index, ctx);
            } else if new_index == self.active_tab_index {
                self.set_active_tab_index(current_index, ctx);
            }

            ctx.notify();
        }
    }

    /// Determines the appropriate index for a tab that is being dragged, based on its current
    /// index and drag position
    ///
    /// We check if the midpoint of the dragged tab has crossed into the boundary of either
    /// surrounding tab. For the tab immediately to the left, this means checking against the
    /// rightmost boundary, while for the tab immediately to the right, we check against the
    /// leftmost boundary.
    ///
    /// If the midpoint is not in either location, then we return the current index, as the tab has
    /// not moved out of its position
    fn calculate_updated_tab_index(
        &self,
        current_index: usize,
        drag_position: RectF,
        ctx: &mut ViewContext<Self>,
    ) -> usize {
        let midpoint_drag_x = (drag_position.min_x() + drag_position.max_x()) / 2.;

        let maybe_left_tab = if current_index > 0 {
            ctx.element_position_by_id(tab_position_id(current_index - 1))
        } else {
            None
        };
        if let Some(tab_position) = maybe_left_tab {
            if midpoint_drag_x < tab_position.max_x() {
                return current_index - 1;
            }
        }

        let maybe_right_tab = if current_index < self.tabs.len() - 1 {
            ctx.element_position_by_id(tab_position_id(current_index + 1))
        } else {
            None
        };
        if let Some(tab_position) = maybe_right_tab {
            if midpoint_drag_x > tab_position.min_x() {
                return current_index + 1;
            }
        }

        current_index
    }

    // Move tab, given tab index, left or right
    fn move_tab(&mut self, index: usize, direction: TabMovement, ctx: &mut ViewContext<Self>) {
        let tabs_len = self.tabs.len();
        let new_index = match direction {
            TabMovement::Left if index > 0 => index - 1,
            TabMovement::Right if index < tabs_len - 1 => index + 1,
            _ => return,
        };
        // Don't need to worry about negative numbers because that case is covered above
        self.tabs.swap(index, new_index);

        if index == self.active_tab_index {
            self.set_active_tab_index(new_index, ctx);
        } else {
            // Don't want to change the active tab for the user due to an adjacent
            // tab being moved left/right.
            if new_index == self.active_tab_index {
                self.set_active_tab_index(index, ctx);
            }
        }

        ctx.notify();
    }

    /// How to render the tab bar.
    fn tab_bar_mode(&self, app: &AppContext) -> ShowTabBar {
        // Always show the tab bar during HoA onboarding so that callouts
        // pointing at tabs/inbox render correctly even when the user has
        // "show tab bar on hover" enabled.
        if self.should_show_session_config_tab_config_chip() {
            return ShowTabBar::Stacked;
        }

        if !FeatureFlag::FullScreenZenMode.is_enabled() {
            return ShowTabBar::default();
        }

        let is_fullscreen = app
            .windows()
            .platform_window(self.window_id)
            .is_some_and(|window| window.fullscreen_state() == FullscreenState::Fullscreen);

        let is_hovered = self
            .tab_bar_hover_state
            .lock()
            .is_ok_and(|state| state.is_hovered())
            || self.traffic_light_mouse_states.are_traffic_lights_hovered();

        // Check if any of the menus/popups rendered relative to the tab bar are open.
        let is_tab_menu_open = self.show_tab_bar_overflow_menu
            || self.show_tab_right_click_menu.is_some()
            || self.show_new_session_dropdown_menu.is_some()
            || self.is_user_menu_open
            || self.tab_bar_pinned_by_popup;

        // Check if any panes are being dragged (potentially into a new tab).
        let is_pane_being_dragged = self
            .active_tab_pane_group()
            .as_ref(app)
            .any_pane_being_dragged(app);

        let workspace_decoration_visibility = TabSettings::as_ref(app)
            .workspace_decoration_visibility
            .value();

        let hovered_visibility = if is_pane_being_dragged || is_hovered || is_tab_menu_open {
            ShowTabBar::Stacked
        } else {
            ShowTabBar::Hidden
        };

        match workspace_decoration_visibility {
            WorkspaceDecorationVisibility::OnHover => hovered_visibility,
            // If the tab bar is hidden when fullscreen, show/hide on hover.
            WorkspaceDecorationVisibility::HideFullscreen if is_fullscreen => hovered_visibility,
            // If the user always wants a tab bar OR the window isn't fullscreen, make it
            // persistently stacked above the content area.
            _ => ShowTabBar::Stacked,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn sync_window_button_visibility(&self, ctx: &mut ViewContext<Self>) {
        use warpui::platform::mac::WindowExt;
        let show = if FeatureFlag::FullScreenZenMode.is_enabled()
            && TabSettings::as_ref(ctx)
                .workspace_decoration_visibility
                .value()
                == &WorkspaceDecorationVisibility::OnHover
        {
            self.tab_bar_mode(ctx).has_tab_bar()
        } else {
            TabSettings::as_ref(ctx)
                .workspace_decoration_visibility
                .show_window_decorations()
        };
        if let Some(platform_window) = ctx.windows().platform_window(ctx.window_id()) {
            platform_window.as_ref().set_window_buttons(show);
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn sync_window_button_visibility(&self, _: &mut ViewContext<Self>) {
        // Only macOS uses native window buttons.
    }

    /// Updates the titlebar height to match the scaled tab bar height.
    pub fn update_titlebar_height(&self, ctx: &mut ViewContext<Self>) {
        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let scaled_tab_bar_height = (TOTAL_TAB_BAR_HEIGHT * zoom_factor) as f64;

        if let Some(platform_window) = ctx.windows().platform_window(ctx.window_id()) {
            platform_window
                .as_ref()
                .set_titlebar_height(scaled_tab_bar_height);
        }
    }

    fn request_notification_permissions_if_needed(&mut self, ctx: &mut ViewContext<Self>) {
        // Request permissions any time notifications are currently enabled.
        let current_mode = SessionSettings::as_ref(ctx).notifications.value().mode;

        if current_mode == NotificationsMode::Enabled {
            ctx.request_desktop_notification_permissions(move |view, outcome, ctx| {
                match &outcome {
                    RequestPermissionsOutcome::Accepted => (),
                    RequestPermissionsOutcome::PermissionsDenied => {
                        // Show a helpful toast if the user denied permissions.
                        let url = NOTIFICATIONS_TROUBLESHOOT_URL.to_string();
                        view.toast_stack.update(ctx, |toast_stack, ctx| {
                            let toast = DismissibleToast::error(
                                "Warp doesn't have permission to send desktop notifications.".to_string(),
                            )
                            .with_link(ToastLink::new("Troubleshoot notifications".to_string()).with_href(url));
                            toast_stack.add_persistent_toast(toast, ctx);
                        });
                    }
                    RequestPermissionsOutcome::OtherError { error_message } => {
                        log::error!(
                            "Unknown error when requesting notification permissions. error_msg: {error_message}"
                        );
                    }
                }
            });
        }
    }

    fn toggle_notifications(&mut self, ctx: &mut ViewContext<Self>) {
        let current_settings = SessionSettings::as_ref(ctx).notifications.value().clone();
        let previous_mode = current_settings.mode;
        let new_mode = match previous_mode {
            NotificationsMode::Unset | NotificationsMode::Dismissed => NotificationsMode::Enabled,
            NotificationsMode::Enabled => NotificationsMode::Disabled,
            NotificationsMode::Disabled => NotificationsMode::Enabled,
        };

        SessionSettings::handle(ctx).update(ctx, |settings, ctx| {
            let new_notifications = NotificationsSettings {
                mode: new_mode,
                ..current_settings
            };
            if let Err(e) = settings.notifications.set_value(new_notifications, ctx) {
                log::error!("Error persisting notifications setting: {e}");
            }
        });
    }

    fn open_command_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::CommandPalette),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
        });
    }

    fn open_files_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.tips_completed.update(ctx, |tips_completed, ctx| {
            mark_feature_used_and_write_to_user_defaults(
                Tip::Action(TipAction::CommandPalette),
                tips_completed,
                ctx,
            );
            ctx.notify();
        });

        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            // Reset mixer with correct file data source before setting filter
            let mixer = view.search_bar.as_ref(ctx).mixer().clone();
            view.data_source_store.update(ctx, |store, ctx| {
                store.reset_search_mixer(mixer, false, ctx);
            });
            view.set_active_query_filter(QueryFilter::Files, ctx);
        });
    }
    fn open_navigation_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::Sessions, ctx);
            view.set_initial_selection_offset(0, ctx);
        });
        ctx.notify();
    }

    fn open_launch_config_palette(&mut self, ctx: &mut ViewContext<Self>) {
        self.palette.update(ctx, |view, ctx| {
            view.reset(ctx);
            view.set_active_query_filter(QueryFilter::LaunchConfigurations, ctx);
        });
    }

    fn close_palette(
        &mut self,
        focus_active_tab: bool,
        accepted_action_type: Option<&'static str>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.current_workspace_state.is_palette_open = false;
        self.current_workspace_state.is_ctrl_tab_palette_open = false;
        self.tab_bar_pinned_by_popup = false;
        self.sync_window_button_visibility(ctx);
        if focus_active_tab
            // If the user did not do any action on the command palette (eg. closed via shortcut or clicking away)
            // we always force the focus back onto the terminal input
            // Otherwise we check if any other views are open before moving focus back to terminal input
            && (accepted_action_type.is_none()
                || !self
                    .current_workspace_state
                    .is_any_non_terminal_view_open(ctx))
        {
            self.focus_active_tab(ctx);
        }
        ctx.notify();
    }

    /// Close all overlays in this workspace and the active pane group.
    fn close_all_overlays(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.close_all_modals();
        self.close_tab_bar_overflow_menu(ctx);
    }

    fn open_palette(
        &mut self,
        mode: PaletteMode,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);

        let active_palette = if matches!(source, PaletteSource::CtrlTab { .. }) {
            &self.ctrl_tab_palette
        } else {
            &self.palette
        };

        if matches!(source, PaletteSource::TitleBarSearchBar) {
            self.tab_bar_pinned_by_popup = true;
        }
        if matches!(source, PaletteSource::CtrlTab { .. }) {
            self.current_workspace_state.is_ctrl_tab_palette_open = true;
        } else {
            self.current_workspace_state.is_palette_open = true;
        }
        match mode {
            PaletteMode::Command => self.open_command_palette(ctx),
            PaletteMode::Navigation => match source {
                PaletteSource::CtrlTab { .. } => self.open_navigation_palette(ctx),
                _ => self.open_navigation_palette(ctx),
            },
            PaletteMode::LaunchConfig => self.open_launch_config_palette(ctx),
            PaletteMode::Files => self.open_files_palette(ctx),
            PaletteMode::WarpDrive
            | PaletteMode::Conversations
            | PaletteMode::ConversationsAndRepos => self.open_navigation_palette(ctx),
        }

        ctx.focus(&self.palette);

        ctx.notify();
    }

    /// Implements the WorkspaceAction::OpenPalette. This method makes sure the palette is open and
    /// has up-to-date sources. Use this if you don't want toggle semantics.
    pub fn is_palette_mode_enabled(&self, palette_mode: PaletteMode, app: &AppContext) -> bool {
        self.palette.as_ref(app).is_mode_enabled(palette_mode, app)
    }

    /// Toggle the open / closed state of the palette (so that hitting shortcut a second time
    /// will close the palette)
    fn toggle_palette(
        &mut self,
        palette_mode: PaletteMode,
        source: PaletteSource,
        ctx: &mut ViewContext<Self>,
    ) {
        // If the invite modal is open, don't show the palette since it won't be visible anyway
        if !self
            .current_workspace_state
            .is_any_non_palette_modal_open(ctx)
        {
            let is_palette_mode_already_open =
                self.palette.as_ref(ctx).is_mode_enabled(palette_mode, ctx)
                    && ((matches!(source, PaletteSource::CtrlTab { .. })
                        && self.current_workspace_state.is_ctrl_tab_palette_open)
                        || self.current_workspace_state.is_palette_open);
            if is_palette_mode_already_open {
                self.close_palette(true, None, ctx);
            } else {
                self.open_palette(palette_mode, source, ctx);
            }
        }
    }

    fn handle_palette_event(&mut self, event: &CommandPaletteEvent, ctx: &mut ViewContext<Self>) {
        match event {
            CommandPaletteEvent::Close {
                accepted_action_type,
            } => self.close_palette(true, *accepted_action_type, ctx),
            _ => {}
        }
    }

    /// This function is used when we set a selected object, which is an object open in an active pane.
    /// We do not want to focus Warp Drive, instead we want to focus the editor of the open object.
    /// This function is used when we want to view an item in Warp Drive AND focus Warp Drive.
    /// Updates the left panel's warp drive view.
    /// View an object in Warp Drive and open its sharing settings.
    fn manual_check_for_update(&self, ctx: &mut ViewContext<Self>) {
        let _ = ctx;
    }

    pub fn is_theme_creator_modal_open(&self) -> bool {
        self.current_workspace_state.is_theme_creator_modal_open
    }

    pub fn is_theme_deletion_modal_open(&self) -> bool {
        self.current_workspace_state.is_theme_deletion_modal_open
    }

    pub fn is_palette_open(&self) -> bool {
        self.current_workspace_state.is_palette_open
            || self.current_workspace_state.is_ctrl_tab_palette_open
    }

    pub fn is_left_panel_open(&self, ctx: &AppContext) -> bool {
        self.active_tab_pane_group().as_ref(ctx).left_panel_open
    }

    fn refresh_working_directories_for_pane_group(
        &mut self,
        pane_group: &ViewHandle<PaneGroup>,
        ctx: &mut ViewContext<Self>,
    ) {
        let pane_group_id = pane_group.id();
        let terminal_cwds: Vec<(EntityId, String)> = pane_group
            .as_ref(ctx)
            .terminal_view_working_directories(ctx)
            .filter_map(|(id, cwd)| cwd.map(|c| (id, c)))
            .collect();
        let notebook_local_paths: Vec<(EntityId, String)> = pane_group
            .as_ref(ctx)
            .file_notebook_local_paths(ctx)
            .filter_map(|(id, cwd)| cwd.map(|c| (id, c)))
            .collect();
        let local_paths: Vec<(EntityId, String)> = notebook_local_paths;

        // Get the focused terminal ID to prioritize it in the repo_to_terminal map
        let focused_terminal_id = pane_group
            .as_ref(ctx)
            .active_session_view(ctx)
            .map(|terminal_view| terminal_view.id());

        self.working_directories_model.update(ctx, |model, ctx| {
            model.refresh_working_directories_for_pane_group(
                pane_group_id,
                terminal_cwds,
                local_paths,
                focused_terminal_id,
                ctx,
            );
        });
    }

    fn handle_file_tree_event(
        &mut self,
        pane_group: ViewHandle<PaneGroup>,
        event: &pane_group::Event,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            pane_group::Event::AppStateChanged => {
                ctx.dispatch_global_action("workspace:save_app", ());
                self.refresh_working_directories_for_pane_group(&pane_group, ctx);
                self.update_active_session(ctx);

                if FeatureFlag::DirectoryTabColors.is_enabled() {
                    if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|t| t.pane_group.id() == pane_group.id())
                    {
                        Self::sync_codebase_tab_color(tab, ctx);
                    }
                }
            }
            pane_group::Event::ActiveSessionChanged => {
                self.update_active_session(ctx);
                // ctx.notify();
            }
            pane_group::Event::Escape => {
                if self.current_workspace_state.is_resource_center_open {
                    self.current_workspace_state.is_resource_center_open = false;
                    ctx.notify()
                }
            }
            pane_group::Event::Exited { add_to_undo_stack } => {
                let tab = self.tabs.iter().position(|t| {
                    t.pane_group.id() == pane_group.id()
                        && t.pane_group.window_id(ctx) == pane_group.window_id(ctx)
                });

                if let Some(tab_index) = tab {
                    self.close_tab(tab_index, true, *add_to_undo_stack, ctx);
                }
            }
            pane_group::Event::PaneTitleUpdated => {
                self.update_window_title(ctx);
                ctx.notify();
            }
            pane_group::Event::ShowCommandSearch(options) => {
                self.show_command_search(options.filter, &options.init_content, ctx);
            }
            pane_group::Event::SendNotification {
                notification,
                pane_id,
            } => {
                // Right now, all notifications are block-specific, but in the future,
                // we might want to serialize a block-agnostic notification context.
                let window_id = ctx.window_id();
                let pane_group_id = pane_group.id();
                let pane_id = *pane_id;
                let notification_data = NotificationContext::BlockOrigin {
                    window_id,
                    pane_group_id,
                    pane_id,
                };

                if let Ok(notification_data_str) = serde_json::to_string(&notification_data) {
                    // Read the notification sound setting from SessionSettings
                    let play_sound = SessionSettings::as_ref(ctx)
                        .notifications
                        .play_notification_sound;

                    ctx.send_desktop_notification(
                        UserNotification::new_with_sound(
                            notification.title.to_string(),
                            notification.body.to_string(),
                            Some(notification_data_str),
                            play_sound,
                        ),
                        move |workspace, notification_error, ctx| {
                            // Log to sentry if unknown error
                            if let NotificationSendError::Other { error_message } =
                                &notification_error
                            {
                                log::error!(
                                    "Unknown error when sending notification. error_msg: {error_message}"
                                );
                            }

                            // Surface error to user
                            workspace.show_notification_error(
                                notification_error,
                                pane_group_id,
                                pane_id,
                                ctx,
                            );
                        },
                    )
                }
            }
            pane_group::Event::OpenSettings(section) => {
                self.show_settings_with_section(Some(*section), ctx);
            }
            pane_group::Event::OpenAutoReloadModal { .. } => {}
            #[cfg(not(target_family = "wasm"))]
            pane_group::Event::SyncInput(input_type) => {
                self.process_sync_event_for_all_synced_pane_groups(input_type, ctx);
            }
            pane_group::Event::TerminalViewStateChanged => {
                self.update_active_session(ctx);
                ctx.notify();
            }
            pane_group::Event::OpenFileInWarp { path, session } => {
                let _ = (path, session);
            }
            pane_group::Event::CDToDirectory { path } => {
                self.cd_to_directory(path.clone(), ctx);
            }
            pane_group::Event::OpenDirectoryInNewTab { path } => {
                self.open_directory_in_new_tab(path.clone(), ctx);
            }
            pane_group::Event::CloseSharedSessionPaneRequested { pane_id } => {
                if *SessionSettings::as_ref(ctx).should_confirm_close_session {
                    self.show_close_session_confirmation_dialog(
                        OpenDialogSource::ClosePane {
                            pane_group_id: pane_group.id(),
                            pane_id: *pane_id,
                        },
                        ctx,
                    );
                } else {
                    self.close_pane(pane_group.id(), *pane_id, ctx);
                }
            }
            pane_group::Event::MaximizePaneToggled => {
                ctx.notify();
            }
            pane_group::Event::FocusPaneGroup => {
                for (index, tab) in self.tabs.iter().enumerate() {
                    if tab.pane_group.id() == pane_group.id() {
                        self.activate_tab(index, ctx);
                        break;
                    }
                }
            }
            pane_group::Event::FocusPane { pane_to_focus } => {
                let Some(tab_index_to_focus) = self
                    .tabs
                    .iter()
                    .position(|tab| tab.pane_group.as_ref(ctx).has_pane_id(*pane_to_focus))
                else {
                    log::warn!("Could not find tab to focus pane");
                    return;
                };

                self.activate_tab(tab_index_to_focus, ctx);

                // TODO(CODE-266): This should focus the correct pane in the tab,
                // but for some reason application focus is not being moved to
                // the correct pane.
                if let Some(tab) = self.tabs.get_mut(tab_index_to_focus) {
                    tab.pane_group.update(ctx, |pane_group, ctx| {
                        if let Some(pane) = pane_group.pane_by_id(*pane_to_focus) {
                            pane.focus(ctx);
                        }
                    });
                }
            }
            pane_group::Event::FocusPaneInWorkspace { locator } => {
                // Focus an existing pane by its locator (used when avoiding duplicate file panes during undo close pane)
                self.focus_pane(*locator, ctx);
            }
            pane_group::Event::RepoChanged => {
                self.refresh_working_directories_for_pane_group(&pane_group, ctx);
                if FeatureFlag::DirectoryTabColors.is_enabled() {
                    if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|t| t.pane_group.id() == pane_group.id())
                    {
                        Self::sync_codebase_tab_color(tab, ctx);
                    }
                }
            }
            pane_group::Event::SwitchTabFocusAndMovePane {
                tab_idx,
                pane_id,
                hidden_pane_preview_direction,
            } => {
                #[cfg(feature = "local_fs")]
                let prefers_tabbed_editor_view = FeatureFlag::TabbedEditorView.is_enabled()
                    && *EditorSettings::as_ref(ctx)
                        .prefer_tabbed_editor_view
                        .value();

                #[cfg(not(feature = "local_fs"))]
                let prefers_tabbed_editor_view = false;

                // If a code pane is being dragged over a workspace tab with an existing code pane,
                // we don't allow it to be placed freely. Instead, it should be merged into the existing
                // code pane in the target tab (handled above in the OverTab case).
                let should_not_move_pane = false;

                // If we are already on this tab, then we should just make sure the pane is hidden.
                if self.active_tab_index() == *tab_idx || should_not_move_pane {
                    pane_group.update(ctx, |pane_group, ctx| {
                        pane_group.hide_pane_for_move(*pane_id, ctx)
                    });
                    return;
                };

                if let Some(pane) = pane_group.update(ctx, |pane_group, ctx| {
                    pane_group.remove_pane_for_move(pane_id, ctx)
                }) {
                    self.set_active_tab_index(*tab_idx, ctx);
                    self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
                        pane_group.add_pane_as_hidden(pane, *hidden_pane_preview_direction, ctx)
                    });
                }
            }
            pane_group::Event::UpdateHoveredTabIndex { tab_hover_index } => {
                self.hovered_tab_index = Some(*tab_hover_index);
                ctx.notify();
            }
            pane_group::Event::ClearHoveredTabIndex => self.hovered_tab_index = None,
            pane_group::Event::OpenPalette {
                mode,
                source,
                query,
            } => {
                let _ = query;
                self.open_palette(*mode, source.clone(), ctx);
            }
            pane_group::Event::ShowToast {
                message,
                flavor,
                pane_id,
            } => {
                let pane_group_id = pane_group.id();
                self.toast_stack.update(ctx, |toast_stack, ctx| {
                    let mut toast = DismissibleToast::new(message.clone(), *flavor);
                    if let Some(pane_id) = pane_id {
                        let locator = PaneViewLocator {
                            pane_group_id,
                            pane_id: *pane_id,
                        };
                        toast = toast.with_on_body_click(move |ctx| {
                            ctx.dispatch_typed_action(&WorkspaceAction::FocusPane(locator));
                        });
                    }
                    toast_stack.add_ephemeral_toast(toast, ctx);
                });
            }
            pane_group::Event::OpenThemeChooser => {
                self.show_theme_chooser_for_custom_theme(ctx);
            }
            pane_group::Event::OpenFilesPalette { source } => {
                self.open_palette(PaletteMode::Files, source.clone(), ctx);
            }
            pane_group::Event::ToggleLeftPanel {
                target_view,
                force_open,
            } => {
                let is_target_active =
                    self.left_panel_view
                        .read(ctx, |left_panel, _| match target_view {
                            LeftPanelTargetView::FileTree => left_panel.is_file_tree_active(),
                            LeftPanelTargetView::WarpDrive => false,
                        });

                if self.active_tab_pane_group().as_ref(ctx).left_panel_open && is_target_active {
                    // No-op if we are forcing the target to open when it is already active.
                    if !*force_open {
                        self.toggle_left_panel(ctx);
                    }
                } else {
                    if !self.active_tab_pane_group().as_ref(ctx).left_panel_open {
                        self.toggle_left_panel(ctx);
                    }
                    self.left_panel_view.update(ctx, |left_panel, ctx| {
                        let action = match target_view {
                            LeftPanelTargetView::FileTree => LeftPanelAction::ProjectExplorer,
                            LeftPanelTargetView::WarpDrive => return,
                        };
                        left_panel.handle_action_with_force_open(&action, *force_open, ctx);
                    });
                }
            }
            #[cfg(feature = "local_fs")]
            pane_group::Event::OpenFileWithTarget {
                path,
                target,
                line_col,
            } => {
                self.open_file_with_target(
                    path.clone(),
                    target.clone(),
                    *line_col,
                    CodeSource::Link {
                        path: path.clone(),
                        range_start: None,
                        range_end: None,
                    },
                    ctx,
                );
            }
            pane_group::Event::LeftPanelToggled { is_open } => {
                // Only handle visibility changes from the active pane group.
                if pane_group.id() == self.active_tab_pane_group().id() {
                    self.left_panel_open = *is_open;
                    self.left_panel_view.update(ctx, |left_panel, ctx| {
                        left_panel.on_left_panel_visibility_changed(*is_open, ctx);
                    });
                }
            }
            _ => {}
        }
    }

    fn handle_theme_chooser_event(
        &mut self,
        event: &ThemeChooserEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            ThemeChooserEvent::Click => self.focus_theme_chooser(ctx),
            ThemeChooserEvent::Close(mode) => {
                self.save_theme_chooser(mode, ctx);
            }
            ThemeChooserEvent::OpenThemeCreatorModal => {
                self.open_theme_creator_modal(ctx);
            }
            ThemeChooserEvent::OpenThemeDeletionModal(theme_kind) => {
                self.open_theme_deletion_modal(theme_kind.clone(), ctx);
            }
        };
    }

    fn show_command_search(
        &mut self,
        query_filter: Option<search::QueryFilter>,
        init_content: &InitContent,
        ctx: &mut ViewContext<Self>,
    ) {
        // Close all overlays including chip menus before opening command search
        self.close_all_overlays(ctx);

        if let Some(session_id) = self.active_session_id(ctx) {
            let active_input_handle = self.get_active_input_view_handle(ctx);

            let initial_query = match init_content {
                InitContent::FromInputBuffer => {
                    if let Some(input_handle) = &active_input_handle {
                        input_handle.read(ctx, |input, ctx| input.buffer_text(ctx))
                    } else {
                        "".to_owned()
                    }
                }
                InitContent::Custom(query) => query.to_owned(),
            };

            let session_context = active_input_handle.as_ref().and_then(|input_handle| {
                input_handle.read(ctx, |input, ctx| input.completion_session_context(ctx))
            });

            let menu_positioning = active_input_handle
                .as_ref()
                .map_or_else(MenuPositioning::default, |input_handle| {
                    input_handle.read(ctx, |input, ctx| input.menu_positioning(ctx))
                });

            if !self.current_workspace_state.is_command_search_open {}

            // Make sure we close any already-open input suggestions panel.
            if let Some(input_handle) = &active_input_handle {
                input_handle.update(ctx, |input, ctx| {
                    input.close_input_suggestions(false, ctx);
                });
            };

            self.current_workspace_state.is_command_search_open = true;
            self.command_search_view.update(ctx, |view, ctx| {
                view.reset_state(
                    session_id,
                    session_context,
                    initial_query,
                    query_filter,
                    menu_positioning,
                    ctx,
                );
            });

            let tip = match query_filter {
                Some(search::QueryFilter::History) => Tip::Action(TipAction::HistorySearch),
                _ => Tip::Action(TipAction::CommandSearch),
            };

            self.tips_completed.update(ctx, |tips_completed, ctx| {
                mark_feature_used_and_write_to_user_defaults(tip, tips_completed, ctx);
                ctx.notify();
            });

            ctx.notify();
            ctx.focus(&self.command_search_view);
        } else {
            log::error!("Command search keybinding triggered but no session is active!");
        }
    }

    fn get_active_input_view_handle(&self, app: &AppContext) -> Option<ViewHandle<Input>> {
        app.view(self.active_tab_pane_group())
            .active_session_view(app)
            .map(|terminal_view_handle| app.view(&terminal_view_handle).input().clone())
    }

    fn get_active_session_terminal_model(
        &self,
        app: &AppContext,
    ) -> Option<Arc<FairMutex<TerminalModel>>> {
        self.active_tab_pane_group()
            .as_ref(app)
            .active_session_terminal_model(app)
    }

    /// Replace the active terminal input's buffer with `contents`. Adds to the
    /// undo stack.
    pub fn set_active_terminal_input_contents_and_focus_app(
        &mut self,
        contents: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_id = ctx.window_id();

        if let Some(active_input_view_handle) = self.get_active_input_view_handle(ctx) {
            active_input_view_handle.update(ctx, |input_view, input_ctx| {
                input_view.replace_buffer_content(contents, input_ctx);
            });

            ctx.windows().show_window_and_focus_app(window_id);

            ctx.notify();
        } else {
            log::error!("workspace::view::fill_input(): no active input view handle to fill");
        }
    }

    /// Insert the given command that should open a subshell. And set a flag that we should
    /// automatically bootstrap AKA "warpify" that subshell if we support it. No-op if there is
    /// no active terminal session.
    pub fn insert_subshell_command_and_bootstrap_if_supported(
        &mut self,
        command: &str,
        shell: Option<ShellType>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.active_tab_pane_group()
            .update(ctx, |pane_group_view, pane_group_ctx| {
                pane_group_view
                    .active_session_view(pane_group_ctx)
                    .map(|terminal_view_handle| {
                        terminal_view_handle.update(
                            pane_group_ctx,
                            |terminal_view, terminal_view_ctx| {
                                terminal_view.insert_subshell_command_and_bootstrap_if_supported(
                                    command,
                                    shell,
                                    terminal_view_ctx,
                                );
                            },
                        )
                    })
            });
    }

    /// Update the active session model state.
    fn update_active_session(&mut self, ctx: &mut ViewContext<Self>) {
        let pane_group_handle = self.active_tab_pane_group();
        let file_tree_and_global_search_are_enabled = {
            #[cfg(feature = "local_fs")]
            {
                Self::should_enable_file_tree_and_global_search_for_pane_group(
                    self.active_tab_pane_group().as_ref(ctx),
                )
            }

            #[cfg(not(feature = "local_fs"))]
            {
                false
            }
        };

        // Update working directories for the current pane group
        let pane_group_handle = pane_group_handle.clone();
        self.refresh_working_directories_for_pane_group(&pane_group_handle, ctx);

        if let Some(terminal_handle) = pane_group_handle.as_ref(ctx).active_session_view(ctx) {
            #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
            let (session, path_if_local, is_local, is_wsl_session, session_id, pwd) =
                terminal_handle.read(ctx, |terminal, ctx| {
                    let active_session_id = terminal.active_block_session_id();
                    let session = active_session_id
                        .and_then(|id| terminal.sessions_model().as_ref(ctx).get(id));
                    let path_if_local = terminal.active_session_path_if_local(ctx);
                    let is_local = terminal.active_session_is_local(ctx);
                    let is_wsl_session = session.as_ref().map(|s| s.is_wsl()).unwrap_or(false);
                    let pwd = terminal.pwd();
                    (
                        session,
                        path_if_local,
                        is_local,
                        is_wsl_session,
                        active_session_id,
                        pwd,
                    )
                });

            let window_id = ctx.window_id();
            let working_directory_clone = path_if_local.clone();
            let path_if_local_clone = path_if_local.clone();
            ActiveSession::handle(ctx).update(ctx, |active_session, ctx| {
                active_session.set_session_state(
                    window_id,
                    session,
                    path_if_local_clone.clone(),
                    Some(terminal_handle.id()),
                    ctx,
                );
            });

            let is_remote = false;
            let is_unsupported_session = is_wsl_session;
            let has_remote_server = false;

            let enablement = CodingPanelEnablementState::from_session_env(
                file_tree_and_global_search_are_enabled,
                is_remote,
                is_unsupported_session,
                has_remote_server,
            );

            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.update_coding_panel_enablement(enablement, ctx);
            });

            #[cfg(feature = "local_fs")]
            {}
        } else {
            let enablement = CodingPanelEnablementState::from_session_env(
                file_tree_and_global_search_are_enabled,
                false,
                false,
                false,
            );

            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.update_coding_panel_enablement(enablement, ctx);
            });

            #[cfg(feature = "local_fs")]
            {}
        }
    }

    /// Runs a cloud workflow in whichever input is currently active.
    /// Focus and return the active terminal input. If there is no active terminal input (either
    /// because a command is running or because there are no terminal panes), this may create a new
    /// terminal pane according to the [`UnavailableTerminalBehavior`].
    /// Opens the LSP log file in a new terminal pane using `tail -f`.
    /// Runs a workflow in whichever terminal input is currently active.
    /// No-ops if the active session is long-running.
    /// Inserts given command into active Input Editor, optionally replacing the current buffer. No-ops if
    /// there is no active terminal pane open, with an input box active.
    fn insert_in_input(
        &mut self,
        content: &str,
        replace_buffer: bool,
        should_submit: bool,
        ensure_agent_mode: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let active_input_handle = self.get_active_input_view_handle(ctx);

        if let Some(active_input_handle) = active_input_handle {
            active_input_handle.update(ctx, |input, ctx| {
                if replace_buffer {
                    input.replace_buffer_content(content, ctx);
                } else {
                    input.append_to_buffer(content, ctx);
                }

                if should_submit {
                    input.input_enter(ctx);
                }
                ctx.notify();
            });
        }
    }

    fn handle_command_search_event(
        &mut self,
        event: &CommandSearchEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        use CommandSearchEvent::*;
        let Some(active_input_handle) = self.get_active_input_view_handle(ctx) else {
            return;
        };
        match event {
            Close {
                query: query_when_closed,
                filter: filter_when_closed,
            } => {
                self.current_workspace_state.is_command_search_open = false;

                active_input_handle.update(ctx, |input, ctx| {
                    input.handle_command_search_closed(query_when_closed, filter_when_closed, ctx);
                    ctx.notify();
                });

                ctx.notify();
            }
            Blur => {
                self.current_workspace_state.is_command_search_open = false;
                ctx.notify();
            }
            ItemSelected { query, payload } => {
                use CommandSearchItemAction::*;
                match payload.as_ref() {
                    AcceptHistory(AcceptedHistoryItem {
                        command,
                        linked_workflow_data,
                    }) => {
                        // Switch to shell input mode so the history command is
                        // treated as a shell command, not an agent prompt.
                        active_input_handle.update(ctx, |input, ctx| {
                            input.set_input_mode_terminal(false, ctx);
                            input.replace_buffer_content(command.as_str(), ctx);
                            input.focus_input_box(ctx);
                        });

                        let _ = linked_workflow_data;
                    }
                    ExecuteHistory(command) => {
                        active_input_handle.update(ctx, |input, ctx| {
                            input.try_execute_command(command.as_str(), ctx);
                            ctx.notify();
                        });
                    }
                    _ => {}
                }
            }
            Resize => {
                // A resize of universal search should write the app snapshot to sqlite.
                ctx.dispatch_global_action("workspace:save_app", ());
            }
        }
    }

    fn should_keep_theme(system_theme: SystemTheme, ctx: &mut ViewContext<Self>) -> bool {
        if system_theme == ctx.system_theme() {
            let respect_system_theme = respect_system_theme(ThemeSettings::as_ref(ctx));
            if let RespectSystemTheme::On { .. } = respect_system_theme {
                return true;
            }
        }
        false
    }

    fn save_theme_chooser(&mut self, mode: &ThemeChooserMode, ctx: &mut ViewContext<Self>) {
        let keep_theme = match mode {
            ThemeChooserMode::SystemAgnostic => true,
            ThemeChooserMode::SystemLight => Workspace::should_keep_theme(SystemTheme::Light, ctx),
            ThemeChooserMode::SystemDark => Workspace::should_keep_theme(SystemTheme::Dark, ctx),
        };
        if keep_theme {
            self.keep_theme(ctx);
        } else {
            self.revert_theme(ctx);
        }
    }

    fn revert_theme(&mut self, ctx: &mut ViewContext<Self>) {
        AppearanceManager::handle(ctx).update(ctx, |appearance_manager, ctx| {
            appearance_manager.clear_transient_theme(ctx);
        });
        self.current_workspace_state.is_theme_chooser_open = false;
        self.previous_theme = None;
        ctx.notify();
    }

    fn keep_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_chooser_open = false;
        self.previous_theme = None;
        ctx.notify();
    }

    fn apply_update(&mut self, ctx: &mut ViewContext<Self>) {
        self.close_tab_bar_overflow_menu(ctx);
    }

    fn download_new_version(&mut self, ctx: &mut ViewContext<Self>) {
        self.close_tab_bar_overflow_menu(ctx);
    }

    fn active_session_ps1_grid_info(&self, app: &AppContext) -> Option<(BlockGrid, SizeInfo)> {
        self.get_active_session_terminal_model(app)
            .and_then(|model| {
                let lock = model.lock();
                lock.prompt_grid()
                    .cloned()
                    .zip(Some(*lock.block_list().size()))
            })
            .or_else(|| {
                (0..self.tabs.len()).find_map(|i| {
                    self.get_pane_group_view(i)?
                        .as_ref(app)
                        .active_session_terminal_model(app)
                        .and_then(|model| {
                            let lock = model.lock();
                            lock.prompt_grid()
                                .cloned()
                                .zip(Some(*lock.block_list().size()))
                        })
                })
            })
    }

    fn show_close_session_confirmation_dialog(
        &mut self,
        source: OpenDialogSource,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_session_confirmation_dialog
            .update(ctx, |view, _| {
                view.set_open_confirmation_source(source);
            });
        self.current_workspace_state
            .is_close_session_confirmation_dialog_open = true;
        ctx.focus(&self.close_session_confirmation_dialog);
        ctx.notify();
    }

    pub fn show_native_modal(
        &mut self,
        dialog: AlertDialogWithCallbacks<AppModalCallback>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.native_modal.update(ctx, |view, ctx| {
            view.set_alert_dialog(dialog);
            ctx.notify();
        });
        self.current_workspace_state.is_native_quit_modal_open = true;
        ctx.focus(&self.native_modal);
        ctx.notify();
    }

    fn handle_native_modal_event(&mut self, event: &NativeModalEvent, ctx: &mut ViewContext<Self>) {
        match event {
            NativeModalEvent::Close => {
                self.current_workspace_state.is_native_quit_modal_open = false;
                ctx.notify();
            }
        }
    }

    /// Mock pressing a button on the native quit modal. This function has an unusual signature so
    /// that the workspace view is not borrowed while the button press is handled.
    #[cfg(any(test, feature = "integration_tests"))]
    pub fn press_native_modal_button(
        handle: &ViewHandle<Self>,
        button_index: usize,
        app: &mut AppContext,
    ) {
        use super::native_modal::NativeModalAction;
        let modal_handle = handle.as_ref(app).native_modal.clone();
        modal_handle.update(app, |modal, ctx| {
            modal.handle_action(&NativeModalAction::TriggerButtonCallback(button_index), ctx);
        });
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn is_native_quit_modal_open(&self, ctx: &AppContext) -> bool {
        self.current_workspace_state.is_native_quit_modal_open
            && self.native_modal.as_ref(ctx).has_alert_dialog()
    }

    fn show_settings(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_settings_with_section(None, ctx);
    }

    fn show_settings_with_section(
        &mut self,
        section: Option<SettingsSection>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);
        self.open_settings_pane(section, None, ctx);
    }

    fn show_settings_with_search(
        &mut self,
        search_query: &str,
        section: Option<SettingsSection>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.close_all_overlays(ctx);
        self.open_settings_pane(section, Some(search_query), ctx);
    }

    /// Opens the team settings page and fills the invite field with the given email. This is used when linking directing to
    /// settings with the intent of inviting a user.
    pub fn show_team_settings_page_with_email_invite(
        &mut self,
        email_invite: Option<&String>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.show_settings_with_section(Some(SettingsSection::Teams), ctx);

        self.settings_pane.update(ctx, |view, ctx| {
            view.open_teams_page_email_invite(email_invite, ctx);
        });
    }

    /// Shows the theme chooser so the user can change the active theme.
    pub fn show_theme_chooser_for_active_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_theme_chooser(Some(ThemeChooserMode::for_active_theme(ctx)), ctx)
    }

    pub fn show_theme_chooser_for_custom_theme(&mut self, ctx: &mut ViewContext<Self>) {
        self.show_theme_chooser(None, ctx)
    }

    /// Shows the theme chooser so the user can change a specific theme.
    pub fn show_theme_chooser(
        &mut self,
        theme_chooser_mode: Option<ThemeChooserMode>,
        ctx: &mut ViewContext<Self>,
    ) {
        let current_theme = active_theme_kind(ThemeSettings::as_ref(ctx), ctx);

        self.close_tab_bar_overflow_menu(ctx);

        self.current_workspace_state.close_all_left_panels();

        // When showing the theme chooser, let's close the command palette
        // in case it was used to open the theme chooser.
        self.current_workspace_state.is_palette_open = false;
        self.current_workspace_state.is_ctrl_tab_palette_open = false;
        self.previous_workspace_state = Some(self.current_workspace_state);
        self.current_workspace_state.is_ai_assistant_panel_open = false;
        self.current_workspace_state.is_theme_chooser_open = true;

        self.previous_theme = Some(current_theme);

        self.theme_chooser_view.update(ctx, |view, ctx| {
            view.record_open_theme(ctx);
            if let Some(theme_chooser_mode) = theme_chooser_mode {
                view.select_theme(theme_chooser_mode.into_theme_kind(ctx), ctx);
                view.set_mode(theme_chooser_mode);
            } else {
                view.reload_and_set_latest_theme(ctx);
            }
        });

        self.focus_theme_chooser(ctx);
    }

    pub fn show_keyboard_settings(
        &mut self,
        keybinding_name: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.show_settings_with_section(Some(SettingsSection::Keybindings), ctx);
        if let Some(keybinding_name) = keybinding_name {
            self.settings_pane.update(ctx, |settings_pane, ctx| {
                settings_pane.search_for_keybinding(keybinding_name, ctx);
            });
        }
    }

    pub fn is_theme_chooser_open(&self) -> bool {
        self.current_workspace_state.is_theme_chooser_open
    }

    /// Returns whether the workspace is currently showing a settings file
    /// error banner (i.e. settings_file_error is set and not dismissed).
    #[cfg(feature = "integration_tests")]
    pub fn has_settings_file_error_banner(&self) -> bool {
        self.settings_file_error.is_some() && !self.settings_error_banner_dismissed
    }

    fn increase_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_terminal_font_size(FONT_SIZE_INCREMENT, ctx);
    }

    fn decrease_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_terminal_font_size(-FONT_SIZE_INCREMENT, ctx);
    }

    fn reset_font_size(&mut self, ctx: &mut ViewContext<Self>) {
        self.set_terminal_font_size(MonospaceFontSize::default_value(), ctx);
    }

    fn increase_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_zoom(true /* increase */, ctx);
    }

    fn decrease_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        self.adjust_zoom(false /* increase */, ctx);
    }

    fn reset_zoom(&mut self, ctx: &mut ViewContext<Self>) {
        WindowSettings::handle(ctx).update(ctx, |window_settings, ctx| {
            report_if_error!(window_settings
                .zoom_level
                .set_value(ZoomLevel::default_value(), ctx));
        });
    }

    fn adjust_zoom(&mut self, increase: bool, ctx: &mut ViewContext<Self>) {
        let current_zoom = *WindowSettings::as_ref(ctx).zoom_level.value();
        let Some(current_index) = crate::window_settings::ZoomLevel::VALUES
            .iter()
            .position(|zoom| *zoom == current_zoom)
        else {
            return;
        };

        let next_index = if increase {
            (current_index + 1).min(crate::window_settings::ZoomLevel::VALUES.len() - 1)
        } else {
            current_index.saturating_sub(1)
        };

        WindowSettings::handle(ctx).update(ctx, |window_settings, ctx| {
            report_if_error!(window_settings
                .zoom_level
                .set_value(crate::window_settings::ZoomLevel::VALUES[next_index], ctx));
        });
    }

    fn adjust_terminal_font_size(&mut self, font_size_delta: f32, ctx: &mut ViewContext<Self>) {
        let appearance = Appearance::as_ref(ctx);
        let new_font_size = (appearance.monospace_font_size() + font_size_delta)
            .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.set_terminal_font_size(new_font_size, ctx);
    }

    fn set_terminal_font_size(&mut self, new_font_size: f32, ctx: &mut ViewContext<Self>) {
        FontSettings::handle(ctx).update(ctx, |font_settings, ctx| {
            report_if_error!(font_settings
                .monospace_font_size
                .set_value(new_font_size, ctx));
        });
    }

    fn toggle_mouse_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        let prev_mouse_reporting_enabled =
            AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
                let prev_mouse_reporting_enabled = *reporting.mouse_reporting_enabled.value();
                reporting
                    .mouse_reporting_enabled
                    .set_value(!prev_mouse_reporting_enabled, ctx)
                    .expect("MouseReportingEnabled failed to serialize");
                prev_mouse_reporting_enabled
            });

        let verb = if prev_mouse_reporting_enabled {
            "disabled"
        } else {
            "enabled"
        };
        let mut message = format!("You {verb} mouse reporting.");
        if let Some(keystroke) =
            keybinding_name_to_keystroke("workspace:toggle_mouse_reporting", ctx)
        {
            let _ = write!(message, " Press {} to undo.", keystroke.displayed());
        }

        self.toast_stack.update(ctx, |view, ctx| {
            let new_toast = DismissibleToast::default(message);
            view.add_ephemeral_toast(new_toast, ctx);
        });
    }

    fn toggle_scroll_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
            reporting
                .scroll_reporting_enabled
                .toggle_and_save_value(ctx)
                .expect("ScrollReportingEnabled failed to serialize");
        });
    }

    fn toggle_focus_reporting(&mut self, ctx: &mut ViewContext<Self>) {
        AltScreenReporting::handle(ctx).update(ctx, |reporting, ctx| {
            reporting
                .focus_reporting_enabled
                .toggle_and_save_value(ctx)
                .expect("FocusReportingEnabled failed to serialize");
        });
    }

    /// Handle an event from the referral theme status model, showing the reward modal if necessary
    /// This listens for changes to keybindings and keeps the cached versions up-to-date in our
    /// tooltips.
    fn handle_keybinding_changed(
        &mut self,
        event: &KeybindingChangedEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match &event {
            KeybindingChangedEvent::BindingChanged {
                binding_name,
                new_trigger: new_trigger_option,
            } => self
                .cached_keybindings
                .entry(binding_name.to_owned())
                .and_modify(|keystroke| {
                    *keystroke = new_trigger_option.as_ref().map(|key| key.displayed())
                }),
        };
        ctx.notify()
    }

    fn handle_window_state_change(&mut self, event: &StateEvent, ctx: &mut ViewContext<Self>) {
        match &event {
            StateEvent::ValueChanged { current, previous } => {
                // Re-render if fullscreen state for active window has changed.
                if current.is_active_window_fullscreen != previous.is_active_window_fullscreen {
                    ctx.notify();
                } else if WindowManager::did_window_change_focus(self.window_id, current, previous)
                {
                    // Re-render if this window's focus state has changed.
                    ctx.notify();
                } else if current.stage != previous.stage {
                    // Re-render if the app's focus state has changed (Active/Inactive)
                    // This ensures dimming updates properly when the app gains/loses focus
                    ctx.notify();
                }
            }
        };
    }

    #[cfg(not(target_family = "wasm"))]
    /// Opens a new tab and enters agent view with a prompt from a Linear deeplink.
    fn focus_active_tab(&mut self, ctx: &mut ViewContext<Self>) {
        self.active_tab_pane_group().update(ctx, |tab, ctx| {
            tab.focus(ctx);
        })
    }

    fn focus_theme_chooser(&self, ctx: &mut ViewContext<Self>) {
        ctx.focus(&self.theme_chooser_view);
        ctx.notify();
    }

    fn open_theme_creator_modal(&mut self, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_creator_modal_open = true;
        ctx.focus(&self.theme_creator_modal);
        ctx.notify();
    }

    fn open_theme_deletion_modal(&mut self, theme_kind: ThemeKind, ctx: &mut ViewContext<Self>) {
        self.current_workspace_state.is_theme_deletion_modal_open = true;
        self.theme_deletion_modal
            .update(ctx, |theme_deletion_modal, ctx| {
                theme_deletion_modal.set_theme_kind(theme_kind, ctx);
            });
        ctx.focus(&self.theme_deletion_modal);
        ctx.notify();
    }

    /// Opens the workflow modal in the provided space and folder with no existing content (i.e. a new workflow modal).
    /// Opens the workflow from a given [`CloudWorkflow`]'s server ID.
    /// Opens the workflow using a mocked [`Workflow`] object as the base
    /// Opens the workflow for create with a prepopulated command specified
    fn render_tab_in_tab_bar(
        &self,
        tab_index: usize,
        tab_bar_state: TabBarState,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let tab = &self.tabs[tab_index];
        let close_button_position = if FeatureFlag::TabCloseButtonOnLeft.is_enabled() {
            TabSettings::as_ref(ctx).close_button_position
        } else {
            TabCloseButtonPosition::default()
        };

        let is_drag_target = self
            .hovered_tab_index
            .as_ref()
            .is_some_and(|hovered_index| match hovered_index {
                TabBarHoverIndex::OverTab(idx) => *idx == tab_index,
                TabBarHoverIndex::BeforeTab(_) => false,
            });

        TabComponent::new(
            tab_index,
            tab_bar_state,
            tab,
            self.tab_rename_editor.clone(),
            close_button_position,
            is_drag_target,
            ctx,
        )
        .build()
        .finish()
    }

    fn render_left_toggle_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let (is_active, tooltip_text, action, keybinding_name, save_position_id) = {
            let tooltip = if self.left_panel_views.len() <= 1 {
                match self
                    .left_panel_views
                    .first()
                    .copied()
                    .unwrap_or(ToolPanelView::ProjectExplorer)
                {
                    ToolPanelView::ProjectExplorer => "Project explorer",
                    ToolPanelView::GlobalSearch { .. } => "Global search",
                    ToolPanelView::ProjectExplorer => "Warp Drive",
                }
            } else {
                "Tools panel"
            };
            (
                self.active_tab_pane_group().as_ref(ctx).left_panel_open,
                tooltip,
                WorkspaceAction::ToggleLeftPanel,
                "workspace:toggle_left_panel",
                "workspace:toggle_left_panel",
            )
        };

        SavePosition::new(
            Container::new(
                Align::new(
                    self.render_tab_bar_icon_button(
                        appearance,
                        icons::Icon::Menu,
                        &self.mouse_states.left_panel_icon,
                        action,
                        tooltip_text.to_string(),
                        keybinding_name_to_display_string(keybinding_name, ctx),
                        is_active,
                        false,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
            save_position_id,
        )
        .finish()
    }

    #[allow(dead_code)]
    fn render_tools_panel_button(
        &self,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let is_active = self.active_tab_pane_group().as_ref(ctx).left_panel_open;

        let tooltip_text = if self.left_panel_views.len() <= 1 {
            match self
                .left_panel_views
                .first()
                .copied()
                .unwrap_or(ToolPanelView::ProjectExplorer)
            {
                ToolPanelView::ProjectExplorer => "Project explorer",
                ToolPanelView::GlobalSearch { .. } => "Global search",
            }
        } else {
            "Tools panel"
        };

        SavePosition::new(
            Container::new(
                Align::new(
                    self.render_tab_bar_icon_button(
                        appearance,
                        icons::Icon::Tool2,
                        &self.mouse_states.tools_panel_icon,
                        WorkspaceAction::ToggleLeftPanel,
                        tooltip_text.to_string(),
                        keybinding_name_to_display_string("workspace:toggle_left_panel", ctx),
                        is_active,
                        false,
                    )
                    .finish(),
                )
                .finish(),
            )
            .finish(),
            "workspace:toggle_left_panel",
        )
        .finish()
    }

    fn should_enable_file_tree_and_global_search_for_pane_group(pane_group: &PaneGroup) -> bool {
        pane_group
            .pane_ids()
            .filter(|id| !pane_group.is_pane_hidden_for_close(*id))
            .any(|id| {
                id.is_terminal_pane()
                    || id.is_file_pane()
                    || id.is_code_pane()
                    || id.is_code_diff_pane()
            })
    }

    /// Renders an invisible rect for detecting hovers over the tab bar.
    fn render_tab_bar_hover_area(&self) -> Box<dyn Element> {
        self.render_tab_bar_hoverable(
            ConstrainedBox::new(Empty::new().finish())
                .with_height(TAB_BAR_HOVER_HEIGHT)
                .finish(),
        )
    }

    /// Renders the provided content wrapped in the tab bar hover behavior.
    fn render_tab_bar_hoverable(&self, content: Box<dyn Element>) -> Box<dyn Element> {
        Hoverable::new(self.tab_bar_hover_state.clone(), |_| content)
            .with_hover_out_delay(Duration::from_millis(500))
            .on_hover(|_is_hovered, ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::SyncTrafficLights);
            })
            .finish()
    }

    fn render_tab_hover_indicator(&self, appearance: &Appearance) -> Box<dyn Element> {
        ConstrainedBox::new(
            Rect::new()
                .with_background(appearance.theme().accent())
                .finish(),
        )
        .with_height(32.)
        .with_width(4.)
        .finish()
    }

    #[allow(dead_code)]
    fn render_title_bar_search_bar(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        let text_color = theme.sub_text_color(theme.background());

        Hoverable::new(
            self.mouse_states.title_bar_search_bar.clone(),
            |mouse_state| {
                let row = Flex::row()
                    .with_cross_axis_alignment(CrossAxisAlignment::Center)
                    .with_spacing(10.)
                    .with_child(
                        ConstrainedBox::new(
                            icons::Icon::Search.to_warpui_icon(text_color).finish(),
                        )
                        .with_width(16.)
                        .with_height(16.)
                        .finish(),
                    )
                    .with_child(
                        Shrinkable::new(
                            1.,
                            Text::new_inline(
                                "Search sessions, agents, files...",
                                appearance.ui_font_family(),
                                14.,
                            )
                            .with_color(text_color.into())
                            .with_clip(ClipConfig::ellipsis())
                            .finish(),
                        )
                        .finish(),
                    )
                    .finish();

                ConstrainedBox::new(
                    Container::new(row)
                        .with_background(if mouse_state.is_hovered() {
                            internal_colors::fg_overlay_2(theme)
                        } else {
                            internal_colors::fg_overlay_1(theme)
                        })
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                        .with_padding_left(16.)
                        .with_padding_right(16.)
                        .with_padding_top(4.)
                        .with_padding_bottom(4.)
                        .finish(),
                )
                .with_width(TITLE_BAR_SEARCH_BAR_MAX_WIDTH)
                .finish()
            },
        )
        .with_cursor(Cursor::PointingHand)
        .on_click(|ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::OpenPalette {
                mode: PaletteMode::Command,
                source: PaletteSource::TitleBarSearchBar,
                query: None,
            });
        })
        .finish()
    }

    fn render_tab_bar_contents(
        &self,
        hover_fixed_width: Option<f32>,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let mut tab_bar = Flex::row().with_cross_axis_alignment(CrossAxisAlignment::Center);

        // Simplified mode for viewing Warp Drive objects, shared sessions, or conversation transcripts on WASM
        #[cfg(target_family = "wasm")]
        if let Some(content_type) = self.get_simplified_wasm_tab_bar_content(ctx) {
            // Use MainAxisAlignment::SpaceBetween and expand to fill width
            tab_bar = tab_bar
                .with_main_axis_alignment(MainAxisAlignment::SpaceBetween)
                .with_main_axis_size(MainAxisSize::Max);
            let bg_color = blended_colors::neutral_1(appearance.theme());

            // Left: Warp logo - clickable to link to warp.dev
            let warp_logo = Hoverable::new(self.mouse_states.warp_logo.clone(), |_state| {
                ConstrainedBox::new(
                    warp_core::ui::Icon::Warp
                        .to_warpui_icon(appearance.theme().foreground())
                        .finish(),
                )
                .with_height(24.)
                .with_width(24.)
                .finish()
            })
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::OpenLink("https://warp.dev".to_owned()));
            })
            .with_cursor(Cursor::PointingHand)
            .finish();
            tab_bar.add_child(warp_logo);

            // Right: Info button + "View all cloud runs" button (for ambient agent sessions) + "Open in Warp" button
            let mut right_row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);

            // Extract task_id from conversation transcripts and shared sessions
            let task_id = match content_type {
                SimplifiedWasmTabBarContent::ConversationTranscript { task_id }
                | SimplifiedWasmTabBarContent::SharedSession { task_id } => task_id,
                SimplifiedWasmTabBarContent::WarpDriveObject => None,
            };

            // Show info button for conversation transcripts and shared sessions (if there's content to display)
            let should_show_info_button =
                !matches!(content_type, SimplifiedWasmTabBarContent::WarpDriveObject)
                    && self
                        .active_tab_pane_group()
                        .as_ref(ctx)
                        .focused_session_view(ctx)
                        .is_some_and(|view| {
                            Self::should_show_conversation_details_panel(&view, ctx)
                        });

            if should_show_info_button {
                right_row.add_child(
                    Container::new(ChildView::new(&self.transcript_info_button).finish())
                        .with_margin_right(8.)
                        .finish(),
                );

                // Add "View all cloud runs" button when task_id exists (with 4px gap)
                if task_id.is_some() {
                    right_row.add_child(
                        Container::new(ChildView::new(&self.view_cloud_runs_button).finish())
                            .with_margin_right(4.)
                            .finish(),
                    );
                }
            }

            // Hide "Open in Warp" button on mobile devices
            if !warpui::platform::wasm::is_mobile_device() {
                right_row.add_child(ChildView::new(&self.open_in_warp_button).finish());
            }
            tab_bar.add_child(right_row.finish());

            return Container::new(tab_bar.finish())
                .with_background_color(bg_color)
                .with_border(
                    Border::bottom(1.0)
                        .with_border_fill(blended_colors::neutral_2(appearance.theme())),
                )
                .with_padding_left(24.)
                .with_padding_right(24.)
                .with_padding_top(4.)
                .with_padding_bottom(4.)
                .finish();
        }

        // Render config-driven left-side toolbar buttons.
        let knowledge_center_closed = true;
        let config = TabSettings::as_ref(ctx)
            .header_toolbar_chip_selection
            .clone();
        if knowledge_center_closed && !self.is_theme_chooser_open() {
            let left_toolbar_buttons = config
                .left_items()
                .into_iter()
                .filter_map(|item| self.render_header_toolbar_button(&item, appearance, ctx))
                .collect::<Vec<_>>();
            let left_toolbar_button_count = left_toolbar_buttons.len();
            for (index, button) in left_toolbar_buttons.into_iter().enumerate() {
                let is_last_left_toolbar_button = index + 1 == left_toolbar_button_count;
                if is_last_left_toolbar_button {
                    tab_bar.add_child(Container::new(button).with_margin_right(8.).finish());
                } else {
                    tab_bar.add_child(button);
                }
            }
        }

        {
            // Copy from our saved tab_bar_state to ensure all tabs get rendered with the same state
            let active_tab_index = if FeatureFlag::AgentManagementView.is_enabled()
                && self.current_workspace_state.is_agent_management_view_open
            {
                None
            } else {
                Some(self.active_tab_index)
            };

            let tab_bar_state = TabBarState {
                tab_count: self.tabs.len(),
                active_tab_index,
                is_any_tab_renaming: self.current_workspace_state.is_tab_being_renamed(),
                is_any_tab_dragging: self.current_workspace_state.is_tab_being_dragged,
                hover_fixed_width,
            };

            for i in 0..self.tabs.len() {
                // If we are hovered between two tabs, show the drop hover indicator
                if self.hovered_tab_index.as_ref().is_some_and(
                    |hovered_index| match hovered_index {
                        TabBarHoverIndex::BeforeTab(idx) => i == *idx,
                        TabBarHoverIndex::OverTab(_) => false,
                    },
                ) {
                    tab_bar.add_child(self.render_tab_hover_indicator(appearance));
                }
                tab_bar.add_child(self.render_tab_in_tab_bar(i, tab_bar_state, ctx));
            }

            // Fencepost problem - add the indicator at the end if needed
            if self
                .hovered_tab_index
                .as_ref()
                .is_some_and(|hovered_index| match hovered_index {
                    TabBarHoverIndex::BeforeTab(idx) => self.tabs.len() == *idx,
                    TabBarHoverIndex::OverTab(_) => false,
                })
            {
                tab_bar.add_child(self.render_tab_hover_indicator(appearance));
            }

            if ContextFlag::CreateNewSession.is_enabled() {
                tab_bar.add_child(self.render_new_session_button(ctx));
            }
        }

        // Placeholder to make sure the flex row expands across the entire width of the app.
        tab_bar.add_child(Shrinkable::new(0.5, Empty::new().finish()).finish());

        self.add_configurable_right_side_tab_bar_controls(
            &mut tab_bar,
            &config,
            false,
            appearance,
            ctx,
        );

        let left_padding = self.compute_tab_bar_left_padding(ctx);

        EventHandler::new(
            Container::new(tab_bar.finish())
                .with_padding_left(left_padding)
                .with_padding_right(TAB_BAR_PADDING_RIGHT)
                .finish(),
        )
        .on_right_mouse_down(|ctx, _, position| {
            ctx.dispatch_typed_action(WorkspaceAction::ShowHeaderToolbarContextMenu { position });
            DispatchEventResult::StopPropagation
        })
        .finish()
    }

    /// Renders a single header toolbar button for the given item kind.
    /// Returns `None` if the item is not currently available.
    /// The button is wrapped with a right-click handler that opens the
    /// toolbar configurator.
    fn render_header_toolbar_button(
        &self,
        item: &HeaderToolbarItemKind,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !item.is_available(ctx) {
            return None;
        }
        let inner = match item {
            HeaderToolbarItemKind::TabsPanel => self.render_left_toggle_button(appearance, ctx),
            HeaderToolbarItemKind::ToolsPanel => {
                if self.left_panel_views.is_empty() {
                    return None;
                }
                self.render_left_toggle_button(appearance, ctx)
            }
        };
        Some(
            Container::new(
                EventHandler::new(inner)
                    .on_right_mouse_down(|ctx, _, position| {
                        ctx.dispatch_typed_action(WorkspaceAction::ShowHeaderToolbarContextMenu {
                            position,
                        });
                        DispatchEventResult::StopPropagation
                    })
                    .finish(),
            )
            .with_margin_left(TAB_BAR_ICON_PADDING)
            .finish(),
        )
    }

    /// Renders the notifications mailbox button (extracted for reuse from
    /// add_right_side_tab_bar_controls).
    /// Adds the configurable right-side toolbar items plus the fixed controls
    /// (update pill, offline indicator, avatar, etc.) that are not configurable.
    fn add_configurable_right_side_tab_bar_controls(
        &self,
        target: &mut Flex,
        config: &crate::workspace::tab_settings::HeaderToolbarChipSelection,
        _is_web_anonymous_user: bool,
        appearance: &Appearance,
        ctx: &AppContext,
    ) {
        let is_online = NetworkStatus::as_ref(ctx).is_online();

        if !is_online {
            target.add_child(
                Container::new(self.render_offline_button(appearance))
                    .with_margin_right(4.)
                    .finish(),
            );
        }

        for item in config.right_items() {
            if let Some(button) = self.render_header_toolbar_button(&item, appearance, ctx) {
                target.add_child(button);
            }
        }

        target.add_child(
            Container::new(self.render_settings_button(appearance))
                .with_margin_left(TAB_BAR_PADDING_LEFT)
                .finish(),
        );

        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let traffic_light_data = traffic_light_data(ctx, self.window_id);
        if let Some(traffic_light_data) = traffic_light_data.as_ref() {
            let right_panel_open = self.current_workspace_state.is_right_panel_open();
            let should_reserve_right_traffic_light_space = !right_panel_open;

            if traffic_light_data.side == TrafficLightSide::Right
                && should_reserve_right_traffic_light_space
            {
                target.add_child(
                    ConstrainedBox::new(Empty::new().finish())
                        .with_width(traffic_light_data.width(zoom_factor))
                        .finish(),
                );
            }
        }
    }

    fn compute_tab_bar_left_padding(&self, ctx: &AppContext) -> f32 {
        let zoom_factor = WindowSettings::as_ref(ctx).zoom_level.as_zoom_factor();
        let traffic_light_data = traffic_light_data(ctx, self.window_id);
        let is_window_fullscreen = ctx
            .windows()
            .platform_window(self.window_id)
            .map(|window| window.fullscreen_state() == FullscreenState::Fullscreen)
            .unwrap_or(false);
        if self.current_workspace_state.is_left_panel_open() {
            0.
        } else if is_window_fullscreen && cfg!(target_os = "macos") {
            // Full-screen mode on MacOS does not need as much padding (traffic lights are hidden).
            TAB_BAR_PADDING_LEFT
        } else {
            traffic_light_data
                .as_ref()
                .filter(|data| data.side == TrafficLightSide::Left)
                .map(|data| data.width(zoom_factor))
                .unwrap_or(0.)
                + 16.
        }
    }

    /// Renders the tab bar contents, wrapped in hover and drag-drop behaviors.
    fn render_tab_bar(
        &self,
        tab_fixed_width: Option<f32>,
        appearance: &Appearance,
        ctx: &AppContext,
    ) -> Box<dyn Element> {
        let bar_contents = ConstrainedBox::new(
            // We can wrap the whole tab bar in the a drop target with the `AfterTabIndex` drop target data since the API for accepting a drop target with nested
            // drop target elements will default to the inner ones (in this case the tabs or the button before the tabs)
            DropTarget::new(
                self.render_tab_bar_contents(tab_fixed_width, appearance, ctx),
                TabBarDropTargetData {
                    tab_bar_location: TabBarLocation::AfterTabIndex(self.tabs.len()),
                },
            )
            .finish(),
        )
        .with_height(TAB_BAR_HEIGHT)
        .finish();

        let tab_bar_border =
            Border::bottom(TAB_BAR_BORDER_HEIGHT).with_border_fill(appearance.theme().outline());

        let mut tab_bar_container = Container::new(
            EventHandler::new(Clipped::new(self.render_tab_bar_hoverable(bar_contents)).finish())
                .on_back_mouse_down(move |ctx, _app, _position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ActivatePrevTab);
                    DispatchEventResult::StopPropagation
                })
                .on_forward_mouse_down(move |ctx, _app, _position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ActivateNextTab);
                    DispatchEventResult::StopPropagation
                })
                .finish(),
        )
        .with_border(tab_bar_border);
        if FeatureFlag::NewTabStyling.is_enabled() {
            tab_bar_container = tab_bar_container
                .with_background(internal_colors::fg_overlay_1(appearance.theme()));
        }
        let tab_bar_element = tab_bar_container.finish();

        let dimming_color = appearance.theme().background().into();
        SavePosition::new(
            WindowFocusDimming::apply_panel_header_dimming(
                tab_bar_element,
                self.mouse_states.header_dimming.clone(),
                TAB_BAR_HEIGHT,
                dimming_color,
                self.window_id,
                ctx,
            ),
            TAB_BAR_POSITION_ID,
        )
        .finish()
    }

    // Render traffic lights, if appropriate for the current platform.
    fn maybe_render_traffic_lights(&self, stack: &mut Stack, app: &AppContext) {
        let Some(traffic_light_data) = traffic_light_data(app, self.window_id) else {
            return;
        };

        let appearance = Appearance::as_ref(app);
        let fullscreen_state = app
            .windows()
            .platform_window(self.window_id)
            .map(|window| window.fullscreen_state())
            .unwrap_or_default();
        stack.add_positioned_child(
            traffic_light_data.render(
                fullscreen_state,
                &self.traffic_light_mouse_states,
                appearance.theme(),
                app,
            ),
            OffsetPositioning::offset_from_parent(
                Vector2F::zero(),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
    }

    fn render_new_session_button(&self, ctx: &AppContext) -> Box<dyn Element> {
        const CORNER_RADIUS: Radius = Radius::Pixels(4.);
        const BUTTON_HEIGHT: f32 = 24.;
        const SIDE_MENU_WIDTH: f32 = 16.;
        const BUTTON_WIDTH: f32 = 24. + SIDE_MENU_WIDTH;
        const BUTTON_LEFT_MARGIN: f32 = 4.;

        let new_tab_tool_tip_label_text = "New Tab".to_string();
        let new_tab_tool_tip_sublabel_text =
            keybinding_name_to_display_string(NEW_TAB_BINDING_NAME, ctx);
        let tab_configs_tool_tip_label_text = "Tab configs".to_string();
        let tab_configs_tool_tip_sublabel_text =
            keybinding_name_to_display_string(TOGGLE_TAB_CONFIGS_MENU_BINDING_NAME, ctx);
        let appearance = Appearance::as_ref(ctx);

        if !FeatureFlag::ShellSelector.is_enabled() {
            // Legacy new tab button, which shows the menu on right click.
            let new_tab_button = self
                .render_tab_bar_icon_button(
                    appearance,
                    icons::Icon::Plus,
                    &self.mouse_states.new_tab_button.clone(),
                    WorkspaceAction::AddDefaultTab,
                    new_tab_tool_tip_label_text,
                    new_tab_tool_tip_sublabel_text,
                    false,
                    false,
                )
                .on_right_click(move |ctx, _, position| {
                    ctx.dispatch_typed_action(WorkspaceAction::ToggleNewSessionMenu { position });
                })
                .finish();
            return Container::new(
                SavePosition::new(
                    Align::new(new_tab_button).finish(),
                    NEW_TAB_BUTTON_POSITION_ID,
                )
                .finish(),
            )
            .with_margin_left(BUTTON_LEFT_MARGIN)
            .finish();
        }

        let theme = appearance.theme();

        Hoverable::new(self.mouse_states.new_tab.clone(), |state| {
            let window_id = self.window_id;
            let is_active = self.show_new_session_dropdown_menu.is_some();

            let new_tab_button = combo_inner_button(
                appearance,
                icons::Icon::Plus,
                false,
                self.mouse_states.new_tab_button.clone(),
            )
            .with_style(
                UiComponentStyles::default()
                    .set_border_radius(CornerRadius::with_left(CORNER_RADIUS)),
            )
            .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                appearance,
                new_tab_tool_tip_label_text.clone(),
                new_tab_tool_tip_sublabel_text.clone(),
            ))
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::AddDefaultTab);
            })
            .finish();

            let new_session_menu_button = combo_inner_button(
                appearance,
                icons::Icon::ChevronDown,
                is_active,
                self.mouse_states.new_tab_menu.clone(),
            )
            .with_style(
                UiComponentStyles::default()
                    .set_border_radius(CornerRadius::with_right(CORNER_RADIUS))
                    .set_width(SIDE_MENU_WIDTH),
            )
            .with_active_styles(
                UiComponentStyles::default()
                    .set_background(internal_colors::fg_overlay_3(theme).into()),
            )
            .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                appearance,
                tab_configs_tool_tip_label_text.clone(),
                tab_configs_tool_tip_sublabel_text.clone(),
            ))
            .build()
            .on_click(move |ctx, app, _| {
                // We are positioning the menu to the lower-left corner of the new tab button.
                // This gives the impression that both individual buttons are one big button.
                if let Some(position) =
                    app.element_position_by_id_at_last_frame(window_id, NEW_TAB_BUTTON_POSITION_ID)
                {
                    ctx.dispatch_typed_action(WorkspaceAction::ToggleNewSessionMenu {
                        position: position.lower_left(),
                    });
                }
            })
            .finish();

            let row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(
                    SavePosition::new(
                        Align::new(new_tab_button).finish(),
                        NEW_TAB_BUTTON_POSITION_ID,
                    )
                    .finish(),
                )
                .with_child(
                    SavePosition::new(
                        Align::new(new_session_menu_button).finish(),
                        NEW_SESSION_MENU_BUTTON_POSITION_ID,
                    )
                    .finish(),
                )
                .finish();

            let mut ret = Container::new(
                ConstrainedBox::new(row)
                    .with_height(BUTTON_HEIGHT)
                    .with_width(BUTTON_WIDTH)
                    .finish(),
            )
            .with_corner_radius(CornerRadius::with_all(CORNER_RADIUS))
            .with_margin_left(BUTTON_LEFT_MARGIN);

            if state.is_hovered() {
                ret = ret.with_background(internal_colors::neutral_1(theme));
            }
            ret.finish()
        })
        .finish()
    }

    fn render_avatar_button(&self, appearance: &Appearance, _ctx: &AppContext) -> Box<dyn Element> {
        let display_name = DEFAULT_USER_DISPLAY_NAME.to_owned();
        let avatar_content = AvatarContent::Icon(icons::Icon::Gear);

        let mut avatar = Avatar::new(
            avatar_content,
            UiComponentStyles {
                width: Some(20.),
                height: Some(20.),
                border_radius: Some(CornerRadius::with_all(Radius::Percentage(50.))),
                font_family_id: Some(appearance.ui_font_family()),
                font_weight: Some(Weight::Bold),
                background: Some(appearance.theme().accent().into()),
                font_size: Some(12.),
                font_color: Some(ColorU::black()),
                ..Default::default()
            },
        );

        let button = Hoverable::new(self.mouse_states.avatar_icon.clone(), |state| {
            let mut stack = Stack::new();
            let mut container = Container::new(avatar.build().finish())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_uniform_padding(2.);

            if state.is_mouse_over_element() {
                if !state.is_clicked() {
                    container = container.with_background(appearance.theme().surface_2());
                }
                // On hover, show tooltip of user's display name (if it exists)
                if !self.is_user_menu_open {
                    stack.add_positioned_overlay_child(
                        appearance
                            .ui_builder()
                            .tool_tip(display_name.clone())
                            .with_style(UiComponentStyles {
                                background: Some(appearance.theme().tooltip_background().into()),
                                font_color: Some(appearance.theme().background().into_solid()),
                                ..Default::default()
                            })
                            .build()
                            .finish(),
                        OffsetPositioning::offset_from_parent(
                            vec2f(0., 4.),
                            ParentOffsetBounds::WindowByPosition,
                            ParentAnchor::BottomMiddle,
                            ChildAnchor::TopMiddle,
                        ),
                    );
                }
            }
            stack.add_child(container.finish());
            stack.finish()
        })
        .on_click(move |ctx, _, _| {
            ctx.dispatch_typed_action(WorkspaceAction::ToggleUserMenu);
        })
        .with_cursor(Cursor::PointingHand)
        .finish();

        SavePosition::new(Align::new(button).finish(), USER_AVATAR_BUTTON_POSITION_ID).finish()
    }

    fn render_settings_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        Align::new(
            self.render_tab_bar_icon_button(
                appearance,
                icons::Icon::Gear,
                &self.mouse_states.settings_icon,
                WorkspaceAction::ShowSettings,
                "Settings".to_string(),
                self.cached_keybindings[SHOW_SETTINGS_KEYBINDING_NAME].clone(),
                false,
                false,
            )
            .finish(),
        )
        .finish()
    }

    fn render_offline_button(&self, appearance: &Appearance) -> Box<dyn Element> {
        let ui_builder = appearance.ui_builder().clone();

        let tool_tip_label_text = "Some features may be unavailable offline".to_string();
        let icon = ConstrainedBox::new(
            Container::new(
                icons::Icon::CloudOffline
                    .to_warpui_icon(appearance.theme().foreground())
                    .finish(),
            )
            .with_uniform_padding(3.)
            .finish(),
        )
        .with_width(icons::ICON_DIMENSIONS)
        .with_height(icons::ICON_DIMENSIONS)
        .finish();

        let hoverable = Hoverable::new(self.mouse_states.offline_icon.clone(), |state| {
            let mut stack = Stack::new().with_child(icon);
            if state.is_hovered() {
                let tool_tip = ui_builder.tool_tip(tool_tip_label_text);
                stack.add_positioned_overlay_child(
                    tool_tip.build().finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(0., 4.),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::BottomMiddle,
                        ChildAnchor::TopMiddle,
                    ),
                );
            }
            stack.finish()
        });

        Align::new(hoverable.finish()).finish()
    }

    fn render_tab_bar_icon_button_tooltip(
        &self,
        appearance: &Appearance,
        tool_tip_label_text: String,
        tool_tip_sublabel_text: Option<String>,
    ) -> Box<dyn FnOnce() -> Box<dyn Element>> {
        let ui_builder = appearance.ui_builder().clone();

        Box::new(move || {
            if let Some(tool_tip_sublabel_text) = tool_tip_sublabel_text {
                ui_builder
                    .tool_tip_with_sublabel(tool_tip_label_text, tool_tip_sublabel_text)
                    .build()
                    .finish()
            } else {
                ui_builder.tool_tip(tool_tip_label_text).build().finish()
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tab_bar_icon_button(
        &self,
        appearance: &Appearance,
        icon_type: icons::Icon,
        mouse_state_handle: &MouseStateHandle,
        action: WorkspaceAction,
        tool_tip_label_text: String,
        tool_tip_sublabel_text: Option<String>,
        is_active: bool,
        disable: bool,
    ) -> Hoverable {
        let theme = appearance.theme();
        let icon_color = if is_active {
            theme.main_text_color(theme.background())
        } else {
            theme.sub_text_color(theme.background())
        };
        let mut button = icon_button_with_color(
            appearance,
            icon_type,
            is_active,
            mouse_state_handle.clone(),
            icon_color,
        );
        button = button
            .with_hovered_styles(UiComponentStyles {
                font_color: Some(icon_color.into()),
                background: Some(theme.surface_2().into()),
                ..UiComponentStyles::default()
            })
            .with_clicked_styles(UiComponentStyles {
                font_color: Some(icon_color.into()),
                background: Some(theme.background().into()),
                ..UiComponentStyles::default()
            });

        if is_active {
            button = button.with_active_styles(UiComponentStyles {
                background: Some(internal_colors::fg_overlay_3(theme).into()),
                ..UiComponentStyles::default()
            });
        }

        if disable {
            button = button.with_style(UiComponentStyles {
                font_color: Some(theme.disabled_text_color(theme.background()).into()),
                ..UiComponentStyles::default()
            });
            button.build().disable()
        } else {
            button
                .with_tooltip(self.render_tab_bar_icon_button_tooltip(
                    appearance,
                    tool_tip_label_text,
                    tool_tip_sublabel_text,
                ))
                .build()
                .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        }
    }

    fn render_banner_and_active_tab(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let active_tab_data = &self.tabs[self.active_tab_index];

        let active_content = ChildView::new(&active_tab_data.pane_group).finish();

        let terminal_content = match self.maybe_render_workspace_banner(app, appearance) {
            Some(banner_element) => Flex::column()
                .with_child(banner_element)
                .with_child(Shrinkable::new(1., active_content).finish())
                .finish(),
            None => active_content,
        };

        let pane_group = self.active_tab_pane_group().as_ref(app);

        let mut main_content = Flex::row();

        let config = TabSettings::as_ref(app)
            .header_toolbar_chip_selection
            .clone();
        let mut prev_panel_added = false;
        for item in config.left_items() {
            Self::add_panel_with_separator(
                &mut main_content,
                &mut prev_panel_added,
                self.render_config_panel(&item, pane_group, &config, app),
                app,
            );
        }
        if prev_panel_added {
            main_content.add_child(Self::render_panel_separator(app));
        }
        main_content = main_content.with_child(Shrinkable::new(1.0, terminal_content).finish());
        prev_panel_added = true;
        for item in config.right_items() {
            Self::add_panel_with_separator(
                &mut main_content,
                &mut prev_panel_added,
                self.render_config_panel(&item, pane_group, &config, app),
                app,
            );
        }

        let clickable_element = EventHandler::new(main_content.finish())
            .on_back_mouse_down(|ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::ActivatePrevTab);
                DispatchEventResult::StopPropagation
            })
            .on_forward_mouse_down(|ctx, _app, _position| {
                ctx.dispatch_typed_action(WorkspaceAction::ActivateNextTab);
                DispatchEventResult::StopPropagation
            })
            .finish();

        Shrinkable::new(
            THEME_CHOOSER_RATIO,
            SavePosition::new(clickable_element, TAB_CONTENT_POSITION_ID).finish(),
        )
        .finish()
    }

    fn render_theme_chooser(&self) -> Box<dyn Element> {
        let theme_chooser = ChildView::new(&self.theme_chooser_view).finish();
        ConstrainedBox::new(theme_chooser)
            .with_max_width(240.0)
            .finish()
    }

    #[cfg(not(target_family = "wasm"))]
    // Allow let and return because of the conditional linux compilation (otherwise we get a clippy
    // warning on mac)
    #[allow(clippy::let_and_return)]
    fn banner_fields(&self, app: &AppContext) -> Option<WorkspaceBannerFields> {
        let _ = app;
        None
    }

    fn maybe_render_workspace_banner(
        &self,
        app: &AppContext,
        appearance: &Appearance,
    ) -> Option<Box<dyn Element>> {
        self.banner_fields(app)
            .map(|fields| self.render_workspace_banner(fields, appearance))
    }

    fn render_workspace_banner(
        &self,
        fields: WorkspaceBannerFields,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let bg_color = match fields.severity {
            BannerSeverity::Warning => theme.ansi_fg_yellow(),
            BannerSeverity::Error => theme.ansi_fg_red(),
        };
        let text_color = theme.main_text_color(Fill::Solid(bg_color)).into_solid();

        // Left side: alert icon + bold heading + regular description, all inline.
        let icon =
            ConstrainedBox::new(Icon::AlertCircle.to_warpui_icon(text_color.into()).finish())
                .with_width(16.)
                .with_height(16.)
                .finish();

        let ui_font_family = appearance.ui_font_family();
        const BANNER_FONT_SIZE: f32 = 12.;

        // Combine heading and description into a single `Text` so it can
        // elide with a trailing ellipsis when there isn't enough room for the
        // buttons. The heading portion is highlighted with Semibold weight.
        // See `ConversationSearchItem::render_item` for the same pattern.
        let heading_char_count = fields
            .heading
            .as_ref()
            .map(|heading| heading.chars().count())
            .unwrap_or(0);
        let combined_text = match fields.heading {
            Some(heading) => format!("{heading} {}", fields.description),
            None => fields.description,
        };
        let mut text = Text::new_inline(combined_text, ui_font_family, BANNER_FONT_SIZE)
            .with_color(text_color)
            .with_clip(ClipConfig::ellipsis());
        if heading_char_count > 0 {
            text = text.with_single_highlight(
                Highlight::new().with_properties(Properties::default().weight(Weight::Semibold)),
                (0..heading_char_count).collect(),
            );
        }

        let mut banner = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_child(Container::new(icon).with_margin_right(8.).finish())
            // `Expanded` (not `Shrinkable`) so the text fills the remaining
            // row width and pushes the action buttons to the right even when
            // the text is short. Truncation still applies when the text would
            // otherwise overflow.
            .with_child(Expanded::new(1., text.finish()).finish());

        if let Some(secondary_button) = fields.secondary_button {
            banner.add_child(
                Container::new(self.render_banner_action_button(
                    secondary_button,
                    self.mouse_states.banner_secondary_button.clone(),
                    text_color,
                    appearance,
                ))
                .with_margin_left(4.)
                .finish(),
            );
        }

        if let Some(button) = fields.button {
            let more_info_button_action = button.more_info_button_action.clone();
            banner.add_child(
                Container::new(self.render_banner_action_button(
                    button,
                    self.mouse_states.banner_button.clone(),
                    text_color,
                    appearance,
                ))
                .with_margin_left(4.)
                .finish(),
            );

            if let Some(more_info_button_action) = more_info_button_action {
                let more_info_details = WorkspaceBannerButtonDetails {
                    text: "More info".to_owned(),
                    action: more_info_button_action,
                    variant: BannerButtonVariant::Outlined,
                    icon: None,
                    more_info_button_action: None,
                };
                banner.add_child(
                    Container::new(self.render_banner_action_button(
                        more_info_details,
                        self.mouse_states.more_info_banner_button.clone(),
                        text_color,
                        appearance,
                    ))
                    .with_margin_left(4.)
                    .finish(),
                );
            }
        }

        if fields.banner_type.is_dismissible() {
            let dismiss_target = fields.banner_type;
            banner.add_child(
                Container::new(
                    Hoverable::new(
                        self.mouse_states.dismiss_banner_button.clone(),
                        move |state| {
                            let mut container = Container::new(
                                ConstrainedBox::new(
                                    // Plain x-close glyph (`Icon::X` →
                                    // `x-close.svg`), matching the Figma
                                    // design. `Icon::XCircle` wraps the x in
                                    // a circle which is not what we want.
                                    Icon::X.to_warpui_icon(text_color.into()).finish(),
                                )
                                .with_width(16.)
                                .with_height(16.)
                                .finish(),
                            )
                            .with_uniform_padding(2.)
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)));
                            if state.is_hovered() {
                                container = container
                                    .with_background_color(coloru_with_opacity(text_color, 20));
                            }
                            container.finish()
                        },
                    )
                    .with_cursor(Cursor::PointingHand)
                    .on_click(move |ctx, _, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::DismissWorkspaceBanner(
                            dismiss_target,
                        ));
                    })
                    .finish(),
                )
                .with_margin_left(4.)
                .finish(),
            );
        }

        ConstrainedBox::new(
            Container::new(banner.finish())
                .with_background_color(bg_color)
                .with_uniform_padding(8.)
                .finish(),
        )
        .finish()
    }

    /// Renders a single banner action button using the Figma-spec'd Naked or
    /// Secondary variants: no fill by default, optional 1px border, text and
    /// icon tinted with the banner's contrast-safe text color.
    fn render_banner_action_button(
        &self,
        details: WorkspaceBannerButtonDetails,
        mouse_state: MouseStateHandle,
        text_color: ColorU,
        appearance: &Appearance,
    ) -> Box<dyn Element> {
        let WorkspaceBannerButtonDetails {
            text,
            action,
            variant,
            icon,
            ..
        } = details;
        let ui_font_family = appearance.ui_font_family();
        Hoverable::new(mouse_state, move |state| {
            let mut row = Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_main_axis_size(MainAxisSize::Min);
            if let Some(icon) = icon {
                row.add_child(
                    Container::new(
                        ConstrainedBox::new(icon.to_warpui_icon(text_color.into()).finish())
                            .with_width(14.)
                            .with_height(14.)
                            .finish(),
                    )
                    .with_margin_right(4.)
                    .finish(),
                );
            }
            row.add_child(
                Text::new_inline(text.clone(), ui_font_family, 12.)
                    .with_color(text_color)
                    .with_style(Properties {
                        weight: Weight::Semibold,
                        ..Default::default()
                    })
                    .finish(),
            );

            let mut container = Container::new(row.finish())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
                .with_horizontal_padding(8.);
            if matches!(variant, BannerButtonVariant::Outlined) {
                container = container.with_border(Border::all(1.).with_border_color(text_color));
            }
            if state.is_hovered() {
                container = container.with_background_color(coloru_with_opacity(text_color, 20));
            }

            ConstrainedBox::new(container.finish())
                .with_height(24.)
                .finish()
        })
        .with_cursor(Cursor::PointingHand)
        .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
        .finish()
    }

    fn dismiss_workspace_banner(
        &mut self,
        ctx: &mut ViewContext<Self>,
        banner_type: &WorkspaceBanner,
    ) {
        match banner_type {
            WorkspaceBanner::UnableToUpdateToNewVersion => {
                self.autoupdate_unable_to_update_banner_dismissed = true;
            }
            WorkspaceBanner::UnableToLaunchNewVersion => {
                self.autoupdate_unable_to_launch_new_version = true;
            }
            WorkspaceBanner::VersionDeprecated => {}
            WorkspaceBanner::AnonymousUserAuth => {}
            WorkspaceBanner::Reauth => {
                self.reauth_banner_dismissed = true;
            }
            #[cfg(all(enable_crash_recovery, target_os = "linux"))]
            WorkspaceBanner::WaylandCrashRecovery => {
                crash_recovery::dismiss_workspace_banner(ctx);
            }
            WorkspaceBanner::InvalidSettings => {
                self.settings_error_banner_dismissed = true;
                self.sync_settings_error_state_into_settings_pane(ctx);
            }
        }
        ctx.notify();
    }

    fn render_panel(&self, app: &AppContext, contents: Box<dyn Element>) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let mut col = Flex::column().with_main_axis_size(MainAxisSize::Max);
        let mut contents = contents;

        col.add_child(Shrinkable::new(1.0, contents).finish());

        self.wrap_in_panel_surface(appearance, col.finish(), *PANEL_CORNER_RADIUS)
    }

    fn wrap_in_panel_surface(
        &self,
        appearance: &Appearance,
        contents: Box<dyn Element>,
        corner_radius: CornerRadius,
    ) -> Box<dyn Element> {
        let mut container = Container::new(contents)
            .with_background(appearance.theme().surface_1().with_opacity(90))
            .with_corner_radius(corner_radius);

        container = container.with_margin_right(2.0);

        container.finish()
    }

    fn render_panel_separator(app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        ConstrainedBox::new(
            Rect::new()
                .with_background_color(appearance.theme().outline().into_solid())
                .finish(),
        )
        .with_width(1.0)
        .finish()
    }

    fn add_panel_with_separator(
        panels_view: &mut Flex,
        prev_panel_added: &mut bool,
        panel: Option<Box<dyn Element>>,
        app: &AppContext,
    ) {
        if let Some(panel) = panel {
            if *prev_panel_added {
                panels_view.add_child(Self::render_panel_separator(app));
            }
            panels_view.add_child(panel);
            *prev_panel_added = true;
        }
    }

    fn render_panels(&self, app: &AppContext, terminal_view: Box<dyn Element>) -> Box<dyn Element> {
        let mut panels_view = Flex::row();
        let mut prev_panel_added = false;

        // Theme chooser (workspace-level, not configurable).
        // Uses wrap_in_panel_surface which adds margin for its own visual separation,
        // so we add a separator before it only if a config panel is to its left, then
        // reset the flag so no separator is added between the theme chooser and the terminal.
        if self.current_workspace_state.is_theme_chooser_open {
            if prev_panel_added {
                panels_view.add_child(Self::render_panel_separator(app));
            }
            panels_view.add_child(self.render_panel(app, self.render_theme_chooser()));
            prev_panel_added = false;
        }

        if prev_panel_added {
            panels_view.add_child(Self::render_panel_separator(app));
        }
        // The outer workspace container in `render` already paints the terminal
        // background fill, so don't paint it again here (see APP-4328).
        panels_view = panels_view.with_child(Shrinkable::new(1.0, terminal_view).finish());

        #[cfg(target_family = "wasm")]
        if !warpui::platform::wasm::is_mobile_device()
            && self
                .current_workspace_state
                .is_transcript_details_panel_open
        {
            if let Some(panel_content) = self.render_transcript_details_panel(app) {
                panels_view = panels_view.with_child(panel_content);
            }
        }

        panels_view.finish()
    }

    /// Renders a configurable panel for the given toolbar item, if it is open.
    /// Returns `None` if the panel should not be rendered (item not available,
    /// panel not open, or item is not a panel type).
    fn render_config_panel(
        &self,
        item: &HeaderToolbarItemKind,
        pane_group: &PaneGroup,
        _config: &HeaderToolbarChipSelection,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        if !item.is_available(app) || !item.is_panel() {
            return None;
        }
        match item {
            HeaderToolbarItemKind::TabsPanel => None,
            HeaderToolbarItemKind::ToolsPanel => {
                if !pane_group.left_panel_open || warpui::platform::is_mobile_device() {
                    return None;
                }
                Some(ChildView::new(&self.left_panel_view).finish())
            }
        }
    }

    /// Renders the maximized code review panel if it is configured and maximized.
    fn render_config_panel_maximized(
        &self,
        pane_group: &PaneGroup,
        _config: &HeaderToolbarChipSelection,
        app: &AppContext,
    ) -> Option<Box<dyn Element>> {
        let _ = (pane_group, app);
        None
    }

    /// Offset positioning for agent toasts.
    /// TODO: update positioning based on input mode.
    fn agent_toast_positioning(&self) -> OffsetPositioning {
        OffsetPositioning::offset_from_save_position_element(
            TAB_CONTENT_POSITION_ID,
            vec2f(0., 16.),
            PositionedElementOffsetBounds::WindowByPosition,
            PositionedElementAnchor::TopRight,
            ChildAnchor::TopRight,
        )
    }

    /// Offset positioning for global toasts.
    // TODO: update positioning based on input mode.
    fn global_toast_positioning(&self) -> OffsetPositioning {
        OffsetPositioning::offset_from_save_position_element(
            TAB_CONTENT_POSITION_ID,
            vec2f(0., 16.),
            PositionedElementOffsetBounds::WindowByPosition,
            PositionedElementAnchor::TopMiddle,
            ChildAnchor::TopMiddle,
        )
    }

    /// Offset positioning for the update toast.
    fn update_toast_positioning(
        &self,
        input_position_id: String,
        app: &AppContext,
    ) -> OffsetPositioning {
        let input_mode = InputModeSettings::as_ref(app).input_mode.value();

        match input_mode {
            InputMode::PinnedToBottom => OffsetPositioning::offset_from_save_position_element(
                input_position_id,
                vec2f(-16., -16.),
                PositionedElementOffsetBounds::WindowByPosition,
                PositionedElementAnchor::TopRight,
                ChildAnchor::BottomRight,
            ),
            InputMode::PinnedToTop => OffsetPositioning::offset_from_save_position_element(
                input_position_id,
                vec2f(-16., 16.),
                PositionedElementOffsetBounds::WindowByPosition,
                PositionedElementAnchor::BottomRight,
                ChildAnchor::TopRight,
            ),
            InputMode::Waterfall => OffsetPositioning::offset_from_parent(
                vec2f(-16., -16.),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::BottomRight,
                ChildAnchor::BottomRight,
            ),
        }
    }

    /// Send SyncEvent to all synced pane groups.
    fn process_sync_event_for_all_synced_pane_groups(
        &mut self,
        event: &SyncEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        for tab in self.tab_views() {
            // We have to get the latest SyncInputStatus each iteration because
            // tab.update below could potentially change it.
            let synced_pane_group_ids = SyncedInputState::as_ref(ctx);

            if synced_pane_group_ids.should_sync_this_pane_group(tab.id(), ctx.window_id()) {
                tab.update(ctx, |pane_group, ctx| {
                    pane_group.send_sync_event_to_panes(event, ctx);
                });
            }
        }

        self.update_pane_dimming_for_current_focus_region(ctx);
    }

    /// Sends SyncEvent to all synced terminal views.
    /// The purpose of the event could be match the active terminal input,
    /// expand the terminal input box, or collapse the terminal input box.
    fn process_updated_sync_state(&self, ctx: &mut ViewContext<Self>) {
        // If there is an active terminal, return a sync event that all
        // other synced terminals should apply to match it.
        // If there is no active terminal (like when all Warp windows are
        // minimized), return an event to start syncing.
        let sync_event = self
            .active_tab_pane_group()
            .as_ref(ctx)
            .active_session_view(ctx)
            .map_or(
                SyncEvent {
                    source_view_id: ctx.view_id(),
                    data: SyncInputType::StartSyncing,
                },
                |terminal_view_handle| {
                    terminal_view_handle
                        .as_ref(ctx)
                        .create_sync_event_based_on_terminal_state(ctx)
                },
            );

        let stop_syncing_event = SyncEvent {
            source_view_id: ctx.view_id(),
            data: terminal::view::SyncInputType::StopSyncing,
        };

        for tab in self.tab_views() {
            // We have to get the latest SyncInputStatus each iteration because
            // tab.update below could potentially change it.
            let synced_pane_group_ids = SyncedInputState::as_ref(ctx);

            if synced_pane_group_ids.should_sync_this_pane_group(tab.id(), ctx.window_id()) {
                tab.update(ctx, |pane_group, pane_group_ctx| {
                    pane_group.send_sync_event_to_panes(&sync_event, pane_group_ctx);
                });
            } else {
                // Note: we're sending StopSyncing to tabs that could already
                // know they're not syncing. We can optimize this later.
                tab.update(ctx, |pane_group, pane_group_ctx| {
                    pane_group.send_sync_event_to_panes(&stop_syncing_event, pane_group_ctx);
                });
            }
        }

        // Update tab indicators based on the new sync state.
        ctx.notify();
    }

    fn all_pane_group_ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.tab_views().map(|tab| tab.id())
    }

    /// Triggers the necessary cleanup for when a user logs out.
    pub fn on_log_out(&mut self, ctx: &mut ViewContext<Self>) {
        // Logging out should mimic the same behaviour as closing a window.
        // This gives views a chance to clean up any state through on_view_detached before being dropped.
        self.on_window_closed(ctx);
    }

    fn open_left_panel_view(&mut self, action: &LeftPanelAction, ctx: &mut ViewContext<Self>) {
        if !self.active_tab_pane_group().as_ref(ctx).left_panel_open {
            self.toggle_left_panel(ctx);
        }

        if self.active_tab_pane_group().as_ref(ctx).left_panel_open {
            self.left_panel_view.update(ctx, |left_panel, ctx| {
                left_panel.handle_action_with_force_open(action, false, ctx);
                left_panel.focus_active_view_on_entry(ctx);
            });
        }
    }

    fn toggle_left_panel_view(
        &mut self,
        action: &LeftPanelAction,
        is_showing_target_view: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let is_left_panel_open = self.active_tab_pane_group().as_ref(ctx).left_panel_open;

        if is_left_panel_open && is_showing_target_view {
            // If we're showing the target view for this action,
            // toggle the left panel closed.
            self.toggle_left_panel(ctx);
        } else {
            self.open_left_panel_view(action, ctx);
        }
    }

    /// Computes the list of available left panel views based on current AI settings and feature flags.
    fn compute_left_panel_views(ctx: &AppContext) -> Vec<ToolPanelView> {
        let mut views = vec![];
        if cfg!(feature = "local_fs") && *CodeSettings::as_ref(ctx).show_project_explorer.value() {
            views.push(ToolPanelView::ProjectExplorer);
        }
        if cfg!(feature = "local_fs")
            && FeatureFlag::GlobalSearch.is_enabled()
            && *CodeSettings::as_ref(ctx).show_global_search.value()
        {
            views.push(ToolPanelView::GlobalSearch {
                entry_focus: GlobalSearchEntryFocus::Results,
            });
        }
        views
    }

    /// Recomputes the available left panel views based on current AI settings and feature flags,
    /// then updates both the workspace's left_panel_views and the LeftPanelView's toolbelt buttons.
    fn update_left_panel_available_views(&mut self, ctx: &mut ViewContext<Self>) {
        let views = Self::compute_left_panel_views(ctx);
        self.left_panel_views = views.clone();
        self.left_panel_view.update(ctx, |left_panel, ctx| {
            left_panel.update_available_views(views, ctx);
        });
    }

    /// Opens a given URL in the desktop Warp app if installed, or redirects to download page.
    #[cfg(target_family = "wasm")]
    fn open_link_on_desktop(&mut self, url: &Url, ctx: &mut ViewContext<Self>) {
        use crate::settings::app_installation_detection::{
            UserAppInstallDetectionSettings, UserAppInstallStatus,
        };

        // Check if the desktop app is installed
        let is_app_installed = *UserAppInstallDetectionSettings::as_ref(ctx)
            .user_app_installation_detected
            .value()
            == UserAppInstallStatus::Detected;

        if !is_app_installed {
            // App not installed - redirect to download page
            ctx.open_url("https://warp.dev/download");
            // In webapp code we cannot distinguish between
            // the localhost:9277/install_detection endpoint not running (not installed) vs
            // the browser blocking Local Network Access which results in CORS error;
            // the browser intentionally obscures the error root cause for privacy reasons.
            // Many users' browser settings will block Local Network Access so this will end up redirecting to download page,
            // even if they have the app installed.
            let toast_message = format!(
                "Have Warp installed but redirecting to download page?\nEnable Local Network Access for {} in your browser.",
                ChannelState::server_root_url()
            );
            self.toast_stack.update(ctx, |toast_stack, ctx| {
                toast_stack.add_persistent_toast(DismissibleToast::default(toast_message), ctx)
            });
            // Still try to open the url on desktop below
        }

        // Open the URL on desktop. This does nothing if the app isn't installed.
        crate::uri::web_intent_parser::open_url_on_desktop(url);
    }
}

impl Entity for Workspace {
    type Event = ();
}

impl TypedActionView for Workspace {
    type Action = WorkspaceAction;

    fn action_accessibility_contents(
        &mut self,
        action: &WorkspaceAction,
        _: &mut ViewContext<Self>,
    ) -> ActionAccessibilityContent {
        match action {
            WorkspaceAction::SetA11yVerbosityLevel(verbosity) => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                    format!("{verbosity:?} accessibility announcements set"),
                    WarpA11yRole::UserAction,
                ))
            }
            _ => ActionAccessibilityContent::from_debug(),
        }
    }

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        use WorkspaceAction::*;
        let window_id = ctx.window_id();

        match action {
            ActivateTab(index) => self.activate_tab(*index, ctx),
            ActivateTabByNumber(num) => self.activate_tab(num.saturating_sub(1), ctx),
            ActivatePrevTab => self.activate_prev_tab(ctx),
            OpenLaunchConfigSaveModal => self.open_launch_config_save_modal(ctx),
            ActivateNextTab => self.activate_next_tab(ctx),
            ActivateLastTab => self.activate_last_tab(ctx),
            CyclePrevSession => self.cycle_prev_session(ctx),
            CycleNextSession => self.cycle_next_session(ctx),
            MoveActiveTabLeft => self.move_tab(self.active_tab_index, TabMovement::Left, ctx),
            MoveActiveTabRight => self.move_tab(self.active_tab_index, TabMovement::Right, ctx),
            MoveTabLeft(index) => self.move_tab(*index, TabMovement::Left, ctx),
            MoveTabRight(index) => self.move_tab(*index, TabMovement::Right, ctx),
            RenameTab(index) => self.rename_tab(*index, ctx),
            ResetTabName(index) => self.clear_tab_name(*index, ctx),
            RenamePane(locator) => self.rename_pane(*locator, ctx),
            ResetPaneName(locator) => self.clear_pane_name(*locator, ctx),
            RenameActiveTab => self.rename_tab(self.active_tab_index, ctx),
            SetActiveTabName(name) => self.set_active_tab_name(name, ctx),
            SetActiveTabColor(color) => self.set_tab_color(self.active_tab_index, *color, ctx),
            ToggleTabRightClickMenu { tab_index, anchor } => {
                self.toggle_tab_right_click_menu(*tab_index, *anchor, ctx)
            }
            ToggleTabBarOverflowMenu => self.toggle_tab_bar_overflow_menu(ctx),
            ToggleBlockSnackbar => self.toggle_block_snackbar(ctx),
            CloseTab(index) => self.close_tab(*index, false, true, ctx),
            CloseActiveTab => self.close_tab(self.active_tab_index, false, true, ctx),
            CloseOtherTabs(index) => self.close_other_tabs(*index, false, ctx),
            CloseNonActiveTabs => self.close_other_tabs(self.active_tab_index, false, ctx),
            CloseTabsRight(index) => {
                self.close_tabs_direction(*index, TabMovement::Right, false, ctx)
            }
            CloseTabsRightActiveTab => {
                self.close_tabs_direction(self.active_tab_index, TabMovement::Right, false, ctx)
            }
            AddDefaultTab => self.add_terminal_tab(false, ctx),
            AddTerminalTab { hide_homepage } => {
                self.add_new_session_tab_internal_with_default_session_mode_behavior(
                    NewSessionSource::Tab,
                    Some(window_id),
                    None,
                    *hide_homepage,
                    DefaultSessionModeBehavior::Ignore,
                    ctx,
                );
                ctx.notify();
            }
            AddTabWithShell { shell, source } => {
                self.add_tab_with_shell(shell.clone(), *source, ctx)
            }
            OpenNewSessionMenu { position } => self.open_new_session_dropdown_menu(*position, ctx),
            ToggleTabConfigsMenu => self.toggle_tab_configs_menu(ctx),
            ShowSessionConfigModal => self.show_session_config_modal(ctx),
            DismissSessionConfigTabConfigChip => {
                self.dismiss_session_config_tab_config_chip(ctx);
            }
            SaveCurrentTabAsNewConfig(tab_index) => {
                self.save_current_tab_as_new_config(*tab_index, ctx)
            }
            ToggleNewSessionMenu { position } => {
                self.toggle_new_session_dropdown_menu(*position, ctx)
            }
            SelectTabConfig(tab_config) => {
                self.open_tab_config(tab_config.clone(), ctx);
            }
            OpenTabConfigErrorFile {
                #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
                path,
                toast_object_id,
            } => {
                #[cfg(feature = "local_fs")]
                {
                    let settings = EditorSettings::as_ref(ctx);
                    let target = resolve_file_target_with_editor_choice(
                        path,
                        *settings.open_code_panels_file_editor,
                        *settings.prefer_markdown_viewer,
                        *settings.open_file_layout,
                        None,
                    );
                    self.open_file_with_target(
                        path.clone(),
                        target,
                        None,
                        CodeSource::Link {
                            path: path.clone(),
                            range_start: None,
                            range_end: None,
                        },
                        ctx,
                    );
                }
                self.dismiss_older_toasts(toast_object_id, ctx);
            }
            TabConfigSidecarEditConfig {
                #[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
                path,
            } => {
                #[cfg(feature = "local_fs")]
                {
                    let settings = EditorSettings::as_ref(ctx);
                    let target = resolve_file_target_with_editor_choice(
                        path,
                        *settings.open_code_panels_file_editor,
                        *settings.prefer_markdown_viewer,
                        *settings.open_file_layout,
                        None,
                    );
                    self.open_file_with_target(
                        path.clone(),
                        target,
                        None,
                        CodeSource::Link {
                            path: path.clone(),
                            range_start: None,
                            range_end: None,
                        },
                        ctx,
                    );
                }
                self.close_new_session_dropdown_menu(ctx);
            }
            TabConfigSidecarRemoveConfig { name, path } => {
                self.remove_tab_config_confirmation_dialog
                    .update(ctx, |dialog, ctx| {
                        dialog.set_config(name.clone(), path.clone());
                        ctx.notify();
                    });
                self.close_new_session_dropdown_menu(ctx);
                self.current_workspace_state
                    .is_remove_tab_config_dialog_open = true;
                ctx.focus(&self.remove_tab_config_confirmation_dialog);
                ctx.notify();
            }
            OpenSettingsFile => {
                self.show_settings(ctx);
            }
            FixSettingsWithOz { .. } => {}
            ApplyUpdate => self.apply_update(ctx),
            CopyVersion(version) => self.copy_version(version, ctx),
            DownloadNewVersion => self.download_new_version(ctx),
            ConfigureKeybindingSettings { keybinding_name } => {
                self.show_keyboard_settings(keybinding_name.as_deref(), ctx)
            }
            ShowSettings => self.show_settings(ctx),
            ShowSettingsPage(section) => self.show_settings_with_section(Some(*section), ctx),
            ShowSettingsPageWithSearch {
                search_query,
                section,
            } => self.show_settings_with_search(search_query, *section, ctx),
            ShowThemeChooser(mode) => self.show_theme_chooser(Some(*mode), ctx),
            ShowThemeChooserForActiveTheme => self.show_theme_chooser_for_active_theme(ctx),
            IncreaseFontSize => self.increase_font_size(ctx),
            DecreaseFontSize => self.decrease_font_size(ctx),
            ResetFontSize => self.reset_font_size(ctx),
            IncreaseZoom => self.increase_zoom(ctx),
            DecreaseZoom => self.decrease_zoom(ctx),
            ResetZoom => self.reset_zoom(ctx),
            OpenPalette {
                mode,
                source,
                query,
            } => {
                let _ = query;
                self.open_palette(*mode, source.clone(), ctx)
            }
            TogglePalette {
                mode: palette_mode,
                source,
            } => self.toggle_palette(*palette_mode, source.clone(), ctx),
            ShowReferralSettingsPage => {
                self.show_settings_with_section(Some(SettingsSection::Referrals), ctx);
            }
            JoinSlack => self.join_slack(ctx),
            ViewUserDocs => self.view_user_docs(ctx),
            DispatchToSettingsTab(action) => {
                let window_id = ctx.window_id();
                ctx.dispatch_typed_action_for_view(window_id, self.settings_pane.id(), action)
            }
            OpenLink(link) => ctx.open_url(link),
            #[cfg(target_family = "wasm")]
            OpenLinkOnDesktop(url) => self.open_link_on_desktop(url, ctx),
            DumpDebugInfo => self.dump_debug_info(ctx),
            #[cfg(target_os = "macos")]
            InstallCLI => self.install_cli(ctx),
            #[cfg(target_os = "macos")]
            UninstallCLI => self.uninstall_cli(ctx),
            ToggleRecordingMode => self.toggle_recording_mode(ctx),
            ToggleInBandGenerators => self.toggle_in_band_generators(ctx),
            ToggleDebugNetworkStatus => self.toggle_debug_network_status(ctx),
            ToggleShowMemoryStats => self.toggle_show_memory_stats(ctx),
            ToggleResourceCenter => self.toggle_resource_center(ctx),
            ToggleUserMenu => self.toggle_user_menu(ctx),
            ToggleKeybindingsPage => self.toggle_keybindings_page(ctx),
            ShowCommandSearch(CommandSearchOptions {
                filter,
                init_content,
            }) => self.show_command_search(*filter, init_content, ctx),
            ToggleMouseReporting => self.toggle_mouse_reporting(ctx),
            ToggleScrollReporting => self.toggle_scroll_reporting(ctx),
            ToggleFocusReporting => self.toggle_focus_reporting(ctx),
            StartTabDrag => {
                // If we are renaming a tab, finish the rename before dragging.
                self.finish_tab_rename(ctx);
                self.current_workspace_state.is_tab_being_dragged = true;
            }
            ToggleLeftPanel => {
                let active_pane_group = self.active_tab_pane_group().clone();
                let was_open = active_pane_group.read(ctx, |pg, _| pg.left_panel_open);

                // Don't open the panel if no views are available.
                if !was_open && self.left_panel_views.is_empty() {
                    return;
                }

                let file_tree_active = self
                    .left_panel_view
                    .read(ctx, |lp, _| lp.is_file_tree_active());
                self.toggle_left_panel(ctx);

                let is_open = active_pane_group.read(ctx, |pg, _| pg.left_panel_open);

                if !was_open && is_open {
                    self.left_panel_view.update(ctx, |left_panel, ctx| {
                        left_panel.focus_active_view_on_entry(ctx);
                    });

                    let _ = file_tree_active;
                }
            }
            ClosePanel => {
                if self.left_panel_view.is_self_or_child_focused(ctx) {
                    self.close_left_panel(ctx);
                }
            }
            OpenInExplorer { path } => {
                ctx.open_file_path_in_explorer(path);
            }
            OpenFilePath { path } => {
                ctx.open_file_path(path);
            }
            DragTab {
                tab_index,
                tab_position,
            } => self.on_tab_drag(*tab_index, *tab_position, ctx),
            DropTab => {
                self.current_workspace_state.is_tab_being_dragged = false;
            }
            CopyTextToClipboard(text) => {
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(text.to_string()));
            }
            DismissWorkspaceBanner(banner_type) => self.dismiss_workspace_banner(ctx, banner_type),
            Crash => {
                #[cfg(feature = "crash_reporting")]
                crate::crash_reporting::crash();
            }
            Panic => {
                panic!("WorkspaceAction::Panic triggered from command palette");
            }
            DumpHeapProfile => {
                #[cfg(feature = "dhat_heap_profiling")]
                crate::profiling::dump_dhat_heap_profile();
            }
            OpenViewTreeDebugWindow => {
                let window_id = ctx.window_id();
                ctx.open_view_tree_debug_window(window_id);
            }
            ToggleSyncAllTerminalInputsInAllTabs => {
                let enabled = SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    status.toggle_sync_all_terminal_inputs_in_all_tabs(window_id);

                    status.is_syncing_all_inputs(window_id)
                });
                let verb = if enabled { "enabled" } else { "disabled" };
                let mut message = format!("You {verb} synchronized inputs in all tabs.");
                if let Some(keystroke) = keybinding_name_to_keystroke(
                    "workspace:toggle_sync_all_terminal_inputs_in_all_tabs",
                    ctx,
                ) {
                    let _ = write!(message, " Press {} to undo.", keystroke.displayed());
                }
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::default(message);
                    view.add_ephemeral_toast(new_toast, ctx);
                });

                self.process_updated_sync_state(ctx);
            }
            ToggleSyncTerminalInputsInTab => {
                let enabled = SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    let current_pane_group_id = self.active_tab_pane_group().id();

                    status.toggle_sync_terminal_inputs_in_tab(
                        current_pane_group_id,
                        self.all_pane_group_ids(),
                        self.tab_count(),
                        window_id,
                    );

                    status.should_sync_this_pane_group(current_pane_group_id, window_id)
                });
                let verb = if enabled { "enabled" } else { "disabled" };
                let mut message = format!("You {verb} synchronized inputs in this tab.");
                if let Some(keystroke) = keybinding_name_to_keystroke(
                    "workspace:toggle_sync_terminal_inputs_in_tab",
                    ctx,
                ) {
                    let _ = write!(message, " Press {} to undo.", keystroke.displayed());
                }
                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast = DismissibleToast::default(message);
                    view.add_ephemeral_toast(new_toast, ctx);
                });

                self.process_updated_sync_state(ctx);
            }
            DisableTerminalInputSync => {
                SyncedInputState::handle(ctx).update(ctx, |status, _| {
                    status.disable_sync_terminal_inputs(window_id);
                });

                self.process_updated_sync_state(ctx);

                self.toast_stack.update(ctx, |view, ctx| {
                    let new_toast =
                        DismissibleToast::success("Disabled all synchronized inputs.".to_string());
                    view.add_ephemeral_toast(new_toast, ctx);
                });
            }
            ShowHeaderToolbarContextMenu { position } => {
                self.show_header_toolbar_context_menu(*position, ctx);
            }
            ReopenClosedSession => {
                // While we could grab the UndoCloseStack singleton entity and
                // directly call undo_close(), it would fail when attempting to
                // restore a closed tab as we would attempt to update the
                // workspace while we are currently updating the workspace.
                // Instead, we use a global action to ensure we don't try to
                // perform nested updates on the workspace.
                ctx.dispatch_global_action("app:undo_close", ());
            }
            AddWindow => {
                ctx.dispatch_global_action("root_view:open_new", ());
            }
            AddWindowWithShell { shell } => {
                ctx.dispatch_global_action("root_view:open_new_with_shell", Some(shell.clone()));
            }
            NavigatePrevPaneOrPanel => {
                self.navigate_pane_or_panel(PanePanelDirection::Prev, ctx);
            }
            NavigateNextPaneOrPanel => {
                self.navigate_pane_or_panel(PanePanelDirection::Next, ctx);
            }
            FocusLeftPanel => self.focus_left_panel(ctx),
            TerminateApp => {
                ctx.terminate_app(TerminationMode::Cancellable, None);
            }
            CloseWindow => {
                if ContextFlag::CloseWindow.is_enabled() {
                    ctx.close_window();
                }
            }
            RunCommand(code) => {
                let command = code.trim().to_string();
                self.insert_in_input(&command, true, true, false, ctx);
                ctx.notify();
            }
            InsertInInput {
                content,
                replace_buffer,
                ensure_agent_mode,
            } => {
                self.insert_in_input(content, *replace_buffer, false, *ensure_agent_mode, ctx);
                ctx.notify();
            }
            #[cfg(all(enable_crash_recovery, target_os = "linux"))]
            DismissWaylandCrashRecoveryBannerAndOpenLink => {
                self.dismiss_workspace_banner(ctx, &WorkspaceBanner::WaylandCrashRecovery);
                ctx.open_url("https://docs.warp.dev/terminal/more-features/linux#native-wayland");
            }
            TabHoverWidthStart { width } => {
                // Store the fixed width value for the tab to maintain consistent size during hover
                self.tab_fixed_width = Some(*width);
                ctx.notify();
            }
            TabHoverWidthEnd => {
                // Clear the stored width when hover ends
                self.tab_fixed_width = None;
                ctx.notify();
            }
            FocusTerminalViewInWorkspace { terminal_view_id } => {
                if !self.focus_terminal_view_locally(*terminal_view_id, ctx) {
                    self.focus_terminal_view_in_other_window(*terminal_view_id, ctx);
                }
            }
            FocusPane(locator) => {
                self.focus_pane(*locator, ctx);
            }
            ScrollToSettingsWidget { page, widget_id } => {
                self.open_settings_pane(Some(*page), None, ctx);
                self.settings_pane.update(ctx, |settings, ctx| {
                    settings.scroll_to_settings_widget(*page, widget_id, ctx);
                });
                ctx.notify();
            }
            OpenRepository { path } => {
                self.open_repository(path.as_deref(), ctx);
            }
            #[cfg(not(target_family = "wasm"))]
            InsertForkSlashCommand => {
                self.active_tab_pane_group().update(ctx, |pane_group, ctx| {
                    if let Some(terminal_view) = pane_group.active_session_view(ctx) {
                        terminal_view.update(ctx, |terminal, ctx| {
                            terminal.input().update(ctx, |input, ctx| {
                                input.replace_buffer_content(
                                    &format!("{} ", commands::FORK.name),
                                    ctx,
                                );
                                ctx.focus_self();
                            });
                        });
                    }
                });
            }
            #[cfg(feature = "local_fs")]
            #[cfg(debug_assertions)]
            OpenBuildPlanMigrationModal => {
                // Force open the modal for debugging
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.force_open_build_plan_migration_modal(ctx);
                });
                ctx.notify();
            }
            #[cfg(debug_assertions)]
            ResetBuildPlanMigrationModalState => {
                // Reset the dismissed state for debugging
                let general_settings = GeneralSettings::handle(ctx);
                general_settings.update(ctx, |settings, ctx| {
                    if let Err(e) = settings
                        .build_plan_migration_modal_dismissed
                        .set_value(false, ctx)
                    {
                        log::warn!(
                            "Failed to reset build plan migration modal dismissed setting: {e}"
                        );
                    }
                });
                log::info!("Build plan migration modal dismissed state has been reset");
            }
            #[cfg(debug_assertions)]
            #[cfg(debug_assertions)]
            OpenOzLaunchModal => {
                // Force open the Oz launch modal for debugging
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.force_open_oz_launch_modal(ctx);
                });
                ctx.notify();
            }
            #[cfg(debug_assertions)]
            OpenOpenWarpLaunchModal => {
                // Force open the OpenWarp launch modal for debugging
                OneTimeModalModel::handle(ctx).update(ctx, |model, ctx| {
                    model.force_open_openwarp_launch_modal(ctx);
                });
                ctx.notify();
            }
            #[cfg(debug_assertions)]
            ResetOpenWarpLaunchModalState => {
                // Reset the OpenWarp launch modal dismissed state for debugging
                let old_value = *GeneralSettings::as_ref(ctx)
                    .did_check_to_trigger_openwarp_launch_modal
                    .value();
                GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                    if let Err(e) = settings
                        .did_check_to_trigger_openwarp_launch_modal
                        .set_value(false, ctx)
                    {
                        log::warn!("Failed to reset OpenWarp launch modal dismissed setting: {e}");
                    }
                });
                let new_value = *GeneralSettings::as_ref(ctx)
                    .did_check_to_trigger_openwarp_launch_modal
                    .value();
                log::info!(
                    "OpenWarp launch modal state: old={}, new={}, feature_flag_enabled={}",
                    old_value,
                    new_value,
                    FeatureFlag::OpenWarpLaunchModal.is_enabled()
                );
            }
            #[cfg(debug_assertions)]
            InstallOpenCodeWarpPlugin => {
                let message = set_opencode_warp_plugin("github:warpdotdev/opencode-warp-internal");
                self.toast_stack.update(ctx, |view, ctx| {
                    view.add_ephemeral_toast(DismissibleToast::default(message), ctx);
                });
            }
            #[cfg(debug_assertions)]
            UseLocalOpenCodeWarpPlugin => {
                let message = match dirs::home_dir() {
                    Some(home) => {
                        let plugin_path = home.join("opencode-warp/src/index.ts");
                        let entry = format!("file://{}", plugin_path.display());
                        set_opencode_warp_plugin(&entry)
                    }
                    None => "Failed to determine home directory".to_string(),
                };
                self.toast_stack.update(ctx, |view, ctx| {
                    view.add_ephemeral_toast(DismissibleToast::default(message), ctx);
                });
            }
            #[cfg(target_os = "macos")]
            SampleProcess => {
                let pid = process::id();
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let output_path = env::temp_dir()
                    .join(format!("warp_sample_{timestamp}.txt"))
                    .display()
                    .to_string();

                self.toast_stack.update(ctx, |view, ctx| {
                    view.add_ephemeral_toast(
                        DismissibleToast::default("Sampling process for 3 seconds...".to_string()),
                        ctx,
                    );
                });

                let output_path_clone = output_path.clone();
                ctx.spawn(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            // `sample` is the macOS CLI that Activity Monitor uses for "Sample Process".
                            Command::new("sample")
                                .args([
                                    // process ID
                                    &pid.to_string(),
                                    // duration in seconds
                                    "3",
                                    // sampling interval in milliseconds
                                    "1",
                                    // write output to file
                                    "-file",
                                    &output_path_clone,
                                ])
                                .output()
                        })
                        .await
                    },
                    move |me, result, ctx| {
                        let message = match result {
                            Ok(Ok(output)) if output.status.success() => {
                                ctx.open_file_path_in_explorer(Path::new(&output_path));

                                #[cfg(feature = "crash_reporting")]
                                if ChannelState::channel().is_dogfood() {
                                    // For dogfood process samples, we raise a sentry warning with the sample attatched.
                                    // We do this so that our performance bot can then read through the performance logs
                                    // in sentry and write up a report of findings/possible optimizations.
                                    if let Ok(sample_data) = fs::read(&output_path) {
                                        let filename = Path::new(&output_path)
                                            .file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "process_sample.txt".to_string());
                                        let attachment = Attachment {
                                            buffer: sample_data,
                                            filename,
                                            ty: Some(AttachmentType::Attachment),
                                            ..Default::default()
                                        };
                                        sentry::with_scope(
                                            |scope| {
                                                scope.add_attachment(attachment);
                                            },
                                            || {
                                                sentry::capture_message(
                                                    "[FOR PERFORMANCE BOT] Dev took performance sample with results: ",
                                                    sentry::Level::Warning,
                                                )
                                            },
                                        );
                                    }
                                }

                                format!("Process sample saved to {output_path}")
                            }
                            Ok(Ok(output)) => {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                log::error!("sample command failed ({}): {stderr}", output.status);
                                "Failed to sample process (check logs)".to_string()
                            }
                            Ok(Err(io_err)) => {
                                log::error!("Failed to run sample command: {io_err}");
                                "Failed to sample process (check logs)".to_string()
                            }
                            Err(join_err) => {
                                log::error!("Sample task panicked: {join_err}");
                                "Failed to sample process (check logs)".to_string()
                            }
                        };
                        me.toast_stack.update(ctx, |view, ctx| {
                            view.add_ephemeral_toast(DismissibleToast::default(message), ctx);
                        });
                    },
                );
            }
            ToggleProjectExplorer => {
                if *CodeSettings::as_ref(ctx).show_project_explorer {
                    let is_showing = self.left_panel_view.as_ref(ctx).active_view()
                        == ToolPanelView::ProjectExplorer;
                    self.toggle_left_panel_view(&LeftPanelAction::ProjectExplorer, is_showing, ctx);
                }
            }
            ToggleGlobalSearch => {
                if FeatureFlag::GlobalSearch.is_enabled()
                    && *CodeSettings::as_ref(ctx).show_global_search
                {
                    let is_showing = matches!(
                        self.left_panel_view.as_ref(ctx).active_view(),
                        ToolPanelView::GlobalSearch { .. }
                    );
                    self.toggle_left_panel_view(
                        &LeftPanelAction::GlobalSearch {
                            entry_focus: GlobalSearchEntryFocus::QueryEditor,
                        },
                        is_showing,
                        ctx,
                    );
                }
            }
            OpenGlobalSearch => {
                if FeatureFlag::GlobalSearch.is_enabled()
                    && *CodeSettings::as_ref(ctx).show_global_search
                {
                    if let Some(selected_text) = self.get_selected_text_from_focused_view(ctx) {
                        if let Some(global_search_view) = self
                            .left_panel_view
                            .as_ref(ctx)
                            .active_global_search_view(ctx)
                        {
                            // If we detect selected text in the active pane, pre-populate the global search input
                            global_search_view.update(ctx, |view, ctx| {
                                view.set_initial_query(selected_text, ctx);
                            });
                        }
                    }

                    self.open_left_panel_view(
                        &LeftPanelAction::GlobalSearch {
                            entry_focus: GlobalSearchEntryFocus::QueryEditor,
                        },
                        ctx,
                    );
                }
            }
            #[cfg(target_family = "wasm")]
            ToggleConversationTranscriptDetailsPanel => {
                let is_open = !self
                    .current_workspace_state
                    .is_transcript_details_panel_open;
                self.current_workspace_state
                    .is_transcript_details_panel_open = is_open;

                self.transcript_info_button.update(ctx, |button, ctx| {
                    button.set_active(is_open, ctx);
                });

                if is_open {
                    self.update_transcript_details_panel_data(ctx);
                }

                ctx.notify();
            }
            OpenLightbox {
                images,
                initial_index,
            } => {
                let params = LightboxParams {
                    images: images.clone(),
                    initial_index: *initial_index,
                };
                if let Some(handle) = &self.lightbox_view {
                    handle.update(ctx, |view, ctx| view.update_params(params, ctx));
                } else {
                    let handle = ctx.add_typed_action_view(|ctx| LightboxView::new(params, ctx));
                    ctx.subscribe_to_view(&handle, |me, _, event, ctx| match event {
                        LightboxViewEvent::Close => {
                            me.lightbox_view = None;
                            me.focus_active_tab(ctx);
                            ctx.notify();
                        }
                        LightboxViewEvent::FocusLost => {
                            // Focus already moved elsewhere; just tear down the view.
                            me.lightbox_view = None;
                            ctx.notify();
                        }
                    });
                    ctx.focus(&handle);
                    self.lightbox_view = Some(handle);
                }
                ctx.notify();
            }
            UpdateLightboxImage { index, image } => {
                if let Some(handle) = &self.lightbox_view {
                    handle.update(ctx, |view, ctx| {
                        view.update_image_at(*index, image.clone(), ctx);
                    });
                    ctx.notify();
                }
            }
            HandoffPendingTransfer { .. } => {}
            ReverseHandoff { .. } => {}
            FinalizeDropTab => {}
            SyncTrafficLights => {
                self.sync_window_button_visibility(ctx);
            }
        };
        if action.should_save_app_state_on_action() {
            ctx.dispatch_global_action("workspace:save_app", ());
        }
    }
}

impl View for Workspace {
    fn ui_name() -> &'static str {
        "Workspace"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let tab_bar_mode = self.tab_bar_mode(app);

        // For WASM simplified tab bar views (Warp Drive objects, shared sessions, conversation transcripts),
        // we render the tab bar outside of panels so that the details panel only affects content below the tab bar.
        cfg_if::cfg_if! {
            if #[cfg(target_family = "wasm")] {
                let use_simplified_wasm_tab_bar = self.get_simplified_wasm_tab_bar_content(app).is_some();
            } else {
                let use_simplified_wasm_tab_bar = false;
            }
        }

        let panels = if use_simplified_wasm_tab_bar {
            // For the simplified WASM tab bar, we want to render the tab bar on top of all other content
            // so that content being added/moved around in the workspace (for example the details panel being toggled)
            // does not affect the tab.
            let mut outer_column = Flex::column();
            if tab_bar_mode == ShowTabBar::Stacked {
                outer_column.add_child(self.render_tab_bar(self.tab_fixed_width, appearance, app));
            }
            let content = self.render_banner_and_active_tab(app, appearance);
            // Hide the vertical tab rail for simplified WASM views (notebooks, shared sessions, etc.)
            let panels_row = self.render_panels(app, Shrinkable::new(1.0, content).finish());
            outer_column.add_child(Shrinkable::new(1.0, panels_row).finish());
            Container::new(outer_column.finish())
                .with_background(util::get_terminal_background_fill(self.window_id, app))
                .finish()
        } else {
            let mut outer_column = Flex::column();
            if tab_bar_mode == ShowTabBar::Stacked {
                outer_column.add_child(self.render_tab_bar(self.tab_fixed_width, appearance, app));
            }
            let content = self.render_banner_and_active_tab(app, appearance);
            let panels_row = self.render_panels(app, Shrinkable::new(1.0, content).finish());
            outer_column.add_child(Shrinkable::new(1.0, panels_row).finish());
            Container::new(outer_column.finish())
                .with_background(util::get_terminal_background_fill(self.window_id, app))
                .finish()
        };
        let mut stack = Stack::new();

        #[cfg(target_family = "wasm")]
        {
            let pane_group = self.active_tab_pane_group().as_ref(app);
            if warpui::platform::wasm::is_mobile_device() && pane_group.left_panel_open {
                let scrim = Rect::new()
                    .with_background(Fill::Solid(ColorU::new(
                        0,
                        0,
                        0,
                        MOBILE_OVERLAY_SCRIM_ALPHA,
                    )))
                    .finish();
                let clickable_scrim = EventHandler::new(scrim)
                    .on_left_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(WorkspaceAction::ToggleLeftPanel);
                        DispatchEventResult::StopPropagation
                    })
                    .finish();
                stack.add_positioned_overlay_child(
                    Percentage::width(1.0 - MOBILE_OVERLAY_PANEL_WIDTH_RATIO, clickable_scrim)
                        .finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        TAB_BAR_POSITION_ID,
                        vec2f(0., 0.),
                        PositionedElementOffsetBounds::WindowBySize,
                        PositionedElementAnchor::BottomRight,
                        ChildAnchor::TopRight,
                    ),
                );

                let panel_content = Container::new(ChildView::new(&self.left_panel_view).finish())
                    .with_background(appearance.theme().surface_1())
                    .finish();
                stack.add_positioned_overlay_child(
                    Percentage::width(MOBILE_OVERLAY_PANEL_WIDTH_RATIO, panel_content).finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        TAB_BAR_POSITION_ID,
                        vec2f(0., 0.),
                        PositionedElementOffsetBounds::WindowBySize,
                        PositionedElementAnchor::BottomLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        stack.add_child(
            Container::new(panels)
                .with_uniform_padding(WORKSPACE_PADDING)
                .finish(),
        );

        // Transcript details panel overlay (right side, mobile only)
        #[cfg(target_family = "wasm")]
        if warpui::platform::wasm::is_mobile_device()
            && self
                .current_workspace_state
                .is_transcript_details_panel_open
        {
            // Dimming scrim on the left (10% width); tapping closes the panel
            let scrim = Rect::new()
                .with_background(Fill::Solid(ColorU::new(
                    0,
                    0,
                    0,
                    MOBILE_OVERLAY_SCRIM_ALPHA,
                )))
                .finish();
            let clickable_scrim = EventHandler::new(scrim)
                .on_left_mouse_down(|ctx, _, _| {
                    ctx.dispatch_typed_action(
                        WorkspaceAction::ToggleConversationTranscriptDetailsPanel,
                    );
                    DispatchEventResult::StopPropagation
                })
                .finish();
            stack.add_positioned_overlay_child(
                Percentage::width(1.0 - MOBILE_OVERLAY_PANEL_WIDTH_RATIO, clickable_scrim).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    TAB_BAR_POSITION_ID,
                    vec2f(0., 0.),
                    PositionedElementOffsetBounds::WindowBySize,
                    PositionedElementAnchor::BottomLeft,
                    ChildAnchor::TopLeft,
                ),
            );

            // Details panel overlay (90% width, positioned on the right)
            let panel_content = ChildView::new(&self.transcript_details_panel).finish();
            stack.add_positioned_overlay_child(
                Percentage::width(MOBILE_OVERLAY_PANEL_WIDTH_RATIO, panel_content).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    TAB_BAR_POSITION_ID,
                    vec2f(0., 0.),
                    PositionedElementOffsetBounds::WindowBySize,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        // Conditionally render tab bar menus.
        if tab_bar_mode.has_tab_bar() && self.show_tab_bar_overflow_menu {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.tab_bar_overflow_menu).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    "tab_bar_overflow_button",
                    vec2f(0., 10.),
                    PositionedElementOffsetBounds::Unbounded,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        if let Some((_, TabContextMenuAnchor::Pointer(position))) = self.show_tab_right_click_menu {
            if tab_bar_mode.has_tab_bar() {
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.tab_right_click_menu).finish(),
                    OffsetPositioning::offset_from_parent(
                        position,
                        ParentOffsetBounds::Unbounded,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        if let Some(position) = self.show_header_toolbar_context_menu {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.header_toolbar_context_menu).finish(),
                OffsetPositioning::offset_from_parent(
                    position,
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );
        }

        // Render the new session dropdown menu. This is outside the tab bar visibility
        // gate because it can also be opened from a keyboard shortcut.
        if self.show_new_session_dropdown_menu.is_some() {
            // TODO(CORE-2300): In the new version of the shell selector, this is not a
            // context menu but a dropdown. Since it is quite wide, we need to reposition
            // it so it does not render outside the bounds of the window.
            let new_session_menu_position = self.show_new_session_dropdown_menu.unwrap();
            let bounds = if FeatureFlag::ShellSelector.is_enabled() {
                ParentOffsetBounds::WindowByPosition
            } else {
                ParentOffsetBounds::Unbounded
            };
            stack.add_positioned_overlay_child(
                ChildView::new(&self.new_session_dropdown_menu).finish(),
                OffsetPositioning::offset_from_parent(
                    new_session_menu_position,
                    bounds,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );

            // Sidecar menu for submenu parents (New worktree config).
            if self.show_new_session_sidecar {
                let anchor_label = self.new_session_dropdown_menu.read(app, |menu, _| {
                    menu.hovered_index().and_then(|idx| {
                        menu.items().get(idx).and_then(|item| match item {
                            MenuItem::Item(fields) => Some(fields.label().to_string()),
                            _ => None,
                        })
                    })
                });

                if let Some(anchor_label) = anchor_label {
                    let sidecar_element = SavePosition::new(
                        ChildView::new(&self.new_session_sidecar_menu).finish(),
                        NEW_SESSION_SIDECAR_POSITION_ID,
                    )
                    .finish();

                    let render_left = false;
                    let (offset, parent_anchor, child_anchor) = if render_left {
                        (
                            vec2f(-4., 0.),
                            PositionedElementAnchor::TopLeft,
                            ChildAnchor::TopRight,
                        )
                    } else {
                        (
                            vec2f(4., 0.),
                            PositionedElementAnchor::TopRight,
                            ChildAnchor::TopLeft,
                        )
                    };

                    stack.add_positioned_overlay_child(
                        sidecar_element,
                        OffsetPositioning::offset_from_save_position_element(
                            anchor_label,
                            offset,
                            PositionedElementOffsetBounds::WindowByPosition,
                            parent_anchor,
                            child_anchor,
                        ),
                    );
                }
            }
        }

        match tab_bar_mode {
            ShowTabBar::Stacked => (), // The tab bar was rendered in the content column.
            ShowTabBar::Hidden => {
                // Hide the tab bar, but include a hover area.
                stack.add_positioned_child(
                    self.render_tab_bar_hover_area(),
                    OffsetPositioning::offset_from_parent(
                        Vector2F::zero(),
                        ParentOffsetBounds::WindowByPosition,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        }

        // If the tab bar is being shown in "stacked" mode, we want to render
        // the traffic lights relative to the full workspace, so they appear
        // in the top-right corner even if a right-side panel is open.
        if tab_bar_mode == ShowTabBar::Stacked {
            self.maybe_render_traffic_lights(&mut stack, app);
        }

        if self.current_workspace_state.is_command_search_open {
            if let Some(active_input_handle) = self.get_active_input_view_handle(app) {
                let input_position = app.view(&active_input_handle).save_position_id();
                let menu_positioning = app.view(&self.command_search_view).menu_positioning();
                // Position the CommandSearchView over the active pane's input.
                let search_panel_margin = 4.;
                let positioning = match menu_positioning {
                    MenuPositioning::AboveInputBox => {
                        OffsetPositioning::offset_from_save_position_element(
                            input_position,
                            vec2f(search_panel_margin, -search_panel_margin),
                            PositionedElementOffsetBounds::WindowBySize,
                            PositionedElementAnchor::BottomLeft,
                            ChildAnchor::BottomLeft,
                        )
                    }
                    MenuPositioning::BelowInputBox => {
                        OffsetPositioning::offset_from_save_position_element(
                            input_position,
                            vec2f(search_panel_margin, 0.),
                            PositionedElementOffsetBounds::WindowBySize,
                            PositionedElementAnchor::TopLeft,
                            ChildAnchor::TopLeft,
                        )
                    }
                };

                stack.add_positioned_child(
                    Container::new(ChildView::new(&self.command_search_view).finish())
                        .with_margin_right(search_panel_margin)
                        .finish(),
                    positioning,
                );
            }
        }

        if self.current_workspace_state.is_palette_open {
            stack.add_overlay_child(ChildView::new(&self.palette).finish());
        }

        if self.current_workspace_state.is_ctrl_tab_palette_open {
            stack.add_child(ChildView::new(&self.ctrl_tab_palette).finish());
        }

        if self.current_workspace_state.is_theme_creator_modal_open {
            stack.add_child(ChildView::new(&self.theme_creator_modal).finish());
        }

        if self.current_workspace_state.is_theme_deletion_modal_open {
            stack.add_child(ChildView::new(&self.theme_deletion_modal).finish());
        }

        if self.launch_config_save_modal.is_open() {
            stack.add_child(self.launch_config_save_modal.render());
        }

        if self.tab_config_params_modal.is_open() {
            stack.add_child(self.tab_config_params_modal.render());
        }

        if self.session_config_modal.is_open() {
            stack.add_child(self.session_config_modal.render());
        }

        if self.current_workspace_state.is_prompt_editor_open {}

        if let Some(lightbox_view) = &self.lightbox_view {
            stack.add_child(ChildView::new(lightbox_view).finish());
        }

        if FeatureFlag::CreatingSharedSessions.is_enabled()
            && ContextFlag::CreateSharedSession.is_enabled()
            && self
                .current_workspace_state
                .is_close_session_confirmation_dialog_open
        {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.close_session_confirmation_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if self.current_workspace_state.is_native_quit_modal_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.native_modal).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if self
            .current_workspace_state
            .is_remove_tab_config_dialog_open
        {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.remove_tab_config_confirmation_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::Center,
                    ChildAnchor::Center,
                ),
            );
        }

        if FeatureFlag::AvatarInTabBar.is_enabled() && self.is_user_menu_open {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.user_menu).finish(),
                OffsetPositioning::offset_from_save_position_element(
                    USER_AVATAR_BUTTON_POSITION_ID,
                    Vector2F::zero(),
                    PositionedElementOffsetBounds::WindowByPosition,
                    PositionedElementAnchor::BottomRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        let window_corner_radius = app.windows().window_corner_radius();
        let workspace = Container::new(stack.finish()).with_corner_radius(window_corner_radius);

        let mut stack = Stack::new();
        let theme = appearance.theme();
        let window_settings = WindowSettings::as_ref(app);
        let background_opacity = window_settings
            .background_opacity
            .effective_opacity(self.window_id, app);

        if let Some(img) = theme.background_image() {
            let opacity_ratio = background_opacity as f32 / 100.;
            stack.add_child(
                Shrinkable::new(
                    1.,
                    Image::new(img.source(), CacheOption::Original)
                        .cover()
                        .with_opacity(opacity_ratio)
                        .with_corner_radius(window_corner_radius)
                        .finish(),
                )
                .finish(),
            );
            stack.add_child(workspace.finish());
        } else {
            stack.add_child(
                workspace
                    .with_background(theme.surface_2().with_opacity(background_opacity))
                    .finish(),
            );
        }

        let input_position_id = self
            .get_active_input_view_handle(app)
            .map(|input| app.view(&input).save_position_id());

        stack.add_positioned_overlay_child(
            ChildView::new(&self.toast_stack).finish(),
            self.global_toast_positioning(),
        );

        if let Some(input_position_id) = input_position_id {
            if FeatureFlag::AvatarInTabBar.is_enabled() && self.is_input_box_visible(app) {
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.update_toast_stack).finish(),
                    self.update_toast_positioning(input_position_id, app),
                );
            }
        }

        #[cfg(target_family = "wasm")]
        if self.show_wasm_nux_dialog {
            stack.add_positioned_overlay_child(
                ChildView::new(&self.wasm_nux_dialog).finish(),
                OffsetPositioning::offset_from_parent(
                    vec2f(-10., 67.),
                    ParentOffsetBounds::WindowByPosition,
                    ParentAnchor::TopRight,
                    ChildAnchor::TopRight,
                ),
            );
        }

        // Add workspace-wide UI event handling.
        let stack = stack.finish();

        #[cfg_attr(not(any(windows, target_os = "linux")), allow(unused_mut))]
        let mut event_handler = EventHandler::new(stack);

        #[cfg(any(windows, target_os = "linux"))]
        {
            event_handler =
                event_handler.on_scroll_wheel(move |ctx, _app, delta, modifiers_state| {
                    if !modifiers_state.ctrl {
                        return DispatchEventResult::PropagateToParent;
                    }

                    // If the control key is being held, scrolling should scale the zoom level or font size
                    if FeatureFlag::UIZoom.is_enabled() {
                        if delta.y() > 0.0 {
                            ctx.dispatch_typed_action(WorkspaceAction::IncreaseZoom);
                        } else if delta.y() < 0.0 {
                            ctx.dispatch_typed_action(WorkspaceAction::DecreaseZoom);
                        }
                    } else if delta.y() > 0.0 {
                        ctx.dispatch_typed_action(WorkspaceAction::IncreaseFontSize);
                    } else if delta.y() < 0.0 {
                        ctx.dispatch_typed_action(WorkspaceAction::DecreaseFontSize);
                    }
                    DispatchEventResult::StopPropagation
                });
        }

        event_handler.finish()
    }
}

fn compute_default_panel_widths(
    app: &AppContext,
    window_id: WindowId,
    has_horizontal_split: bool,
) -> (f32, f32) {
    if let Some(bounds) = app.window_bounds(&window_id) {
        let window_width = bounds.width();
        let left_ratio = 0.15;
        let right_ratio = if has_horizontal_split { 0.3 } else { 0.5 };
        let left = window_width * left_ratio;
        let right = window_width * right_ratio;
        (left, right)
    } else {
        (DEFAULT_LEFT_PANEL_WIDTH, DEFAULT_RIGHT_PANEL_WIDTH)
    }
}

/// Idempotently sets the opencode-warp plugin entry in `~/.config/opencode/opencode.json`.
/// Removes any existing opencode-warp plugin entries (both local file:// and github:) and adds
/// the given `new_entry`. Creates the config file with a default structure if it doesn't exist.
#[cfg(debug_assertions)]
fn set_opencode_warp_plugin(new_entry: &str) -> String {
    let Some(home) = dirs::home_dir() else {
        return "Failed to determine home directory".to_string();
    };

    let config_dir = home.join(".config/opencode");
    let config_path = config_dir.join("opencode.json");

    let mut config: serde_json::Value = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(val) => val,
                Err(e) => return format!("Failed to parse opencode.json: {e}"),
            },
            Err(e) => return format!("Failed to read opencode.json: {e}"),
        }
    } else {
        serde_json::json!({
            "$schema": "https://opencode.ai/config.json"
        })
    };

    let plugins = config.as_object_mut().and_then(|obj| {
        obj.entry("plugin")
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
    });

    let Some(plugins) = plugins else {
        return "opencode.json has unexpected structure (plugin is not an array)".to_string();
    };

    // Remove any existing opencode-warp entries
    plugins.retain(|entry| {
        let s = entry.as_str().unwrap_or("");
        !s.contains("opencode-warp")
    });

    plugins.push(serde_json::Value::String(new_entry.to_string()));

    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        return format!("Failed to create config directory: {e}");
    }

    match serde_json::to_string_pretty(&config) {
        Ok(json_str) => match std::fs::write(&config_path, format!("{json_str}\n")) {
            Ok(()) => format!("OpenCode plugin set to: {new_entry}"),
            Err(e) => format!("Failed to write opencode.json: {e}"),
        },
        Err(e) => format!("Failed to serialize opencode.json: {e}"),
    }
}
