#[cfg(feature = "local_fs")]
use indexmap::IndexSet;
#[cfg(feature = "local_fs")]
use repo_metadata::repositories::DetectedRepositories;
use std::collections::HashMap;
#[cfg(feature = "local_fs")]
use std::collections::HashSet;
use std::path::{Path, PathBuf};
#[cfg(feature = "local_fs")]
use warpui::{AppContext, SingletonEntity as _};
use warpui::{Entity, EntityId, ModelContext};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingDirectory {
    pub path: PathBuf,
    pub terminal_id: Option<EntityId>,
}

/// Events emitted when the set of working directories changes
#[derive(Clone, Debug)]
pub enum WorkingDirectoriesEvent {
    /// The set of working directories has changed for a specific pane group.
    DirectoriesChanged {
        /// The PaneGroup whose directories changed
        pane_group_id: EntityId,
        /// All active directories (deduplicated) in most to least recently added order.
        directories: Vec<WorkingDirectory>,
    },
    /// The set of repositories has changed for a specific pane group.
    RepositoriesChanged {
        /// The PaneGroup whose repositories changed
        pane_group_id: EntityId,
        /// All active repository roots (deduplicated) in most to least recently added order.
        repositories: Vec<PathBuf>,
    },
    /// The focused repository changed for a specific pane group.
    /// This fires when the user focuses a different pane or CDs within the focused pane.
    FocusedRepoChanged {
        /// The PaneGroup whose focused repo changed
        pane_group_id: EntityId,
        /// All active repository-terminal ID pairs (deduplicated)
        repository_terminal_map: HashMap<PathBuf, EntityId>,
        /// The repository path of the focused terminal, if any
        focused_repo: Option<PathBuf>,
    },
}

#[derive(Default)]
#[cfg(feature = "local_fs")]
/// Workspace model that tracks working directories across all pane groups.
/// Emits events when the set of directories changes for any pane group.
pub struct WorkingDirectoriesModel {
    /// Per-pane-group tracking of active directories as a deduplicated, ordered set.
    ///
    /// IMPORTANT: This stores the *display roots* for the left panel (file tree / global search),
    /// not the raw working directories reported by each pane.
    ///
    /// Concretely, for each pane group's active paths we store:
    /// - the detected repository root when the path belongs to a repo
    /// - otherwise, the normalized path itself
    ///
    /// IndexSet maintains insertion order - most recently added directories appear later.
    pane_groups: HashMap<EntityId, IndexSet<PathBuf>>,
    /// Per-pane-group tracking of active repository roots as a deduplicated, ordered set.
    /// IndexSet maintains insertion order - most recently added repositories appear later.
    repository_roots: HashMap<EntityId, IndexSet<PathBuf>>,
    /// Per-pane-group mapping from root paths to a matching terminal view ID.
    /// This allows looking up which terminal is associated with each root path.
    /// Note, a single root path can be associated with multiple terminals.
    /// we're just storing an arbitrary terminal ID for each root path.
    directory_to_terminal: HashMap<EntityId, HashMap<PathBuf, EntityId>>,
    /// Global mapping from repository root paths to their DiffStateModel.
    /// Since git state is inherently tied to a repository (not a pane group),
    /// this is stored globally and shared across all pane groups viewing the same repo.
    /// Global mapping from repository root paths to their CommentBatch.
    /// Like the DiffStateModel mapping, comments are inherently tied to git diffs
    /// and are shared across all pane groups viewing the same repo.
    /// Per-pane-group mapping from repository root paths to their CodeReviewView.
    /// This allows reusing code review views across multiple requests for the same repo.
    /// Per-pane-group tracking of the focused repository root path.
    focused_repo: HashMap<EntityId, Option<PathBuf>>,
}

#[derive(Default)]
#[cfg(not(feature = "local_fs"))]
/// Does nothing without a local file system
pub struct WorkingDirectoriesModel {}

/// Index Sets are ordered by insertion order. This function updates an index set to match a new set of items.
#[cfg(feature = "local_fs")]
pub fn update_index_set(
    index_set: &mut IndexSet<PathBuf>,
    new_items: impl IntoIterator<Item = PathBuf>,
) {
    let new_items: Vec<PathBuf> = new_items.into_iter().collect();
    index_set.retain(|item| new_items.iter().any(|new_item| new_item == item));
    for item in new_items {
        index_set.insert(item);
    }
}

