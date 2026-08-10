//! Module containing the definition of [`OpenedFilesModel`],
//! which tracks files that have been opened, organized by repository.

use std::{collections::HashMap, path::PathBuf};

use instant::Instant;
use warpui::{Entity, SingletonEntity};

#[derive(Default, Clone)]
pub struct OpenedFilesInRepo(HashMap<PathBuf, Instant>);

impl OpenedFilesInRepo {
    pub fn get(&self, file_path: &PathBuf) -> Option<&Instant> {
        self.0.get(file_path)
    }
}

/// Model that tracks files that have been opened, organized by repository.
/// Maps repository paths to files and when they were last opened.
#[derive(Default)]
pub struct OpenedFilesModel {
    opened_files: HashMap<PathBuf, OpenedFilesInRepo>,
}

impl Entity for OpenedFilesModel {
    type Event = ();
}

impl SingletonEntity for OpenedFilesModel {}

impl OpenedFilesModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all opened files for a specific repository.
    pub fn opened_files_for_repo(&self, repo_path: &PathBuf) -> Option<&OpenedFilesInRepo> {
        self.opened_files.get(repo_path)
    }
}
