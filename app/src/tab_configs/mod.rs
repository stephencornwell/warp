pub(crate) mod action_sidecar;
pub mod params_modal;
pub(crate) mod remove_confirmation_dialog;
pub mod session_config;
pub mod session_config_modal;
pub mod session_config_rendering;
pub mod tab_config;

use warp_core::ui::theme::Fill;

pub use params_modal::{TabConfigParamsModal, TabConfigParamsModalEvent};
#[cfg(feature = "local_fs")]
pub(crate) use tab_config::build_worktree_config_toml;
pub use tab_config::{
    render_tab_config, TabConfig, TabConfigError, TabConfigParam, TabConfigParamType,
};
