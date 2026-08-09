#![cfg_attr(not(feature = "local_fs"), allow(dead_code))]

cfg_if::cfg_if! {
    if #[cfg(feature = "local_fs")] {
        mod block_list;
        mod sqlite;
    }
}

pub use persistence::model;
#[cfg_attr(not(feature = "local_fs"), expect(unused_imports))]
pub use persistence::schema;

#[cfg(feature = "integration_tests")]
pub mod testing;

use instant::Instant;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread::JoinHandle;

use chrono::{DateTime, Local};
use warp_core::command::ExitCode;
use warpui::{AppContext, Entity, SingletonEntity};

use crate::app_state::AppState;
use crate::terminal::history::PersistedCommand;
use crate::terminal::model::block::SerializedBlock;
use crate::terminal::model::session::SessionId;

use self::model::Project;

#[cfg(any(feature = "local_fs", feature = "integration_tests"))]
pub use sqlite::database_file_path;
#[cfg(any(feature = "local_fs", feature = "integration_tests"))]
pub use sqlite::establish_ro_connection;

/// Initializes the persistence "subsystem".
///
/// Returns the previously-persisted data, if any, and handles for
/// writing updated data to persist, if the persistence subsystem is
/// available.
#[cfg_attr(not(feature = "local_fs"), allow(unused_variables))]
pub fn initialize(ctx: &mut AppContext) -> (Option<PersistedData>, Option<WriterHandles>) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "local_fs")] {
            sqlite::initialize(ctx)
        } else {
            (None, None)
        }
    }
}

/// Holds interfaces to the writer thread.
pub struct WriterHandles {
    pub handle: JoinHandle<()>,
    pub sender: SyncSender<ModelEvent>,
}

/// Model for interacting with the writer thread.
pub struct PersistenceWriter {
    thread_handle: Option<JoinHandle<()>>,
    model_event_sender: Option<SyncSender<ModelEvent>>,
}

impl PersistenceWriter {
    pub fn new(handle: Option<WriterHandles>) -> Self {
        let (thread_handle, model_event_sender) = match handle {
            Some(handle) => (Some(handle.handle), Some(handle.sender)),
            None => (None, None),
        };
        Self {
            thread_handle,
            model_event_sender,
        }
    }

    /// Sending half for sending model updates to the persistence writer thread.
    pub fn sender(&self) -> Option<SyncSender<ModelEvent>> {
        self.model_event_sender.clone()
    }

    /// Synchronously terminate the SQLite writer thread.
    pub fn terminate(&mut self) {
        if let Some(handle) = self.thread_handle.take() {
            let start = Instant::now();
            let Some(sender) = self.sender() else {
                log::error!("Model event sender should exist if thread handle is set");
                return;
            };
            if let Err(err) = sender.send(ModelEvent::Terminate) {
                log::error!("Could not terminate SQLite writer thread: {err}");
            }
            if handle.join().is_err() {
                // If crash reporting is enabled, Sentry will have already handled the panic.
                log::error!("SQLite writer thread panicked");
            }
            log::info!("Shut down SQLite writer in {:?}", start.elapsed());
        }
    }
}

impl Drop for PersistenceWriter {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl Entity for PersistenceWriter {
    type Event = ();
}

impl SingletonEntity for PersistenceWriter {}

/// TODO: all of this data should eventually be indexed by user_id so that
/// the logged in user sees the data for their user (and if another user logs in,
/// they see their respective data). To do this, we can simply return a mapping
/// of user ID->SqliteData and get the respective AppState after the user logs in.
///
/// For now, to address the global scoping here, we clear all persisted data on logout.
pub struct PersistedData {
    /// Session restoration data
    pub app_state: AppState,

    pub command_history: Vec<PersistedCommand>,
    pub projects: Vec<Project>,
}

#[derive(Clone, Debug)]
pub struct BlockCompleted {
    pub pane_id: Vec<u8>,
    /// Indicates if the block was created locally (e.g. not in a remote session)
    pub is_local: bool,
    pub block: Arc<SerializedBlock>,
}

#[derive(Debug)]
pub struct StartedCommandMetadata {
    pub command: String,
    pub start_ts: Option<DateTime<Local>>,
    pub pwd: Option<String>,
    pub shell: Option<String>,
    pub username: Option<String>,
    pub hostname: Option<String>,
    pub session_id: Option<SessionId>,
    pub git_branch: Option<String>,
    pub workflow_command: Option<String>,
    pub is_agent_executed: bool,
}

#[derive(Debug)]
pub struct FinishedCommandMetadata {
    pub exit_code: ExitCode,
    pub start_ts: DateTime<Local>,
    pub completed_ts: DateTime<Local>,
    pub session_id: SessionId,
}

#[derive(Debug)]
pub enum ModelEvent {
    SaveBlock(BlockCompleted),
    DeleteBlocks(Vec<u8>),
    Snapshot(AppState),
    InsertCommand {
        metadata: StartedCommandMetadata,
    },
    UpdateFinishedCommand {
        metadata: FinishedCommandMetadata,
    },
    // `PauseAndRemoveDatabase` and `ReconstructAndResume` are used to pause and resume the writer thread.
    // These are employed as part of Logout v0 to ensure that the writer thread
    // does not continue writing to the DB after the user has logged out and the DB is deleted.
    PauseAndRemoveDatabase,
    #[cfg(feature = "local_fs")]
    ReconstructAndResume,
    /// Close the SQLite writer thread when the app is about to quit.
    Terminate,
    UpsertProject {
        project: Project,
    },
    DeleteProject {
        path: String,
    },
}
