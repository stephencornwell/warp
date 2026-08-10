use std::path::PathBuf;

use warp_util::path::LineAndColumnArg;

use crate::palette::PaletteMode;
use crate::pane_group::PaneGroup;
use crate::search;
use crate::settings_view::{SettingsAction as SettingsTabAction, SettingsSection};
use crate::sync_ids::SyncId;
use crate::tab::{NewSessionMenuItem, SelectedTabColor};
use crate::tab_configs::TabConfig;
use crate::terminal::available_shells::AvailableShell;
use crate::themes::theme::AnsiColorIdentifier;
use crate::themes::theme_chooser::ThemeChooserMode;
use crate::workspace::PaneViewLocator;

use ui_components::lightbox;
use warpui::accessibility::AccessibilityVerbosity;
use warpui::geometry::rect::RectF;
use warpui::geometry::vector::Vector2F;
use warpui::platform::Cursor;
use warpui::{EntityId, WeakViewHandle, WindowId};

use super::view::WorkspaceBanner;

/// This enum determines how the search query is initialized when opening command search.
#[derive(Clone, Default, Debug)]
pub enum InitContent {
    /// Read the content of the active terminal input, and make that the initial search query.
    #[default]
    FromInputBuffer,
    /// Specify an exact string to initialize the query to.
    Custom(String),
}

/// To initialize command search, we may want to specify a search filter, or the content of the
/// query itself.
#[derive(Clone, Default, Debug)]
pub struct CommandSearchOptions {
    pub filter: Option<search::QueryFilter>,
    pub init_content: InitContent,
}

#[derive(Clone, Copy, Debug)]
pub enum AddTabWithShellSource {
    CommandPalette,
    ShellSelectorMenu,
}

#[derive(Clone, Debug)]
pub enum PaletteSource {
    ContextChip,
    CtrlTab { query: Option<String> },
    IntegrationTest,
    Keybinding,
    QuitModal,
    TitleBarSearchBar,
}

/// Specifies how to restore a conversation when it's not already open in a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum RestoreConversationLayout {
    /// Restore the conversation into the currently active pane.
    ActivePane,
    /// Restore the conversation in a new split pane.
    SplitPane,
    /// Restore the conversation in a new tab.
    #[default]
    NewTab,
}

#[derive(Debug, Clone, Copy)]
pub enum TabContextMenuAnchor {
    Pointer(Vector2F),
}

