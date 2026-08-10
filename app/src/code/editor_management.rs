use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use warp_util::path::LineAndColumnArg;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum CodeSource {
    New {
        default_directory: Option<PathBuf>,
    },
    Link {
        path: PathBuf,
        range_start: Option<LineAndColumnArg>,
        range_end: Option<LineAndColumnArg>,
    },
    ProjectRules {
        path: PathBuf,
    },
    FileTree {
        path: PathBuf,
    },
    Finder {
        path: PathBuf,
    },
}

impl CodeSource {
    pub fn default_directory(&self) -> Option<&PathBuf> {
        match self {
            Self::New { default_directory } => default_directory.as_ref(),
            Self::Link { .. }
            | Self::ProjectRules { .. }
            | Self::FileTree { .. }
            | Self::Finder { .. } => None,
        }
    }

    pub fn path(&self) -> Option<PathBuf> {
        match self {
            Self::New { .. } => None,
            Self::Link { path, .. }
            | Self::ProjectRules { path }
            | Self::FileTree { path }
            | Self::Finder { path } => Some(path.clone()),
        }
    }

    pub fn omit_line_col(&self) -> Self {
        match self {
            Self::Link { path, .. } => Self::Link {
                path: path.clone(),
                range_start: None,
                range_end: None,
            },
            _ => self.clone(),
        }
    }
}
