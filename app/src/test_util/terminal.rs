use warpui::{platform::WindowStyle, App, ViewHandle, WindowId};

use crate::terminal::model::block::SerializedBlock;
use crate::{resource_center::TipsCompleted, terminal::TerminalView};

type SerializedBlockListItem = SerializedBlock;

pub fn initialize_app_for_terminal_view(_app: &mut App) {}

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