#[cfg(feature = "local_fs")]
impl WorkingDirectoriesModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the unique directories for a specific pane group in insertion order (oldest first).
    fn least_recent_directories_for_pane_group(
        &self,
        pane_group_id: EntityId,
    ) -> Option<&IndexSet<PathBuf>> {
        self.pane_groups.get(&pane_group_id)
    }

    /// Get the unique directories for a specific pane group in most to least recently added order.
    pub fn most_recent_directories_for_pane_group(
        &self,
        pane_group_id: EntityId,
    ) -> Option<impl Iterator<Item = WorkingDirectory> + '_> {
        self.least_recent_directories_for_pane_group(pane_group_id)
            .map(move |dirs| {
                dirs.iter().rev().map(move |path| WorkingDirectory {
                    path: path.clone(),
                    terminal_id: self.get_terminal_id_for_root_path(pane_group_id, path),
                })
            })
    }

    /// Get the unique repository roots for a specific pane group in insertion order (oldest first).
    fn least_recent_repositories_for_pane_group(
        &self,
        pane_group_id: EntityId,
    ) -> Option<&IndexSet<PathBuf>> {
        self.repository_roots.get(&pane_group_id)
    }

    /// Get the unique repository roots for a specific pane group in most to least recently added order.
    pub fn most_recent_repositories_for_pane_group(
        &self,
        pane_group_id: EntityId,
    ) -> Option<impl Iterator<Item = PathBuf> + '_> {
        self.least_recent_repositories_for_pane_group(pane_group_id)
            .map(|repos| repos.iter().rev().cloned())
    }

    /// Get the terminal view ID associated with a specific root path in a pane group.
    pub fn get_terminal_id_for_root_path(
        &self,
        pane_group_id: EntityId,
        root_path: &Path,
    ) -> Option<EntityId> {
        self.directory_to_terminal
            .get(&pane_group_id)
            .and_then(|roots| roots.get(root_path).copied())
    }

    /// Permanently removes all state associated with a pane group.
    /// This should be called when a tab is closed (pane group is destroyed),
    /// as opposed to handle_empty_pane_group which is called when working directories
    /// become empty but the pane group still exists (e.g., settings page).
    pub fn remove_pane_group(&mut self, pane_group_id: EntityId, ctx: &mut ModelContext<Self>) {
        // Clean up directories, terminals, and repos (emits events for subscribers)
        self.handle_empty_pane_group(pane_group_id, ctx);

        // Clean up views that should persist in handle_empty_pane_group e.g. there's only a settings pane in the pane group
        // but need to be removed when the pane group is destroyed
        self.focused_repo.remove(&pane_group_id);
    }

    fn handle_empty_pane_group(&mut self, pane_group_id: EntityId, ctx: &mut ModelContext<Self>) {
        let did_remove_dirs = self.pane_groups.remove(&pane_group_id).is_some();
        let did_remove_terminals = self.directory_to_terminal.remove(&pane_group_id).is_some();
        let removed_repos = self.repository_roots.remove(&pane_group_id);
        let did_remove_repos = removed_repos.is_some();

        let _ = removed_repos;

        if did_remove_dirs {
            ctx.emit(WorkingDirectoriesEvent::DirectoriesChanged {
                pane_group_id,
                directories: vec![],
            });
        }
        if did_remove_repos {
            ctx.emit(WorkingDirectoriesEvent::RepositoriesChanged {
                pane_group_id,
                repositories: vec![],
            });
        }
        if did_remove_terminals {
            ctx.emit(WorkingDirectoriesEvent::FocusedRepoChanged {
                pane_group_id,
                repository_terminal_map: HashMap::new(),
                focused_repo: None,
            });
        }
    }

    /// If `focused_terminal_id` is provided, the repo_to_terminal map will prioritize
    pub fn refresh_working_directories_for_pane_group(
        &mut self,
        pane_group_id: EntityId,
        terminal_cwds: Vec<(EntityId, String)>,
        local_paths: Vec<(EntityId, String)>,
        focused_terminal_id: Option<EntityId>,
        ctx: &mut ModelContext<Self>,
    ) {
        if terminal_cwds.is_empty() && local_paths.is_empty() {
            self.handle_empty_pane_group(pane_group_id, ctx);
            return;
        }

        let old_directories: Vec<WorkingDirectory> = self
            .least_recent_directories_for_pane_group(pane_group_id)
            .map(|dirs| {
                dirs.iter()
                    .map(|dir| WorkingDirectory {
                        path: dir.clone(),
                        terminal_id: self.get_terminal_id_for_root_path(pane_group_id, dir),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let old_repos: Vec<PathBuf> = self
            .least_recent_repositories_for_pane_group(pane_group_id)
            .map(|repos| repos.iter().cloned().collect())
            .unwrap_or_default();
        let old_focused_repo: Option<PathBuf> =
            self.focused_repo.get(&pane_group_id).cloned().flatten();

        // Resolve a path to its detected repository root, or keep the path as-is if no repo is found.
        let root_for_path = |path: PathBuf| {
            DetectedRepositories::as_ref(ctx)
                .get_root_for_path(&path)
                .unwrap_or(path)
        };

        let root_for_raw_path = |raw_path: &str| normalize_cwd(raw_path).map(root_for_path);

        // Collapse working directories to their nearest repository root (when detected).
        let mut file_path_ancestors: HashSet<PathBuf> = terminal_cwds
            .iter()
            .filter_map(|(_, cwd)| root_for_raw_path(cwd))
            .collect();

        let local_cwds: Vec<(EntityId, String)> = local_paths
            .into_iter()
            .filter_map(|(view_id, path)| {
                let path_buf = PathBuf::from(&path);
                let resolved_path = self
                    .get_repo_root_for_path(&path_buf, ctx)
                    .or_else(|| path_buf.parent().map(|p| p.to_path_buf()))?;

                if file_path_ancestors.insert(resolved_path.clone()) {
                    Some((view_id, resolved_path.display().to_string()))
                } else {
                    None
                }
            })
            .collect();

        // FYI we have the 3 entity types terminal, code, and notebook below but we're merging them in a way that we only care about the actual paths
        // Be careful to not mix the entity IDs if we end up using them in the future!!!
        //
        // NOTE: We intentionally collapse paths to their repo root when possible, so this is a
        // "working roots" list rather than raw per-pane working directories.
        let new_root_paths: Vec<PathBuf> = terminal_cwds
            .iter()
            .chain(local_cwds.iter())
            .filter_map(|(_, cwd)| root_for_raw_path(cwd))
            .collect();

        // Get or create the IndexSet for this pane group
        // (IndexSet maintains insertion order and auto-deduplicates)
        let pane_group_roots = self.pane_groups.entry(pane_group_id).or_default();
        update_index_set(pane_group_roots, new_root_paths.clone());

        // Build repo roots and their terminal associations
        // First pass: collect all repo roots and build initial mapping
        let new_repo_roots: Vec<PathBuf> = self
            .pane_groups
            .get(&pane_group_id)
            .into_iter()
            .flat_map(|dirs| dirs.iter())
            .filter_map(|dir| self.get_repo_root_for_path(dir, ctx))
            .collect();
        let mut new_roots: HashSet<PathBuf> = HashSet::from_iter(new_repo_roots.iter().cloned());
        new_roots.extend(new_root_paths.iter().cloned());

        // Build mapping from directories to their terminal IDs
        let mut new_root_to_terminal: HashMap<PathBuf, EntityId> = terminal_cwds
            .iter()
            .filter_map(|(terminal_id, cwd)| root_for_raw_path(cwd).map(|p| (p, *terminal_id)))
            .collect();
        new_root_to_terminal.retain(|cwd, _terminal_id| new_roots.contains(cwd));

        // Second pass: if we have a focused terminal, ensure its repo maps to it
        // This ensures the dropdown selects the correct repo when a pane is focused or CD'd
        let mut focused_repo: Option<PathBuf> = None;
        if let Some(focused_id) = focused_terminal_id {
            let mut repos_to_insert = Vec::new();
            for (dir, terminal_id) in &new_root_to_terminal {
                if *terminal_id == focused_id {
                    if let Some(repo_root) = self.get_repo_root_for_path(dir, ctx) {
                        repos_to_insert.push((repo_root.clone(), focused_id));
                        focused_repo = Some(repo_root);
                    }
                }
            }
            for (repo_root, focused_id) in repos_to_insert {
                new_root_to_terminal.insert(repo_root, focused_id);
            }
        }

        // Get or create the IndexSet for repository roots
        // (IndexSet maintains insertion order and auto-deduplicates)
        let pane_group_repos = self.repository_roots.entry(pane_group_id).or_default();
        update_index_set(pane_group_repos, new_repo_roots);

        // Update the repo to terminal mapping
        self.directory_to_terminal
            .insert(pane_group_id, new_root_to_terminal);

        let new_directories: Vec<WorkingDirectory> = self
            .pane_groups
            .get(&pane_group_id)
            .map(|dirs| {
                dirs.iter()
                    .map(|dir| WorkingDirectory {
                        path: dir.clone(),
                        terminal_id: self.get_terminal_id_for_root_path(pane_group_id, dir),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let new_deduplicated_repos: Vec<PathBuf> = self
            .repository_roots
            .get(&pane_group_id)
            .map(|repos| repos.iter().cloned().collect())
            .unwrap_or_default();
        if old_directories != new_directories {
            self.emit_directories_changed(pane_group_id, ctx);
        }

        if old_repos != new_deduplicated_repos {
            self.emit_repositories_changed(pane_group_id, ctx);
        }

        if old_focused_repo != focused_repo {
            self.focused_repo
                .insert(pane_group_id, focused_repo.clone());
            self.emit_focused_repo_changed(pane_group_id, focused_repo, ctx);
        }
    }

    /// Get the repository root for a given path.
    fn get_repo_root_for_path(&self, path: &Path, ctx: &AppContext) -> Option<PathBuf> {
        DetectedRepositories::as_ref(ctx).get_root_for_path(path)
    }

    /// Emit a DirectoriesChanged event with the current state for a specific pane group.
    /// Directories are returned in most recent first order for use in the UI.
    fn emit_directories_changed(&mut self, pane_group_id: EntityId, ctx: &mut ModelContext<Self>) {
        ctx.emit(WorkingDirectoriesEvent::DirectoriesChanged {
            pane_group_id,
            directories: self
                .most_recent_directories_for_pane_group(pane_group_id)
                .map(|iter| iter.collect())
                .unwrap_or_default(),
        });
    }

    /// Emit a RepositoriesChanged event with the current state for a specific pane group.
    /// Repositories are returned in most recent first order for use in the UI.
    fn emit_repositories_changed(&mut self, pane_group_id: EntityId, ctx: &mut ModelContext<Self>) {
        ctx.emit(WorkingDirectoriesEvent::RepositoriesChanged {
            pane_group_id,
            repositories: self
                .most_recent_repositories_for_pane_group(pane_group_id)
                .map(|iter| iter.collect())
                .unwrap_or_default(),
        });
    }

    fn emit_focused_repo_changed(
        &mut self,
        pane_group_id: EntityId,
        focused_repo: Option<PathBuf>,
        ctx: &mut ModelContext<Self>,
    ) {
        ctx.emit(WorkingDirectoriesEvent::FocusedRepoChanged {
            pane_group_id,
            repository_terminal_map: self
                .directory_to_terminal
                .get(&pane_group_id)
                .cloned()
                .unwrap_or_default(),
            focused_repo,
        });
    }
}

#[cfg(not(feature = "local_fs"))]
impl WorkingDirectoriesModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the unique directories for a specific pane group in most to least recently added order.
    pub fn most_recent_directories_for_pane_group(
        &self,
        _pane_group_id: EntityId,
    ) -> Option<impl Iterator<Item = WorkingDirectory> + '_> {
        Option::<std::iter::Empty<WorkingDirectory>>::None
    }

    /// Get the unique repository roots for a specific pane group in most to least recently added order.
    pub fn most_recent_repositories_for_pane_group(
        &self,
        _pane_group_id: EntityId,
    ) -> Option<impl Iterator<Item = PathBuf> + '_> {
        Option::<std::iter::Empty<PathBuf>>::None
    }

    /// Get the terminal view ID associated with a specific repository in a pane group.
    pub fn get_terminal_id_for_root_path(
        &self,
        _pane_group_id: EntityId,
        _root_path: &Path,
    ) -> Option<EntityId> {
        None
    }

    pub fn refresh_working_directories_for_pane_group(
        &mut self,
        _pane_group_id: EntityId,
        _terminal_cwds: Vec<(EntityId, String)>,
        _local_paths: Vec<(EntityId, String)>,
        _focused_terminal_id: Option<EntityId>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn remove_pane_group(&mut self, _pane_group_id: EntityId, _ctx: &mut ModelContext<Self>) {}
}

impl Entity for WorkingDirectoriesModel {
    type Event = WorkingDirectoriesEvent;
}

/// Normalize a CWD path string to a canonical PathBuf
///
/// This function attempts to canonicalize (resolve symlinks, make absolute)
///
/// Returns None if the path is empty, invalid, or cannot be canonicalized.
/// Canonicalization failure may indicate remote paths or non-existent directories,
/// which could be supported in the future.
#[cfg(feature = "local_fs")]
fn normalize_cwd(raw_cwd: &str) -> Option<PathBuf> {
    if raw_cwd.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw_cwd.to_string());
    // Use dunce::canonicalize to avoid Windows extended-length path prefix (\\?\)
    // which would cause path comparison mismatches with CanonicalizedPath.
    dunce::canonicalize(&path).ok()
}

#[cfg(test)]
#[path = "working_directories_tests.rs"]
mod tests;
