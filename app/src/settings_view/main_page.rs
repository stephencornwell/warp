use super::{
    settings_page::{PageType, SettingsPageMeta, SettingsPageViewHandle},
    SettingsAction, SettingsSection,
};
use warpui::{
    elements::Element,
    Action, AppContext, Entity, TypedActionView, View, ViewContext, ViewHandle,
};

#[derive(Debug, Clone)]
pub enum MainPageAction {
    Relaunch,
    DownloadUpdate,
    CheckForUpdate,
    OpenUrl(String),
}

#[derive(Clone, Copy)]
pub enum MainSettingsPageEvent {
    CheckForUpdate,
}

pub struct MainSettingsPageView {
    page: PageType<Self>,
}

impl Entity for MainSettingsPageView {
    type Event = MainSettingsPageEvent;
}

impl TypedActionView for MainSettingsPageView {
    type Action = MainPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            MainPageAction::Relaunch | MainPageAction::DownloadUpdate => {}
            MainPageAction::CheckForUpdate => ctx.emit(MainSettingsPageEvent::CheckForUpdate),
            MainPageAction::OpenUrl(url) => ctx.open_url(url),
        }
    }
}

impl View for MainSettingsPageView {
    fn ui_name() -> &'static str {
        "MainSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl MainSettingsPageView {
    pub fn new(_: &mut ViewContext<MainSettingsPageView>) -> Self {
        Self {
            page: PageType::new_uncategorized(Vec::new(), None),
        }
    }
}

impl SettingsPageMeta for MainSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Account
    }

    fn should_render(&self, _: &AppContext) -> bool {
        true
    }

    fn on_page_selected(&mut self, _: bool, _: &mut ViewContext<Self>) {}

    fn update_filter(
        &mut self,
        query: &str,
        ctx: &mut ViewContext<Self>,
    ) -> super::settings_page::MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<MainSettingsPageView>> for SettingsPageViewHandle {
    fn from(view: ViewHandle<MainSettingsPageView>) -> Self {
        SettingsPageViewHandle::Main(view)
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    _: &mut AppContext,
    _: &warpui::keymap::ContextPredicate,
    _: fn(SettingsAction) -> T,
) {
}

pub fn handle_experiment_change(_: &mut AppContext) {}
