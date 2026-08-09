//! Implementation of terminal panes.
use std::sync::mpsc::SyncSender;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use url::Url;

use warpui::{
    AppContext, EntityId, ModelHandle, SingletonEntity, ViewContext, ViewHandle, WindowId,
};

use crate::{
    app_state::{LeafContents, TerminalPaneSnapshot},
    pane_group::{self, Direction, Event::OpenConversationHistory, PaneGroup},
    persistence::{BlockCompleted, ModelEvent},
    session_management::SessionNavigationData,
    terminal::{
        general_settings::GeneralSettings,
        shared_session::SharedSessionStatus,
        view::Event,
        TerminalManager, TerminalView,
    },
    view_components::ToastFlavor,
    workspace::{sync_inputs::SyncedInputState, PaneViewLocator},
};


use warp_core::execution_mode::AppExecutionMode;

use super::{
    DetachType, PaneConfiguration, PaneContent, PaneId, PaneStackEvent, PaneView, ShareableLink,
    ShareableLinkError, TerminalPaneId,
};

pub type TerminalPaneView = PaneView<TerminalView>;

/// Data kept for terminal panes.
pub struct TerminalPane {
    model_event_sender: Option<SyncSender<ModelEvent>>,

    /// Used to uniquely identify the pane, even across separate runs of the app.
    uuid: Vec<u8>,

    pane_configuration: ModelHandle<PaneConfiguration>,

    /// Defining `terminal_manager` before `view` means that `terminal_manager`
    /// gets dropped first (guaranteed by the language), which halts the event
    /// loop and avoids possible deadlocks during session cleanup. This is enforced
    /// by the `PaneStack`, since the terminal manager is the associated data for
    /// the backing pane view.
    view: ViewHandle<TerminalPaneView>,
}

impl TerminalPane {
    pub(in crate::pane_group) fn new(
        uuid: Vec<u8>,
        terminal_manager: ModelHandle<Box<dyn TerminalManager>>,
        terminal_view: ViewHandle<TerminalView>,
        model_event_sender: Option<SyncSender<ModelEvent>>,
        ctx: &mut ViewContext<PaneGroup>,
    ) -> Self {
        let pane_configuration = terminal_view.as_ref(ctx).pane_configuration().to_owned();
        let view = ctx.add_typed_action_view(|ctx| {
            let pane_id = PaneId::from_terminal_pane_ctx(ctx);
            PaneView::new(
                pane_id,
                terminal_view,
                terminal_manager,
                pane_configuration.clone(),
                ctx,
            )
        });

        Self {
            model_event_sender,
            uuid,
            pane_configuration,
            view,
        }
    }

    /// The [`PaneView<TerminalView>`] for this pane.
    #[cfg(any(test, feature = "integration_tests"))]
    pub(in crate::pane_group) fn pane_view(&self) -> ViewHandle<TerminalPaneView> {
        self.view.to_owned()
    }

    /// The [`TerminalView`] backing the [`PaneView`] for this terminal pane.
    pub(crate) fn terminal_view(&self, ctx: &AppContext) -> ViewHandle<TerminalView> {
        self.view.as_ref(ctx).child(ctx)
    }

    /// The UUID that identifies this terminal session across app restarts.
    pub(in crate::pane_group) fn session_uuid(&self) -> Vec<u8> {
        self.uuid.clone()
    }

    /// The terminal manager responsible for this session's event loop.
    pub(in crate::pane_group) fn terminal_manager(
        &self,
        ctx: &AppContext,
    ) -> ModelHandle<Box<dyn TerminalManager>> {
        self.view.as_ref(ctx).child_data(ctx).clone()
    }

    /// Instructs the SQLite thread to delete blocks for this session.
    pub(in crate::pane_group) fn delete_blocks(&self, ctx: &AppContext) {
        if !AppExecutionMode::as_ref(ctx).can_save_session() {
            return;
        }

        if let Some(sender) = &self.model_event_sender {
            let model_event = ModelEvent::DeleteBlocks(self.uuid.clone());
            if let Err(err) = sender.send(model_event) {
                log::error!(
                    "Error sending blocks deleted event for terminal id {} {:?}",
                    self.terminal_view(ctx).id(),
                    err
                );
            }
        }
    }