#[derive(Debug, Clone)]
pub enum WorkspaceAction {
    ActivateTab(usize),
    ActivatePrevTab,
    ActivateNextTab,
    ActivateLastTab,
    CyclePrevSession,
    CycleNextSession,
    MoveActiveTabLeft,
    MoveActiveTabRight,
    MoveTabLeft(usize),
    MoveTabRight(usize),
    RenameTab(usize),
    ResetTabName(usize),
    RenamePane(PaneViewLocator),
    ResetPaneName(PaneViewLocator),
    RenameActiveTab,
    SetActiveTabName(String),
    /// Sets the manual color override for the active tab.
    ///
    /// - `Color(_)` — apply that color.
    /// - `Cleared` — explicitly clear (suppresses any directory default).
    /// - `Unset` — remove the manual override (lets the directory default apply, if any).
    SetActiveTabColor(SelectedTabColor),
    ToggleTabRightClickMenu {
        tab_index: usize,
        anchor: TabContextMenuAnchor,
    },
    TabHoverWidthStart {
        width: f32,
    },
    TabHoverWidthEnd,
    ToggleTabBarOverflowMenu,
    ToggleWelcomeTips,
    CloseTab(usize),
    CloseActiveTab,
    CloseOtherTabs(usize),
    CloseNonActiveTabs,
    CloseTabsRight(usize),
    CloseTabsRightActiveTab,
    AddDefaultTab,
    AddTerminalTab {
        hide_homepage: bool,
    },
    AddTabWithShell {
        shell: AvailableShell,
        source: AddTabWithShellSource,
    },
    AddGetStartedTab,
    AddAmbientAgentTab,
    /// Add a new tab that immediately enters agent view with a new conversation.
    AddAgentTab,
    /// Add a new tab running a local Docker sandbox via `sbx`.
    AddDockerSandboxTab,
    OpenNewSessionMenu {
        position: Vector2F,
    },
    ToggleTabConfigsMenu,
    ToggleNewSessionMenu {
        position: Vector2F,
    },
    SelectNewSessionMenuItem(NewSessionMenuItem),
    ApplyUpdate,
    LogOut,
    CopyVersion(&'static str),
    DownloadNewVersion,
    ConfigureKeybindingSettings {
        keybinding_name: Option<String>,
    },
    ShowSettings,
    ShowSettingsPage(SettingsSection),
    ShowSettingsPageWithSearch {
        search_query: String,
        section: Option<SettingsSection>,
    },
    ShowThemeChooser(ThemeChooserMode),
    ShowThemeChooserForActiveTheme,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    IncreaseZoom,
    DecreaseZoom,
    ResetZoom,
    ActivateTabByNumber(usize),
    OpenPalette {
        mode: PaletteMode,
        source: PaletteSource,
        query: Option<String>,
    },
    TogglePalette {
        mode: PaletteMode,
        source: PaletteSource,
    },
    ShowUpgrade,
    ShowReferralSettingsPage,
    JoinSlack,
    ViewUserDocs,
    ViewLatestChangelog,
    ViewPrivacyPolicy,
    SendFeedback,
    /// Open the log directory in the system file explorer with the current log file selected.
    #[cfg(not(target_family = "wasm"))]
    ViewLogs,
    ChangeCursor(Cursor),
    ToggleBlockSnackbar,
    ToggleErrorUnderlining,
    ToggleSyntaxHighlighting,
    CheckForUpdate,
    ExportAllWarpDriveObjects,
    SetA11yVerbosityLevel(AccessibilityVerbosity),
    ToggleNotifications,
    ToggleTabColor {
        color: AnsiColorIdentifier,
        tab_index: usize,
    },
    OpenLaunchConfigSaveModal,
    SelectTabConfig(TabConfig),
    DispatchToSettingsTab(SettingsTabAction),
    ToggleResourceCenter,
    ToggleUserMenu,
    ToggleAIAssistant,
    ClickedAIAssistantIcon,
    ToggleKeybindingsPage,
    ShowCommandSearch(CommandSearchOptions),
    CreatePersonalNotebook,
    ImportToPersonalDrive,
    ImportToTeamDrive,
    CreateTeamNotebook,
    CreatePersonalWorkflow,
    CreateTeamWorkflow,
    CreatePersonalFolder,
    CreateTeamFolder,
    CreatePersonalAIPrompt,
    CreateTeamAIPrompt,
    ToggleMouseReporting,
    ToggleScrollReporting,
    ToggleFocusReporting,
    StartTabDrag,
    DragTab {
        tab_index: usize,
        tab_position: RectF,
    },
    HandoffPendingTransfer {
        target_window_id: WindowId,
        insertion_index: usize,
    },
    ReverseHandoff {
        target_window_id: WindowId,
        target_insertion_index: usize,
    },
    DropTab,
    FinalizeDropTab,
    /// Toggles the left panel. In Code Mode V1 this toggles Warp Drive.
    /// In Code Mode V2 this toggles the left panel which contains both the project explorer and
    /// Warp Drive. This happens as explicit action from the user.
    ToggleLeftPanel,
    /// Toggles directly to the Warp Drive tab of the left panel in Code Mode V2
    ToggleWarpDrive,
    /// Unconditionally opens Warp Drive. This is used in the case of user lifecycle
    /// events like new user onboarding or when the user joins a team.
    OpenWarpDrive,
    /// Toggles the right panel. This happens as an explicit action from the user.
    ToggleRightPanel,
    /// Opens the code review panel (right panel) without toggling. If already open,
    /// switches to the target pane's repo. Used by vertical tabs diff stats chip.
    OpenCodeReviewPanel(PaneViewLocator),
    /// Closes the focused panel. This happens as an explicit action from the user.
    ClosePanel,
    CopyTextToClipboard(String),
    /// An action only registered in dev and local builds, which writes the user's current access
    /// token to the system clipboard to aid debugging and development.
    CopyAccessTokenToClipboard,
    DismissWorkspaceBanner(WorkspaceBanner),
    /// An action only registered in dev and local builds, which crashes the
    /// app (via a Sentry helper method) immediately when called.
    Crash,
    /// An action only registered in dev and local builds, which triggers a
    /// panic immediately when called.
    Panic,
    /// Stops the heap profiler (if one is running) and writes the profiling
    /// data to disk.
    DumpHeapProfile,
    ShowAIAssistantWarmWelcome,
    ClickedAIAssistantWarmWelcome,
    /// An action to open a new window with a view hierarchy debugger.
    OpenViewTreeDebugWindow,
    DismissAIAssistantWarmWelcome,
    /// An action to either upgrade syncing status from none or just in one tab
    /// to syncing all tabs, or downgrade from syncing all tabs to no syncing
    ToggleSyncAllTerminalInputsInAllTabs,
    /// An action to either cancel syncing
    /// or switch from no syncing/syncing all tabs to syncing within one tab
    ToggleSyncTerminalInputsInTab,
    /// An action to force terminal input syncing off
    DisableTerminalInputSync,
    HandleConflictingWorkflow(SyncId),
    OpenAgentToolbarEditor,
    OpenCLIAgentToolbarEditor,
    ShowHeaderToolbarContextMenu {
        position: Vector2F,
    },
    Reauth,
    SignupAnonymousUser,
    SignInAnonymousWebUser,
    OpenLink(String),
    /// On WASM, opens a given URL in the desktop Warp app (if installed) or redirects to download page.
    #[cfg(target_family = "wasm")]
    OpenLinkOnDesktop(url::Url),
    ReopenClosedSession,
    OpenShareSessionModal(usize),
    StopSharingSessionFromTabMenu {
        terminal_view_id: EntityId,
    },
    StopSharingAllSessionsInTab {
        pane_group: WeakViewHandle<PaneGroup>,
    },
    CopySharedSessionLinkFromTab {
        tab_index: usize,
    },
    AddWindow,
    AddWindowWithShell {
        shell: AvailableShell,
    },
    /// Moves focus to the panel on the left
    FocusLeftPanel,
    /// Moves focus to the panel on the right
    FocusRightPanel,
    /// Open a local path in the file explorer.
    OpenInExplorer {
        path: PathBuf,
    },
    /// Open a local file with the system's default application.
    OpenFilePath {
        path: PathBuf,
    },
    TerminateApp,
    CloseWindow,
    /// Help the user call the Warp executable with the [`crate::args::DEBUG_DUMP_FLAG`].
    DumpDebugInfo,
    /// Log review comment send eligibility for panes in the active tab.
    LogReviewCommentSendStatusForActiveTab,
    ToggleRecordingMode,
    ToggleInBandGenerators,
    ToggleDebugNetworkStatus,
    ToggleShowMemoryStats,
    RunAISuggestedCommand(String),
    RunCommand(String),
    InsertInInput {
        content: String,
        replace_buffer: bool,
        /// Whether to ensure agent mode is enabled when inserting content
        ensure_agent_mode: bool,
    },
    OpenCloudAgentSetupGuide,
    AttemptLoginGatedAIUpgrade,
    /// Open a new pane with its input in AI mode
    /// with query "Fix this" with error name and details from AI summary.
    FixInAgentMode {
        query: String,
    },
    OpenAIFactCollection,
    OpenMCPServerCollection,
    FocusTerminalViewInWorkspace {
        terminal_view_id: EntityId,
    },
    /// Focus a specific pane by its locator (pane_group_id and pane_id).
    FocusPane(PaneViewLocator),
    /// Start a new AI conversation in a terminal view. This sets the pending query state
    /// to default and focuses the terminal view.
    StartNewConversation {
        terminal_view_id: EntityId,
    },
    /// Jump to the terminal pane of the most recent agent toast
    JumpToLatestToast,
    /// Open a file in a new tab with a code pane
    OpenFileInNewTab {
        full_path: PathBuf,
        line_and_column: Option<LineAndColumnArg>,
    },
    OpenNotebook {
        id: SyncId,
    },
    ScrollToSettingsWidget {
        page: SettingsSection,
        widget_id: &'static str,
    },
    /// Install the Warp CLI command to /usr/local/bin
    #[cfg(target_os = "macos")]
    InstallCLI,
    /// Uninstall the Warp CLI command from /usr/local/bin
    #[cfg(target_os = "macos")]
    UninstallCLI,
    UndoRevertInCodeReviewPane {
        window_id: WindowId,
        view_id: EntityId,
    },
    /// Handle a file being renamed in the file tree
    #[cfg(feature = "local_fs")]
    FileRenamed {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    /// Handle a file being deleted in the file tree
    #[cfg(feature = "local_fs")]
    FileDeleted {
        path: PathBuf,
    },
    /// Open a repository directory via file picker. The `path` is an `Option` because some
    /// dispatchers don't know the path to open yet (so the Workspace must open the file picker)
    /// and some do, e.g. the GetStartedView. The GetStartedView needs to handle the file picker
    /// because it needs to determine whether or not to close itself based on whether the user
    /// actually selects a file in the file picker or cancels it.
    OpenRepository {
        path: Option<String>,
    },
    /// Open a new blank code file in the current tab
    NewCodeFile,
    NavigatePrevPaneOrPanel,
    NavigateNextPaneOrPanel,
    ToggleProjectExplorer,
    ToggleGlobalSearch,
    OpenGlobalSearch,
    /// Open the Build Plan Migration Modal (for debugging)
    #[cfg(debug_assertions)]
    OpenBuildPlanMigrationModal,
    /// Reset the build plan migration modal dismissed state (for debugging)
    #[cfg(debug_assertions)]
    ResetBuildPlanMigrationModalState,
    /// Reset the AWS Bedrock login banner dismissed state (for debugging).
    #[cfg(debug_assertions)]
    DebugResetAwsBedrockLoginBannerDismissed,
    /// Open the OpenWarp Launch Modal (for debugging)
    #[cfg(debug_assertions)]
    OpenOpenWarpLaunchModal,
    /// Reset the OpenWarp launch modal dismissed state (for debugging)
    #[cfg(debug_assertions)]
    ResetOpenWarpLaunchModalState,
    /// Install the opencode-warp plugin from GitHub into the global opencode config.
    #[cfg(debug_assertions)]
    InstallOpenCodeWarpPlugin,
    /// Use a local checkout of the opencode-warp plugin (for testing/development).
    #[cfg(debug_assertions)]
    UseLocalOpenCodeWarpPlugin,
    /// Take a process sample of the app (equivalent to Activity Monitor > Sample Process).
    #[cfg(target_os = "macos")]
    SampleProcess,
    ToggleNotificationMailbox {
        select_first: bool,
    },
    ToggleAgentManagementView,
    ViewAgentRunsForEnvironment {
        environment_id: String,
    },
    /// Toggle the conversation transcript details panel (WASM-only).
    #[cfg(target_family = "wasm")]
    ToggleConversationTranscriptDetailsPanel,
    /// Open a full-window lightbox displaying the given images.
    OpenLightbox {
        images: Vec<lightbox::LightboxImage>,
        /// The index of the image to display initially.
        initial_index: usize,
    },
    /// Update a single image in the currently open lightbox.
    UpdateLightboxImage {
        index: usize,
        image: lightbox::LightboxImage,
    },
    ShowSessionConfigModal,
    DismissSessionConfigTabConfigChip,
    SaveCurrentTabAsNewConfig(usize),
    SyncTrafficLights,
    /// Opens a tab config file in the editor and dismisses the associated error toast.
    OpenTabConfigErrorFile {
        path: PathBuf,
        toast_object_id: String,
    },
    /// Sidecar action: open the tab config TOML in the user's editor.
    TabConfigSidecarEditConfig {
        path: PathBuf,
    },
    /// Sidecar action: show the remove confirmation dialog for a tab config.
    TabConfigSidecarRemoveConfig {
        name: String,
        path: PathBuf,
    },
    /// Opens the settings.toml file in a code editor pane.
    OpenSettingsFile,
    /// Opens a new agent session to fix settings.toml errors using the modify-settings skill.
    FixSettingsWithOz {
        error_description: String,
    },
}

impl WorkspaceAction {
    /// Matches what actions require the app state to be saved, and which don't. We match all
    /// actions directly, rather than using _, so we're forced to make a conscious decision for each
    /// of them, rather than following some default.
    pub fn should_save_app_state_on_action(&self) -> bool {
        use WorkspaceAction::*;
        match self {
            #[cfg(not(target_family = "wasm"))]
            ActivateTab(_)
            | ActivateTabByNumber(_)
            | ActivatePrevTab
            | ActivateNextTab
            | ActivateLastTab
            | CyclePrevSession
            | CycleNextSession
            | MoveActiveTabLeft
            | MoveActiveTabRight
            | MoveTabLeft(_)
            | MoveTabRight(_)
            | DropTab
            | RenameTab(_)
            | ResetTabName(_)
            | RenamePane(_)
            | ResetPaneName(_)
            | RenameActiveTab
            | SetActiveTabName(_)
            | SetActiveTabColor(_)
            | CloseTab(_)
            | CloseActiveTab
            | CloseOtherTabs(_)
            | CloseNonActiveTabs
            | CloseTabsRight(_)
            | CloseTabsRightActiveTab
            | ToggleTabColor { .. }
            | AddDefaultTab
            | AddTerminalTab { .. }
            | AddTabWithShell { .. }
            | AddGetStartedTab
            | AddAgentTab
            | AddAmbientAgentTab
            | AddDockerSandboxTab
            | AddWindow
            | AddWindowWithShell { .. }
            | CloseWindow
            | ScrollToSettingsWidget { .. }
            | FixInAgentMode { .. }
            | OpenNotebook { .. }
            | OpenFileInNewTab { .. }
            | NewCodeFile
            | OpenRepository { .. }
            | SelectTabConfig(_) => true, // actions that actually change a state of the state of user's
            // workspace would most likely require a save, so that if the app gets
            // restarted, the user can continue working
            ApplyUpdate
            | CopyVersion(_)
            | DownloadNewVersion
            | ConfigureKeybindingSettings { .. }
            | ExportAllWarpDriveObjects
            | ShowSettings
            | ShowSettingsPage(_)
            | ShowSettingsPageWithSearch { .. }
            | ShowThemeChooser(_)
            | ShowThemeChooserForActiveTheme
            | IncreaseFontSize
            | DecreaseFontSize
            | ResetFontSize
            | IncreaseZoom
            | DecreaseZoom
            | ResetZoom
            | OpenPalette { .. }
            | TogglePalette { mode: _, source: _ }
            | ShowUpgrade
            | ShowReferralSettingsPage
            | JoinSlack
            | ViewUserDocs
            | ViewLatestChangelog
            | ViewPrivacyPolicy
            | SendFeedback
            | ChangeCursor(_)
            | ToggleBlockSnackbar
            | ToggleErrorUnderlining
            | ToggleSyntaxHighlighting
            | OpenLaunchConfigSaveModal
            | ToggleTabRightClickMenu { .. }
            | OpenNewSessionMenu { .. }
            | ToggleTabConfigsMenu
            | ToggleNewSessionMenu { .. }
            | SelectNewSessionMenuItem(_)
            | ToggleTabBarOverflowMenu
            | CheckForUpdate
            | SetA11yVerbosityLevel(_)
            | ToggleNotifications
            | DispatchToSettingsTab { .. }
            | ToggleResourceCenter
            | ToggleUserMenu
            | ClickedAIAssistantIcon
            | ToggleAIAssistant
            | OpenCloudAgentSetupGuide
            | ToggleKeybindingsPage
            | ShowCommandSearch(_)
            | ToggleMouseReporting
            | ToggleScrollReporting
            | ToggleFocusReporting
            | ImportToPersonalDrive
            | ImportToTeamDrive
            | CreatePersonalNotebook
            | CreateTeamNotebook
            | CreatePersonalWorkflow
            | CreateTeamWorkflow
            | CreatePersonalFolder
            | CreateTeamFolder
            | CreatePersonalAIPrompt
            | CreateTeamAIPrompt
            | OpenInExplorer { .. }
            | DragTab { .. }
            | HandoffPendingTransfer { .. }
            | ReverseHandoff { .. }
            | StartTabDrag
            | FinalizeDropTab
            | ToggleLeftPanel
            | ToggleWarpDrive
            | OpenWarpDrive
            | ClosePanel
            | ToggleRightPanel
            | OpenCodeReviewPanel(..)
            | ToggleWelcomeTips
            | CopyTextToClipboard(_)
            | CopyAccessTokenToClipboard
            | Crash
            | Panic
            | DumpHeapProfile
            | OpenViewTreeDebugWindow
            | ShowAIAssistantWarmWelcome
            | ClickedAIAssistantWarmWelcome
            | DismissAIAssistantWarmWelcome
            | DismissWorkspaceBanner(..)
            | ToggleSyncAllTerminalInputsInAllTabs
            | ToggleSyncTerminalInputsInTab
            | DisableTerminalInputSync
            | HandleConflictingWorkflow(_)
            | OpenAgentToolbarEditor
            | OpenCLIAgentToolbarEditor
            | ShowHeaderToolbarContextMenu { .. }
            | Reauth
            | SignupAnonymousUser
            | LogOut
            | OpenLink(_)
            | OpenShareSessionModal(_)
            | StopSharingSessionFromTabMenu { .. }
            | StopSharingAllSessionsInTab { .. }
            | CopySharedSessionLinkFromTab { .. }
            | ReopenClosedSession
            | FocusLeftPanel
            | FocusRightPanel
            | DumpDebugInfo
            | LogReviewCommentSendStatusForActiveTab
            | ToggleRecordingMode
            | ToggleInBandGenerators
            | ToggleDebugNetworkStatus
            | ToggleShowMemoryStats
            | RunAISuggestedCommand { .. }
            | RunCommand { .. }
            | InsertInInput { .. }
            | AttemptLoginGatedAIUpgrade
            | OpenFilePath { .. }
            | TerminateApp
            | SignInAnonymousWebUser
            | TabHoverWidthStart { .. }
            | TabHoverWidthEnd
            | OpenAIFactCollection
            | OpenMCPServerCollection
            | FocusTerminalViewInWorkspace { .. }
            | FocusPane(..)
            | StartNewConversation { .. }
            | UndoRevertInCodeReviewPane { .. }
            | JumpToLatestToast
            | NavigatePrevPaneOrPanel
            | NavigateNextPaneOrPanel
            | ToggleProjectExplorer
            | ToggleGlobalSearch
            | OpenGlobalSearch
            | ToggleNotificationMailbox { .. }
            | ToggleAgentManagementView
            | ViewAgentRunsForEnvironment { .. }
            | OpenLightbox { .. }
            | UpdateLightboxImage { .. }
            | ShowSessionConfigModal
            | DismissSessionConfigTabConfigChip
            | SaveCurrentTabAsNewConfig(_)
            | SyncTrafficLights
            | OpenTabConfigErrorFile { .. }
            | TabConfigSidecarEditConfig { .. }
            | TabConfigSidecarRemoveConfig { .. }
            | OpenSettingsFile
            | FixSettingsWithOz { .. } => false,
            #[cfg(debug_assertions)]
            #[cfg(target_family = "wasm")]
            ToggleConversationTranscriptDetailsPanel => false,
            #[cfg(debug_assertions)]
            OpenBuildPlanMigrationModal
            | ResetBuildPlanMigrationModalState
            | DebugResetAwsBedrockLoginBannerDismissed
            | OpenOpenWarpLaunchModal
            | ResetOpenWarpLaunchModalState
            | InstallOpenCodeWarpPlugin
            | UseLocalOpenCodeWarpPlugin => false,
            #[cfg(not(target_family = "wasm"))]
            ViewLogs => false,
            #[cfg(target_os = "macos")]
            SampleProcess => false,
            #[cfg(target_os = "macos")]
            InstallCLI | UninstallCLI => false,
            #[cfg(feature = "local_fs")]
            FileRenamed { .. } => false, // File rename doesn't change workspace state
            #[cfg(feature = "local_fs")]
            FileDeleted { .. } => false, // File deletion doesn't change workspace state
            #[cfg(target_family = "wasm")]
            OpenLinkOnDesktop(_) => false,
            // actions that are related to updating user settings or
            // managing some ui elements (like closing/opening modals)
            // that don't reflect on actual workspace and don't need to
            // be preserved between restarts.
        }
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
