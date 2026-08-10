#[cfg(test)]
use warpui::App;

#[cfg(test)]
pub fn initialize_settings_for_tests(app: &mut App) {
    use warp_core::execution_mode::ExecutionMode;
    initialize_settings_for_tests_with_mode(app, ExecutionMode::App, false);
}

#[cfg(test)]
pub fn initialize_settings_for_tests_with_mode(
    app: &mut App,
    mode: warp_core::execution_mode::ExecutionMode,
    is_sandboxed: bool,
) {
    use crate::{
        settings::{
            init_and_register_user_preferences, manager::SettingsManager, AccessibilitySettings,
            app_icon::AppIconSettings, AliasExpansionSettings, AppEditorSettings,
            BlockVisibilitySettings, ChangelogSettings,
            CodeSettings, DebugSettings, EmacsBindingsSettings, FontSettings, GPUSettings,
            InputModeSettings, InputSettings, NativePreferenceSettings, PaneSettings,
            SameLinePromptBlockSettings, ScrollSettings, SelectionSettings, SshSettings,
            ThemeSettings, VimBannerSettings,
        },
        terminal::{
            general_settings::GeneralSettings, keys_settings::KeysSettings,
            ligature_settings::LigatureSettings, safe_mode_settings::SafeModeSettings,
            session_settings::SessionSettings, settings::TerminalSettings, BlockListSettings,
        },
        search::command_search::settings::CommandSearchSettings,
        undo_close::UndoCloseSettings,
        user_config::WarpConfig,
        window_settings::WindowSettings,
        workspace::tab_settings::TabSettings,
    };
    use warp_core::execution_mode::AppExecutionMode;
    app.add_singleton_model(|ctx| AppExecutionMode::new(mode, is_sandboxed, ctx));
    app.update(init_and_register_user_preferences);
    app.add_singleton_model(|_ctx| SettingsManager::default());
    app.add_singleton_model(WarpConfig::mock);
    AccessibilitySettings::register(app);
    AppIconSettings::register(app);
    AliasExpansionSettings::register(app);
    AppEditorSettings::register(app);
    BlockVisibilitySettings::register(app);
    BlockListSettings::register(app);
    ChangelogSettings::register(app);
    CommandSearchSettings::register(app);
    CodeSettings::register(app);
    DebugSettings::register(app);
    EmacsBindingsSettings::register(app);
    FontSettings::register(app);
    GeneralSettings::register(app);
    GPUSettings::register(app);
    InputModeSettings::register(app);
    InputSettings::register(app);
    KeysSettings::register(app);
    LigatureSettings::register(app);
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    crate::settings::LinuxAppConfiguration::register(app);
    NativePreferenceSettings::register(app);
    SafeModeSettings::register(app);
    SameLinePromptBlockSettings::register(app);
    ScrollSettings::register(app);
    SelectionSettings::register(app);
    SessionSettings::register(app);
    SshSettings::register(app);
    TabSettings::register(app);
    TerminalSettings::register(app);
    PaneSettings::register(app);
    ThemeSettings::register(app);
    UndoCloseSettings::register(app);
    VimBannerSettings::register(app);
    WindowSettings::register(app);
    warp_core::semantic_selection::SemanticSelection::register(app);
    #[cfg(feature = "local_fs")]
    crate::util::file::external_editor::EditorSettings::register(app);
}
