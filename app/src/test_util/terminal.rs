use warpui::{platform::WindowStyle, App, ViewHandle, WindowId};

use crate::context_chips::prompt::Prompt;
use crate::terminal::model::block::SerializedBlock;
use crate::terminal::history::History;
use crate::settings_view::keybindings::KeybindingChangedNotifier;
use crate::settings_view::DisplayCount;
use crate::appearance::Appearance;
use crate::terminal::alt_screen_reporting::AltScreenReporting;
use crate::{settings::PrivacySettings, test_util::settings::initialize_settings_for_tests};
use crate::{resource_center::TipsCompleted, terminal::TerminalView};
use crate::workspace::{ActiveSession, OneTimeModalModel};
use crate::workspace::sync_inputs::SyncedInputState;

type SerializedBlockListItem = SerializedBlock;

pub fn initialize_app_for_terminal_view(app: &mut App) {
    initialize_settings_for_tests(app);
    app.add_singleton_model(|_| Appearance::mock());
    app.add_singleton_model(|_| DisplayCount(1));
    app.add_singleton_model(PrivacySettings::mock);
    app.add_singleton_model(|_| ActiveSession::default());
    app.add_singleton_model(|_| History::default());
    app.add_singleton_model(OneTimeModalModel::new);
    app.add_singleton_model(|_| SyncedInputState::new());
    app.add_singleton_model(|_| KeybindingChangedNotifier::new());
    app.add_singleton_model(Prompt::new);
    AltScreenReporting::register(app);
}

pub fn add_window_with_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlockListItem]>,
) -> ViewHandle<TerminalView> {
    add_window_with_id_and_terminal(app, restored_blocks).1
}

pub fn add_window_with_id_and_terminal(
    app: &mut App,
    restored_blocks: Option<&[SerializedBlockListItem]>,
) -> (WindowId, ViewHandle<TerminalView>) {
    let tips_model = app.add_model(|_| TipsCompleted::default());
    app.add_window(WindowStyle::NotStealFocus, |ctx| {
        TerminalView::new_for_test(tips_model, restored_blocks, ctx)
    })
}