    pub fn session_navigation_data(
        &self,
        pane_group_id: EntityId,
        window_id: WindowId,
        app: &AppContext,
    ) -> SessionNavigationData {
        let view = self.terminal_view(app).as_ref(app);
        SessionNavigationData::new(
            view.full_prompt(app),
            view.prompt_elements(app),
            view.session_command_context(app),
            PaneViewLocator {
                pane_group_id,
                pane_id: self.id(),
            },
            view.last_focus_ts(),
            view.is_read_only(),
            window_id,
            SharedSessionStatus::NotShared,
        )
    }

    pub fn terminal_pane_id(&self) -> TerminalPaneId {
        self.id()
            .as_terminal_pane_id()
            .expect("Should be able to derive a TerminalPaneId from TerminalPane")
    }
}

impl PaneContent for TerminalPane {
    fn id(&self) -> PaneId {
        PaneId::from_terminal_pane_view(&self.view)
    }

    fn attach(
        &self,
        group: &PaneGroup,
        focus_handle: crate::pane_group::focus_state::PaneFocusHandle,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        // TODO(ben): As much as possible, logic from PaneGroup::add_session should go here.
        //  This will simplify PaneGroup, especially when implementing pane management.
        let terminal_pane_id = self.terminal_pane_id();

        self.view
            .update(ctx, |view, ctx| view.set_focus_handle(focus_handle, ctx));

        // Attach the initial terminal view in the stack.
        attach_terminal_view(&self.terminal_view(ctx), terminal_pane_id, ctx);

        // Subscribe to the pane stack to handle views being pushed/popped.
        let pane_stack = self.view.as_ref(ctx).pane_stack().clone();
        ctx.subscribe_to_model(&pane_stack, move |group, _, event, ctx| {
            handle_pane_stack_event(group, event, terminal_pane_id, ctx);
        });

        ctx.subscribe_to_view(&self.view, move |group, _, event, ctx| {
            group.handle_pane_view_event(terminal_pane_id.into(), event, ctx);
        });

        if SyncedInputState::as_ref(ctx).should_sync_this_pane_group(ctx.view_id(), ctx.window_id())
        {
            if let Some(active_pane_view) = group.active_session_view(ctx) {
                let event = active_pane_view
                    .as_ref(ctx)
                    .create_sync_event_based_on_terminal_state(ctx);

                group.send_sync_event_to_session(terminal_pane_id, &event, ctx);
            }
        }

    }

    fn detach(
        &self,
        _group: &PaneGroup,
        detach_type: DetachType,
        ctx: &mut ViewContext<PaneGroup>,
    ) {
        if matches!(detach_type, DetachType::Closed) {
            self.delete_blocks(ctx);
        }

        // Unsubscribe from all views in the pane stack.
        let pane_stack = self.view.as_ref(ctx).pane_stack().clone();
        let contents = pane_stack.as_ref(ctx).entries().to_vec();
        for (manager, view) in contents {
            // Notify the view that it's being detached so it can react appropriately
            // (e.g. the shared-session viewer tears down its network only when the detach
            // is not reversible).
            manager.update(ctx, |terminal_manager, ctx| {
                terminal_manager.on_view_detached(detach_type, ctx);
            });
            ctx.unsubscribe_to_view(&view);
        }

        ctx.unsubscribe_to_model(&pane_stack);

        ctx.unsubscribe_to_view(&self.view);

    }

    fn snapshot(&self, app: &AppContext) -> LeafContents {
        let view = self.terminal_view(app).as_ref(app);
        LeafContents::Terminal(TerminalPaneSnapshot {
            uuid: self.uuid.clone(),
            cwd: view.pwd_if_local(app),
            is_active: view.is_active_session(app),
            is_read_only: view.model.lock().is_read_only(),
            shell_launch_data: view.shell_launch_data_if_local(app),
            input_config: Some(view.input_config(app)),
            llm_model_override: None,
            active_profile_id: None,
            conversation_ids_to_restore: vec![],
            active_conversation_id: None,
        })
    }

