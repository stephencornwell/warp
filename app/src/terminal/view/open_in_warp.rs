use crate::report_if_error;
use std::path::PathBuf;

use warpui::{
    accessibility::{AccessibilityContent, ActionAccessibilityContent, WarpA11yRole},
    SingletonEntity, ViewContext,
};

#[cfg(feature = "local_fs")]
use crate::code::editor_management::CodeSource;
use crate::{
    terminal::{general_settings::GeneralSettings, view::inline_banner::OpenInWarpBannerAction},
    util::openable_file_type::OpenableFileType,
};
use settings::Setting as _;

use super::{Event, TerminalView};

const LEARN_MORE_MARKDOWN_URL: &str =
    "https://docs.warp.dev/terminal/more-features/markdown-viewer";
const LEARN_MORE_CODE_URL: &str = "https://docs.warp.dev/code/overview#built-in-code-editor";

/// A path to a file that can be opened in Warp, along with its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenablePath {
    pub path: PathBuf,
    pub file_type: OpenableFileType,
}

impl TerminalView {
    /// Whether or not the "Open in Warp" banner is open.
    #[cfg(feature = "integration_tests")]
    pub fn is_open_in_warp_banner_open(&self) -> bool {
        self.inline_banners_state.open_in_warp_banner.is_some()
    }

    fn close_open_in_warp_banner(&mut self, banner_id: usize) {
        self.model
            .lock()
            .block_list_mut()
            .remove_inline_banner(banner_id);
    }

    pub fn handle_open_in_warp_banner_action(
        &mut self,
        action: OpenInWarpBannerAction,
        ctx: &mut ViewContext<Self>,
    ) {
        match action {
            OpenInWarpBannerAction::OpenFile => {
                if let Some(banner_state) = self.inline_banners_state.open_in_warp_banner.take() {
                    match banner_state.target.file_type {
                        OpenableFileType::Markdown => {
                            ctx.emit(Event::OpenFileInWarp {
                                path: banner_state.target.path,
                                session: banner_state.session,
                            });
                        }
                        OpenableFileType::Code | OpenableFileType::Text => {
                            #[cfg(feature = "local_fs")]
                            ctx.emit(Event::OpenCodeInWarp {
                                source: CodeSource::Link {
                                    path: banner_state.target.path,
                                    range_start: None,
                                    range_end: None,
                                },
                                layout: *crate::terminal::view::EditorSettings::as_ref(ctx)
                                    .open_file_layout
                                    .value(),
                            });
                        }
                    }
                    self.close_open_in_warp_banner(banner_state.id);
                    ctx.notify();
                }
            }
            OpenInWarpBannerAction::LearnMore => {
                if let Some(banner_state) = &self.inline_banners_state.open_in_warp_banner {
                    let url = match banner_state.target.file_type {
                        OpenableFileType::Markdown => LEARN_MORE_MARKDOWN_URL,
                        OpenableFileType::Code | OpenableFileType::Text => LEARN_MORE_CODE_URL,
                    };
                    ctx.open_url(url);
                }
            }
            OpenInWarpBannerAction::Close => {
                if let Some(banner_state) = self.inline_banners_state.open_in_warp_banner.take() {
                    self.close_open_in_warp_banner(banner_state.id);
                    match banner_state.target.file_type {
                        OpenableFileType::Markdown => {
                            GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .open_in_warp_banner_dismissed_for_markdown
                                    .set_value(true, ctx));
                            });
                        }
                        OpenableFileType::Code | OpenableFileType::Text => {
                            GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                                report_if_error!(settings
                                    .open_in_warp_banner_dismissed_for_code_and_text
                                    .set_value(true, ctx));
                            });
                        }
                    }
                    ctx.notify();
                }
            }
        }
    }

    pub fn open_in_warp_banner_accessibility_content(
        &self,
        action: OpenInWarpBannerAction,
    ) -> ActionAccessibilityContent {
        match action {
            OpenInWarpBannerAction::OpenFile => {
                match &self.inline_banners_state.open_in_warp_banner {
                    Some(banner_state) => {
                        ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                            format!("Open {} in Warp", banner_state.target.path.display()),
                            WarpA11yRole::UserAction,
                        ))
                    }
                    None => ActionAccessibilityContent::Empty,
                }
            }
            OpenInWarpBannerAction::Close => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new_without_help(
                    "Close View in Warp banner",
                    WarpA11yRole::UserAction,
                ))
            }
            OpenInWarpBannerAction::LearnMore => {
                ActionAccessibilityContent::Custom(AccessibilityContent::new(
                    "Learn more",
                    "Learn more about opening Markdown files in Warp",
                    WarpA11yRole::UserAction,
                ))
            }
        }
    }
}