    fn has_application_focus(&self, ctx: &mut ViewContext<PaneGroup>) -> bool {
        self.view.is_self_or_child_focused(ctx)
    }

    fn focus(&self, ctx: &mut ViewContext<PaneGroup>) {
        self.terminal_view(ctx)
            .update(ctx, |view, ctx| view.redetermine_global_focus(ctx));
    }

    fn shareable_link(
        &self,
        ctx: &mut ViewContext<PaneGroup>,
    ) -> Result<ShareableLink, ShareableLinkError> {
        let manager = self.terminal_manager(ctx);
        let the_model = manager.as_ref(ctx).model();
        let lock = the_model.lock();

        Ok(ShareableLink::Base)
    }

    fn pane_configuration(&self) -> ModelHandle<PaneConfiguration> {
        self.pane_configuration.clone()
    }

    fn is_pane_being_dragged(&self, ctx: &AppContext) -> bool {
        self.view.as_ref(ctx).is_being_dragged()
    }
}

/// Attaches a terminal view to the pane group by subscribing to its events
/// and setting the file tree code model.
fn attach_terminal_view(
    terminal_view: &ViewHandle<TerminalView>,
    terminal_pane_id: TerminalPaneId,
    ctx: &mut ViewContext<PaneGroup>,
) {
    ctx.subscribe_to_view(
        terminal_view,
        move |group: &mut PaneGroup, _, event, ctx| {
            handle_terminal_view_event(group, terminal_pane_id, event, ctx);
        },
    );
}

/// Handles events from the pane stack when views are added or removed.
fn handle_pane_stack_event(
    group: &mut PaneGroup,
    event: &PaneStackEvent<TerminalView>,
    terminal_pane_id: TerminalPaneId,
    ctx: &mut ViewContext<PaneGroup>,
) {
    match event {
        PaneStackEvent::ViewAdded(terminal_view) => {
            attach_terminal_view(terminal_view, terminal_pane_id, ctx);
        }
        PaneStackEvent::ViewRemoved(terminal_view) => {
            ctx.unsubscribe_to_view(terminal_view);
        }
    }

    // Ensure we use the new top-level view's title and active session status.
    // TODO(ben): This shouldn't be necessary once titles are set declaratively.
    if let Some(active_terminal) = group.terminal_view_from_pane_id(terminal_pane_id, ctx) {
        active_terminal.update(ctx, |view, ctx| view.on_pane_state_change(ctx));
    }
}

fn handle_terminal_view_event(
    group: &mut PaneGroup,
    terminal_pane_id: TerminalPaneId,
    event: &Event,
    ctx: &mut ViewContext<PaneGroup>,
) {
    let pane_id = terminal_pane_id.into();

    if group.pane_contents.contains_key(&pane_id) {
        match event {
            Event::Escape => ctx.emit(pane_group::Event::Escape),
            Event::ExecuteCommand(event) => {
                ctx.emit(pane_group::Event::ExecuteCommand(event.clone()));
            }
            Event::Exited => {
                // If the shell process exited before it successfully bootstrapped,
                // keep the pane open.  There might be useful information visible
                // in the output, and if this was the first shell spawned when the
                // user started the app, it will prevent it from suddenly quitting.
                if group
                    .terminal_view_from_pane_id(terminal_pane_id, ctx)
                    .is_some_and(|terminal_view| {
                        !terminal_view.as_ref(ctx).is_login_shell_bootstrapped()
                    })
                {
                    return;
                }

                group.close_pane(pane_id, ctx);
            }
            Event::CloseRequested => {
                group.close_pane_with_confirmation(pane_id, ctx);
            }
            Event::Pane(pane_event) => group.handle_pane_event(pane_id, pane_event, ctx),
            Event::BlockListCleared => {
                // Capture CMD-K to clear blocks here so we could remove
                // all the associated blocks stored in the history.
                if let Some(terminal_pane) = group.terminal_session_by_id(pane_id) {
                    terminal_pane.delete_blocks(ctx);
                }
            }
            Event::ShareModalOpened(block_id) => {
                group.terminal_with_open_share_block_modal = Some(terminal_pane_id);
                group.share_block_modal.update(ctx, |share_modal, ctx| {
                    if let Some(session) = group.terminal_view_from_pane_id(pane_id, ctx) {
                        let model = session.read(ctx, |view, _| view.model.clone());
                        share_modal.open_with_model_update(model, *block_id, ctx);
                        ctx.notify();
                    }
                });
                ctx.notify();
            }
            Event::SendNotification(notification) => {
                ctx.emit(pane_group::Event::SendNotification {
                    notification: notification.clone(),
                    pane_id,
                })
            }
            Event::PluggableNotification { title, body } => {
                let message = if let Some(t) = title {
                    format!("{t}: {body}")
                } else {
                    body.clone()
                };
                ctx.emit(pane_group::Event::ShowToast {
                    message,
                    flavor: ToastFlavor::Default,
                    pane_id: Some(pane_id),
                })
            }
            Event::AppStateChanged => {
                ctx.emit(pane_group::Event::AppStateChanged);
            }
            Event::BlockCompleted { block, is_local } => {
                match group.terminal_session_by_id(pane_id) {
                    Some(pane) => {
                        if *GeneralSettings::as_ref(ctx).restore_session
                            && AppExecutionMode::as_ref(ctx).can_save_session()
                        {
                            if let Some(sender) = &group.model_event_sender {
                                let block_completed_event = ModelEvent::SaveBlock(BlockCompleted {
                                    pane_id: pane.session_uuid(),
                                    block: block.clone(),
                                    is_local: *is_local,
                                });

                                let sender_clone = sender.clone();
                                let _ = ctx.spawn(async move {
                                // Sending over a sync sender can block the current thread, so we do this async.
                                sender_clone.send(block_completed_event)
                            }, move |_, res, _| {
                                if let Err(err) = res {
                                    log::error!("Error sending block completed event for terminal id {terminal_pane_id:?} {err:?}");
                                }
                            });
                            }
                        }
                        ctx.emit(pane_group::Event::ActiveSessionChanged);
                    }
                    None => {
                        log::error!("Could not find uuid for terminal id: {terminal_pane_id:?}");
                    }
                };
            }
            Event::SessionBootstrapped => {
                ctx.emit(pane_group::Event::ActiveSessionChanged);
            }
            Event::OpenSettings(section) => {
                ctx.emit(pane_group::Event::OpenSettings(*section));
            }
            Event::OpenAutoReloadModal { purchased_credits } => {
                ctx.emit(pane_group::Event::OpenAutoReloadModal {
                    purchased_credits: *purchased_credits,
                });
            }
            #[cfg(not(target_family = "wasm"))]
            Event::OpenPluginInstructionsPane(agent, kind) => {
                ctx.emit(pane_group::Event::OpenPluginInstructionsPane(*agent, *kind));
            }
            Event::AskAIAssistant(ask_type) => {
                ctx.emit(pane_group::Event::AskAIAssistant(ask_type.to_owned()))
            }
            Event::SyncInput(sync_event) => {
                if SyncedInputState::as_ref(ctx)
                    .should_sync_this_pane_group(ctx.view_id(), ctx.window_id())
                {
                    ctx.emit(pane_group::Event::SyncInput(sync_event.clone()));
                }
            }
            Event::ShowCommandSearch(options) => {
                ctx.emit(pane_group::Event::ShowCommandSearch(options.clone()));
            }
            Event::TerminalViewStateChanged => {
                ctx.emit(pane_group::Event::TerminalViewStateChanged);
            }
            Event::OnboardingTutorialCompleted => {
                ctx.emit(pane_group::Event::OnboardingTutorialCompleted);
            }
            Event::OpenWorkflowModalWithCommand(command) => {
                ctx.emit(pane_group::Event::OpenWorkflowModalWithCommand(
                    command.clone(),
                ));
            }
            Event::OpenWorkflowModalWithCloudWorkflow(workflow_id) => {
                ctx.emit(pane_group::Event::OpenCloudWorkflowForEdit(*workflow_id));
            }
            Event::OpenPromptEditor => {
                ctx.emit(pane_group::Event::OpenPromptEditor);
            }
            Event::OpenAgentToolbarEditor => {
                ctx.emit(pane_group::Event::OpenAgentToolbarEditor);
            }
            Event::OpenCLIAgentToolbarEditor => {
                ctx.emit(pane_group::Event::OpenCLIAgentToolbarEditor);
            }
            Event::OpenFileInWarp { path, session } => {
                ctx.emit(pane_group::Event::OpenFileInWarp {
                    path: path.clone(),
                    session: session.clone(),
                });
            }
            #[cfg(feature = "local_fs")]
            Event::PreviewCodeInWarp { source } => {
                ctx.emit(pane_group::Event::PreviewCodeInWarp {
                    source: source.clone(),
                });
            }
            #[cfg(feature = "local_fs")]
            Event::OpenCodeInWarp { source, layout } => {
                ctx.emit(pane_group::Event::OpenCodeInWarp {
                    source: source.clone(),
                    layout: *layout,
                    line_col: None,
                });
            }
            Event::OpenCodeDiff { view } => {
                ctx.emit(pane_group::Event::OpenCodeDiff { view: view.clone() });
            }
            Event::OpenShareSessionModal { open_source } => {
                group.open_share_session_modal(terminal_pane_id, *open_source, ctx)
            }
            Event::OpenShareSessionDeniedModal => {
                group.open_share_session_denied_modal(terminal_pane_id, ctx);
            }
            Event::FocusSession => {
                group.focus_pane(terminal_pane_id.into(), true, ctx);
                ctx.emit(pane_group::Event::FocusPaneGroup);
            }
            Event::OpenWarpDriveObjectInPane(uid) => {
                ctx.emit(pane_group::Event::OpenWarpDriveObjectInPane(uid.clone()));
            }
            Event::OpenSuggestedAgentModeWorkflowModal { workflow_and_id } => {
                ctx.emit(pane_group::Event::OpenSuggestedAgentModeWorkflowModal {
                    workflow_and_id: workflow_and_id.clone(),
                });
            }
            Event::OpenSuggestedRuleDialog { rule_and_id } => {
                ctx.emit(pane_group::Event::OpenSuggestedRuleModal {
                    rule_and_id: rule_and_id.clone(),
                });
            }
            Event::OpenAIFactCollection { sync_id } => {
                ctx.emit(pane_group::Event::OpenAIFactCollection { sync_id: *sync_id });
            }
            Event::SummarizationCancelDialogToggled { is_open } => {
                group.terminal_with_open_summarization_dialog = is_open.then_some(terminal_pane_id);
                ctx.notify();
            }
            Event::EnvironmentSetupModeSelectorToggled { is_open } => {
                group.pane_with_open_environment_setup_mode_selector = is_open.then_some(pane_id);
                ctx.notify();
            }
            Event::AnonymousUserSignup => ctx.emit(pane_group::Event::AnonymousUserSignup),
            #[cfg(feature = "local_fs")]
            Event::OpenFileWithTarget {
                path,
                target,
                line_col,
            } => {
                ctx.emit(pane_group::Event::OpenFileWithTarget {
                    path: path.clone(),
                    target: target.clone(),
                    line_col: *line_col,
                });
            }
            Event::CopyFileToRemote { command, upload_id } => {
                let new_pane_id = group.insert_terminal_pane(
                    Direction::Right,
                    pane_id,
                    None, /*chosen_shell*/
                    ctx,
                );

                group.hide_pane_for_job(new_pane_id.into(), ctx);

                let new_terminal_view = group
                    .active_session_view(ctx)
                    .expect("should have new terminal view");
                new_terminal_view.update(ctx, |terminal_view, ctx| {
                    terminal_view.set_pending_command(command, ctx);
                    terminal_view.set_is_ssh_uploader(true);
                });

                ctx.emit(pane_group::Event::FileUploadCommand {
                    upload_id: *upload_id,
                    command: command.to_owned(),
                    remote_pane_id: terminal_pane_id,
                    local_pane_id: new_pane_id,
                });

                group.focus_pane(pane_id, true, ctx);
            }
            Event::FileUploadPasswordPending => {
                ctx.emit(pane_group::Event::FileUploadPasswordPending {
                    local_pane_id: terminal_pane_id,
                });
            }
            Event::OpenConversationHistory => {
                ctx.emit(OpenConversationHistory);
            }
            Event::FileUploadFinished(exit_code) => {
                ctx.emit(pane_group::Event::FileUploadFinished {
                    local_pane_id: terminal_pane_id,
                    exit_code: *exit_code,
                });

                // Each upload spawns its own new terminal pane. Once an upload
                // has finished, we know that its terminal session will no
                // longer be responsible for any UI-based uploads.
                if let Some(uploader_terminal_view) =
                    group.terminal_view_from_pane_id(terminal_pane_id, ctx)
                {
                    uploader_terminal_view.update(ctx, |terminal_view, _ctx| {
                        terminal_view.set_is_ssh_uploader(false);
                    });
                }
            }
            Event::OpenFileUploadSession(upload_id) => {
                ctx.emit(pane_group::Event::OpenFileUploadSession {
                    remote_pane_id: terminal_pane_id,
                    upload_id: *upload_id,
                })
            }
            Event::TerminateFileUploadSession(upload_id) => {
                ctx.emit(pane_group::Event::TerminateFileUploadSession {
                    remote_pane_id: terminal_pane_id,
                    upload_id: *upload_id,
                })
            }
            Event::SignupAnonymousUser { entrypoint } => {
                ctx.emit(pane_group::Event::SignupAnonymousUser {
                    entrypoint: *entrypoint,
                });
            }
            Event::OpenThemeChooser => {
                ctx.emit(pane_group::Event::OpenThemeChooser);
            }
            Event::OpenMCPSettingsPage { page } => {
                ctx.emit(pane_group::Event::OpenMCPSettingsPage { page: *page });
            }
            Event::OpenFilesPalette { source } => {
                ctx.emit(pane_group::Event::OpenFilesPalette { source: *source })
            }
            #[cfg(feature = "local_fs")]
            Event::FileRenamed { old_path, new_path } => {
                ctx.emit(pane_group::Event::FileRenamed {
                    old_path: old_path.clone(),
                    new_path: new_path.clone(),
                });
            }
            #[cfg(feature = "local_fs")]
            Event::FileDeleted { path } => {
                ctx.emit(pane_group::Event::FileDeleted { path: path.clone() });
            }
            Event::ToggleLeftPanel {
                target_view,
                force_open,
            } => {
                ctx.emit(pane_group::Event::ToggleLeftPanel {
                    target_view: *target_view,
                    force_open: *force_open,
                });
            }
            Event::StartAgentConversation(request) => {
                let request = request.clone();
                match request.execution_mode.clone() {
                    #[cfg(not(target_family = "wasm"))]
                    StartAgentExecutionMode::Local {
                        harness_type: Some(harness_type),
                    } => {
                        let startup_directory =
                            group.startup_path_for_new_session(Some(terminal_pane_id), ctx);
                        let ai_client = ServerApiProvider::handle(ctx).as_ref(ctx).get_ai_client();
                        let parent_pane_id = pane_id;
                        let request_name = request.name.clone();
                        let parent_conversation_id = request.parent_conversation_id;
                        let parent_run_id = request.parent_run_id.clone();
                        let prompt = request.prompt.clone();
                        let shell_type = group
                            .terminal_view_from_pane_id(parent_pane_id, ctx)
                            .and_then(|terminal_view| {
                                terminal_view.as_ref(ctx).active_session_shell_type(ctx)
                            });

                        let _ = ctx.spawn(
                            async move {
                                prepare_local_harness_child_launch(
                                    prompt,
                                    harness_type,
                                    parent_run_id,
                                    shell_type,
                                    startup_directory,
                                    ai_client,
                                )
                                .await
                            },
                            move |group, result, ctx| match result {
                                Ok(launch) => {
                                    let PreparedLocalHarnessLaunch {
                                        command,
                                        env_vars,
                                        run_id,
                                        task_id,
                                    } = launch;
                                    if let Some(HiddenChildAgentConversation {
                                        terminal_view: new_terminal_view,
                                        terminal_view_id,
                                        conversation_id,
                                        ..
                                    }) = create_hidden_child_agent_conversation(
                                        group,
                                        parent_pane_id,
                                        request_name.clone(),
                                        parent_conversation_id,
                                        env_vars,
                                        ctx,
                                    ) {
                                        BlocklistAIHistoryModel::handle(ctx).update(
                                            ctx,
                                            |history_model, ctx| {
                                                history_model.assign_run_id_for_conversation(
                                                    conversation_id,
                                                    run_id,
                                                    Some(task_id),
                                                    terminal_view_id,
                                                    ctx,
                                                );
                                            },
                                        );

                                        new_terminal_view.update(ctx, |terminal_view, ctx| {
                                            terminal_view.execute_command_or_set_pending(
                                                &command,
                                                ctx,
                                            );
                                            terminal_view.enter_agent_view(
                                                None,
                                                Some(conversation_id),
                                                AgentViewEntryOrigin::ChildAgent,
                                                ctx,
                                            );
                                        });
                                    } else {
                                        create_error_child_agent_conversation(
                                            group,
                                            parent_pane_id,
                                            request_name,
                                            parent_conversation_id,
                                            "Failed to create a hidden pane for the local child harness."
                                                .to_string(),
                                            ctx,
                                        );
                                    }
                                }
                                Err(error_message) => {
                                    create_error_child_agent_conversation(
                                        group,
                                        parent_pane_id,
                                        request_name,
                                        parent_conversation_id,
                                        error_message,
                                        ctx,
                                    );
                                }
                            },
                        );
                    }
                    #[cfg(target_family = "wasm")]
                    StartAgentExecutionMode::Local { .. } => {
                        create_error_child_agent_conversation(
                            group,
                            pane_id,
                            request.name,
                            request.parent_conversation_id,
                            "Local harness child agents are not supported in WASM builds."
                                .to_string(),
                            ctx,
                        );
                    }
                    StartAgentExecutionMode::Remote {
                        environment_id,
                        skill_references,
                        model_id,
                        computer_use_enabled,
                        worker_host,
                        harness_type,
                        title,
                    } => {
                        let Some(parent_run_id) = request.parent_run_id.clone() else {
                            log::error!(
                                "Remote StartAgent request missing parent_run_id for {:?}",
                                request.parent_conversation_id
                            );
                            return;
                        };

                        let new_pane_id =
                            group.insert_ambient_agent_pane_hidden_for_child_agent(pane_id, ctx);

                        if let Some(new_terminal_view) =
                            group.terminal_view_from_pane_id(new_pane_id, ctx)
                        {
                            let terminal_view_id = new_terminal_view.id();
                            let conversation_id = BlocklistAIHistoryModel::handle(ctx).update(
                                ctx,
                                |history_model, ctx| {
                                    let id = history_model.start_new_child_conversation(
                                        terminal_view_id,
                                        request.name,
                                        request.parent_conversation_id,
                                        ctx,
                                    );
                                    // Mark as remote so the parent's TaskStatusSyncModel
                                    // skips status reporting — the remote worker handles it.
                                    if let Some(c) = history_model.conversation_mut(&id) {
                                        c.mark_as_remote_child();
                                    }
                                    id
                                },
                            );

                            let runtime_skills = match resolve_runtime_skills(
                                &skill_references,
                                ctx,
                            ) {
                                Ok(runtime_skills) => runtime_skills,
                                Err(unresolved_references) => {
                                    let error_message = format!(
                                        "Failed to resolve child agent skills: {}",
                                        unresolved_references.join(", ")
                                    );
                                    log::error!(
                                        "Failed to resolve StartAgentV2 skill references for remote child {:?}: {}",
                                        conversation_id,
                                        unresolved_references.join(", ")
                                    );
                                    BlocklistAIHistoryModel::handle(ctx).update(
                                        ctx,
                                        |history_model, ctx| {
                                            history_model
                                                .update_conversation_status_with_error_message(
                                                    terminal_view_id,
                                                    conversation_id,
                                                    ConversationStatus::Error,
                                                    Some(error_message),
                                                    ctx,
                                                );
                                        },
                                    );
                                    return;
                                }
                            };
                            // Treat an empty environment_id as "no environment specified" so the
                            // spawn request leaves the config.environment_id field unset. The
                            // server's StartAgent producer defaults to the parent's environment
                            // when available, so an empty value here means the caller explicitly
                            // opted into running with an empty environment.
                            let environment_id =
                                Some(environment_id).filter(|s| !s.trim().is_empty());
                            // Unrecognized harness types collapse to None so the server picks
                            // its default, matching the behavior of an empty `harness_type`.
                            // We deliberately do NOT round-trip `Harness::Unknown` to the server;
                            // that variant is for representing server-originated unknowns to the
                            // user, not for writes.
                            let harness_override = if harness_type.is_empty() {
                                None
                            } else {
                                match <Harness as clap::ValueEnum>::from_str(&harness_type, true) {
                                    Ok(harness) => Some(HarnessConfig::from_harness_type(harness)),
                                    Err(_) => {
                                        log::warn!(
                                            "Unknown harness type from StartAgentV2 proto: {harness_type:?}; omitting harness override so the server picks its default"
                                        );
                                        None
                                    }
                                }
                            };
                            let spawn_request = SpawnAgentRequest {
                                prompt: request.prompt,
                                // Agents spawned during orchestrations are always run in normal mode.
                                mode: UserQueryMode::Normal,
                                config: Some(AgentConfigSnapshot {
                                    environment_id,
                                    model_id: (!model_id.is_empty()).then_some(model_id),
                                    worker_host: (!worker_host.is_empty()).then_some(worker_host),
                                    computer_use_enabled: Some(computer_use_enabled),
                                    harness: harness_override,
                                    ..Default::default()
                                }),
                                title: (!title.is_empty()).then_some(title),
                                team: None,
                                skill: None,
                                attachments: vec![],
                                interactive: Some(true),
                                parent_run_id: Some(parent_run_id),
                                runtime_skills,
                                referenced_attachments: vec![],
                            };

                            new_terminal_view.update(ctx, |terminal_view, ctx| {
                                terminal_view.enter_agent_view(
                                    None,
                                    Some(conversation_id),
                                    AgentViewEntryOrigin::CloudAgent,
                                    ctx,
                                );
                                if let Some(ambient_agent_view_model) =
                                    terminal_view.ambient_agent_view_model()
                                {
                                    ambient_agent_view_model.update(ctx, |model, ctx| {
                                        model.set_conversation_id(Some(conversation_id));
                                        model.spawn_agent_with_request(spawn_request, ctx);
                                    });
                                } else {
                                    log::error!(
                                        "Remote StartAgent child pane missing ambient agent view model"
                                    );
                                }
                            });

                            group
                                .child_agent_panes
                                .insert(conversation_id, new_pane_id.into());
                        } else {
                            log::error!(
                                "Failed to get terminal view for new remote StartAgent pane"
                            );
                            group.discard_pane(new_pane_id.into(), ctx);
                        }
                    }
                }
            }
            _ => {}
        }
    } else {
        log::warn!("Session {terminal_pane_id:?} not found");
    }
}
