use crate::client::PiDaemonClient;
use crate::client::native_index::{
    FileMatch, fuzzy_search_files, pi_sessions_root, scan_local_sessions, user_home,
};
use crate::client::session_controller::{
    SessionController, apply_optimistic_todo_args, apply_todo_tool_output_text,
    build_diff_from_tool, compute_context_tokens_from_payload, compute_stats_from_jsonl,
    current_local_hhmm, extract_artifacts_from_markdown, extract_model_and_thinking_from_jsonl,
    extract_subagent_session_id, extract_todos_from_jsonl,
    extract_todos_from_payload, extract_turn_usage, flatten_content_text, flatten_tool_args,
    merge_turn_usage, parse_compaction_summary, parse_message_content,
    parse_session_jsonl_with_siblings, parse_session_messages, parse_subagent_async_json_widget,
    parse_todo_tool_details, parse_todo_widget_lines,
};
use crate::client::sidecar::SidecarProcess;
use crate::client::sse::{AgentSseEvent, consume_agent_sse};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use crate::client::types::*;
use crate::i18n::AppLocale;
use gpui_kit::{AppContext as _, Context, Entity, Pixels, SharedString, Window, px};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAddKind {
    AddProviderPicker,
    ProviderModel,
    AddModelToProvider {
        provider_id: String,
    },
    DiscoveredModelsPicker {
        provider_id: String,
        discovered: Vec<ConfiguredModelInfo>,
    },
    EditProvider {
        provider_id: String,
    },
    EditModel {
        provider_id: String,
        model_id: String,
    },
    PluginInfo(PluginRow),
    SkillInfo(SkillRow),
    ProviderApiKey {
        provider_id: String,
        provider_name: String,
    },
    McpServer,
    SubagentProfile,
    Skill,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspacePage {
    Chat,
    Tasks,
    Scheduled,
    Artifacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelTab {
    Artifact,
    Files,
    FileViewer,
    Terminal,
    Inspector,
    GitGraph,
}

pub struct AppState {
    pub client: PiDaemonClient,
    pub _sidecar: Option<SidecarProcess>,
    pub worker_manager: crate::engine::worker_manager::ScaleToZeroWorkerManager,
    pub sidecar_connected: bool,
    pub controller: SessionController,

    pub active_page: WorkspacePage,
    pub sidebar_collapsed: bool,
    pub right_panel_open: bool,
    pub right_panel_tab: RightPanelTab,
    pub open_right_tabs: Vec<RightPanelTab>,
    pub right_panel_width: Pixels,
    pub settings_open: bool,
    /// Page the settings modal should open on (0 = General, 1 = Models).
    pub settings_initial_page: usize,
    pub settings_busy: bool,
    /// Shows a transient "Saved" confirmation in the settings footer.
    pub settings_saved_flash: bool,
    pub settings_add_kind: Option<SettingsAddKind>,
    pub find_open: bool,
    pub find_query: SharedString,

    pub dark_mode: bool,
    pub theme_mode: String,
    pub has_unread_completed_tasks: bool,
    pub light_theme_name: SharedString,
    pub dark_theme_name: SharedString,
    pub chat_font_size: f32,
    pub ui_font_size: f32,
    pub preview_font_size: f32,
    pub chat_content_width: f32,
    pub chat_font_slider: Entity<SliderState>,
    pub ui_font_slider: Entity<SliderState>,
    pub preview_font_slider: Entity<SliderState>,
    pub chat_width_slider: Entity<SliderState>,
    pub send_shortcut: SendShortcut,
    pub locale: AppLocale,
    pub user_collapse_chars: usize,
    pub notify_enabled: bool,
    pub notify_sound: bool,
    pub thinking_expanded_default: bool,
    pub diff_view_mode: String,
    /// `auto` probes local cores; `manual` pins `core_url`.
    pub core_mode: String,
    pub core_url: String,
    pub core_probe_results: Vec<crate::core_probe::CoreCandidate>,
    pub core_probing: bool,
    pub artifacts_prompt_enabled: bool,
    pub auto_title_enabled: bool,
    pub active_cwd: PathBuf,
    pub active_workspace_kind: WorkspaceKind,
    pub active_is_temp_chat: bool,
    pub temp_chat_ids: HashSet<String>,
    pub worktrees: Vec<WorktreeInfo>,
    pub active_branch: Option<SharedString>,

    pub sessions: Vec<SessionSummary>,
    pub running_session_ids: HashSet<String>,
    pub unread_session_ids: HashSet<String>,
    pub artifacts_page_index: usize,
    pub artifacts_search_query: String,
    pub artifacts_type_filter: String,
    pub has_earlier_messages: bool,
    pub is_loading_earlier: bool,
    pub collapsed_projects: HashSet<String>,
    pub archived_projects: HashSet<String>,
    pub archived_session_ids: HashSet<String>,
    pub extra_projects: Vec<PathBuf>,
    pub project_order: Vec<String>,
    pub projects_section_collapsed: bool,
    pub chats_section_collapsed: bool,
    pub renaming_session_id: Option<String>,
    pub active_session_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub branch_siblings: Vec<BranchSibling>,
    pub is_agent_running: bool,
    pub is_compacting: bool,
    pub outbox: Vec<QueuedPrompt>,
    pub pending_ui_request: Option<ExtensionUiRequest>,
    /// Further extension dialogs waiting behind `pending_ui_request` (an extension
    /// walking asks sequentially sends one request per question).
    pub dialog_queue: Vec<ExtensionUiRequest>,
    /// Extension dialog matched to a finished `ask_user_question` tool call, answered
    /// with the canonical questionnaire payload shape.
    pub questionnaire_request_id: Option<String>,

    pub models: Vec<ModelInfo>,
    pub selected_model: Option<ModelInfo>,
    pub enabled_models_overview: EnabledModelsOverview,
    pub configured_providers: Vec<ConfiguredProviderInfo>,
    pub configured_providers_raw: Value,
    pub expanded_configured_providers: HashSet<String>,
    pub discovering_provider_id: Option<String>,
    /// Last model-discovery failure, keyed by provider id (`None` = clear).
    pub discover_error: Option<(String, String)>,
    pub auth_providers: Vec<(String, String, bool)>,
    pub oauth_providers: Vec<(String, String, bool)>,
    pub running_events_subscribed: bool,
    pub thinking_level: ThinkingLevel,
    pub tool_preset: ToolPreset,
    pub auto_compaction: bool,
    pub auto_retry: bool,
    pub steering_mode: SteeringMode,
    pub follow_up_mode: SteeringMode,
    pub stats: SessionStats,
    #[allow(dead_code)]
    pub extension_statuses: Vec<ExtensionStatusItem>,
    pub subagent_runs: Vec<SubagentRunInfo>,
    pub tool_subagent_runs: Vec<SubagentRunInfo>,
    pub subagent_status_text: Option<String>,

    pub artifacts: Vec<ArtifactRecord>,
    pub active_artifact_id: Option<String>,

    pub tasks: Vec<TaskSnapshot>,
    pub scheduled_tasks: Vec<ScheduledTask>,
    pub scheduler_owner: SchedulerOwnerStatus,
    pub scheduled_running_ids: HashSet<String>,
    pub mcp_servers: Vec<McpServerRow>,
    pub mcp_project_trusted: bool,
    pub skills: Vec<SkillRow>,
    pub plugins: Vec<PluginRow>,
    pub plugins_updating: bool,
    pub settings_loading: bool,
    pub subagents: Vec<SubagentProfileRow>,
    pub subagent_built_in_enabled: bool,
    pub subagent_max_concurrent: usize,
    pub network_proxy: NetworkProxyState,

    pub open_file_path: Option<PathBuf>,
    pub open_file_content: SharedString,
    pub open_file_diff_patch: Option<SharedString>,
    pub file_search_results: Vec<FileMatch>,
    pub todos: Vec<TodoWidgetItem>,
    pub todos_expanded: bool,
    /// Items the user dismissed from the rpiv-todos panel.
    pub dismissed_todo_ids: std::collections::HashSet<String>,
    /// Files changed in the project working tree, refreshed when a run settles.
    /// Set once the panel is dismissed: the todo panel stays hidden for the rest
    /// of this session no matter how often the extension re-emits its widget.
    /// Only switching/starting a session clears it.
    pub todos_hidden: bool,
    /// Session whose todo panel the user dismissed, so re-selecting the same
    /// session (or a refresh that re-runs select_session) keeps it hidden.
    pub todos_hidden_session: Option<String>,
    /// True once structured `todo` tool calls or `rpiv-todo-state` entries populated
    /// `self.todos` in the current session, preventing lossy widget scraping from
    /// clearing completed or collapsed tasks.
    pub todos_from_tool: bool,
    pub retry_status: Option<SharedString>,
    pub draft_restore_text: Option<String>,
    pub pending_composer_mention: Option<String>,
    #[allow(dead_code)]
    pub status_banner: Option<SharedString>,
    pub agy_link_state: Option<AgyLinkState>,
    pub agy_link_status_text: Option<String>,
    pub agy_link_modal_open: bool,
    pub agy_link_modal_tab: usize,
    pub agy_link_loading: bool,
    pub agy_link_copy_auth_url_pending: bool,
    pub agy_link_auth_copied: bool,
    pub agy_link_details_account_id: Option<String>,
    pub agy_link_proxy_input: Option<Entity<gpui_kit::component::input::InputState>>,
    pub settings_deleting_keys: std::collections::HashSet<String>,
    pub suppress_next_prompt_reload: bool,
    pub settings_open_nonce: u64,
    pub continuing_msg_id: Option<u64>,
    pub continuing_prefix_text: String,
}

impl AppState {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let port: u16 = std::env::var("PI_SIDECAR_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(38471);
        let token = std::env::var("PI_DESKTOP_API_TOKEN")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
        let instance_id = std::env::var("PI_DESKTOP_INSTANCE_ID")
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        let sidecar = SidecarProcess::spawn_if_available(port, token.clone(), instance_id.clone());
        let base_url = sidecar
            .as_ref()
            .map(SidecarProcess::base_url)
            .unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
        let client = sidecar
            .as_ref()
            .map(|s| PiDaemonClient::new(base_url.clone(), &s.token, &s.instance_id))
            .unwrap_or_else(|| PiDaemonClient::new(base_url.clone(), token, instance_id));

        // A configured manual core (Settings › General › Pi Core) overrides the
        // bundled sidecar address only when mode is "manual".
        let settings_snapshot = crate::app_settings::load_app_settings();
        let configured_core = settings_snapshot
            .core
            .url
            .trim()
            .trim_end_matches('/')
            .to_string();
        let core_url = if settings_snapshot.core.mode == "manual" && !configured_core.is_empty() {
            Some(configured_core)
        } else {
            None
        };
        let client = match core_url {
            Some(base) if base != base_url => {
                PiDaemonClient::new(base, &client.token, &client.instance_id)
            }
            _ => client,
        };

        // A fresh install has no workspace: start at the home directory rather
        // than whatever directory the process happened to inherit, and never
        // fabricate a demo project/session. The chat panel renders its empty
        // state and the model picker stays empty until the user configures a
        // provider.
        let now_ms = crate::client::local_store::now_unix_ms();
        for expired_id in crate::client::local_store::collect_expired_temp_chat_ids(now_ms) {
            let _ = crate::client::local_store::delete_session_local(&expired_id);
        }
        let temp_chat_ids = crate::client::local_store::load_active_temp_chat_ids(now_ms);

        let mut cwd = user_home();
        let sessions = scan_local_sessions(&pi_sessions_root());
        let mut ctrl = SessionController::default();

        let mut messages = Vec::new();
        let mut initial_siblings = Vec::new();
        let mut artifacts = Vec::new();
        let mut todos = Vec::new();
        let mut active_workspace_kind = WorkspaceKind::Project;
        let mut initial_stats = SessionStats::default();
        let models: Vec<ModelInfo> = Vec::new();

        let active_session_id = sessions.first().map(|s| s.id.clone());
        let active_is_temp_chat = active_session_id
            .as_ref()
            .is_some_and(|id| temp_chat_ids.contains(id));
        if let Some(first) = sessions.first() {
            if let Some(ref first_cwd) = first.cwd {
                cwd = PathBuf::from(first_cwd);
            }
            active_workspace_kind = first.workspace_kind.unwrap_or(WorkspaceKind::Project);
            if let Some(ref first_path) = first.path {
                let p = PathBuf::from(first_path);
                let (parsed_msgs, siblings) =
                    parse_session_jsonl_with_siblings(&p, None, &mut ctrl.next_local_msg_id);
                messages = parsed_msgs;
                initial_siblings = siblings;
                initial_stats = compute_stats_from_jsonl(&p, 200_000);
                todos = extract_todos_from_jsonl(&p);
                let sid_ref = active_session_id.as_deref();
                for msg in &messages {
                    artifacts.extend(extract_artifacts_from_markdown(&msg.content, sid_ref));
                }
                let mut scanned_arts =
                    crate::client::native_index::scan_all_local_artifacts(&sessions);
                for a in artifacts {
                    if !scanned_arts.iter().any(|sa| sa.identifier == a.identifier) {
                        scanned_arts.push(a);
                    }
                }
                artifacts = scanned_arts;
            }
        }

        let initial_branch = resolve_git_branch(&cwd);

        let active_artifact_id = artifacts.first().map(|a| a.identifier.clone());

        let appearance = crate::themes::load_appearance_prefs();
        let initial_chat_font_size = appearance.chat_font_size.clamp(9.0, 22.0);
        let initial_ui_font_size = appearance.ui_font_size.clamp(5.0, 21.0);
        let initial_preview_font_size = appearance.preview_font_size.clamp(9.0, 22.0);
        let initial_chat_width = appearance.chat_content_width.clamp(756.0, 1512.0);
        let initial_locale = crate::themes::locale_from_prefs(&appearance.locale);
        let initial_send_shortcut = crate::themes::send_shortcut_from_prefs(&appearance.send_shortcut);

        let chat_font_slider = cx.new(|_| {
            SliderState::new()
                .min(9.0)
                .max(22.0)
                .step(1.0)
                .default_value(initial_chat_font_size)
        });
        let ui_font_slider = cx.new(|_| {
            SliderState::new()
                .min(5.0)
                .max(21.0)
                .step(1.0)
                .default_value(initial_ui_font_size)
        });
        let preview_font_slider = cx.new(|_| {
            SliderState::new()
                .min(9.0)
                .max(22.0)
                .step(1.0)
                .default_value(initial_preview_font_size)
        });
        let chat_width_slider = cx.new(|_| {
            SliderState::new()
                .min(756.0)
                .max(1512.0)
                .step(10.0)
                .default_value(initial_chat_width)
        });

        let app_cfs = cx.entity().clone();
        cx.subscribe(&chat_font_slider, move |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(val) | SliderEvent::Release(val) = ev {
                this.set_chat_font_size(val.start(), cx);
            }
        }).detach();

        let app_ufs = cx.entity().clone();
        cx.subscribe(&ui_font_slider, move |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(val) | SliderEvent::Release(val) = ev {
                this.set_ui_font_size(val.start(), cx);
            }
        }).detach();

        let app_pfs = cx.entity().clone();
        cx.subscribe(&preview_font_slider, move |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(val) | SliderEvent::Release(val) = ev {
                this.set_preview_font_size(val.start(), cx);
            }
        }).detach();

        let app_cws = cx.entity().clone();
        cx.subscribe(&chat_width_slider, move |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(val) | SliderEvent::Release(val) = ev {
                this.set_chat_content_width(val.start(), cx);
            }
        }).detach();

        let worker_manager = crate::engine::worker_manager::ScaleToZeroWorkerManager::new(
            port,
            client.token.clone(),
            client.instance_id.clone(),
        );
        let mut state = Self {
            client,
            _sidecar: sidecar,
            worker_manager,
            sidecar_connected: false,
            controller: ctrl,
            active_page: WorkspacePage::Chat,
            sidebar_collapsed: false,
            right_panel_open: false,
            right_panel_tab: RightPanelTab::Inspector,
            open_right_tabs: Vec::new(),
            right_panel_width: px(440.),
            settings_open: false,
            settings_initial_page: 0,
            settings_busy: false,
            settings_saved_flash: false,
            settings_add_kind: None,
            find_open: false,
            find_query: SharedString::default(),
            dark_mode: appearance.dark_mode,
            theme_mode: appearance.theme_mode,
            has_unread_completed_tasks: false,
            light_theme_name: SharedString::from(appearance.light_theme),
            dark_theme_name: SharedString::from(appearance.dark_theme),
            chat_font_size: initial_chat_font_size,
            ui_font_size: initial_ui_font_size,
            preview_font_size: initial_preview_font_size,
            chat_content_width: initial_chat_width,
            chat_font_slider,
            ui_font_slider,
            preview_font_slider,
            chat_width_slider,
            send_shortcut: initial_send_shortcut,
            locale: initial_locale,
            user_collapse_chars: appearance.user_collapse_chars,
            notify_enabled: appearance.notify_enabled,
            notify_sound: appearance.notify_sound,
            thinking_expanded_default: appearance.thinking_expanded_default,
            diff_view_mode: appearance.diff_view_mode,
            core_mode: appearance.core_mode,
            core_url: appearance.core_url,
            core_probe_results: Vec::new(),
            core_probing: false,
            artifacts_prompt_enabled: true,
            auto_title_enabled: true,
            active_cwd: cwd,
            active_workspace_kind,
            active_is_temp_chat,
            temp_chat_ids,
            worktrees: Vec::new(),
            active_branch: initial_branch,
            sessions,
            running_session_ids: HashSet::new(),
            unread_session_ids: HashSet::new(),
            artifacts_page_index: 0,
            artifacts_search_query: String::new(),
            artifacts_type_filter: "all".to_string(),
            has_earlier_messages: false,
            is_loading_earlier: false,
            collapsed_projects: HashSet::new(),
            archived_projects: HashSet::new(),
            archived_session_ids: HashSet::new(),
            extra_projects: Vec::new(),
            project_order: crate::app_settings::load_app_settings().chat.project_order,
            projects_section_collapsed: false,
            chats_section_collapsed: false,
            renaming_session_id: None,
            active_session_id,
            messages,
            branch_siblings: initial_siblings,
            is_agent_running: false,
            is_compacting: false,
            outbox: Vec::new(),
            pending_ui_request: None,
            dialog_queue: Vec::new(),
            questionnaire_request_id: None,
            selected_model: models.first().cloned(),
            models,
            enabled_models_overview: EnabledModelsOverview::default(),
            configured_providers: Vec::new(),
            configured_providers_raw: json!({ "providers": {} }),
            expanded_configured_providers: HashSet::new(),
            discovering_provider_id: None,
            discover_error: None,
            auth_providers: Vec::new(),
            oauth_providers: Vec::new(),
            running_events_subscribed: false,
            thinking_level: ThinkingLevel::Medium,
            tool_preset: ToolPreset::Default,
            auto_compaction: true,
            auto_retry: true,
            steering_mode: SteeringMode::All,
            follow_up_mode: SteeringMode::All,
            stats: initial_stats,
            extension_statuses: Vec::new(),
            subagent_runs: Vec::new(),
            tool_subagent_runs: Vec::new(),
            subagent_status_text: None,
            artifacts,
            active_artifact_id,
            tasks: Vec::new(),
            scheduled_tasks: Vec::new(),
            scheduler_owner: SchedulerOwnerStatus::default(),
            scheduled_running_ids: HashSet::new(),
            mcp_servers: Vec::new(),
            mcp_project_trusted: false,
            skills: Vec::new(),
            plugins: Vec::new(),
            plugins_updating: false,
            settings_loading: false,
            subagents: Vec::new(),
            subagent_built_in_enabled: true,
            subagent_max_concurrent: 4,
            network_proxy: NetworkProxyState::default(),
            open_file_path: None,
            open_file_content: SharedString::default(),
            open_file_diff_patch: None,
            file_search_results: Vec::new(),
            todos,
            todos_expanded: false,
            dismissed_todo_ids: std::collections::HashSet::new(),
            todos_hidden: false,
            todos_hidden_session: None,
            todos_from_tool: false,
            retry_status: None,
            draft_restore_text: None,
            pending_composer_mention: None,
            status_banner: None,
            agy_link_state: None,
            agy_link_status_text: None,
            agy_link_modal_open: false,
            agy_link_modal_tab: 0,
            agy_link_loading: false,
            agy_link_copy_auth_url_pending: false,
            agy_link_auth_copied: false,
            agy_link_details_account_id: None,
            agy_link_proxy_input: None,
            settings_deleting_keys: std::collections::HashSet::new(),
            suppress_next_prompt_reload: false,
            settings_open_nonce: 0,
            continuing_msg_id: None,
            continuing_prefix_text: String::new(),
        };

        // Populate initial state immediately from local disk store (Phase 6.3.1 zero-RPC path)
        state.load_local_store_snapshot();
        state.sync_tool_subagent_runs_from_messages();

        // Kick off initial sidecar probe & refresh + lightweight external session watcher
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            smol::Timer::after(std::time::Duration::from_millis(250)).await;
            let _ = entity.update(cx, |this, cx| {
                this.refresh_from_sidecar(cx);
                this.start_external_jsonl_watcher(cx);
                this.start_native_scheduler_and_worker_reaper(cx);
                this.start_sidecar_watchdog(cx);
                if this.core_mode == "auto" {
                    this.probe_core_candidates(cx);
                }
            });
        })
        .detach();

        let _ = &mut state;
        state
    }

    /// Loads static configuration, models, MCP servers, subagents, scheduled tasks,
    /// and Git worktrees directly from `~/.pi/agent/` and `<cwd>/.pi/` without RPC.
    pub fn load_local_store_snapshot(&mut self) {
        use crate::client::local_store;
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();

        let models_cfg = local_store::read_models_config_local();
        self.apply_models_config_response(&models_cfg);

        // Synthesize `self.models`, `self.enabled_models_overview`, `self.auth_providers`, and
        // `self.selected_model` directly from `~/.pi/agent/{models.json, auth.json, settings.json}`
        // so the model picker and native Rust LLM engine work with 0 Node processes.
        let global_settings = local_store::read_global_settings();
        let enabled_patterns: Vec<String> = global_settings
            .get("enabledModels")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let all_enabled = enabled_patterns.is_empty();

        let auth_map = local_store::read_json_object_or_empty(
            &local_store::pi_agent_dir().join("auth.json"),
        );

        let mut local_models: Vec<ModelInfo> = Vec::new();
        let mut overview_providers: Vec<EnabledProviderGroup> = Vec::new();

        for cp in &self.configured_providers {
            let prov_name = cp.display_name.clone().unwrap_or_else(|| cp.id.clone());
            let mut prov_items = Vec::new();
            let mut enabled_cnt = 0usize;
            for cm in &cp.models {
                let model_ref = format!("{}/{}", cp.id, cm.id);
                let is_enabled = all_enabled
                    || enabled_patterns
                        .iter()
                        .any(|pat| pat == &model_ref || pat == &format!("{}/*", cp.id));
                if is_enabled {
                    enabled_cnt += 1;
                }
                let levels = if cm.reasoning {
                    vec![
                        "off".to_string(),
                        "low".to_string(),
                        "medium".to_string(),
                        "high".to_string(),
                    ]
                } else {
                    vec!["off".to_string()]
                };
                local_models.push(ModelInfo {
                    provider: cp.id.clone(),
                    id: cm.id.clone(),
                    name: cm.name.clone(),
                    provider_name: Some(prov_name.clone()),
                    reasoning: cm.reasoning,
                    context_window: cm.context_window,
                    enabled: Some(is_enabled),
                    thinking_levels: levels,
                    thinking_level_map: cm
                        .thinking_level_map
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect(),
                });
                prov_items.push(EnabledModelItem {
                    id: cm.id.clone(),
                    name: cm.name.clone().unwrap_or_else(|| cm.id.clone()),
                    model_ref,
                    enabled: is_enabled,
                    context_window: cm.context_window,
                });
            }
            overview_providers.push(EnabledProviderGroup {
                id: cp.id.clone(),
                name: prov_name,
                kind: "custom".to_string(),
                enabled_count: enabled_cnt,
                models: prov_items,
            });
        }

        // Include built-in providers that have credentials in auth.json or env vars
        // Every known provider is listed even before the sidecar answers, and
        // providers with a stored credential get a short curated model seed so
        // the first run works without waiting for a remote model list. The
        // seeds are not persisted and are replaced by pi's catalog as soon as
        // the sidecar reports its models.
        let mut local_auth: Vec<(String, String, bool)> = Vec::new();
        for preset in crate::provider_catalog::all() {
            let has_auth = auth_map.contains_key(preset.id)
                || preset.env_vars.iter().any(|var| {
                    std::env::var(var)
                        .map(|v| !v.trim().is_empty())
                        .unwrap_or(false)
                });
            local_auth.push((
                preset.id.to_string(),
                preset.name.to_string(),
                has_auth,
            ));
            if !has_auth || preset.models.is_empty() {
                continue;
            }
            if local_models.iter().any(|m| m.provider == preset.id) {
                continue;
            }
            let mut prov_items = Vec::new();
            let mut enabled_cnt = 0usize;
            for seed in preset.models {
                let model_ref = format!("{}/{}", preset.id, seed.id);
                let is_enabled = all_enabled
                    || enabled_patterns.iter().any(|pat| {
                        pat == &model_ref || pat == &format!("{}/*", preset.id)
                    });
                if is_enabled {
                    enabled_cnt += 1;
                }
                local_models.push(ModelInfo {
                    provider: preset.id.to_string(),
                    id: seed.id.to_string(),
                    name: Some(seed.name.to_string()),
                    provider_name: Some(preset.name.to_string()),
                    reasoning: seed.reasoning,
                    context_window: Some(seed.context_window),
                    enabled: Some(is_enabled),
                    thinking_levels: if seed.reasoning {
                        vec![
                            "off".into(),
                            "low".into(),
                            "medium".into(),
                            "high".into(),
                        ]
                    } else {
                        vec!["off".into()]
                    },
                    thinking_level_map: HashMap::new(),
                });
                prov_items.push(EnabledModelItem {
                    id: seed.id.to_string(),
                    name: seed.name.to_string(),
                    model_ref,
                    enabled: is_enabled,
                    context_window: Some(seed.context_window),
                });
            }
            overview_providers.push(EnabledProviderGroup {
                id: preset.id.to_string(),
                name: preset.name.to_string(),
                kind: "builtin".to_string(),
                enabled_count: enabled_cnt,
                models: prov_items,
            });
        }

        // Custom providers configured in models.json (including catalog
        // entries pi does not ship, such as SiliconFlow or Ollama) are
        // configured even though they have no auth.json credential.
        let configured_snapshot = self.configured_providers.clone();
        for cp in &configured_snapshot {
            let name = cp.display_name.clone().unwrap_or_else(|| cp.id.clone());
            merge_auth_provider(&mut local_auth, &cp.id, &name, true, true);
        }

        // Merge the static catalog with whatever the sidecar has already
        // reported (custom providers, OAuth providers, configured flags).
        // The old code only replaced the list while it was empty, which left
        // the picker stuck on a tiny offline fallback.
        let mut merged_auth = local_auth;
        for (id, name, configured) in self.auth_providers.drain(..) {
            if let Some(existing) = merged_auth.iter_mut().find(|(i, _, _)| *i == id) {
                existing.2 = existing.2 || configured;
                if existing.1.is_empty() {
                    existing.1 = name;
                }
            } else {
                merged_auth.push((id, name, configured));
            }
        }
        self.auth_providers = merged_auth;

        let mut merged_oauth: Vec<(String, String, bool)> = crate::provider_catalog::all()
            .iter()
            .filter(|p| p.oauth)
            .map(|p| (p.id.to_string(), p.name.to_string(), false))
            .collect();
        for (id, name, logged_in) in self.oauth_providers.drain(..) {
            merge_auth_provider(&mut merged_oauth, &id, &name, logged_in, false);
        }
        self.oauth_providers = merged_oauth;

        if !local_models.is_empty() && (self.models.is_empty() || !self.sidecar_connected) {
            let enabled_total = local_models
                .iter()
                .filter(|m| m.enabled.unwrap_or(true))
                .count();
            let available_total = local_models.len();
            self.models = local_models;
            self.enabled_models_overview = EnabledModelsOverview {
                all_enabled,
                enabled_total,
                available_total,
                scope: "global".to_string(),
                settings_path: local_store::global_settings_path()
                    .to_string_lossy()
                    .into_owned(),
                editable: true,
                providers: overview_providers,
            };
        }

        self.enrich_thinking_levels_from_config();

        if self.selected_model.is_none() && !self.models.is_empty() {
            let def_prov = global_settings
                .get("defaultProvider")
                .and_then(Value::as_str);
            let def_model = global_settings
                .get("defaultModel")
                .and_then(Value::as_str);
            if let (Some(dp), Some(dm)) = (def_prov, def_model) {
                if let Some(found) = self
                    .models
                    .iter()
                    .find(|m| m.provider == dp && m.id == dm)
                    .cloned()
                {
                    self.selected_model = Some(found);
                }
            }
            if self.selected_model.is_none() {
                self.selected_model = self
                    .models
                    .iter()
                    .find(|m| m.enabled.unwrap_or(true))
                    .or_else(|| self.models.first())
                    .cloned();
            }
            if let Some(tl) = global_settings
                .get("defaultThinkingLevel")
                .and_then(Value::as_str)
            {
                if let Some(parsed) = parse_thinking_level_str(tl) {
                    self.thinking_level = parsed;
                }
            }
            self.clamp_thinking_level_to_model();
        }

        let pi_web = local_store::read_pi_web_settings();
        if let Some(en) = pi_web.get("artifactsPrompt").and_then(Value::as_bool) {
            self.artifacts_prompt_enabled = en;
        }
        if let Some(en) = pi_web.get("autoTitle").and_then(Value::as_bool) {
            self.auto_title_enabled = en;
        }
        if let Some(np) = pi_web.get("networkProxy").and_then(Value::as_object) {
            self.network_proxy.enabled = np.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            self.network_proxy.url = np
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            self.network_proxy.no_proxy = np
                .get("noProxy")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }

        let sub_settings = local_store::read_subagent_settings_local();
        self.subagent_built_in_enabled = sub_settings.enabled;
        self.subagent_max_concurrent = sub_settings.max_concurrent;
        self.subagents = local_store::list_subagent_profiles_local(Some(&cwd_str));

        self.mcp_project_trusted = local_store::is_project_trusted_local(&cwd_str);
        self.mcp_servers = local_store::read_mcp_servers_local(Some(&cwd_str));

        let local_tasks = local_store::load_scheduled_tasks_local();
        if !local_tasks.is_empty() || self.scheduled_tasks.is_empty() {
            self.scheduled_tasks = local_tasks;
        }

        let wt_snap = local_store::inspect_git_worktrees_local(&cwd_str);
        if wt_snap.is_git {
            self.worktrees = wt_snap.worktrees;
        }
    }

    pub fn open_right_tab(&mut self, tab: RightPanelTab, cx: &mut Context<Self>) {
        if !self.open_right_tabs.contains(&tab) {
            self.open_right_tabs.push(tab);
        }
        self.right_panel_tab = tab;
        self.right_panel_open = true;
        cx.notify();
    }

    pub fn toggle_right_tab(&mut self, tab: RightPanelTab, cx: &mut Context<Self>) {
        if self.right_panel_open && self.right_panel_tab == tab {
            self.right_panel_open = false;
            cx.notify();
        } else {
            self.open_right_tab(tab, cx);
        }
    }

    pub fn toggle_git_graph(&mut self, cx: &mut Context<Self>) {
        self.toggle_right_tab(RightPanelTab::GitGraph, cx);
    }

    pub fn close_right_tab(&mut self, tab: RightPanelTab, cx: &mut Context<Self>) {
        self.open_right_tabs.retain(|t| *t != tab);
        if self.open_right_tabs.is_empty() {
            self.right_panel_open = false;
        } else if self.right_panel_tab == tab {
            self.right_panel_tab = *self.open_right_tabs.last().unwrap();
        }
        cx.notify();
    }

    fn apply_running_ids(&mut self, mut next_running: HashSet<String>) {
        if self.suppress_next_prompt_reload {
            if let Some(ref sid) = self.active_session_id {
                next_running.remove(sid);
            }
        }
        // Any session that just transitioned from running -> idle while not selected becomes unread
        let active_id = self.active_session_id.as_deref();
        for prev_id in &self.running_session_ids {
            if !next_running.contains(prev_id) && active_id != Some(prev_id.as_str()) {
                self.unread_session_ids.insert(prev_id.clone());
            }
        }
        self.running_session_ids = next_running;
        for sess in &mut self.sessions {
            sess.is_running = self.running_session_ids.contains(&sess.id);
        }
    }

    pub fn grouped_project_sessions(&self) -> BTreeMap<String, Vec<&SessionSummary>> {
        let mut map: BTreeMap<String, Vec<&SessionSummary>> = BTreeMap::new();
        for sess in &self.sessions {
            if sess.is_subagent() {
                continue;
            }
            if sess.archived || self.archived_session_ids.contains(&sess.id) || sess.pinned {
                continue;
            }
            if sess.workspace_kind == Some(WorkspaceKind::Chat) {
                continue;
            }
            let group = sess
                .project_name
                .clone()
                .or_else(|| {
                    sess.cwd
                        .as_deref()
                        .and_then(|c| PathBuf::from(c).file_name().map(|n| n.to_string_lossy().into_owned()))
                })
                .unwrap_or_else(|| "Workspace".to_string());
            if self.archived_projects.contains(&group) {
                continue;
            }
            map.entry(group).or_default().push(sess);
        }
        for path in &self.extra_projects {
            let group = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace")
                .to_string();
            if !self.archived_projects.contains(&group) {
                map.entry(group).or_default();
            }
        }
        map
    }

    pub fn switch_git_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        let cwd = self.active_cwd.clone();
        let target = branch.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = smol::unblock(move || {
                crate::client::hidden_command("git")
                    .arg("-C")
                    .arg(&cwd)
                    .args(&["checkout", &target])
                    .output()
                    .ok()
            })
            .await;
            let _ = entity.update(cx, |this, cx| {
                this.active_branch = resolve_git_branch(&this.active_cwd);
                cx.notify();
            });
        })
        .detach();
        self.active_branch = Some(SharedString::from(branch));
        cx.notify();
    }

    pub fn git_pull(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        let cwd = self.active_cwd.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = smol::unblock(move || {
                let mut cmd = crate::client::hidden_command("git");
                cmd.arg("-C").arg(&cwd).arg("pull");
                if let Some(b) = branch {
                    cmd.arg("origin").arg(b);
                }
                cmd.output().ok()
            })
            .await;
            let _ = entity.update(cx, |this, cx| {
                this.active_branch = resolve_git_branch(&this.active_cwd);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn git_push(&mut self, branch: Option<String>, cx: &mut Context<Self>) {
        let cwd = self.active_cwd.clone();
        cx.spawn(async move |_, _| {
            let _ = smol::unblock(move || {
                let mut cmd = crate::client::hidden_command("git");
                cmd.arg("-C").arg(&cwd).arg("push");
                if let Some(b) = branch {
                    cmd.arg("origin").arg(b);
                }
                cmd.output().ok()
            })
            .await;
        })
        .detach();
    }

    pub fn delete_git_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        let cwd = self.active_cwd.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = smol::unblock(move || {
                crate::client::hidden_command("git")
                    .arg("-C")
                    .arg(&cwd)
                    .args(&["branch", "-D", &branch])
                    .output()
                    .ok()
            })
            .await;
            let _ = entity.update(cx, |this, cx| {
                this.active_branch = resolve_git_branch(&this.active_cwd);
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_archive_session(&mut self, session_id: String, cx: &mut Context<Self>) {
        if let Some(sess) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            sess.archived = !sess.archived;
        }
        if self.archived_session_ids.contains(&session_id) {
            self.archived_session_ids.remove(&session_id);
        } else {
            self.archived_session_ids.insert(session_id);
        }
        cx.notify();
    }

    pub fn reorder_project_group(&mut self, from_name: &str, to_name: &str, cx: &mut Context<Self>) {
        if from_name == to_name {
            return;
        }
        let grouped = self.grouped_project_sessions();
        let mut names: Vec<String> = Vec::new();
        // Start with existing saved order for present groups
        for p in &self.project_order {
            if grouped.contains_key(p) && !names.contains(p) {
                names.push(p.clone());
            }
        }
        // Append any remaining present groups alphabetically
        for k in grouped.keys() {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
        if let Some(from_idx) = names.iter().position(|n| n == from_name) {
            let item = names.remove(from_idx);
            let to_idx = names.iter().position(|n| n == to_name).unwrap_or(names.len());
            names.insert(to_idx, item);
            self.project_order = names.clone();
            crate::app_settings::update_app_settings(|s| {
                s.chat.project_order = names;
            });
            cx.notify();
        }
    }

    pub fn add_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !self.extra_projects.contains(&path) {
            self.extra_projects.push(path.clone());
        }
        self.active_cwd = path;
        self.active_branch = resolve_git_branch(&self.active_cwd);
        self.projects_section_collapsed = false;
        self.start_new_composer(WorkspaceKind::Project, cx);
    }

    pub fn collapse_all_projects(&mut self, cx: &mut Context<Self>) {
        let keys: Vec<String> = self.grouped_project_sessions().keys().cloned().collect();
        for k in keys {
            self.collapsed_projects.insert(k);
        }
        cx.notify();
    }

    pub fn expand_all_projects(&mut self, cx: &mut Context<Self>) {
        self.collapsed_projects.clear();
        cx.notify();
    }

    fn persist_appearance_only(&self) {
        crate::themes::save_appearance_prefs(&crate::themes::AppearancePrefs {
            dark_mode: self.dark_mode,
            theme_mode: self.theme_mode.clone(),
            light_theme: self.light_theme_name.to_string(),
            dark_theme: self.dark_theme_name.to_string(),
            chat_font_size: self.chat_font_size,
            ui_font_size: self.ui_font_size,
            preview_font_size: self.preview_font_size,
            chat_content_width: self.chat_content_width,
            locale: self.locale.code().to_string(),
            send_shortcut: self.send_shortcut.as_str().to_string(),
            user_collapse_chars: self.user_collapse_chars,
            notify_enabled: self.notify_enabled,
            notify_sound: self.notify_sound,
            thinking_expanded_default: self.thinking_expanded_default,
            diff_view_mode: self.diff_view_mode.clone(),
            core_mode: self.core_mode.clone(),
            core_url: self.core_url.clone(),
            todos_hidden: false,
        });
    }

    pub fn set_send_shortcut(&mut self, sc: SendShortcut, cx: &mut Context<Self>) {
        self.send_shortcut = sc;
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_core_mode(&mut self, mode: String, cx: &mut Context<Self>) {
        self.core_mode = mode;
        self.persist_appearance_only();
        if self.core_mode == "auto" {
            self.probe_core_candidates(cx);
        }
        cx.notify();
    }

    pub fn set_core_url(&mut self, url: String, cx: &mut Context<Self>) {
        let normalized = url.trim().trim_end_matches('/').to_string();
        self.core_url = normalized.clone();
        self.persist_appearance_only();
        if !normalized.is_empty() && normalized != self.client.base_url {
            // Point every request at the selected service and probe it again.
            self.client = PiDaemonClient::new(
                normalized,
                self.client.token.clone(),
                self.client.instance_id.clone(),
            );
            self.controller.stop_sse();
            self.running_events_subscribed = false;
            self.refresh_from_sidecar(cx);
        }
        cx.notify();
    }

    /// Probes the documented local core addresses off the UI thread.
    pub fn probe_core_candidates(&mut self, cx: &mut Context<Self>) {
        if self.core_probing {
            return;
        }
        self.core_probing = true;
        let (tx, rx) = smol::channel::unbounded();
        std::thread::spawn(move || {
            let _ = tx.send_blocking(crate::core_probe::probe_candidates());
        });
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(found) = rx.recv().await {
                let _ = entity.update(cx, |s, cx| {
                    s.core_probe_results = found;
                    s.core_probing = false;
                    cx.notify();
                });
            } else {
                let _ = entity.update(cx, |s, cx| {
                    s.core_probing = false;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Dismiss the rpiv-todos panel: remember every current item so replays of the
    /// same list stay hidden and it is never carried into the next conversation.
    pub fn dismiss_todos(&mut self, cx: &mut Context<Self>) {
        for item in &self.todos {
            self.dismissed_todo_ids.insert(item.id.to_string());
        }
        self.todos.clear();
        self.todos_expanded = false;
        self.todos_hidden = true;
        self.todos_hidden_session = self.active_session_id.clone();
        cx.notify();
    }

    /// Drops any dismissed item that a replay or an optimistic update re-added.
    fn prune_dismissed_todos(&mut self) {
        if self.todos_hidden {
            self.todos.clear();
            return;
        }
        if self.dismissed_todo_ids.is_empty() {
            return;
        }
        self.todos
            .retain(|t| !self.dismissed_todo_ids.contains(t.id.as_ref()));
    }

    /// Applies a todo list while keeping dismissed items hidden.
    pub fn set_todos(&mut self, items: Vec<TodoWidgetItem>) {
        if self.todos_hidden {
            self.todos.clear();
            return;
        }
        if self.dismissed_todo_ids.is_empty() {
            self.todos = items;
        } else {
            self.todos = items
                .into_iter()
                .filter(|t| !self.dismissed_todo_ids.contains(t.id.as_ref()))
                .collect();
        }
    }

    pub fn set_user_collapse_chars(&mut self, chars: usize, cx: &mut Context<Self>) {
        self.user_collapse_chars = chars;
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_notify_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.notify_enabled = enabled;
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_notify_sound(&mut self, sound: bool, cx: &mut Context<Self>) {
        self.notify_sound = sound;
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_thinking_expanded_default(&mut self, expanded: bool, cx: &mut Context<Self>) {
        self.thinking_expanded_default = expanded;
        self.persist_appearance_only();
        cx.notify();
    }

    /// Kept for config compatibility: diffs are always unified now.
    #[allow(dead_code)]
    pub fn set_diff_view_mode(&mut self, mode: String, cx: &mut Context<Self>) {
        self.diff_view_mode = mode;
        self.persist_appearance_only();
        cx.notify();
    }

    fn persist_and_apply_appearance(&self, cx: &mut gpui_kit::App) {
        self.persist_appearance_only();
        crate::themes::apply_appearance(
            self.dark_mode,
            self.light_theme_name.as_ref(),
            self.dark_theme_name.as_ref(),
            cx,
        );
    }

    pub fn set_locale(&mut self, locale: AppLocale, cx: &mut Context<Self>) {
        self.locale = locale;
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_chat_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.chat_font_size = size.clamp(9.0, 22.0);
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_preview_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.preview_font_size = size.clamp(9.0, 22.0);
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn load_artifact_versions(&mut self, session_id: &str, identifier: &str, cx: &mut Context<Self>) {
        let sid = session_id.to_string();
        let aid = identifier.to_string();
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.get_artifact_item(&sid, &aid).await {
                if let Some(versions) = val.get("versions").and_then(Value::as_array) {
                    let latest_content = versions
                        .last()
                        .and_then(|v| v.get("content"))
                        .and_then(Value::as_str)
                        .map(String::from);
                    let prev_content = if versions.len() >= 2 {
                        versions[versions.len() - 2]
                            .get("content")
                            .and_then(Value::as_str)
                            .map(String::from)
                    } else {
                        None
                    };
                    let _ = entity.update(cx, |this, cx| {
                        if let Some(art) = this.artifacts.iter_mut().find(|a| a.identifier == aid) {
                            if art.content.is_empty() {
                                if let Some(c) = latest_content {
                                    art.content = c;
                                }
                            }
                            if prev_content.is_some() {
                                art.previous_content = prev_content;
                            }
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn set_ui_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.ui_font_size = size.clamp(5.0, 21.0);
        crate::themes::apply_ui_font_size(self.ui_font_size, cx);
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_chat_content_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.chat_content_width = width.clamp(756.0, 1512.0);
        self.persist_appearance_only();
        cx.notify();
    }

    pub fn set_theme_mode(&mut self, mode: String, cx: &mut Context<Self>) {
        self.theme_mode = mode.clone();
        if mode == "dark" {
            self.dark_mode = true;
        } else if mode == "light" {
            self.dark_mode = false;
        }
        self.persist_and_apply_appearance(cx);
        cx.notify();
    }

    pub fn mark_tasks_read(&mut self, cx: &mut Context<Self>) {
        self.has_unread_completed_tasks = false;
        cx.notify();
    }

    pub fn set_dark_mode(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark_mode = dark;
        self.persist_and_apply_appearance(cx);
        cx.notify();
    }

    pub fn set_light_theme(&mut self, name: SharedString, cx: &mut Context<Self>) {
        self.light_theme_name = name;
        self.persist_and_apply_appearance(cx);
        cx.notify();
    }

    pub fn set_dark_theme(&mut self, name: SharedString, cx: &mut Context<Self>) {
        self.dark_theme_name = name;
        self.persist_and_apply_appearance(cx);
        cx.notify();
    }

    pub fn toggle_theme(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.dark_mode = !self.dark_mode;
        self.persist_and_apply_appearance(cx);
        cx.notify();
    }

    pub fn navigate_to_branch_entry(&mut self, target_entry_id: &str, cx: &mut Context<Self>) {
        let Some(sid) = self.active_session_id.clone() else {
            return;
        };
        if let Some(path_str) = self
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .and_then(|s| s.path.as_deref())
        {
            let p = PathBuf::from(path_str);
            let (parsed, siblings) = parse_session_jsonl_with_siblings(
                &p,
                Some(target_entry_id),
                &mut self.controller.next_local_msg_id,
            );
            if !parsed.is_empty() {
                self.messages = parsed;
                self.branch_siblings = siblings;
            }
        }
        if self.sidecar_connected {
            let client = self.client.clone();
            let target = target_entry_id.to_string();
            let entity = cx.entity().clone();
            let sid_clone = sid.clone();
            cx.spawn(async move |_, cx| {
                let _ = client.navigate_tree(&sid_clone, &target).await;
                let path = format!("/api/sessions/{sid_clone}/context?leafId={target}");
                if let Ok(val) = client.get_json_path(&path).await {
                    let _ = entity.update(cx, |this, cx| {
                        if this.active_session_id.as_deref() == Some(sid_clone.as_str()) {
                            let parsed = parse_session_messages(
                                &val,
                                &mut this.controller.next_local_msg_id,
                            );
                            if !parsed.is_empty() {
                                this.messages = parsed;
                            }
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub fn edit_user_message(
        &mut self,
        text: String,
        entry_id: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.draft_restore_text = Some(text);
        if let Some(eid) = entry_id {
            self.navigate_to_branch_entry(eid.as_ref(), cx);
        } else {
            cx.notify();
        }
    }

    /// Retries a failed turn that produced no actual assistant output:
    /// prunes the invalid user + failed assistant error entries from the session JSONL,
    /// rolls back the sidecar session tree to the parent entry, and re-dispatches the prompt.
    pub fn retry_failed_turn(&mut self, msg_id: u64, cx: &mut Context<Self>) {
        if self.is_agent_running || self.controller.send_in_flight {
            return;
        }
        let Some(assistant_idx) = self.messages.iter().position(|m| m.id == msg_id) else {
            return;
        };
        if self.messages[assistant_idx].role != MessageRole::Assistant {
            return;
        }
        let Some(user_idx) = (0..assistant_idx)
            .rev()
            .find(|&i| self.messages[i].role == MessageRole::User)
        else {
            return;
        };

        let user_text = self.messages[user_idx].content.clone();
        let user_entry_id = self.messages[user_idx]
            .entry_id
            .as_deref()
            .map(str::to_string);
        let assistant_entry_id = self.messages[assistant_idx]
            .entry_id
            .as_deref()
            .map(str::to_string);

        self.messages.remove(assistant_idx);
        self.messages.remove(user_idx);

        let rollback_parent_id = self
            .active_session_id
            .as_deref()
            .and_then(|sid| {
                self.sessions
                    .iter()
                    .find(|s| s.id == sid)
                    .and_then(|s| s.path.as_deref())
            })
            .and_then(|path_str| {
                crate::client::session_controller::prune_failed_turn_from_jsonl(
                    std::path::Path::new(path_str),
                    user_entry_id.as_deref(),
                    assistant_entry_id.as_deref(),
                )
            });

        if self.sidecar_connected {
            if let (Some(sid), Some(pid)) = (self.active_session_id.clone(), rollback_parent_id) {
                let client = self.client.clone();
                let entity = cx.entity().clone();
                cx.spawn(async move |_, cx| {
                    let _ = client.navigate_tree(&sid, &pid).await;
                    let _ = entity.update(cx, |this, cx| {
                        this.dispatch_single_prompt(user_text, cx);
                    });
                })
                .detach();
                cx.notify();
                return;
            }
        }

        self.dispatch_single_prompt(user_text, cx);
    }

    /// Sends an implicit continuation prompt for an assistant turn that was interrupted
    /// mid-response by an error, appending the subsequent output onto the same turn.
    pub fn continue_interrupted_turn(&mut self, msg_id: u64, cx: &mut Context<Self>) {
        if self.is_agent_running || self.controller.send_in_flight {
            return;
        }
        let Some(sid) = self.active_session_id.clone() else {
            return;
        };
        if !self.sidecar_connected {
            return;
        }
        let Some(idx) = self.messages.iter().position(|m| m.id == msg_id) else {
            return;
        };
        if self.messages[idx].role != MessageRole::Assistant {
            return;
        }

        self.continuing_msg_id = Some(msg_id);
        self.continuing_prefix_text = self.messages[idx].content.clone();
        self.messages[idx].error_message = None;
        self.messages[idx].stop_reason = None;
        self.messages[idx].is_streaming = true;
        self.is_agent_running = true;
        self.controller.send_in_flight = true;

        if self.controller.sse_task.is_none() {
            self.connect_session_sse(sid.clone(), cx);
        }

        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let res = client
                .send_agent_command(
                    &sid,
                    serde_json::json!({
                        "type": "prompt",
                        "message": crate::client::session_controller::IMPLICIT_CONTINUE_PROMPT,
                    }),
                )
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.controller.send_in_flight = false;
                if let Err(err) = res {
                    this.is_agent_running = false;
                    this.continuing_msg_id = None;
                    this.continuing_prefix_text.clear();
                    if let Some(m) = this.messages.iter_mut().find(|m| m.id == msg_id) {
                        m.is_streaming = false;
                        m.stop_reason = Some(SharedString::from("error"));
                        m.error_message = Some(SharedString::from(format!("{err:#}")));
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn fork_session_from_entry(
        &mut self,
        id: String,
        entry_id: String,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected {
            if let Ok((new_id, _path)) =
                crate::client::local_store::fork_session_local(&id, Some(&entry_id))
            {
                let scanned = scan_local_sessions(&pi_sessions_root());
                if !scanned.is_empty() {
                    self.sessions = scanned;
                }
                self.select_session(new_id, cx);
            }
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.fork_session(&id, &entry_id).await {
                let _ = entity.update(cx, |this, cx| {
                    this.refresh_from_sidecar(cx);
                    if let Some(new_id) = val
                        .get("session")
                        .and_then(|s| s.get("id"))
                        .or_else(|| val.get("sessionId"))
                        .and_then(Value::as_str)
                    {
                        this.select_session(new_id.to_string(), cx);
                    }
                });
            }
        })
        .detach();
    }

    pub fn active_subagent_runs(&self) -> Vec<SubagentRunInfo> {
        let mut merged = self.subagent_runs.clone();
        for tool_run in &self.tool_subagent_runs {
            if !merged.iter().any(|r| r.id == tool_run.id) {
                let mut run = tool_run.clone();
                if let Some(ref sid) = run.session_id {
                    if self.running_session_ids.contains(sid) {
                        run.state = "running".to_string();
                    } else if run.is_active() && self.running_events_subscribed {
                        // Keep running only if its tool call itself is still running
                        let tc_still_running = self.messages.iter().rev().take(4).any(|m| {
                            m.tool_calls.iter().any(|t| t.id.as_ref() == run.id && t.is_running)
                        });
                        if !tc_still_running && !self.is_agent_running {
                            run.state = "complete".to_string();
                        }
                    }
                }
                merged.push(run);
            }
        }
        merged
    }

    pub fn apply_pi_web_subagent_details(
        &mut self,
        tool_call_id: &str,
        details: &Value,
        is_error: bool,
    ) {
        let sid = details
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let profile = details
            .get("profile")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("subagent")
            .to_string();
        let desc = details
            .get("description")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.chars().take(80).collect::<String>());
        let raw_status = details
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(if is_error { "failed" } else { "completed" });
        let normalized_state = match raw_status {
            "starting" => "running",
            "completed" => "complete",
            "aborted" | "interrupted" => "stopped",
            other => other,
        }
        .to_string();

        if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
            run.label = profile;
            run.state = normalized_state;
            if desc.is_some() {
                run.progress_text = desc;
            }
            if sid.is_some() {
                run.session_id = sid;
            }
            run.current_tool = None;
        } else {
            self.tool_subagent_runs.push(SubagentRunInfo {
                id: tool_call_id.to_string(),
                kind: "subagent".to_string(),
                label: profile,
                state: normalized_state,
                started_at: None,
                ended_at: None,
                model: None,
                thinking: None,
                current_tool: None,
                turn_count: None,
                tool_count: None,
                progress_text: desc,
                session_id: sid,
                children: Vec::new(),
            });
        }
    }

    pub fn sync_tool_subagent_runs_from_messages(&mut self) {
        for msg in &self.messages {
            for tc in &msg.tool_calls {
                if !matches!(tc.name.as_ref(), "Agent" | "subagent" | "subagent_launch") {
                    continue;
                }
                let sid = tc
                    .subagent_session_id
                    .as_ref()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        tc.arguments
                            .iter()
                            .find(|(k, _)| k.as_ref() == "resume")
                            .map(|(_, v)| v.to_string())
                    });
                let state = if let Some(ref child_sid) = sid {
                    if self.running_session_ids.contains(child_sid) {
                        "running"
                    } else if tc.is_error {
                        "failed"
                    } else if tc.is_running {
                        "running"
                    } else {
                        "complete"
                    }
                } else if tc.is_error {
                    "failed"
                } else if tc.is_running {
                    "running"
                } else {
                    "complete"
                };
                if let Some(existing) = self
                    .tool_subagent_runs
                    .iter_mut()
                    .find(|r| r.id == tc.id.as_ref())
                {
                    if existing.session_id.is_none() && sid.is_some() {
                        existing.session_id = sid;
                    }
                    existing.state = state.to_string();
                    continue;
                }
                let is_builtin = tc.name.as_ref() == "Agent";
                let label = tc
                    .arguments
                    .iter()
                    .find(|(k, _)| {
                        if is_builtin {
                            k.as_ref() == "subagent_type"
                        } else {
                            k.as_ref() == "agent" || k.as_ref() == "key"
                        }
                    })
                    .map(|(_, v)| v.to_string())
                    .unwrap_or_else(|| {
                        if is_builtin {
                            "general-purpose".to_string()
                        } else {
                            "subagent".to_string()
                        }
                    });
                let desc = tc
                    .arguments
                    .iter()
                    .find(|(k, _)| matches!(k.as_ref(), "description" | "task" | "prompt"))
                    .map(|(_, v)| v.chars().take(80).collect::<String>());
                let model = tc
                    .arguments
                    .iter()
                    .find(|(k, _)| k.as_ref() == "model")
                    .map(|(_, v)| v.to_string());
                self.tool_subagent_runs.push(SubagentRunInfo {
                    id: tc.id.to_string(),
                    kind: "subagent".to_string(),
                    label,
                    state: state.to_string(),
                    started_at: None,
                    ended_at: None,
                    model,
                    thinking: None,
                    current_tool: None,
                    turn_count: None,
                    tool_count: None,
                    progress_text: desc,
                    session_id: sid,
                    children: Vec::new(),
                });
            }
        }
    }

    #[allow(dead_code)]
    pub fn active_subagent_count(&self) -> usize {
        self.active_subagent_runs()
            .iter()
            .filter(|r| r.is_active())
            .count()
    }

    pub fn refresh_from_sidecar(&mut self, cx: &mut Context<Self>) {
        if self.core_mode == "auto" {
            let needs_spawn = self
                ._sidecar
                .as_mut()
                .map(|sc| !sc.is_alive())
                .unwrap_or(true);
            if needs_spawn {
                let port: u16 = std::env::var("PI_SIDECAR_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(38471);
                let token = self.client.token.clone();
                let inst = self.client.instance_id.clone();
                if let Some(sc) = SidecarProcess::spawn_if_available(port, token, inst) {
                    let base_url = sc.base_url();
                    self.client =
                        PiDaemonClient::new(base_url.clone(), &sc.token, &sc.instance_id);
                    self.core_url = base_url;
                    self._sidecar = Some(sc);
                    self.controller.stop_sse();
                    self.running_events_subscribed = false;
                }
            }
        }

        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let active_sid = self.active_session_id.clone();
        let entity = cx.entity().clone();

        cx.spawn(async move |_, cx| {
            let mut health_ok = client.health().await.is_ok();
            if !health_ok {
                // Retry briefly in case the sidecar is still finishing cold boot
                for _ in 0..4 {
                    smol::Timer::after(std::time::Duration::from_millis(350)).await;
                    if client.health().await.is_ok() {
                        health_ok = true;
                        break;
                    }
                }
            }
            if !health_ok {
                let _ = entity.update(cx, |this, cx| {
                    let was_connected = this.sidecar_connected;
                    this.sidecar_connected = false;
                    if was_connected {
                        this.controller.stop_sse();
                        this.running_events_subscribed = false;
                        cx.notify();
                    }
                });
                return;
            }

            let sessions_val = client.list_sessions(false).await.ok();
            let running_val = client.get_json_path("/api/agent/running").await.ok();
            let models_val = client.get_models(Some(&cwd_str)).await.ok();
            let enabled_models_val = client.get_enabled_models(Some(&cwd_str)).await.ok();
            let models_config_val = client.get_models_config().await.ok();
            let auth_val = client.get_auth_all_providers().await.ok();
            let oauth_val = client.get_oauth_providers().await.ok();
            let artifacts_settings_val = client.get_artifacts_settings().await.ok();
            let auto_title_settings_val = client.get_session_title_settings().await.ok();
            let tasks_val = client.get_agent_tasks().await.ok();
            let sched_val = client.get_scheduled_tasks().await.ok();
            let mcp_val = client.get_mcp(Some(&cwd_str)).await.ok();
            let skills_val = client.get_skills(Some(&cwd_str)).await.ok();
            let plugins_val = client.get_plugins(Some(&cwd_str)).await.ok();
            let subagent_settings_val = client.get_subagent_settings().await.ok();
            let subagents_val = client.get_subagents(Some(&cwd_str)).await.ok();
            let proxy_val = client.get_network_proxy().await.ok();
            let artifacts_val = client.get_artifacts(None).await.ok();
            let worktrees_val = client.list_worktrees(&cwd_str).await.ok();
            let agent_state_val = match active_sid {
                Some(ref sid) => client.get_agent_state(sid).await.ok(),
                None => None,
            };

            let _ = entity.update(cx, |this, cx| {
                this.sidecar_connected = true;
                this.subscribe_running_events(cx);
                if let Some(val) = sessions_val {
                    if let Some(arr) = val.get("sessions").and_then(Value::as_array) {
                        let parsed: Vec<SessionSummary> = arr
                            .iter()
                            .filter_map(|v| serde_json::from_value(v.clone()).ok())
                            .collect();
                        // Always trust the sidecar's session list, including an
                        // empty one: otherwise deleted sessions (and the old
                        // demo workspace) survived in the sidebar forever.
                        this.sessions = parsed;
                        this.cleanup_expired_temp_chats(cx);
                    }
                    if let Some(ids) = val.get("runningSessionIds").and_then(Value::as_array) {
                        let running_set: HashSet<String> = ids
                            .iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect();
                        this.apply_running_ids(running_set);
                    }
                }
                if let Some(val) = running_val {
                    if let Some(ids) = val.get("runningSessionIds").and_then(Value::as_array) {
                        let running_set: HashSet<String> = ids
                            .iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect();
                        this.apply_running_ids(running_set);
                    }
                }
                if let Some(val) = models_val {
                    this.apply_models_response(&val);
                }
                if let Some(val) = enabled_models_val {
                    this.apply_enabled_models_response(&val);
                }
                if let Some(val) = models_config_val {
                    this.apply_models_config_response(&val);
                    this.enrich_thinking_levels_from_config();
                }
                if let Some(val) = artifacts_settings_val {
                    if let Some(en) = val.get("enabled").and_then(Value::as_bool) {
                        this.artifacts_prompt_enabled = en;
                    }
                }
                if let Some(val) = auto_title_settings_val {
                    if let Some(en) = val.get("enabled").and_then(Value::as_bool) {
                        this.auto_title_enabled = en;
                    }
                }
                if let Some(val) = auth_val {
                    this.apply_auth_providers_response(&val);
                }
                if let Some(val) = oauth_val {
                    this.apply_oauth_providers_response(&val);
                }
                if let Some(val) = tasks_val {
                    this.apply_tasks_response(&val);
                } else {
                    this.apply_tasks_response(&json!({ "tasks": [] }));
                }
                if let Some(val) = sched_val {
                    this.apply_scheduled_tasks_response(&val);
                }
                if let Some(val) = mcp_val {
                    this.apply_mcp_response(&val);
                }
                if let Some(val) = skills_val {
                    this.apply_skills_response(&val);
                }
                if let Some(val) = plugins_val {
                    this.apply_plugins_response(&val);
                }
                this.settings_loading = false;
                if let Some(val) = subagent_settings_val {
                    if let Some(en) = val.get("enabled").and_then(Value::as_bool) {
                        this.subagent_built_in_enabled = en;
                    }
                    if let Some(mc) = val.get("maxConcurrent").and_then(Value::as_u64) {
                        this.subagent_max_concurrent = mc as usize;
                    }
                }
                if let Some(val) = subagents_val {
                    this.apply_subagents_response(&val);
                }
                if let Some(val) = proxy_val {
                    this.apply_network_proxy_response(&val);
                }
                if let Some(val) = artifacts_val {
                    this.apply_artifacts_response(&val);
                }
                if let Some(val) = worktrees_val {
                    if let Some(arr) = val.get("worktrees").and_then(Value::as_array) {
                        this.worktrees = arr
                            .iter()
                            .filter_map(|v| serde_json::from_value(v.clone()).ok())
                            .collect();
                    }
                }
                if let Some(val) = agent_state_val {
                    this.apply_agent_state_response(&val);
                }
                this.sync_tool_subagent_runs_from_messages();
                if let Some(sid) = this.active_session_id.clone() {
                    if this.messages.is_empty() {
                        this.select_session(sid, cx);
                    } else if this.controller.sse_task.is_none() {
                        this.connect_session_sse(sid, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn refresh_tasks_from_sidecar(&mut self, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            self.apply_tasks_response(&json!({ "tasks": [] }));
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let tasks_val = client.get_agent_tasks().await.ok();
            let _ = entity.update(cx, |this, cx| {
                if let Some(val) = tasks_val {
                    this.apply_tasks_response(&val);
                } else {
                    this.apply_tasks_response(&json!({ "tasks": [] }));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn sync_active_agent_state(&mut self, session_id: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.get_agent_state(&session_id).await {
                let _ = entity.update(cx, |this, cx| {
                    if this.active_session_id.as_deref() == Some(session_id.as_str()) {
                        this.apply_agent_state_response(&val);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub fn sanitize_thinking_levels(
        levels: &mut Vec<String>,
        map: &std::collections::HashMap<String, String>,
        nulled: &std::collections::HashSet<String>,
    ) {
        let filtered: Vec<String> = levels
            .iter()
            .filter(|l| !nulled.contains(l.as_str()))
            .cloned()
            .collect();
        let canonical_self_targets: std::collections::HashSet<String> = filtered
            .iter()
            .filter_map(|l| {
                let lower = l.to_lowercase();
                let target = map
                    .get(l.as_str())
                    .map(|v| v.to_lowercase())
                    .unwrap_or_else(|| lower.clone());
                if lower == target {
                    Some(target)
                } else {
                    None
                }
            })
            .collect();
        let mut seen_targets = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(filtered.len());
        for l in filtered {
            let lower = l.to_lowercase();
            let target = map
                .get(l.as_str())
                .map(|v| v.to_lowercase())
                .unwrap_or_else(|| lower.clone());
            if lower != target && canonical_self_targets.contains(&target) {
                continue;
            }
            if seen_targets.insert(target) {
                deduped.push(l);
            }
        }
        *levels = deduped;
    }

    pub fn visible_picker_models(&self) -> Vec<ModelInfo> {
        let hidden: std::collections::HashSet<&str> = self
            .agy_link_state
            .as_ref()
            .map(|s| s.settings.hidden_models.iter().map(|id| id.trim()).collect())
            .unwrap_or_default();
        if hidden.is_empty() {
            return self.models.clone();
        }
        self.models
            .iter()
            .filter(|m| !(m.provider == "agy-link" && hidden.contains(m.id.as_str())))
            .cloned()
            .collect()
    }

    fn apply_models_response(&mut self, val: &Value) {
        let arr = val
            .get("modelList")
            .and_then(Value::as_array)
            .or_else(|| val.get("models").and_then(Value::as_array));
        if let Some(items) = arr {
            let levels_obj = val.get("thinkingLevels").and_then(Value::as_object);
            let maps_obj = val.get("thinkingLevelMaps").and_then(Value::as_object);
            let mut parsed: Vec<ModelInfo> = items
                .iter()
                .filter_map(|v| serde_json::from_value::<ModelInfo>(v.clone()).ok())
                .collect();
            for m in &mut parsed {
                let key = format!("{}:{}", m.provider, m.id);
                if let Some(lvls) = levels_obj.and_then(|o| o.get(&key)).and_then(Value::as_array) {
                    m.thinking_levels = lvls
                        .iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect();
                }
                let mut nulled = std::collections::HashSet::new();
                if let Some(map_obj) = maps_obj.and_then(|o| o.get(&key)).and_then(Value::as_object) {
                    for (k, v) in map_obj {
                        if v.is_null() {
                            nulled.insert(k.clone());
                        } else if let Some(lbl) = v.as_str() {
                            m.thinking_level_map.insert(k.clone(), lbl.to_string());
                        }
                    }
                }
                Self::sanitize_thinking_levels(&mut m.thinking_levels, &m.thinking_level_map, &nulled);
            }
            // Trust an empty model list too, so removing the last provider
            // immediately clears the composer's model picker.
            self.models = parsed;
            let selected_missing = self
                .selected_model
                .as_ref()
                .map(|selected| {
                    !self
                        .models
                        .iter()
                        .any(|m| m.provider == selected.provider && m.id == selected.id)
                })
                .unwrap_or(false);
            if selected_missing {
                self.selected_model = None;
            }
        }
        if let Some(def) = val.get("defaultModel") {
            let dp = def.get("provider").and_then(Value::as_str);
            let dm = def.get("modelId").and_then(Value::as_str);
            if let (Some(p), Some(id)) = (dp, dm) {
                let existing_idx = self.selected_model.as_ref().and_then(|cur| {
                    self.models
                        .iter()
                        .position(|m| m.provider == cur.provider && m.id == cur.id)
                });
                if let Some(idx) = existing_idx {
                    self.selected_model = Some(self.models[idx].clone());
                } else if let Some(found) = self.models.iter().find(|m| m.provider == p && m.id == id) {
                    self.selected_model = Some(found.clone());
                }
            }
        }
        if self.selected_model.is_none() {
            self.selected_model = self.models.first().cloned();
        }
        if let Some(tl) = val.get("defaultThinkingLevel").and_then(Value::as_str) {
            if let Some(parsed_tl) = parse_thinking_level_str(tl) {
                self.thinking_level = parsed_tl;
            }
        }
        self.clamp_thinking_level_to_model();
    }

    /// pi only advertises the levels of models in its own capability catalog, so a
    /// custom provider configured with `reasoning: true` (or a `thinkingLevelMap`)
    /// still reports just `["off"]`. Fill those in from the model's own
    /// configuration so the composer shows the strengths the user set up.
    pub fn enrich_thinking_levels_from_config(&mut self) {
        if self.configured_providers.is_empty() {
            return;
        }
        let mut changed = false;
        for model in &mut self.models {
            let declared = self
                .configured_providers
                .iter()
                .find(|p| p.id == model.provider)
                .and_then(|p| p.models.iter().find(|m| m.id == model.id));
            let Some(declared) = declared else { continue };
            // Hide only when the model is explicitly declared non-reasoning; a
            // missing flag (deepseek's entries) keeps the ladder visible.
            let wants_reasoning = should_offer_thinking_ladder(declared);
            if !wants_reasoning {
                continue;
            }
            let only_off = model.thinking_levels.is_empty()
                || (model.thinking_levels.len() == 1 && model.thinking_levels[0] == "off");
            if !only_off {
                continue;
            }
            let mut levels: Vec<String> = vec!["off".to_string()];
            for level in ["minimal", "low", "medium", "high", "xhigh"] {
                if let Some(mapped) = declared.thinking_level_map.get(level) {
                    if mapped.is_null() {
                        continue;
                    }
                    levels.push(level.to_string());
                    if let Some(label) = mapped.as_str() {
                        if !label.is_empty() {
                            model
                                .thinking_level_map
                                .insert(level.to_string(), label.to_string());
                        }
                    }
                } else if declared.thinking_level_map.is_empty() {
                    levels.push(level.to_string());
                }
            }
            if levels.len() > 1 {
                model.thinking_levels = levels;
                changed = true;
            }
        }
        if changed {
            if let Some(sel) = &self.selected_model {
                if let Some(fresh) = self
                    .models
                    .iter()
                    .find(|m| m.provider == sel.provider && m.id == sel.id)
                {
                    self.selected_model = Some(fresh.clone());
                }
            }
            self.clamp_thinking_level_to_model();
        }
    }

    pub fn clamp_thinking_level_to_model(&mut self) {
        let supported: Vec<String> = self
            .selected_model
            .as_ref()
            .map(|m| m.thinking_levels.clone())
            .unwrap_or_default();
        if supported.is_empty() {
            self.thinking_level = ThinkingLevel::Off;
            return;
        }
        let cur = self.thinking_level;
        // Check if current level is supported (accounting for "max" == "xhigh")
        let is_supported = supported
            .iter()
            .any(|s| parse_thinking_level_str(s) == Some(cur));
        if !is_supported {
            // Pick the model's preferred reasoning level (medium, or first non-off), fallback to off
            let preferred = supported
                .iter()
                .filter(|s| s.as_str() != "off")
                .find_map(|s| {
                    let lvl = parse_thinking_level_str(s)?;
                    if lvl == ThinkingLevel::Medium {
                        Some(lvl)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    supported
                        .iter()
                        .filter(|s| s.as_str() != "off")
                        .find_map(|s| parse_thinking_level_str(s))
                })
                .or_else(|| supported.first().and_then(|s| parse_thinking_level_str(s)))
                .unwrap_or(ThinkingLevel::Off);
            self.thinking_level = preferred;
        }
    }

    pub fn apply_scheduled_tasks_response(&mut self, val: &Value) {
        if let Some(arr) = val.get("tasks").and_then(Value::as_array) {
            self.scheduled_tasks = arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            self.scheduled_running_ids = self
                .scheduled_tasks
                .iter()
                .filter(|t| {
                    t.last_run
                        .as_ref()
                        .is_some_and(|r| r.status == "running")
                })
                .map(|t| t.id.clone())
                .collect();
        }
        if let Some(sched) = val.get("scheduler") {
            let owner = sched.get("owner").and_then(Value::as_bool).unwrap_or(true);
            let info = sched.get("ownerInfo");
            self.scheduler_owner = SchedulerOwnerStatus {
                owner,
                pid: info.and_then(|i| i.get("pid")).and_then(Value::as_u64),
                heartbeat_at: info
                    .and_then(|i| i.get("heartbeatAt"))
                    .and_then(Value::as_str)
                    .map(String::from),
                started_at: info
                    .and_then(|i| i.get("startedAt"))
                    .and_then(Value::as_str)
                    .map(String::from),
                alive: info.and_then(|i| i.get("alive")).and_then(Value::as_bool).unwrap_or(true),
                fresh: info.and_then(|i| i.get("fresh")).and_then(Value::as_bool).unwrap_or(true),
            };
        }
    }

    pub fn refresh_scheduled_tasks(&mut self, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            self.scheduled_tasks = crate::client::local_store::load_scheduled_tasks_local();
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.get_scheduled_tasks().await {
                let _ = entity.update(cx, |this, cx| {
                    this.apply_scheduled_tasks_response(&val);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn apply_enabled_models_response(&mut self, val: &Value) {
        let mut providers = Vec::new();
        if let Some(arr) = val.get("providers").and_then(Value::as_array) {
            for p in arr {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = p
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let kind = p.get("kind").and_then(Value::as_str).unwrap_or("builtin").to_string();
                let enabled_count = p.get("enabledCount").and_then(Value::as_u64).unwrap_or(0) as usize;
                let mut models = Vec::new();
                if let Some(m_arr) = p.get("models").and_then(Value::as_array) {
                    for m in m_arr {
                        let mid = m.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                        let mname = m.get("name").and_then(Value::as_str).unwrap_or(&mid).to_string();
                        let mref = m
                            .get("ref")
                            .and_then(Value::as_str)
                            .map(String::from)
                            .unwrap_or_else(|| format!("{id}/{mid}"));
                        let enabled = m.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                        let cw = m.get("contextWindow").and_then(Value::as_u64);
                        if let Some(existing) = self.models.iter_mut().find(|x| x.provider == id && x.id == mid) {
                            if existing.context_window.is_none() {
                                existing.context_window = cw;
                            }
                            existing.enabled = Some(enabled);
                        }
                        models.push(EnabledModelItem {
                            id: mid,
                            name: mname,
                            model_ref: mref,
                            enabled,
                            context_window: cw,
                        });
                    }
                }
                providers.push(EnabledProviderGroup {
                    id,
                    name,
                    kind,
                    enabled_count,
                    models,
                });
            }
        }
        self.enabled_models_overview = EnabledModelsOverview {
            all_enabled: val.get("allEnabled").and_then(Value::as_bool).unwrap_or(true),
            enabled_total: val.get("enabledTotal").and_then(Value::as_u64).unwrap_or(0) as usize,
            available_total: val.get("availableTotal").and_then(Value::as_u64).unwrap_or(0) as usize,
            scope: val.get("scope").and_then(Value::as_str).unwrap_or("global").to_string(),
            settings_path: val
                .get("settingsPath")
                .and_then(Value::as_str)
                .unwrap_or("~/.pi/agent/settings.json")
                .to_string(),
            editable: val.get("editable").and_then(Value::as_bool).unwrap_or(true),
            providers,
        };
    }

    fn apply_auth_providers_response(&mut self, val: &Value) {
        if let Some(arr) = val.get("providers").and_then(Value::as_array) {
            for p in arr {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                let name = p
                    .get("name")
                    .or_else(|| p.get("displayName"))
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let configured = p
                    .get("hasKey")
                    .or_else(|| p.get("configured"))
                    .or_else(|| p.get("loggedIn"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                merge_auth_provider(&mut self.auth_providers, &id, &name, configured, true);
            }
        }
    }

    fn apply_oauth_providers_response(&mut self, val: &Value) {
        if let Some(arr) = val
            .get("oauthProviders")
            .or_else(|| val.get("providers"))
            .and_then(Value::as_array)
        {
            let mut out = Vec::new();
            for p in arr {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = p
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let logged_in = p
                    .get("loggedIn")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !id.is_empty() {
                    out.push((id, name, logged_in));
                }
            }
            for (id, name, logged_in) in out {
                merge_auth_provider(&mut self.oauth_providers, &id, &name, logged_in, false);
            }
        }
        // `/api/auth/providers` also carries the API-key provider list. Merge
        // it into the static catalog instead of only accepting it while the
        // list is empty (which is what left OpenRouter & friends invisible).
        if let Some(arr) = val.get("apiKeyProviders").and_then(Value::as_array) {
            for p in arr {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                let name = p
                    .get("displayName")
                    .or_else(|| p.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or(&id)
                    .to_string();
                let configured = p
                    .get("configured")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                merge_auth_provider(&mut self.auth_providers, &id, &name, configured, true);
            }
        }
        if let Some(names) = val.get("providerNames").and_then(Value::as_object) {
            for (id, name) in names {
                if let Some(name) = name.as_str() {
                    if let Some(existing) =
                        self.auth_providers.iter_mut().find(|(i, _, _)| i == id)
                    {
                        if existing.1 == *id {
                            existing.1 = name.to_string();
                        }
                    }
                    if let Some(existing) =
                        self.oauth_providers.iter_mut().find(|(i, _, _)| i == id)
                    {
                        if existing.1 == *id {
                            existing.1 = name.to_string();
                        }
                    }
                }
            }
        }
    }

    pub fn apply_artifacts_response(&mut self, val: &Value) {
        let Some(arr) = val.get("artifacts").and_then(Value::as_array) else {
            return;
        };
        let mut parsed = Vec::new();
        for v in arr {
            let path_opt = v.get("path").or_else(|| v.get("displayPath")).and_then(Value::as_str);
            let id = v.get("identifier")
                .and_then(Value::as_str)
                .or(path_opt)
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let title = v.get("title")
                .or_else(|| v.get("displayPath"))
                .and_then(Value::as_str)
                .unwrap_or(&id)
                .to_string();
            let session_id = v.get("sessionId").and_then(Value::as_str).map(String::from);
            let updated_at = v.get("updated").and_then(Value::as_str).map(String::from);
            let version = v.get("versionCount").and_then(Value::as_u64).unwrap_or(1) as usize;
            let favorite = v.get("favorite").and_then(Value::as_bool).unwrap_or(false);

            let ext = path_opt.and_then(|p| p.rsplit('.').next()).unwrap_or("");
            let kind = match ext.to_lowercase().as_str() {
                "html" | "htm" => ArtifactKind::Html,
                "svg" => ArtifactKind::Svg,
                "tsx" | "jsx" => ArtifactKind::React,
                "md" | "markdown" => ArtifactKind::Markdown,
                "mermaid" => ArtifactKind::Mermaid,
                _ => ArtifactKind::Code,
            };

            let content = v
                .get("content")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_default();

            parsed.push(ArtifactRecord {
                identifier: id,
                title,
                kind,
                language: Some(ext.to_string()),
                file_path: path_opt.map(String::from),
                content,
                previous_content: None,
                version,
                session_id,
                updated_at,
                favorite,
            });
        }
        // Preserve already-fetched content/previous_content when refreshing metadata list
        for art in &mut parsed {
            if let Some(prev) = self.artifacts.iter().find(|a| a.identifier == art.identifier) {
                if art.content.is_empty() && !prev.content.is_empty() {
                    art.content = prev.content.clone();
                }
                if art.previous_content.is_none() && prev.previous_content.is_some() {
                    art.previous_content = prev.previous_content.clone();
                }
            }
        }
        if !parsed.is_empty() {
            self.artifacts = parsed;
        }
    }

    pub fn refresh_artifacts_from_sidecar(&mut self, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.get_artifacts(None).await {
                let _ = entity.update(cx, |this, cx| {
                    this.apply_artifacts_response(&val);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn apply_models_config_response(&mut self, val: &Value) {
        let root = val.get("config").unwrap_or(val);
        self.configured_providers_raw = root.clone();
        let mut out = Vec::new();
        if let Some(provs) = root.get("providers").and_then(Value::as_object) {
            for (pid, pval) in provs {
                let display_name = pval
                    .get("name")
                    .and_then(Value::as_str)
                    .map(String::from);
                let base_url = pval
                    .get("baseUrl")
                    .and_then(Value::as_str)
                    .map(String::from);
                let api = pval.get("api").and_then(Value::as_str).map(String::from);
                let has_api_key = pval
                    .get("apiKey")
                    .and_then(Value::as_str)
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false);
                let mut models = Vec::new();
                if let Some(arr) = pval.get("models").and_then(Value::as_array) {
                    for m in arr {
                        let id = m.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                        if id.is_empty() {
                            continue;
                        }
                        let name = m.get("name").and_then(Value::as_str).map(String::from);
                        let context_window = m.get("contextWindow").and_then(Value::as_u64);
                        let max_tokens = m.get("maxTokens").and_then(Value::as_u64);
                        let input = m
                            .get("input")
                            .and_then(Value::as_array)
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .map(String::from)
                                    .collect()
                            })
                            .unwrap_or_else(|| vec!["text".to_string()]);
                        let reasoning_declared = m.get("reasoning").is_some();
                        let reasoning = m.get("reasoning").and_then(Value::as_bool).unwrap_or(false);
                        let thinking_level_map = m
                            .get("thinkingLevelMap")
                            .and_then(Value::as_object)
                            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                            .unwrap_or_default();
                        models.push(ConfiguredModelInfo {
                            id,
                            name,
                            context_window,
                            max_tokens,
                            input,
                            reasoning,
                            reasoning_declared,
                            thinking_level_map,
                        });
                    }
                }
                out.push(ConfiguredProviderInfo {
                    id: pid.clone(),
                    display_name,
                    base_url,
                    api,
                    has_api_key,
                    models,
                });
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        self.configured_providers = out;
    }

    /// Enables or disables every model of a provider in the enabled-model list.
    pub fn set_provider_enabled(&mut self, provider_id: String, enabled: bool, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            let refs: Vec<String> = self
                .enabled_models_overview
                .providers
                .iter()
                .filter(|p| p.id == provider_id)
                .flat_map(|p| p.models.iter().map(|m| m.model_ref.clone()))
                .chain(
                    self.configured_providers
                        .iter()
                        .filter(|p| p.id == provider_id)
                        .flat_map(|p| p.models.iter().map(|m| format!("{}/{}", p.id, m.id))),
                )
                .collect();
            let _ = crate::client::local_store::update_enabled_models_local(&json!({
                "op": "provider",
                "provider": provider_id,
                "enabled": enabled,
                "providerModels": refs,
            }));
            for prov in &mut self.enabled_models_overview.providers {
                if prov.id == provider_id {
                    for m in &mut prov.models {
                        m.enabled = enabled;
                    }
                    prov.enabled_count = if enabled { prov.models.len() } else { 0 };
                }
            }
            for m in &mut self.models {
                if m.provider == provider_id {
                    m.enabled = Some(enabled);
                }
            }
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let cwd = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(res) = client
                .set_enabled_model(json!({
                    "op": "provider",
                    "provider": provider_id,
                    "enabled": enabled,
                    "cwd": cwd,
                }))
                .await
            {
                let _ = entity.update(cx, |s, cx| {
                    s.apply_enabled_models_response(&res);
                    s.refresh_from_sidecar(cx);
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Enables or disables one model reference (`provider/model`) in the enabled-model list.
    pub fn set_model_enabled(
        &mut self,
        model_ref: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected {
            let _ = crate::client::local_store::update_enabled_models_local(&json!({
                "op": "model",
                "ref": model_ref,
                "enabled": enabled,
            }));
            for prov in &mut self.enabled_models_overview.providers {
                for m in &mut prov.models {
                    if m.model_ref == model_ref || format!("{}/{}", prov.id, m.id) == model_ref {
                        m.enabled = enabled;
                    }
                }
                prov.enabled_count = prov.models.iter().filter(|m| m.enabled).count();
            }
            for m in &mut self.models {
                if format!("{}/{}", m.provider, m.id) == model_ref {
                    m.enabled = Some(enabled);
                }
            }
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let cwd = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(res) = client
                .set_enabled_model(json!({
                    "op": "models",
                    "refs": [model_ref],
                    "enabled": enabled,
                    "cwd": cwd,
                }))
                .await
            {
                let _ = entity.update(cx, |s, cx| {
                    s.apply_enabled_models_response(&res);
                    s.refresh_from_sidecar(cx);
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Rewrites one provider's editable fields in `models.json` (id stays the key).
    pub fn update_configured_provider(
        &mut self,
        provider_id: String,
        display_name: String,
        base_url: String,
        api: String,
        api_key: String,
        cx: &mut Context<Self>,
    ) {
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        if let Some(entry) = config
            .get_mut("providers")
            .and_then(Value::as_object_mut)
            .and_then(|p| p.get_mut(&provider_id))
            .and_then(Value::as_object_mut)
        {
            if display_name.trim().is_empty() {
                entry.remove("name");
            } else {
                entry.insert("name".to_string(), json!(display_name.trim()));
            }
            if base_url.trim().is_empty() {
                entry.remove("baseUrl");
            } else {
                entry.insert("baseUrl".to_string(), json!(base_url.trim()));
            }
            if !api.trim().is_empty() {
                entry.insert("api".to_string(), json!(api.trim()));
            }
            if api_key.trim().is_empty() {
                entry.remove("apiKey");
            } else {
                entry.insert("apiKey".to_string(), json!(api_key.trim()));
            }
        }
        let _ = crate::client::local_store::save_models_config_local(&config);
        self.apply_models_config_response(&crate::client::local_store::read_models_config_local());
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.put_models_config(config).await;
                let _ = entity.update(cx, |s, cx| {
                    s.refresh_from_sidecar(cx);
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Rewrites one model entry of a provider in `models.json`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_configured_model(
        &mut self,
        provider_id: String,
        old_model_id: String,
        model: ConfiguredModelInfo,
        cx: &mut Context<Self>,
    ) {
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), json!(model.id));
        if let Some(name) = model.name.filter(|s| !s.trim().is_empty()) {
            obj.insert("name".to_string(), json!(name));
        }
        if let Some(cw) = model.context_window {
            obj.insert("contextWindow".to_string(), json!(cw));
        }
        if let Some(mt) = model.max_tokens {
            obj.insert("maxTokens".to_string(), json!(mt));
        }
        if !model.input.is_empty() {
            obj.insert("input".to_string(), json!(model.input));
        }
        obj.insert("reasoning".to_string(), json!(model.reasoning));

        if let Some(arr) = config
            .get_mut("providers")
            .and_then(|p| p.get_mut(&provider_id))
            .and_then(|p| p.get_mut("models"))
            .and_then(Value::as_array_mut)
        {
            if let Some(slot) = arr
                .iter_mut()
                .find(|m| m.get("id").and_then(Value::as_str) == Some(old_model_id.as_str()))
            {
                *slot = Value::Object(obj);
            } else {
                arr.push(Value::Object(obj));
            }
        }
        let _ = crate::client::local_store::save_models_config_local(&config);
        self.apply_models_config_response(&crate::client::local_store::read_models_config_local());
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.put_models_config(config).await;
                let _ = entity.update(cx, |s, cx| {
                    s.refresh_from_sidecar(cx);
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub fn delete_configured_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let del_key = format!("provider:{provider_id}");
        if self.settings_deleting_keys.contains(&del_key) {
            return;
        }
        self.settings_deleting_keys.insert(del_key.clone());
        cx.notify();

        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        if let Some(provs) = config.get_mut("providers").and_then(Value::as_object_mut) {
            provs.remove(&provider_id);
        }
        let disable_refs: Vec<String> = self
            .enabled_models_overview
            .providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| p.models.iter().map(|m| m.model_ref.clone()))
            .collect();

        let sidecar_connected = self.sidecar_connected;
        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            smol::Timer::after(std::time::Duration::from_millis(250)).await;
            let _ = crate::client::local_store::save_models_config_local(&config);
            let _ = crate::client::local_store::delete_provider_auth_local(&provider_id);
            if !disable_refs.is_empty() {
                let _ = crate::client::local_store::update_enabled_models_local(&json!({
                    "op": "provider",
                    "provider": provider_id,
                    "enabled": false,
                    "providerModels": disable_refs,
                }));
            }
            if sidecar_connected {
                let _ = client.put_models_config(config).await;
                if !disable_refs.is_empty() {
                    let _ = client
                        .set_enabled_model(json!({
                            "op": "provider",
                            "provider": provider_id,
                            "enabled": false,
                            "cwd": cwd_str,
                        }))
                        .await;
                }
            }
            let local_cfg = crate::client::local_store::read_models_config_local();
            let _ = entity.update(cx, |s, cx| {
                s.expanded_configured_providers.remove(&provider_id);
                s.settings_deleting_keys.remove(&del_key);
                for prov in &mut s.enabled_models_overview.providers {
                    if prov.id == provider_id {
                        for m in &mut prov.models {
                            m.enabled = false;
                        }
                        prov.enabled_count = 0;
                    }
                }
                for m in &mut s.models {
                    if m.provider == provider_id {
                        m.enabled = Some(false);
                    }
                }
                s.apply_models_config_response(&local_cfg);
                if sidecar_connected {
                    s.refresh_from_sidecar(cx);
                } else {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn delete_model_from_configured_provider(
        &mut self,
        provider_id: String,
        model_id: String,
        cx: &mut Context<Self>,
    ) {
        let del_key = format!("model:{provider_id}:{model_id}");
        if self.settings_deleting_keys.contains(&del_key) {
            return;
        }
        self.settings_deleting_keys.insert(del_key.clone());
        cx.notify();

        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        if let Some(arr) = config
            .get_mut("providers")
            .and_then(|p| p.get_mut(&provider_id))
            .and_then(|p| p.get_mut("models"))
            .and_then(Value::as_array_mut)
        {
            arr.retain(|m| m.get("id").and_then(Value::as_str) != Some(model_id.as_str()));
        }
        let _ = crate::client::local_store::save_models_config_local(&config);
        self.apply_models_config_response(&crate::client::local_store::read_models_config_local());
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.put_models_config(config).await;
                let _ = entity.update(cx, |s, cx| {
                    s.settings_deleting_keys.remove(&del_key);
                    s.refresh_from_sidecar(cx);
                });
            })
            .detach();
        } else {
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                smol::Timer::after(std::time::Duration::from_millis(250)).await;
                let _ = entity.update(cx, |s, cx| {
                    s.settings_deleting_keys.remove(&del_key);
                    cx.notify();
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub fn add_models_to_configured_provider(
        &mut self,
        provider_id: String,
        models: Vec<ConfiguredModelInfo>,
        cx: &mut Context<Self>,
    ) {
        if models.is_empty() {
            return;
        }
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };

        if let Some(provs) = config.get_mut("providers").and_then(Value::as_object_mut) {
            let entry = provs.entry(provider_id.clone()).or_insert_with(|| json!({}));
            if let Some(pobj) = entry.as_object_mut() {
                let arr = pobj.entry("models").or_insert_with(|| json!([]));
                if let Some(vec) = arr.as_array_mut() {
                    for model in models {
                        let mut obj = serde_json::Map::new();
                        obj.insert("id".to_string(), json!(model.id));
                        if let Some(name) = model.name.filter(|s| !s.trim().is_empty()) {
                            obj.insert("name".to_string(), json!(name));
                        }
                        if let Some(cw) = model.context_window {
                            obj.insert("contextWindow".to_string(), json!(cw));
                        }
                        if let Some(mt) = model.max_tokens {
                            obj.insert("maxTokens".to_string(), json!(mt));
                        }
                        if !model.input.is_empty() {
                            obj.insert("input".to_string(), json!(model.input));
                        }
                        obj.insert("reasoning".to_string(), json!(model.reasoning));

                        vec.retain(|m| m.get("id").and_then(Value::as_str) != Some(model.id.as_str()));
                        vec.push(Value::Object(obj));
                    }
                }
            }
        }
        self.expanded_configured_providers.insert(provider_id);
        let _ = crate::client::local_store::save_models_config_local(&config);
        self.apply_models_config_response(&crate::client::local_store::read_models_config_local());
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.put_models_config(config).await;
                let _ = entity.update(cx, |s, cx| {
                    s.refresh_from_sidecar(cx);
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub fn add_model_to_configured_provider(
        &mut self,
        provider_id: String,
        model: ConfiguredModelInfo,
        cx: &mut Context<Self>,
    ) {
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), json!(model.id));
        if let Some(name) = model.name.filter(|s| !s.trim().is_empty()) {
            obj.insert("name".to_string(), json!(name));
        }
        if let Some(cw) = model.context_window {
            obj.insert("contextWindow".to_string(), json!(cw));
        }
        if let Some(mt) = model.max_tokens {
            obj.insert("maxTokens".to_string(), json!(mt));
        }
        if !model.input.is_empty() {
            obj.insert("input".to_string(), json!(model.input));
        }
        obj.insert("reasoning".to_string(), json!(model.reasoning));

        if let Some(provs) = config.get_mut("providers").and_then(Value::as_object_mut) {
            let entry = provs.entry(provider_id.clone()).or_insert_with(|| json!({}));
            if let Some(pobj) = entry.as_object_mut() {
                let arr = pobj.entry("models").or_insert_with(|| json!([]));
                if let Some(vec) = arr.as_array_mut() {
                    vec.retain(|m| {
                        m.get("id").and_then(Value::as_str) != Some(model.id.as_str())
                    });
                    vec.push(Value::Object(obj));
                }
            }
        }
        self.expanded_configured_providers.insert(provider_id);
        let _ = crate::client::local_store::save_models_config_local(&config);
        self.apply_models_config_response(&crate::client::local_store::read_models_config_local());
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.put_models_config(config).await;
                let _ = entity.update(cx, |s, cx| {
                    s.refresh_from_sidecar(cx);
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub fn discover_models_for_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        if self.discovering_provider_id.is_some() {
            return;
        }
        // Discovery is a direct HTTP request from the desktop to the provider,
        // so it must work while the sidecar is disconnected too.
        let mut provider_raw = self
            .configured_providers_raw
            .get("providers")
            .and_then(|p| p.get(&provider_id))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Some(preset) = crate::provider_catalog::find(&provider_id) {
            if provider_raw
                .get("baseUrl")
                .and_then(Value::as_str)
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                provider_raw["baseUrl"] = json!(preset.base_url);
            }
            if provider_raw
                .get("api")
                .and_then(Value::as_str)
                .map(|v| v.trim().is_empty())
                .unwrap_or(true)
            {
                provider_raw["api"] = json!(preset.api);
            }
        }
        self.discovering_provider_id = Some(provider_id.clone());
        self.discover_error = None;
        let client = self.client.clone();
        let entity = cx.entity().clone();
        let pid_for_error = provider_id.clone();
        cx.spawn(async move |_, cx| {
            // Strictly the provider's own Base URL: the pi core is never asked to probe
            // on our behalf, so the request always goes to the configured address.
            let has_base_url = provider_raw
                .get("baseUrl")
                .and_then(Value::as_str)
                .map(|u| !u.trim().is_empty())
                .unwrap_or(false);
            let discovered = if has_base_url {
                client
                    .discover_models_from_provider(&provider_id, &provider_raw)
                    .await
                    .map_err(|e| e.to_string())
            } else {
                Err("This provider has no Base URL - set one before probing".to_string())
            };

            let mut failure: Option<String> = None;
            let mut parsed_models: Vec<ConfiguredModelInfo> = Vec::new();
            match discovered.map_err(anyhow::Error::msg) {
                Ok(payload) => {
                    if let Some(models_arr) = payload.get("models").and_then(Value::as_array) {
                        for item in models_arr {
                            let Some(mid) = item.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) else { continue };
                            let name = item.get("name").and_then(Value::as_str).map(String::from);
                            let context_window = item.get("contextWindow").and_then(Value::as_u64);
                            let max_tokens = item.get("maxTokens").and_then(Value::as_u64);
                            let reasoning = item.get("reasoning").and_then(Value::as_bool).unwrap_or(false);
                            let input = item.get("input").and_then(Value::as_array).map(|arr| {
                                arr.iter().filter_map(Value::as_str).map(String::from).collect()
                            }).unwrap_or_else(|| vec!["text".to_string()]);
                            parsed_models.push(ConfiguredModelInfo {
                                id: mid.to_string(),
                                name,
                                context_window,
                                max_tokens,
                                input,
                                reasoning,
                                reasoning_declared: true,
                                thinking_level_map: Default::default(),
                            });
                        }
                    }
                    if parsed_models.is_empty() {
                        failure = Some("No models found for this provider".to_string());
                    }
                }
                Err(err) => failure = Some(err.to_string()),
            }
            let _ = entity.update(cx, |s, cx| {
                s.discovering_provider_id = None;
                s.discover_error = failure.map(|msg| (pid_for_error.clone(), msg));
                s.expanded_configured_providers.insert(provider_id.clone());
                if !parsed_models.is_empty() {
                    s.settings_add_kind = Some(SettingsAddKind::DiscoveredModelsPicker {
                        provider_id: pid_for_error,
                        discovered: parsed_models,
                    });
                }
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn trigger_desktop_notification(&self, title: &str, body: &str) {
        if !self.notify_enabled {
            return;
        }
        let title = title.to_string();
        let body = body.to_string();
        let with_sound = self.notify_sound;
        std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                if !send_macos_bundle_notification(&title, &body, with_sound) {
                    let esc = |s: &str| {
                        s.replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .replace('\n', " ")
                    };
                    let script = if with_sound {
                        format!(
                            "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
                            esc(&body),
                            esc(&title)
                        )
                    } else {
                        format!(
                            "display notification \"{}\" with title \"{}\"",
                            esc(&body),
                            esc(&title)
                        )
                    };
                    let _ = std::process::Command::new("osascript")
                        .arg("-e")
                        .arg(script)
                        .status();
                }
                if with_sound {
                    let _ = std::process::Command::new("afplay")
                        .arg("/System/Library/Sounds/Glass.aiff")
                        .status();
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (title, body, with_sound);
            }
        });
    }

    pub fn start_external_jsonl_watcher(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let mut last_file_sig: Option<(String, u64, std::time::SystemTime)> = None;
            let mut last_dir_sig: Option<(usize, u64)> = None;
            let mut scan_counter = 0usize;
            loop {
                smol::Timer::after(std::time::Duration::from_millis(500)).await;
                scan_counter += 1;
                let mut check_path: Option<(String, PathBuf, u64)> = None;
                let mut skip_active_parse = false;
                let mut alive = false;
                entity.update(cx, |this, _| {
                    alive = true;
                    skip_active_parse = this.sidecar_connected && this.is_agent_running;
                    if let Some(sid) = this.active_session_id.as_deref() {
                        if let Some(sess) = this.sessions.iter().find(|s| s.id == sid) {
                            if let Some(p) = &sess.path {
                                let cw = this
                                    .selected_model
                                    .as_ref()
                                    .and_then(|m| m.context_window)
                                    .unwrap_or(200_000);
                                check_path = Some((sid.to_string(), PathBuf::from(p), cw));
                            }
                        }
                    }
                });
                if !alive {
                    break;
                }

                // Check active session file for new messages from TUI / CLI off the UI thread.
                // Skip while sidecar SSE is actively streaming to avoid redundant 40MB+ re-parses.
                if !skip_active_parse {
                    if let Some((sid, path, cw)) = check_path {
                        if let Ok(meta) = std::fs::metadata(&path) {
                            let len = meta.len();
                            if let Ok(mt) = meta.modified() {
                                let changed = last_file_sig
                                    .as_ref()
                                    .map(|(id, prev_len, prev_mt)| {
                                        id != &sid || *prev_len != len || *prev_mt != mt
                                    })
                                    .unwrap_or(true);
                                last_file_sig = Some((sid.clone(), len, mt));
                                if changed {
                                    let path_bg = path.clone();
                                    let (parsed, siblings, stats, todos) = smol::unblock(move || {
                                        let mut dummy_id = 1u64;
                                        let (parsed, siblings) = parse_session_jsonl_with_siblings(
                                            &path_bg,
                                            None,
                                            &mut dummy_id,
                                        );
                                        let stats = compute_stats_from_jsonl(&path_bg, cw);
                                        let todos = extract_todos_from_jsonl(&path_bg);
                                        (parsed, siblings, stats, todos)
                                    })
                                    .await;

                                    let _ = entity.update(cx, |this, cx| {
                                        if this.active_session_id.as_deref() == Some(sid.as_str())
                                            && !(this.sidecar_connected && this.is_agent_running)
                                        {
                                            if !parsed.is_empty()
                                                && (parsed.len() != this.messages.len()
                                                    || parsed.last().map(|m| &m.content)
                                                        != this.messages.last().map(|m| &m.content))
                                            {
                                                let mut next_parsed = parsed;
                                                for m in &mut next_parsed {
                                                    m.id = this.controller.alloc_msg_id();
                                                }
                                                this.messages = next_parsed;
                                                this.branch_siblings = siblings;
                                                this.stats = stats;
                                                this.set_todos(todos);
                                                this.sync_tool_subagent_runs_from_messages();
                                                cx.notify();
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                }

                // Periodic check for new sessions created in TUI / CLI (~every 3s) off the UI thread.
                // Only re-parse session files when the directory metadata signature changes.
                if scan_counter % 6 == 0 {
                    let root = pi_sessions_root();
                    let prev_sig = last_dir_sig;
                    let (sig, scanned_opt) = smol::unblock(move || {
                        let sig = crate::client::native_index::sessions_dir_signature(&root);
                        if prev_sig == Some(sig) {
                            (sig, None)
                        } else {
                            (sig, Some(scan_local_sessions(&root)))
                        }
                    })
                    .await;
                    last_dir_sig = Some(sig);
                    if let Some(scanned) = scanned_opt {
                        let _ = entity.update(cx, |this, cx| {
                            if !scanned.is_empty() && this.sessions.len() != scanned.len() {
                                this.sessions = scanned;
                                cx.notify();
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// Computes the current `WorkerLivenessState` across foreground turns, subagent runs,
    /// pending `extension_ui_request` dialogs, and connected MCP servers (§6.3.3).
    pub fn current_worker_liveness(&self) -> crate::engine::worker_manager::WorkerLivenessState {
        let active_foreground_runs = if self.is_agent_running { 1 } else { 0 }
            + self.running_session_ids.len();
        let active_subagent_runs = self
            .subagent_runs
            .iter()
            .chain(self.tool_subagent_runs.iter())
            .filter(|r| matches!(r.state.as_str(), "running" | "queued" | "starting"))
            .count();
        let pending_ui_requests = usize::from(self.pending_ui_request.is_some())
            + self.dialog_queue.len();
        let active_mcp_sessions = self
            .mcp_servers
            .iter()
            .filter(|s| s.status == "connected")
            .count();

        crate::engine::worker_manager::WorkerLivenessState {
            active_foreground_runs,
            active_subagent_runs,
            pending_ui_requests,
            active_mcp_sessions,
        }
    }

    /// Persistent connection watchdog: keeps the sidecar permanently connected,
    /// tolerates transient slowness, and seamlessly respawns + rebinds SSE if it ever drops.
    pub fn start_sidecar_watchdog(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let mut consecutive_failures = 0u32;
            let mut boot_grace_ticks = 0u32;

            loop {
                let mut wait_ms = 5000u64;
                let mut client_opt = None;
                let mut alive = false;
                let mut process_alive = false;

                let _ = entity.update(cx, |this, cx| {
                    alive = true;
                    if !this.sidecar_connected {
                        wait_ms = 500;
                    }

                    if this.core_mode == "auto" {
                        let needs_respawn = match this._sidecar.as_mut() {
                            Some(sc) => !sc.is_alive(),
                            None => true,
                        };
                        if needs_respawn {
                            let port: u16 = std::env::var("PI_SIDECAR_PORT")
                                .ok()
                                .and_then(|p| p.parse().ok())
                                .unwrap_or(38471);
                            let token = this.client.token.clone();
                            let inst = this.client.instance_id.clone();
                            if let Some(sc) = SidecarProcess::spawn_if_available(port, token, inst) {
                                let base_url = sc.base_url();
                                this.client =
                                    PiDaemonClient::new(base_url.clone(), &sc.token, &sc.instance_id);
                                this.core_url = base_url;
                                this._sidecar = Some(sc);
                                this.controller.stop_sse();
                                this.running_events_subscribed = false;
                                boot_grace_ticks = 0;
                                consecutive_failures = 0;
                                wait_ms = 400;
                                cx.notify();
                            }
                        }
                        process_alive = this
                            ._sidecar
                            .as_mut()
                            .map(|sc| sc.is_alive())
                            .unwrap_or(false);
                    } else {
                        process_alive = true;
                    }

                    client_opt = Some(this.client.clone());
                });

                if !alive {
                    break;
                }

                smol::Timer::after(std::time::Duration::from_millis(wait_ms)).await;

                if let Some(client) = client_opt {
                    let healthy = client.health().await.is_ok();
                    let _ = entity.update(cx, |this, cx| {
                        if healthy {
                            consecutive_failures = 0;
                            boot_grace_ticks = 0;
                            if !this.sidecar_connected {
                                this.sidecar_connected = true;
                                this.refresh_from_sidecar(cx);
                                cx.notify();
                            } else {
                                // Ensure running events & active session SSE stay bound
                                if !this.running_events_subscribed {
                                    this.subscribe_running_events(cx);
                                }
                                if let Some(sid) = this.active_session_id.clone() {
                                    if this.controller.sse_task.is_none() {
                                        this.connect_session_sse(sid, cx);
                                    }
                                }
                            }
                        } else if process_alive && !this.sidecar_connected && boot_grace_ticks < 20 {
                            // Node.js is still booting up (up to ~10s); keep polling fast without killing
                            boot_grace_ticks += 1;
                        } else {
                            consecutive_failures += 1;
                            // Require 3 consecutive failures before marking disconnected / respawning
                            // so transient load spikes never cause spurious disconnects.
                            if consecutive_failures >= 3 {
                                if this.core_mode == "auto" {
                                    if let Some(ref mut sc) = this._sidecar {
                                        sc.kill();
                                    }
                                    this._sidecar = None;
                                }
                                if this.sidecar_connected {
                                    this.sidecar_connected = false;
                                    this.controller.stop_sse();
                                    this.running_events_subscribed = false;
                                    cx.notify();
                                }
                                consecutive_failures = 0;
                            }
                        }
                    });
                }
            }
        })
        .detach();
    }

    /// Starts the native Rust cron scheduler and Scale-to-Zero RPC worker reaper loop (§6.3.3).
    pub fn start_native_scheduler_and_worker_reaper(&mut self, cx: &mut Context<Self>) {
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            loop {
                smol::Timer::after(std::time::Duration::from_secs(30)).await;
                let mut alive = false;
                let _ = entity.update(cx, |this, cx| {
                    alive = true;
                    // Keep persistent long connection per user instruction; never terminate the worker for idle memory savings
                    // Native scheduler lease & due-task check when sidecar is not owning scheduler
                    if !this.sidecar_connected
                        && crate::engine::scheduler::try_acquire_scheduler_lease(90_000)
                    {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u64)
                            .unwrap_or(0);
                        let due = crate::engine::scheduler::find_due_scheduled_tasks(now_ms);
                        if !due.is_empty() {
                            this.refresh_scheduled_tasks(cx);
                        }
                    }
                });
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    fn apply_tasks_response(&mut self, val: &Value) {
        let mut by_sid: BTreeMap<String, TaskSnapshot> = BTreeMap::new();
        if let Some(arr) = val.get("tasks").and_then(Value::as_array) {
            for item in arr {
                let sid = item
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if sid.is_empty() {
                    continue;
                }
                let running = item.get("running").and_then(Value::as_bool).unwrap_or(false)
                    || self.running_session_ids.contains(&sid);
                let run_started_at = item.get("runStartedAt").and_then(Value::as_u64);
                let waiting_prompts: Vec<String> = item
                    .get("waiting")
                    .and_then(Value::as_array)
                    .map(|w| {
                        w.iter()
                            .filter_map(|req| {
                                req.get("prompt")
                                    .and_then(Value::as_str)
                                    .or_else(|| req.get("method").and_then(Value::as_str))
                                    .map(String::from)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let activity_str = item.get("activity").and_then(|act| {
                    let kind = act.get("kind").and_then(Value::as_str)?;
                    if kind == "tool" {
                        let tname = act.get("name").and_then(Value::as_str).unwrap_or("tool");
                        Some(format!("Tool: {tname}"))
                    } else {
                        Some(match kind {
                            "compacting" => "Compacting context".to_string(),
                            "retrying" => "Retrying".to_string(),
                            _ => "Thinking".to_string(),
                        })
                    }
                });
                let last_run_status = item
                    .get("lastRun")
                    .and_then(|lr| lr.get("status"))
                    .and_then(Value::as_str)
                    .map(String::from);

                let sess = self.sessions.iter().find(|s| s.id == sid);
                let title = sess
                    .map(|s| s.display_title().to_string())
                    .unwrap_or_else(|| format!("Session {}", &sid.chars().take(8).collect::<String>()));
                let project = sess
                    .and_then(|s| {
                        if s.workspace_kind == Some(WorkspaceKind::Chat) {
                            Some("Chats".to_string())
                        } else {
                            s.project_name.clone().or_else(|| {
                                s.cwd
                                    .as_deref()
                                    .and_then(|c| PathBuf::from(c).file_name().map(|n| n.to_string_lossy().into_owned()))
                            })
                        }
                    })
                    .unwrap_or_else(|| "Workspace".to_string());

                let status = if !waiting_prompts.is_empty() {
                    "waiting".to_string()
                } else if running {
                    "running".to_string()
                } else {
                    "recent".to_string()
                };

                by_sid.insert(
                    sid.clone(),
                    TaskSnapshot {
                        session_id: sid,
                        title,
                        project,
                        status,
                        current_step: waiting_prompts.first().cloned().or(activity_str),
                        run_started_at,
                        waiting_prompts,
                        last_run_status,
                        updated_at: sess.and_then(|s| s.updated_at.clone()),
                    },
                );
            }
        }

        // Also ensure any session reported in running_session_ids shows up as running immediately
        for sid in &self.running_session_ids {
            if by_sid.contains_key(sid) {
                continue;
            }
            let sess = self.sessions.iter().find(|s| &s.id == sid);
            let title = sess
                .map(|s| s.display_title().to_string())
                .unwrap_or_else(|| format!("Session {}", &sid.chars().take(8).collect::<String>()));
            let project = sess
                .and_then(|s| s.project_name.clone())
                .unwrap_or_else(|| "Workspace".to_string());
            by_sid.insert(
                sid.clone(),
                TaskSnapshot {
                    session_id: sid.clone(),
                    title,
                    project,
                    status: "running".to_string(),
                    current_step: Some("Running".to_string()),
                    run_started_at: None,
                    waiting_prompts: Vec::new(),
                    last_run_status: None,
                    updated_at: sess.and_then(|s| s.updated_at.clone()),
                },
            );
        }

        let has_done = by_sid.values().any(|t| t.status == "recent");
        if has_done && self.active_page != WorkspacePage::Tasks {
            self.has_unread_completed_tasks = true;
        }
        self.tasks = by_sid.into_values().collect();
    }

    pub fn apply_mcp_response(&mut self, val: &Value) {
        if let Some(trusted) = val
            .get("project")
            .and_then(|p| p.get("trust"))
            .and_then(|t| t.get("trusted"))
            .and_then(Value::as_bool)
        {
            self.mcp_project_trusted = trusted;
        }
        if let Some(arr) = val.get("servers").and_then(Value::as_array) {
            let mut out = Vec::with_capacity(arr.len());
            for s in arr {
                let name = s.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let scope = s.get("scope").and_then(Value::as_str).unwrap_or("global").to_string();
                let transport = s
                    .get("transport")
                    .and_then(Value::as_str)
                    .unwrap_or("stdio")
                    .to_string();
                let enabled = s.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                let trusted = s
                    .get("trusted")
                    .and_then(Value::as_bool)
                    .unwrap_or(scope != "project" || self.mcp_project_trusted);
                let status = s
                    .get("status")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        if !enabled {
                            Some("disabled")
                        } else if !trusted {
                            Some("untrusted")
                        } else {
                            Some("configured")
                        }
                    })
                    .unwrap_or("configured")
                    .to_string();
                let tools_count = s.get("toolsCount").and_then(Value::as_u64).unwrap_or(0) as usize;
                let needs_auth = s.get("needsAuth").and_then(Value::as_bool).unwrap_or(false);
                let error = s.get("error").and_then(Value::as_str).map(String::from);
                out.push(McpServerRow {
                    name,
                    scope,
                    transport,
                    status,
                    enabled,
                    tools_count,
                    trusted,
                    needs_auth,
                    error,
                });
            }
            self.mcp_servers = out;
        }
    }

    fn apply_skills_response(&mut self, val: &Value) {
        if let Some(arr) = val.get("skills").and_then(Value::as_array) {
            let mut out = Vec::with_capacity(arr.len());
            for s in arr {
                let name = s.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let description = s
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let source = s
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("user")
                    .to_string();
                let file_path = s
                    .get("filePath")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let disable_model_invocation = s
                    .get("disableModelInvocation")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                out.push(SkillRow {
                    name,
                    description,
                    source,
                    file_path,
                    disable_model_invocation,
                    enabled: !disable_model_invocation,
                });
            }
            self.skills = out;
        }
    }

    pub fn apply_plugins_response(&mut self, val: &Value) {
        let mut out = Vec::new();
        if let Some(arr) = val.get("packages").and_then(Value::as_array) {
            for p in arr {
                let source = p.get("source").and_then(Value::as_str).unwrap_or("").to_string();
                let pkg_name = p
                    .get("packageName")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                let name = pkg_name.clone().unwrap_or_else(|| source.clone());
                if name.is_empty() {
                    continue;
                }
                let scope = p.get("scope").and_then(Value::as_str).unwrap_or("global").to_string();
                let version = p
                    .get("version")
                    .or_else(|| p.get("configuredVersion"))
                    .and_then(Value::as_str)
                    .unwrap_or("—")
                    .to_string();
                let description = p.get("description").and_then(Value::as_str).map(String::from);
                let disabled = p.get("disabled").and_then(Value::as_bool).unwrap_or(false);
                let status = p
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or(if disabled { "disabled" } else { "loaded" })
                    .to_string();
                let counts = p.get("counts");
                let ext = counts.and_then(|c| c.get("extensions")).and_then(Value::as_u64).unwrap_or(0);
                let skl = counts.and_then(|c| c.get("skills")).and_then(Value::as_u64).unwrap_or(0);
                let prm = counts.and_then(|c| c.get("prompts")).and_then(Value::as_u64).unwrap_or(0);
                let thm = counts.and_then(|c| c.get("themes")).and_then(Value::as_u64).unwrap_or(0);
                let mut parts = Vec::new();
                if ext > 0 { parts.push(format!("{ext} ext")); }
                if skl > 0 { parts.push(format!("{skl} skills")); }
                if prm > 0 { parts.push(format!("{prm} prompts")); }
                if thm > 0 { parts.push(format!("{thm} themes")); }
                out.push(PluginRow {
                    name,
                    source,
                    scope,
                    version,
                    description,
                    latest_version: None,
                    enabled: !disabled,
                    status,
                    resource_summary: parts.join(" · "),
                });
            }
        } else if let Some(arr) = val.get("plugins").and_then(Value::as_array) {
            out = arr
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
        }
        self.plugins = out;
    }

    fn apply_subagents_response(&mut self, val: &Value) {
        if let Some(arr) = val.get("profiles").and_then(Value::as_array) {
            let mut out = Vec::with_capacity(arr.len());
            for p in arr {
                let name = p.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let display_name = p.get("displayName").and_then(Value::as_str).map(String::from);
                let scope = p.get("scope").and_then(Value::as_str).unwrap_or("builtin").to_string();
                let built_in = scope == "builtin"
                    || p.get("builtIn").and_then(Value::as_bool).unwrap_or(false);
                let enabled = p.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                let model = p.get("model").and_then(Value::as_str).map(String::from);
                let description = p
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                out.push(SubagentProfileRow {
                    name,
                    display_name,
                    scope,
                    built_in,
                    enabled,
                    model,
                    description,
                });
            }
            self.subagents = out;
        }
    }

    pub fn apply_network_proxy_response(&mut self, val: &Value) {
        let settings = val.get("settings");
        let effective = val.get("effective");
        self.network_proxy = NetworkProxyState {
            enabled: settings
                .and_then(|s| s.get("enabled"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            url: settings
                .and_then(|s| s.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            no_proxy: settings
                .and_then(|s| s.get("noProxy"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            effective_url: effective
                .and_then(|e| e.get("url"))
                .and_then(Value::as_str)
                .map(String::from),
            effective_source: effective
                .and_then(|e| e.get("source"))
                .and_then(Value::as_str)
                .map(String::from),
            local: val.get("local").and_then(Value::as_bool).unwrap_or(true),
        };
    }

    fn apply_agent_state_response(&mut self, val: &Value) {
        let running = val.get("running").and_then(Value::as_bool).unwrap_or(false);
        let Some(st) = val.get("state") else {
            return;
        };
        if running {
            let streaming = st.get("isStreaming").and_then(Value::as_bool).unwrap_or(false)
                || st.get("isPromptRunning").and_then(Value::as_bool).unwrap_or(false);
            if !self.suppress_next_prompt_reload || !streaming {
                self.is_agent_running = streaming;
            }
            self.is_compacting = st.get("isCompacting").and_then(Value::as_bool).unwrap_or(false);
        }
        if let Some(ac) = st.get("autoCompactionEnabled").and_then(Value::as_bool) {
            self.auto_compaction = ac;
        }
        if let Some(ar) = st.get("autoRetryEnabled").and_then(Value::as_bool) {
            self.auto_retry = ar;
        }
        if let Some(tl) = st.get("thinkingLevel").and_then(Value::as_str) {
            if let Some(parsed) = parse_thinking_level_str(tl) {
                self.thinking_level = parsed;
            }
        }
        if let Some(m_obj) = st.get("model") {
            let prov = m_obj.get("provider").and_then(Value::as_str);
            let mid = m_obj.get("id").and_then(Value::as_str);
            if let (Some(p), Some(id)) = (prov, mid) {
                if let Some(found) = self.models.iter().find(|m| m.provider == p && m.id == id) {
                    self.selected_model = Some(found.clone());
                }
            }
        }
        if running {
            if let Some(streaming_msg) = st.get("streamingMessage") {
                if !streaming_msg.is_null() {
                    self.seed_streaming_snapshot(streaming_msg);
                }
            }
        }
        if let Some(cu) = st.get("contextUsage") {
            if !cu.is_null() {
                let win = cu.get("contextWindow").and_then(Value::as_u64);
                let tok = cu.get("tokens").and_then(Value::as_u64);
                let pct = cu.get("percent").and_then(Value::as_f64).map(|p| p as f32);
                self.stats.context_window = win;
                self.stats.context_tokens = tok;
                self.stats.context_used_pct = pct.unwrap_or(0.0);
                self.stats.has_context_usage = true;
            }
        }
        if let Some(widgets) = st.get("extensionWidgets").and_then(Value::as_array) {
            for w in widgets {
                let key = w
                    .get("key")
                    .or_else(|| w.get("widgetKey"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let lines: Vec<String> = w
                    .get("lines")
                    .or_else(|| w.get("widgetLines"))
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                if key == "subagent-async" {
                    if let Some(runs) = parse_subagent_async_json_widget(&lines) {
                        self.subagent_runs = runs;
                    }
                } else if key.contains("todo") {
                    let parsed = parse_todo_widget_lines(&lines);
                    if !self.todos_from_tool && (!parsed.is_empty() || lines.is_empty()) {
                        self.set_todos(parsed);
                    }
                }
            }
        }
        if let Some(statuses) = st.get("extensionStatuses").and_then(Value::as_array) {
            for s in statuses {
                let key = s.get("key").and_then(Value::as_str).unwrap_or("");
                if key.starts_with("subagent") {
                    self.subagent_status_text = s.get("text").and_then(Value::as_str).map(String::from);
                }
            }
        }
    }

    fn seed_streaming_snapshot(&mut self, msg_val: &Value) {
        let role = msg_val.get("role").and_then(Value::as_str).unwrap_or("");
        if role != "assistant" {
            return;
        }
        let (text, thinking, tool_calls, steps) = parse_message_content(msg_val.get("content"));
        let model_label = msg_val
            .get("model")
            .or_else(|| msg_val.get("modelId"))
            .and_then(Value::as_str)
            .map(|s| SharedString::from(s.to_string()))
            .or_else(|| self.selected_model.as_ref().map(ModelInfo::display_label));

        if let Some(last) = self.messages.last_mut() {
            if last.role == MessageRole::Assistant && last.is_streaming {
                if !text.is_empty() {
                    last.content = text;
                }
                if !thinking.is_empty() {
                    last.thinking = SharedString::from(thinking);
                }
                if !steps.is_empty() {
                    last.process_steps = steps;
                }
                if !tool_calls.is_empty() {
                    last.tool_calls = tool_calls;
                }
                return;
            }
        }
        let id = self.controller.alloc_msg_id();
        self.messages.push(ChatMessage {
            id,
            entry_id: None,
            role: MessageRole::Assistant,
            model_label,
            timestamp: SharedString::from("streaming"),
            thinking: SharedString::from(thinking),
            process_steps: steps,
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: text,
            tool_calls,
            attachments: Vec::new(),
            is_streaming: true,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: None,
            stop_reason: None,
            error_message: None,
        });
    }

    pub fn subscribe_running_events(&mut self, cx: &mut Context<Self>) {
        if self.running_events_subscribed {
            return;
        }
        self.running_events_subscribed = true;
        let entity = cx.entity().clone();

        cx.spawn(async move |_, cx| {
            loop {
                let mut conn_info: Option<(reqwest::Client, String, String, String)> = None;
                let mut is_connected = false;
                let mut alive = false;
                let _ = entity.update(cx, |this, _| {
                    alive = true;
                    is_connected = this.sidecar_connected;
                    if is_connected {
                        conn_info = Some((
                            this.client.raw_http(),
                            this.client.running_events_url(),
                            this.client.token.clone(),
                            this.client.instance_id.clone(),
                        ));
                    }
                });
                if !alive {
                    break;
                }
                if !is_connected {
                    smol::Timer::after(std::time::Duration::from_millis(600)).await;
                    continue;
                }

                let Some((http, url, token, instance_id)) = conn_info else {
                    smol::Timer::after(std::time::Duration::from_millis(600)).await;
                    continue;
                };

                let res = crate::client::sse::consume_json_sse(
                    http,
                    url,
                    token,
                    instance_id,
                    |val| {
                        let ids_opt = val
                            .get("runningSessionIds")
                            .or_else(|| val.get("running"))
                            .and_then(Value::as_array);
                        if let Some(ids) = ids_opt {
                            let running_set: HashSet<String> = ids
                                .iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect();
                            let _ = entity.update(cx, |this, cx| {
                                let unknown_running = running_set
                                    .iter()
                                    .any(|rid| !this.sessions.iter().any(|s| &s.id == rid));
                                this.apply_running_ids(running_set);
                                // The running list is the server's truth for the active
                                // session: it both starts and *ends* a run.
                                if let Some(sid) = this.active_session_id.clone() {
                                    let running = this.running_session_ids.contains(&sid);
                                    if !this.suppress_next_prompt_reload || !running {
                                        this.is_agent_running = running;
                                    }
                                    if running {
                                        if this.controller.sse_task.is_none() {
                                            this.connect_session_sse(sid.clone(), cx);
                                        }
                                    } else {
                                        this.is_compacting = false;
                                        this.retry_status = None;
                                    }
                                }
                                if unknown_running {
                                    let c2 = this.client.clone();
                                    let e2 = cx.entity().clone();
                                    cx.spawn(async move |_, cx| {
                                        if let Ok(sv) = c2.list_sessions(false).await {
                                            if let Some(arr) =
                                                sv.get("sessions").and_then(Value::as_array)
                                            {
                                                let parsed: Vec<SessionSummary> = arr
                                                    .iter()
                                                    .filter_map(|v| {
                                                        serde_json::from_value(v.clone()).ok()
                                                    })
                                                    .collect();
                                                if !parsed.is_empty() {
                                                    let _ = e2.update(cx, |s, cx| {
                                                        s.sessions = parsed;
                                                        cx.notify();
                                                    });
                                                }
                                            }
                                        }
                                    })
                                    .detach();
                                }
                                this.refresh_tasks_from_sidecar(cx);
                                cx.notify();
                            });
                        }
                        true
                    },
                )
                .await;

                let delay_ms = if res.is_err() { 1000 } else { 500 };
                smol::Timer::after(std::time::Duration::from_millis(delay_ms)).await;
            }
            let _ = entity.update(cx, |this, _| {
                this.running_events_subscribed = false;
            });
        })
        .detach();
    }

    pub fn start_new_composer(&mut self, workspace_kind: WorkspaceKind, cx: &mut Context<Self>) {
        self.controller.next_load_id();
        self.controller.stop_sse();
        self.active_session_id = None;
        self.active_workspace_kind = workspace_kind;
        self.active_is_temp_chat = false;
        if workspace_kind == WorkspaceKind::Chat {
            self.active_cwd = crate::client::local_store::standalone_chat_workspace_dir();
        } else if crate::client::local_store::is_chat_workspace_cwd(&self.active_cwd) {
            self.active_cwd = self
                .sessions
                .iter()
                .find(|s| s.workspace_kind != Some(WorkspaceKind::Chat))
                .and_then(|s| s.cwd.as_deref())
                .map(PathBuf::from)
                .or_else(|| self.extra_projects.first().cloned())
                .unwrap_or_else(user_home);
        }
        self.active_branch = if workspace_kind == WorkspaceKind::Chat {
            None
        } else {
            resolve_git_branch(&self.active_cwd)
        };
        if self.active_branch.is_none() && self.right_panel_tab == RightPanelTab::GitGraph {
            self.close_right_tab(RightPanelTab::GitGraph, cx);
        }
        self.active_page = WorkspacePage::Chat;
        self.messages.clear();
        self.branch_siblings.clear();
        self.is_agent_running = false;
        self.is_compacting = false;
        self.retry_status = None;
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
        self.pending_ui_request = None;
        self.dialog_queue.clear();
        self.questionnaire_request_id = None;
        self.todos.clear();
        self.todos_expanded = false;
        self.todos_hidden = false;
        self.todos_hidden_session = None;
        self.todos_from_tool = false;
        self.dismissed_todo_ids.clear();
        
        
        self.stats = SessionStats::default();
        self.has_earlier_messages = false;
        self.is_loading_earlier = false;
        self.subagent_runs.clear();
        self.tool_subagent_runs.clear();
        self.subagent_status_text = None;
        cx.notify();
    }

    /// Starts an ephemeral temporary chat backed by the sidecar's `/api/chat-workspace`
    /// (`~/.pi/agent/chats/<stamp>-<hex>/`), which is automatically released on expiration
    /// or explicit deletion.
    pub fn start_new_temp_chat(&mut self, cx: &mut Context<Self>) {
        self.start_new_composer(WorkspaceKind::Chat, cx);
        self.active_is_temp_chat = true;
        self.active_cwd = crate::client::local_store::temp_chat_root_dir();
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                if let Ok(cwd) = client.get_chat_workspace().await {
                    let _ = entity.update(cx, |this, cx| {
                        if this.active_session_id.is_none() && this.active_is_temp_chat {
                            this.active_cwd = cwd;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        cx.notify();
    }

    pub fn promote_temp_chat(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.temp_chat_ids.remove(&session_id);
        crate::client::local_store::unregister_temp_chat(&session_id);
        if self.active_session_id.as_deref() == Some(session_id.as_str()) {
            self.active_is_temp_chat = false;
        }
        cx.notify();
    }

    pub fn release_temp_chat(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.delete_session_by_id(session_id, cx);
    }

    pub fn is_session_temp_chat(&self, sid: &str) -> bool {
        self.temp_chat_ids.contains(sid)
            || self.sessions.iter().any(|s| {
                s.id == sid
                    && s.cwd.as_deref().is_some_and(|c| {
                        crate::client::local_store::is_temp_chat_folder(Path::new(c))
                    })
            })
    }

    pub fn is_active_temp_chat(&self) -> bool {
        self.active_is_temp_chat
            || crate::client::local_store::is_temp_chat_folder(&self.active_cwd)
            || self
                .active_session_id
                .as_deref()
                .is_some_and(|sid| self.is_session_temp_chat(sid))
    }

    pub fn cleanup_expired_temp_chats(&mut self, cx: &mut Context<Self>) {
        let now_ms = crate::client::local_store::now_unix_ms();
        let expired = crate::client::local_store::collect_expired_temp_chat_ids(now_ms);
        if expired.is_empty() {
            return;
        }
        for id in expired {
            if self.running_session_ids.contains(&id) {
                continue;
            }
            self.delete_session_by_id(id, cx);
        }
    }

    /// Monotonic request_id guarded session selection (per AGENTS.md hard rule).


    pub fn set_artifacts_page_index(&mut self, page: usize, cx: &mut Context<Self>) {
        self.artifacts_page_index = page;
        cx.notify();
    }

    pub fn set_artifacts_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.artifacts_search_query = query;
        self.artifacts_page_index = 0;
        cx.notify();
    }

    pub fn set_artifacts_type_filter(&mut self, filter: String, cx: &mut Context<Self>) {
        self.artifacts_type_filter = filter;
        self.artifacts_page_index = 0;
        cx.notify();
    }

    pub fn load_earlier_messages(&mut self, cx: &mut Context<Self>) {
        if self.is_loading_earlier || !self.has_earlier_messages {
            return;
        }
        let Some(sid) = self.active_session_id.clone() else {
            return;
        };
        let earliest_entry_id = self.messages.first().and_then(|m| m.entry_id.as_ref()).map(|s| s.to_string());
        let Some(before_id) = earliest_entry_id else {
            self.has_earlier_messages = false;
            return;
        };
        self.is_loading_earlier = true;
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let path = format!("/api/sessions/{sid}/context?before={before_id}&tail=50&wholeTurns=1");
            let res = client.get_json_path(&path).await;
            let _ = entity.update(cx, |this, cx| {
                this.is_loading_earlier = false;
                if let Ok(val) = res {
                    let ctx = val.get("context").unwrap_or(&val);
                    this.has_earlier_messages = ctx.get("hasEarlierMessages").and_then(Value::as_bool).unwrap_or(false);
                    let older_msgs = parse_session_messages(ctx, &mut this.controller.next_local_msg_id);
                    if !older_msgs.is_empty() {
                        let mut all = older_msgs;
                        all.extend(std::mem::take(&mut this.messages));
                        this.messages = all;
                    }
                } else {
                    this.has_earlier_messages = false;
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    pub fn select_session(&mut self, session_id: String, cx: &mut Context<Self>) {
        if self.active_session_id.as_deref() == Some(session_id.as_str()) && !self.messages.is_empty() {
            if self.active_page != WorkspacePage::Chat {
                self.active_page = WorkspacePage::Chat;
                cx.notify();
            }
            return;
        }

        let req_id = self.controller.next_load_id();
        self.controller.stop_sse();
        self.active_session_id = Some(session_id.clone());
        self.unread_session_ids.remove(&session_id);
        self.active_page = WorkspacePage::Chat;
        self.pending_ui_request = None;
        self.dialog_queue.clear();
        self.questionnaire_request_id = None;
        if !self.suppress_next_prompt_reload {
            self.is_agent_running = self.running_session_ids.contains(&session_id);
        }
        self.is_compacting = false;
        self.retry_status = None;
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
        self.subagent_runs.clear();
        self.tool_subagent_runs.clear();
        self.subagent_status_text = None;
        self.todos.clear();
        self.todos_expanded = false;
        self.todos_from_tool = false;
        if self.todos_hidden_session.as_deref() != Some(session_id.as_str()) {
            self.todos_hidden = false;
            self.todos_hidden_session = None;
            self.dismissed_todo_ids.clear();
        }
        self.stats = SessionStats::default();
        self.has_earlier_messages = false;
        self.is_loading_earlier = false;

        let mut local_jsonl_path: Option<PathBuf> = None;
        if let Some(summary) = self.sessions.iter().find(|s| s.id == session_id) {
            if let Some(cwd) = &summary.cwd {
                self.active_cwd = PathBuf::from(cwd);
            }
            if let Some(p) = &summary.path {
                local_jsonl_path = Some(PathBuf::from(p));
            }
            self.active_workspace_kind = summary.workspace_kind.unwrap_or(WorkspaceKind::Project);
        }
        self.active_is_temp_chat = self.temp_chat_ids.contains(&session_id);
        self.active_branch = if self.active_workspace_kind == WorkspaceKind::Chat {
            None
        } else {
            resolve_git_branch(&self.active_cwd)
        };
        if self.active_branch.is_none() && self.right_panel_tab == RightPanelTab::GitGraph {
            self.close_right_tab(RightPanelTab::GitGraph, cx);
        }

        let ctx_win = self
            .selected_model
            .as_ref()
            .and_then(|m| m.context_window)
            .unwrap_or(200_000);

        self.messages.clear();
        self.branch_siblings.clear();
        // Fast local JSONL load so switching sessions is instant and works offline
        if let Some(ref p) = local_jsonl_path {
            let (parsed, siblings) =
                parse_session_jsonl_with_siblings(p, None, &mut self.controller.next_local_msg_id);
            if !parsed.is_empty() {
                self.messages = parsed;
            }
            self.branch_siblings = siblings;
            self.stats = compute_stats_from_jsonl(p, ctx_win);
            let local_todos = extract_todos_from_jsonl(p);
            if !local_todos.is_empty() {
                self.todos_from_tool = true;
                self.set_todos(local_todos);
            }
            self.sync_tool_subagent_runs_from_messages();

            let (extracted_model, extracted_thinking) = extract_model_and_thinking_from_jsonl(p);
            if let Some((prov, mid)) = extracted_model {
                if let Some(found) = self.models.iter().find(|m| m.provider == prov && m.id == mid).cloned() {
                    self.selected_model = Some(found);
                }
            }
            if let Some(tl) = extracted_thinking {
                self.thinking_level = tl;
            }
            self.clamp_thinking_level_to_model();

            let sid_ref = Some(session_id.as_str());
            for msg in &self.messages {
                for art in extract_artifacts_from_markdown(&msg.content, sid_ref) {
                    if let Some(existing) = self.artifacts.iter_mut().find(|a| a.identifier == art.identifier) {
                        existing.version += 1;
                        existing.previous_content = Some(existing.content.clone());
                        existing.content = art.content;
                    } else {
                        self.artifacts.push(art);
                    }
                }
            }
        }
        cx.notify();

        if self.sidecar_connected {
            self.connect_session_sse(session_id.clone(), cx);
        }

        let client = self.client.clone();
        let entity = cx.entity().clone();
        let task = cx.spawn(async move |_, cx| {
            let result = client.get_session(&session_id).await;
            let agent_state = client.get_agent_state(&session_id).await.ok();
            let _ = entity.update(cx, |this, cx| {
                // Guard against stale async completion!
                if this.controller.load_request_id != req_id {
                    return;
                }
                if let Ok(payload) = result {
                    let ctx_obj = payload.get("context").unwrap_or(&payload);
                    this.has_earlier_messages = ctx_obj.get("hasEarlierMessages").and_then(Value::as_bool).unwrap_or(false);
                    let parsed =
                        parse_session_messages(&payload, &mut this.controller.next_local_msg_id);
                    if !parsed.is_empty()
                        && (this.messages.is_empty()
                            || this.is_agent_running
                            || parsed.len() > this.messages.len())
                    {
                        let same_messages = parsed.len() == this.messages.len()
                            && parsed.iter().zip(this.messages.iter()).all(|(a, b)| {
                                a.role == b.role
                                    && a.content == b.content
                                    && a.process_steps.len() == b.process_steps.len()
                                    && a.tool_calls.len() == b.tool_calls.len()
                            });
                        if !same_messages {
                            let mut merged = parsed;
                            for (idx, m) in merged.iter_mut().enumerate() {
                                if let Some(prev) = this.messages.get(idx) {
                                    if prev.role == m.role
                                        && (prev.entry_id == m.entry_id || prev.content == m.content)
                                    {
                                        m.id = prev.id;
                                    }
                                }
                            }
                            this.messages = merged;
                        }
                    }
                    if let Some(payload_todos) = extract_todos_from_payload(&payload) {
                        this.todos_from_tool = true;
                        this.set_todos(payload_todos);
                    }
                    this.sync_tool_subagent_runs_from_messages();
                    if let Some(stats_obj) = payload.get("stats") {
                        if let Some(tok) = stats_obj.get("tokens") {
                            this.stats.input_tokens =
                                tok.get("input").and_then(Value::as_u64).unwrap_or(0);
                            this.stats.output_tokens =
                                tok.get("output").and_then(Value::as_u64).unwrap_or(0);
                            this.stats.cache_read_tokens =
                                tok.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                            this.stats.cache_write_tokens =
                                tok.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                        }
                        if let Some(cost) = stats_obj.get("cost").and_then(Value::as_f64) {
                            this.stats.cost_usd = cost;
                        }
                    }
                    if let Some(ctx_tok) = compute_context_tokens_from_payload(&payload) {
                        let window = this
                            .stats
                            .context_window
                            .or_else(|| this.selected_model.as_ref().and_then(|m| m.context_window))
                            .unwrap_or(200_000)
                            .max(1);
                        this.stats.context_tokens = Some(ctx_tok);
                        this.stats.context_window = Some(window);
                        this.stats.context_used_pct =
                            ((ctx_tok as f64 / window as f64) * 100.0).clamp(0.0, 100.0) as f32;
                        this.stats.has_context_usage = true;
                    }
                    // Merge any newly extracted inline artifacts into the cross-session library
                    // without wiping historical artifacts from other sessions.
                    for msg in &this.messages {
                        for art in extract_artifacts_from_markdown(&msg.content, Some(&session_id)) {
                            if let Some(existing) = this
                                .artifacts
                                .iter_mut()
                                .find(|a| a.identifier == art.identifier && a.session_id.as_deref() == Some(session_id.as_str()))
                            {
                                if existing.content != art.content {
                                    existing.previous_content = Some(existing.content.clone());
                                    existing.content = art.content;
                                    existing.version += 1;
                                }
                            } else {
                                this.artifacts.push(art);
                            }
                        }
                    }
                    if this.active_artifact_id.is_none() {
                        this.active_artifact_id = this
                            .artifacts
                            .iter()
                            .find(|a| a.session_id.as_deref() == Some(session_id.as_str()))
                            .or_else(|| this.artifacts.first())
                            .map(|a| a.identifier.clone());
                    }
                    if this.controller.sse_task.is_none() {
                        this.connect_session_sse(session_id, cx);
                    }
                }
                if let Some(st_val) = agent_state {
                    this.apply_agent_state_response(&st_val);
                }
                cx.notify();
            });
        });

        self.controller.load_task = Some(task);
        cx.notify();
    }

    pub fn connect_session_sse(&mut self, session_id: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            return;
        }
        let entity = cx.entity().clone();

        let task = cx.spawn(async move |_, cx| {
            let sid = session_id.clone();
            loop {
                let mut conn_info: Option<(reqwest::Client, String, String, String)> = None;
                let mut still_active = false;
                let mut is_connected = false;
                let _ = entity.update(cx, |this, _| {
                    still_active = this.active_session_id.as_deref() == Some(sid.as_str());
                    is_connected = this.sidecar_connected;
                    if still_active && is_connected {
                        conn_info = Some((
                            this.client.raw_http(),
                            this.client.agent_events_url(&sid),
                            this.client.token.clone(),
                            this.client.instance_id.clone(),
                        ));
                    }
                });

                if !still_active {
                    break;
                }
                if !is_connected {
                    // Sidecar is momentarily reconnecting; wait seamlessly without exiting SSE task
                    smol::Timer::after(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                let Some((http, url, token, instance_id)) = conn_info else {
                    smol::Timer::after(std::time::Duration::from_millis(500)).await;
                    continue;
                };

                let res = consume_agent_sse(
                    http,
                    url,
                    token,
                    instance_id,
                    |event| {
                        let mut keep_going = true;
                        let _ = entity.update(cx, |this, cx| {
                            if this.active_session_id.as_deref() != Some(sid.as_str()) {
                                keep_going = false;
                                return;
                            }
                            this.apply_sse_event(event, cx);
                        });
                        keep_going
                    },
                )
                .await;

                let mut still_same_session = false;
                let _ = entity.update(cx, |this, _| {
                    still_same_session = this.active_session_id.as_deref() == Some(sid.as_str());
                });
                if !still_same_session {
                    break;
                }
                let delay_ms = if res.is_err() { 800 } else { 400 };
                smol::Timer::after(std::time::Duration::from_millis(delay_ms)).await;
            }
            let _ = entity.update(cx, |this, _| {
                if this.active_session_id.as_deref() != Some(sid.as_str()) {
                    return;
                }
                this.controller.sse_task = None;
            });
        });
        self.controller.sse_task = Some(task);
    }

    fn apply_sse_event(&mut self, event: AgentSseEvent, cx: &mut Context<Self>) {
        match event {
            AgentSseEvent::Connected { is_streaming } => {
                if is_streaming && !self.suppress_next_prompt_reload {
                    self.is_agent_running = true;
                    if let Some(sid) = self.active_session_id.clone() {
                        self.sync_active_agent_state(sid, cx);
                    }
                }
            }
            AgentSseEvent::AgentStart | AgentSseEvent::TurnStart => {
                if !self.suppress_next_prompt_reload {
                    self.is_agent_running = true;
                }
            }
            AgentSseEvent::AgentEnd { duration_ms }
            | AgentSseEvent::TurnEnd { duration_ms, .. } => {
                if self.suppress_next_prompt_reload {
                    return;
                }
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.is_streaming = false;
                        if last.timestamp.as_ref() == "streaming" || last.timestamp.is_empty() {
                            last.timestamp = current_local_hhmm();
                        }
                        if let Some(ms) = duration_ms {
                            last.turn_duration_ms = Some(ms);
                        }
                        let extracted = extract_artifacts_from_markdown(
                            &last.content,
                            self.active_session_id.as_deref(),
                        );
                        if !extracted.is_empty() {
                            for art in extracted {
                                if let Some(existing) = self
                                    .artifacts
                                    .iter_mut()
                                    .find(|a| a.identifier == art.identifier)
                                {
                                    existing.version += 1;
                                    existing.previous_content =
                                        Some(std::mem::replace(&mut existing.content, art.content));
                                    existing.title = art.title;
                                } else {
                                    self.active_artifact_id = Some(art.identifier.clone());
                                    self.artifacts.push(art);
                                }
                            }
                            self.open_right_tab(RightPanelTab::Artifact, cx);
                        }
                    }
                }
            }
            AgentSseEvent::PromptDone | AgentSseEvent::AgentSettled => {
                if self.suppress_next_prompt_reload {
                    self.suppress_next_prompt_reload = false;
                    self.is_agent_running = false;
                    self.is_compacting = false;
                    self.retry_status = None;
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
                    cx.notify();
                    return;
                }
                let was_running = self.is_agent_running;
                self.is_agent_running = false;
                self.is_compacting = false;
                self.retry_status = None;
                self.continuing_msg_id = None;
                self.continuing_prefix_text.clear();
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.is_streaming = false;
                        if last.timestamp.as_ref() == "streaming" || last.timestamp.is_empty() {
                            last.timestamp = current_local_hhmm();
                        }
                    }
                }
                        if was_running {
                    let title = crate::i18n::tr(self.locale, "notify.taskCompleteTitle");
                    let body = crate::i18n::tr(self.locale, "notify.taskCompleteBody");
                    self.trigger_desktop_notification(title, body);
                }
                if let Some(sid) = self.active_session_id.clone() {
                    let client = self.client.clone();
                    let entity = cx.entity().clone();
                    cx.spawn(async move |_, cx| {
                        let session_res = client.get_session(&sid).await;
                        let state_res = client.get_agent_state(&sid).await.ok();
                        let _ = entity.update(cx, |this, cx| {
                            if this.active_session_id.as_deref() == Some(sid.as_str()) {
                                if let Ok(payload) = session_res {
                                    if let Some(path_str) = this
                                        .sessions
                                        .iter()
                                        .find(|s| s.id == sid)
                                        .and_then(|s| s.path.as_deref())
                                    {
                                        let p = PathBuf::from(path_str);
                                        let (_, siblings) = parse_session_jsonl_with_siblings(
                                            &p,
                                            None,
                                            &mut this.controller.next_local_msg_id,
                                        );
                                        this.branch_siblings = siblings;
                                    }
                                    let parsed = parse_session_messages(
                                        &payload,
                                        &mut this.controller.next_local_msg_id,
                                    );
                                    if !parsed.is_empty() {
                                        let mut merged = parsed;
                                        for (idx, m) in merged.iter_mut().enumerate() {
                                            if let Some(prev) = this.messages.get(idx) {
                                                if prev.role == m.role
                                                    && (prev.entry_id == m.entry_id
                                                        || prev.content == m.content)
                                                {
                                                    m.id = prev.id;
                                                }
                                            }
                                        }
                                        this.messages = merged;
                                    }
                                    if let Some(stats_obj) = payload.get("stats") {
                                        if let Some(tok) = stats_obj.get("tokens") {
                                            this.stats.input_tokens = tok.get("input").and_then(Value::as_u64).unwrap_or(0);
                                            this.stats.output_tokens = tok.get("output").and_then(Value::as_u64).unwrap_or(0);
                                            this.stats.cache_read_tokens = tok.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                                            this.stats.cache_write_tokens = tok.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                                        }
                                        if let Some(cost) = stats_obj.get("cost").and_then(Value::as_f64) {
                                            this.stats.cost_usd = cost;
                                        }
                                    }
                                    if let Some(ctx_tok) = compute_context_tokens_from_payload(&payload) {
                                        let window = this
                                            .stats
                                            .context_window
                                            .or_else(|| this.selected_model.as_ref().and_then(|m| m.context_window))
                                            .unwrap_or(200_000)
                                            .max(1);
                                        this.stats.context_tokens = Some(ctx_tok);
                                        this.stats.context_window = Some(window);
                                        this.stats.context_used_pct =
                                            ((ctx_tok as f64 / window as f64) * 100.0).clamp(0.0, 100.0) as f32;
                                        this.stats.has_context_usage = true;
                                    }
                                }
                                if let Some(st_val) = state_res {
                                    this.apply_agent_state_response(&st_val);
                                }
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                }
                self.flush_next_outbox(cx);
            }
            AgentSseEvent::MessageStart { message } => {
                let role = message
                    .as_ref()
                    .and_then(|m| m.get("role"))
                    .and_then(Value::as_str)
                    .unwrap_or("assistant");
                if role == "user" {
                    if let Some(msg_val) = message.as_ref() {
                        let user_text = flatten_content_text(msg_val.get("content"));
                        if self.continuing_msg_id.is_some()
                            || user_text.trim()
                                == crate::client::session_controller::IMPLICIT_CONTINUE_PROMPT
                        {
                            return;
                        }
                        if !user_text.trim().is_empty()
                            && !user_text.trim_start().starts_with("/agy ")
                            && user_text.trim() != "/agy"
                        {
                            let already_present = self
                                .messages
                                .iter()
                                .rev()
                                .take(3)
                                .any(|m| {
                                    m.role == MessageRole::User
                                        && m.content.trim() == user_text.trim()
                                });
                            if !already_present {
                                let id = self.controller.alloc_msg_id();
                                self.messages.push(ChatMessage {
                                    id,
                                    entry_id: None,
                                    role: MessageRole::User,
                                    model_label: None,
                                    timestamp: current_local_hhmm(),
                                    thinking: SharedString::default(),
                                    process_steps: Vec::new(),
                                    process_message_count: 1,
                                    thinking_ms: None,
                                    thinking_expanded: false,
                                    content: user_text,
                                    tool_calls: Vec::new(),
                                    attachments: Vec::new(),
                                    is_streaming: false,
                                    turn_duration_ms: None,
                                    token_summary: None,
                                    turn_usage: None,
                                    stop_reason: None,
                                    error_message: None,
                                });
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                if role != "assistant" {
                    return;
                }

                let model_label = message
                    .as_ref()
                    .and_then(|m| m.get("model").or_else(|| m.get("modelId")))
                    .and_then(Value::as_str)
                    .map(|s| SharedString::from(s.to_string()))
                    .or_else(|| self.selected_model.as_ref().map(ModelInfo::display_label));

                let (snap_text, snap_thinking, snap_tools, snap_steps) = message
                    .as_ref()
                    .map(|m| parse_message_content(m.get("content")))
                    .unwrap_or_else(|| (String::new(), String::new(), Vec::new(), Vec::new()));

                if let Some(last) = self.messages.last_mut() {
                    let same_model = match (&last.model_label, &model_label) {
                        (Some(a), Some(b)) => a == b,
                        _ => true,
                    };
                    if last.role == MessageRole::Assistant && same_model {
                        let is_continuing = self.continuing_msg_id == Some(last.id);
                        let was_streaming = last.is_streaming;
                        if is_continuing {
                            last.error_message = None;
                            last.stop_reason = None;
                        } else if !last.content.trim().is_empty() && !was_streaming {
                            let intermediate = std::mem::take(&mut last.content);
                            last.process_steps
                                .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                        }
                        if !snap_text.is_empty() && !is_continuing {
                            last.content = snap_text;
                        }
                        if !snap_thinking.is_empty() {
                            last.thinking = SharedString::from(snap_thinking);
                        }
                        if !snap_steps.is_empty() && last.process_steps.is_empty() {
                            last.process_steps = snap_steps;
                        }
                        if !snap_tools.is_empty() && last.tool_calls.is_empty() {
                            last.tool_calls = snap_tools;
                        }
                        last.is_streaming = true;
                        if !was_streaming {
                            last.process_message_count += 1;
                        }
                        if last.model_label.is_none() {
                            last.model_label = model_label;
                        }
                        cx.notify();
                        return;
                    }
                }

                let id = self.controller.alloc_msg_id();
                self.messages.push(ChatMessage {
                    id,
                    entry_id: None,
                    role: MessageRole::Assistant,
                    model_label,
                    timestamp: SharedString::from("streaming"),
                    thinking: SharedString::from(snap_thinking),
                    process_steps: snap_steps,
                    process_message_count: 1,
                    thinking_ms: None,
                    thinking_expanded: false,
                    content: snap_text,
                    tool_calls: snap_tools,
                    attachments: Vec::new(),
                    is_streaming: true,
                    turn_duration_ms: None,
                    token_summary: None,
                    turn_usage: None,
                    stop_reason: None,
                    error_message: None,
                });
            }
            AgentSseEvent::MessageUpdate {
                assistant_message_event,
                delta,
                thinking_delta,
            } => {
                let ev_type = assistant_message_event
                    .as_ref()
                    .and_then(|ev| ev.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("");

                if self.messages.last().map(|m| m.role != MessageRole::Assistant).unwrap_or(true) {
                    let id = self.controller.alloc_msg_id();
                    let model_label = self.selected_model.as_ref().map(ModelInfo::display_label);
                    self.messages.push(ChatMessage {
                        id,
                        entry_id: None,
                        role: MessageRole::Assistant,
                        model_label,
                        timestamp: SharedString::from("streaming"),
                        thinking: SharedString::default(),
                        process_steps: Vec::new(),
                        process_message_count: 1,
                        thinking_ms: None,
                        thinking_expanded: false,
                        content: String::new(),
                        tool_calls: Vec::new(),
                        attachments: Vec::new(),
                        is_streaming: true,
                        turn_duration_ms: None,
                        token_summary: None,
                        turn_usage: None,
                        stop_reason: None,
                        error_message: None,
                    });
                }

                if let Some(last) = self.messages.last_mut() {
                    last.is_streaming = true;
                    if matches!(ev_type, "start" | "thinking_start" | "text_start") {
                        if let Some(prev_err) = last.error_message.take() {
                            last.process_steps.push(ProcessStep::Thinking(
                                SharedString::from(format!("⚠️ {prev_err}")),
                            ));
                            last.stop_reason = None;
                        }
                    }
                    match ev_type {
                        "thinking_start" => {
                            last.process_steps
                                .push(ProcessStep::Thinking(SharedString::default()));
                        }
                        "thinking_delta" => {
                            if let Some(td) = assistant_message_event
                                .as_ref()
                                .and_then(|ev| ev.get("delta"))
                                .and_then(Value::as_str)
                            {
                                let mut s = last.thinking.to_string();
                                s.push_str(td);
                                last.thinking = SharedString::from(s);
                                if let Some(ProcessStep::Thinking(prev_t)) =
                                    last.process_steps.last_mut()
                                {
                                    let mut next_t = prev_t.to_string();
                                    next_t.push_str(td);
                                    *prev_t = SharedString::from(next_t);
                                } else {
                                    last.process_steps
                                        .push(ProcessStep::Thinking(SharedString::from(
                                            td.to_string(),
                                        )));
                                }
                            }
                        }
                        "thinking_end" => {
                            if let Some(full) = assistant_message_event
                                .as_ref()
                                .and_then(|ev| ev.get("content"))
                                .and_then(Value::as_str)
                            {
                                if let Some(ProcessStep::Thinking(prev_t)) =
                                    last.process_steps.last_mut()
                                {
                                    *prev_t = SharedString::from(full.to_string());
                                }
                            }
                        }
                        "text_start" => {
                            if self.continuing_msg_id == Some(last.id) {
                                last.content = self.continuing_prefix_text.clone();
                            } else {
                                last.content.clear();
                            }
                        }
                        "text_delta" => {
                            if let Some(d) = assistant_message_event
                                .as_ref()
                                .and_then(|ev| ev.get("delta"))
                                .and_then(Value::as_str)
                            {
                                last.content.push_str(d);
                            }
                        }
                        "text_end" => {
                            if let Some(full) = assistant_message_event
                                .as_ref()
                                .and_then(|ev| ev.get("content"))
                                .and_then(Value::as_str)
                            {
                                if self.continuing_msg_id == Some(last.id) {
                                    last.content = format!("{}{}", self.continuing_prefix_text, full);
                                } else {
                                    last.content = full.to_string();
                                }
                            }
                        }
                        // Strictly keep toolcall_start / toolcall_delta / toolcall_end OUT of last.content!
                        "toolcall_start" | "toolcall_delta" | "toolcall_end" | "start" | "done" => {}
                        "error" => {
                            if let Some(err_str) = assistant_message_event
                                .as_ref()
                                .and_then(|ev| {
                                    ev.get("errorMessage")
                                        .or_else(|| ev.get("error"))
                                        .or_else(|| ev.get("message"))
                                })
                                .and_then(Value::as_str)
                            {
                                last.stop_reason = Some(SharedString::from("error"));
                                last.error_message = Some(SharedString::from(err_str.to_string()));
                            }
                        }
                        _ => {
                            // Fallback only when raw delta / thinking_delta fields are explicitly provided
                            if let Some(td) = thinking_delta {
                                let mut s = last.thinking.to_string();
                                s.push_str(&td);
                                last.thinking = SharedString::from(s);
                                if let Some(ProcessStep::Thinking(prev_t)) =
                                    last.process_steps.last_mut()
                                {
                                    let mut next_t = prev_t.to_string();
                                    next_t.push_str(&td);
                                    *prev_t = SharedString::from(next_t);
                                } else {
                                    last.process_steps
                                        .push(ProcessStep::Thinking(SharedString::from(td)));
                                }
                            } else if let Some(d) = delta {
                                last.content.push_str(&d);
                            }
                        }
                    }
                }
            }
            AgentSseEvent::MessageEnd { message } => {
                let role = message
                    .as_ref()
                    .and_then(|m| m.get("role"))
                    .and_then(Value::as_str)
                    .unwrap_or("assistant");
                if role == "toolResult" || role == "tool" {
                    if let Some(msg_val) = message.as_ref() {
                        let tool_call_id = msg_val
                            .get("toolCallId")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let mut resolved_tool_name = msg_val
                            .get("toolName")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let is_error = msg_val
                            .get("isError")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let output_text = flatten_content_text(msg_val.get("content"));
                        let details_val = msg_val.get("details");
                        let subagent_sid = extract_subagent_session_id(details_val);

                        if let Some(last) = self.messages.last_mut() {
                            for tc in last.tool_calls.iter_mut().rev() {
                                if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                                    if resolved_tool_name.is_empty() {
                                        resolved_tool_name = tc.name.to_string();
                                    }
                                    tc.is_running = false;
                                    tc.is_error = is_error;
                                    if !output_text.is_empty() {
                                        tc.output = SharedString::from(output_text.clone());
                                    }
                                    if subagent_sid.is_some() {
                                        tc.subagent_session_id = subagent_sid.clone();
                                    }
                                    break;
                                }
                            }
                            for step in last.process_steps.iter_mut().rev() {
                                if let ProcessStep::ToolCall(tc) = step {
                                    if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                                        if resolved_tool_name.is_empty() {
                                            resolved_tool_name = tc.name.to_string();
                                        }
                                        tc.is_running = false;
                                        tc.is_error = is_error;
                                        if !output_text.is_empty() {
                                            tc.output = SharedString::from(output_text.clone());
                                        }
                                        if subagent_sid.is_some() {
                                            tc.subagent_session_id = subagent_sid.clone();
                                        }
                                        break;
                                    }
                                }
                            }
                        }

                        if let Some(d) = details_val {
                            if d.get("kind").and_then(Value::as_str) == Some("pi-web-subagent") {
                                self.apply_pi_web_subagent_details(tool_call_id, d, is_error);
                            }
                        }

                        if !is_error
                            && (resolved_tool_name == "todo"
                                || details_val.and_then(|d| d.get("tasks")).is_some())
                        {
                            self.todos_from_tool = true;
                            let updated_from_details = details_val
                                .and_then(parse_todo_tool_details)
                                .map(|items| {
                                    if !items.is_empty() {
                                        self.todos_hidden = false;
                                        self.todos_hidden_session = None;
                                    }
                                    self.set_todos(items);
                                })
                                .is_some();
                            if !updated_from_details && !output_text.is_empty() {
                                if !output_text.starts_with("No tasks")
                                    && !output_text.starts_with("Cleared ")
                                {
                                    self.todos_hidden = false;
                                    self.todos_hidden_session = None;
                                }
                                apply_todo_tool_output_text(&mut self.todos, &output_text);
                                self.prune_dismissed_todos();
                            }
                            if let Some(sid) = self.active_session_id.clone() {
                                self.sync_active_agent_state(sid, cx);
                            }
                        }
                    }
                    cx.notify();
                    return;
                }
                if role == "custom" || role == "notice" {
                    if let Some(msg_val) = message.as_ref() {
                        let notice_text = flatten_content_text(msg_val.get("content"));
                        if !notice_text.trim().is_empty() {
                            let id = self.controller.alloc_msg_id();
                            self.messages.push(ChatMessage {
                                id,
                                entry_id: None,
                                role: MessageRole::Notice,
                                model_label: None,
                                timestamp: current_local_hhmm(),
                                thinking: SharedString::default(),
                                process_steps: Vec::new(),
                                process_message_count: 1,
                                thinking_ms: None,
                                thinking_expanded: false,
                                content: notice_text,
                                tool_calls: Vec::new(),
                                attachments: Vec::new(),
                                is_streaming: false,
                                turn_duration_ms: None,
                                token_summary: None,
                                turn_usage: None,
                                stop_reason: None,
                                error_message: None,
                            });
                            cx.notify();
                        }
                    }
                    return;
                }
                if role == "user" {
                    if let Some(msg_val) = message.as_ref() {
                        let user_text = flatten_content_text(msg_val.get("content"));
                        if self.continuing_msg_id.is_some()
                            || user_text.trim()
                                == crate::client::session_controller::IMPLICIT_CONTINUE_PROMPT
                        {
                            return;
                        }
                        if !user_text.trim().is_empty()
                            && !user_text.trim_start().starts_with("/agy ")
                            && user_text.trim() != "/agy"
                        {
                            let already_present = self
                                .messages
                                .iter()
                                .rev()
                                .take(4)
                                .any(|m| {
                                    m.role == MessageRole::User
                                        && m.content.trim() == user_text.trim()
                                });
                            if !already_present {
                                let id = self.controller.alloc_msg_id();
                                self.messages.push(ChatMessage {
                                    id,
                                    entry_id: None,
                                    role: MessageRole::User,
                                    model_label: None,
                                    timestamp: current_local_hhmm(),
                                    thinking: SharedString::default(),
                                    process_steps: Vec::new(),
                                    process_message_count: 1,
                                    thinking_ms: None,
                                    thinking_expanded: false,
                                    content: user_text,
                                    tool_calls: Vec::new(),
                                    attachments: Vec::new(),
                                    is_streaming: false,
                                    turn_duration_ms: None,
                                    token_summary: None,
                                    turn_usage: None,
                                    stop_reason: None,
                                    error_message: None,
                                });
                                cx.notify();
                            }
                        }
                    }
                    return;
                }
                if role != "assistant" {
                    return;
                }
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        // Parse authoritative final assistant message blocks if provided
                        if let Some(msg_val) = message.as_ref() {
                            let sr_str = msg_val.get("stopReason").and_then(Value::as_str);
                            if let Some(sr) = sr_str {
                                last.stop_reason = Some(SharedString::from(sr.to_string()));
                            }
                            if let Some(em) = msg_val
                                .get("errorMessage")
                                .or_else(|| msg_val.get("error"))
                                .and_then(Value::as_str)
                                .filter(|s| !s.trim().is_empty())
                            {
                                last.error_message = Some(SharedString::from(em.to_string()));
                            } else if sr_str != Some("error") {
                                if let Some(prev_err) = last.error_message.take() {
                                    last.process_steps.push(ProcessStep::Thinking(
                                        SharedString::from(format!("⚠️ {prev_err}")),
                                    ));
                                }
                            }
                            if let Some(turn_u) = extract_turn_usage(msg_val) {
                                merge_turn_usage(&mut last.turn_usage, turn_u);
                            }
                            if let Some(usage) = msg_val.get("usage") {
                                let inp = usage.get("input").and_then(Value::as_u64).unwrap_or(0);
                                let out = usage.get("output").and_then(Value::as_u64).unwrap_or(0);
                                let cr = usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                                let cw = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                                let turn_ctx = inp + out + cr + cw;
                                if turn_ctx > 0 {
                                    let win = self
                                        .stats
                                        .context_window
                                        .or_else(|| self.selected_model.as_ref().and_then(|m| m.context_window))
                                        .unwrap_or(200_000)
                                        .max(1);
                                    self.stats.context_tokens = Some(turn_ctx);
                                    self.stats.context_window = Some(win);
                                    self.stats.context_used_pct =
                                        ((turn_ctx as f64 / win as f64) * 100.0).clamp(0.0, 100.0) as f32;
                                    self.stats.has_context_usage = true;
                                }
                            }
                            let parsed = parse_session_messages(
                                &serde_json::json!({ "messages": [msg_val] }),
                                &mut self.controller.next_local_msg_id,
                            );
                            if let Some(completed) = parsed.into_iter().next() {
                                if !completed.content.is_empty() {
                                    if self.continuing_msg_id == Some(last.id) {
                                        last.content = format!("{}{}", self.continuing_prefix_text, completed.content);
                                        self.continuing_prefix_text = last.content.clone();
                                    } else {
                                        last.content = completed.content;
                                    }
                                }
                            }
                        }
                        if !self.is_compacting {
                            last.is_streaming = false;
                        }
                        if last.timestamp.as_ref() == "streaming" || last.timestamp.is_empty() {
                            last.timestamp = current_local_hhmm();
                        }
                    }
                }
            }
            AgentSseEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                if tool_name == "todo" {
                    self.todos_from_tool = true;
                    if let Some(ref a) = args {
                        let action = a.get("action").and_then(Value::as_str).unwrap_or("");
                        if matches!(action, "create" | "update" | "list") {
                            self.todos_hidden = false;
                            self.todos_hidden_session = None;
                        }
                        if let Some(id_val) = a.get("id") {
                            let id_str = match id_val {
                                Value::Number(n) => n.to_string(),
                                Value::String(s) => s.trim_start_matches('#').to_string(),
                                _ => String::new(),
                            };
                            if !id_str.is_empty() {
                                self.dismissed_todo_ids.remove(&id_str);
                            }
                        }
                        apply_optimistic_todo_args(&mut self.todos, a);
                        self.prune_dismissed_todos();
                    }
                }
                if matches!(tool_name.as_str(), "Agent" | "subagent" | "subagent_launch") {
                    let is_builtin_agent = tool_name == "Agent";
                    let agent_lbl = args
                        .as_ref()
                        .and_then(|a| {
                            if is_builtin_agent {
                                a.get("subagent_type").and_then(Value::as_str)
                            } else {
                                a.get("agent")
                                    .and_then(Value::as_str)
                                    .or_else(|| a.get("key").and_then(Value::as_str))
                            }
                        })
                        .unwrap_or(if is_builtin_agent { "general-purpose" } else { "subagent" })
                        .to_string();
                    let task_desc = args
                        .as_ref()
                        .and_then(|a| {
                            a.get("description")
                                .and_then(Value::as_str)
                                .or_else(|| a.get("task").and_then(Value::as_str))
                                .or_else(|| a.get("prompt").and_then(Value::as_str))
                        })
                        .map(|t| t.chars().take(80).collect::<String>());
                    let model = args
                        .as_ref()
                        .and_then(|a| a.get("model").and_then(Value::as_str))
                        .map(String::from);
                    let resume_sid = args
                        .as_ref()
                        .and_then(|a| a.get("resume").and_then(Value::as_str))
                        .filter(|s| !s.trim().is_empty())
                        .map(String::from);
                    self.tool_subagent_runs.retain(|r| r.id != tool_call_id);
                    self.tool_subagent_runs.push(SubagentRunInfo {
                        id: tool_call_id.clone(),
                        kind: "subagent".to_string(),
                        label: agent_lbl,
                        state: "running".to_string(),
                        started_at: None,
                        ended_at: None,
                        model,
                        thinking: None,
                        current_tool: None,
                        turn_count: None,
                        tool_count: None,
                        progress_text: task_desc,
                        session_id: resume_sid,
                        children: Vec::new(),
                    });
                }
                if let Some(last) = self.messages.last_mut() {
                    if !last.content.trim().is_empty() {
                        let intermediate = std::mem::take(&mut last.content);
                        last.process_steps
                            .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                    }
                    let arguments = flatten_tool_args(args.as_ref());
                    let diff_patch = build_diff_from_tool(&tool_name, args.as_ref(), None);
                    let tc = ToolCallItem {
                        id: SharedString::from(tool_call_id),
                        name: SharedString::from(tool_name),
                        arguments,
                        output: SharedString::default(),
                        diff_patch,
                        subagent_session_id: None,
                        is_running: true,
                        is_error: false,
                        expanded: false,
                    };
                    last.process_steps.push(ProcessStep::ToolCall(tc.clone()));
                    last.tool_calls.push(tc);
                }
            }
            AgentSseEvent::ToolExecutionUpdate {
                tool_call_id,
                partial_result,
            } => {
                let details_val = partial_result.as_ref().and_then(|v| v.get("details"));
                let subagent_sid = extract_subagent_session_id(details_val);
                if let Some(d) = details_val {
                    if d.get("kind").and_then(Value::as_str) == Some("pi-web-subagent") {
                        self.apply_pi_web_subagent_details(&tool_call_id, d, false);
                    }
                }
                if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
                    if let Some(details) = details_val {
                        if let Some(first_res) = details
                            .get("results")
                            .and_then(Value::as_array)
                            .and_then(|a| a.first())
                        {
                            if let Some(prog) = first_res.get("progress") {
                                run.current_tool = prog
                                    .get("currentTool")
                                    .and_then(Value::as_str)
                                    .map(String::from);
                                run.tool_count = prog.get("toolCount").and_then(Value::as_u64);
                                run.turn_count = prog.get("turnCount").and_then(Value::as_u64);
                                if let Some(m) = prog.get("model").and_then(Value::as_str) {
                                    run.model = Some(m.to_string());
                                }
                            }
                        }
                    }
                }
                if let Some(last) = self.messages.last_mut() {
                    let out_str = partial_result
                        .as_ref()
                        .map(|v| {
                            let txt = flatten_content_text(v.get("content"));
                            if !txt.trim().is_empty() {
                                txt
                            } else if let Some(s) = v.as_str() {
                                s.to_string()
                            } else {
                                String::new()
                            }
                        })
                        .unwrap_or_default();
                    if let Some(tc) = last.tool_calls.iter_mut().find(|t| t.id.as_ref() == tool_call_id) {
                        if !out_str.is_empty() {
                            tc.output = SharedString::from(out_str.clone());
                        }
                        if subagent_sid.is_some() {
                            tc.subagent_session_id = subagent_sid.clone();
                        }
                    }
                    for step in &mut last.process_steps {
                        if let ProcessStep::ToolCall(tc) = step {
                            if tc.id.as_ref() == tool_call_id {
                                if !out_str.is_empty() {
                                    tc.output = SharedString::from(out_str.clone());
                                }
                                if subagent_sid.is_some() {
                                    tc.subagent_session_id = subagent_sid.clone();
                                }
                            }
                        }
                    }
                }
            }
            AgentSseEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
            } => {
                if tool_name == "todo" {
                    self.todos_from_tool = true;
                    let updated_from_details = result
                        .as_ref()
                        .and_then(|v| v.get("details"))
                        .and_then(parse_todo_tool_details)
                        .map(|parsed| {
                            if !parsed.is_empty() {
                                self.todos_hidden = false;
                                self.todos_hidden_session = None;
                            }
                            self.set_todos(parsed);
                        })
                        .is_some();
                    if !updated_from_details {
                        let output_text = result
                            .as_ref()
                            .and_then(|v| v.get("content"))
                            .map(|c| flatten_content_text(Some(c)))
                            .unwrap_or_default();
                        if !output_text.is_empty() {
                            if !output_text.starts_with("No tasks")
                                && !output_text.starts_with("Cleared ")
                            {
                                self.todos_hidden = false;
                                self.todos_hidden_session = None;
                            }
                            apply_todo_tool_output_text(&mut self.todos, &output_text);
                            self.prune_dismissed_todos();
                        }
                    }
                    if let Some(sid) = self.active_session_id.clone() {
                        self.sync_active_agent_state(sid, cx);
                    }
                }
                let details_val = result.as_ref().and_then(|v| v.get("details"));
                let subagent_sid = extract_subagent_session_id(details_val);
                if let Some(d) = details_val {
                    if d.get("kind").and_then(Value::as_str) == Some("pi-web-subagent") {
                        self.apply_pi_web_subagent_details(&tool_call_id, d, is_error);
                    } else if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
                        run.state = if is_error { "failed" } else { "complete" }.to_string();
                        run.current_tool = None;
                    }
                } else if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
                    run.state = if is_error { "failed" } else { "complete" }.to_string();
                    run.current_tool = None;
                }
                if let Some(last) = self.messages.last_mut() {
                    let extra_diff = if tool_name == "edit" {
                        build_diff_from_tool(&tool_name, None, details_val)
                    } else {
                        None
                    };
                    let text = result
                        .as_ref()
                        .map(|val| {
                            let txt = flatten_content_text(val.get("content"));
                            if !txt.is_empty() {
                                txt
                            } else if let Some(s) = val.as_str() {
                                s.to_string()
                            } else {
                                val.to_string()
                            }
                        })
                        .unwrap_or_default();
                    if let Some(tc) = last.tool_calls.iter_mut().find(|t| t.id.as_ref() == tool_call_id) {
                        tc.is_running = false;
                        tc.is_error = is_error;
                        if tc.diff_patch.is_none() {
                            tc.diff_patch = extra_diff.clone();
                        }
                        if subagent_sid.is_some() {
                            tc.subagent_session_id = subagent_sid.clone();
                        }
                        if !text.is_empty() {
                            tc.output = SharedString::from(text.clone());
                        }
                    }
                    for step in &mut last.process_steps {
                        if let ProcessStep::ToolCall(tc) = step {
                            if tc.id.as_ref() == tool_call_id {
                                tc.is_running = false;
                                tc.is_error = is_error;
                                if tc.diff_patch.is_none() {
                                    tc.diff_patch = extra_diff.clone();
                                }
                                if subagent_sid.is_some() {
                                    tc.subagent_session_id = subagent_sid.clone();
                                }
                                if !text.is_empty() {
                                    tc.output = SharedString::from(text.clone());
                                }
                            }
                        }
                    }
                }
            }
            AgentSseEvent::ExtensionUiRequest { payload } => {
                let method = payload.get("method").and_then(Value::as_str).unwrap_or("");
                if method == "setWidget" {
                    let key = payload
                        .get("widgetKey")
                        .or_else(|| payload.get("key"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let lines: Vec<String> = payload
                        .get("widgetLines")
                        .or_else(|| payload.get("lines"))
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default();

                    if key == "subagent-async" {
                        if lines.is_empty() {
                            self.subagent_runs.clear();
                        } else if let Some(runs) = parse_subagent_async_json_widget(&lines) {
                            self.subagent_runs = runs;
                        }
                    } else if key == "agy-link-state" {
                        self.agy_link_loading = false;
                        self.settings_deleting_keys.retain(|k| !k.starts_with("agy:"));
                        if lines.is_empty() {
                            self.agy_link_state = None;
                        } else if let Some(parsed) = crate::client::session_controller::parse_agy_link_widget_lines(&lines) {
                            if self.agy_link_copy_auth_url_pending {
                                if let Some(url) = parsed.pending_auth_url.as_ref().filter(|u| !u.trim().is_empty()) {
                                    cx.write_to_clipboard(gpui_kit::gpui::ClipboardItem::new_string(url.clone()));
                                    self.agy_link_copy_auth_url_pending = false;
                                    self.agy_link_auth_copied = true;
                                    self.status_banner = Some(SharedString::from("Copied Google OAuth login link to clipboard."));
                                }
                            }
                            let catalog_changed = self.agy_link_state.as_ref().map(|prev| {
                                prev.active_account_id != parsed.active_account_id
                                    || prev.available_models != parsed.available_models
                                    || prev.settings.hidden_models != parsed.settings.hidden_models
                            }).unwrap_or(true);
                            self.agy_link_state = Some(parsed);
                            self.sync_agy_selected_model_visibility(cx);
                            if catalog_changed {
                                self.refresh_available_models_async(cx);
                            }
                        }
                    } else if key.contains("todo") {
                        let parsed = parse_todo_widget_lines(&lines);
                        if !self.todos_from_tool && (!parsed.is_empty() || lines.is_empty()) {
                            self.set_todos(parsed);
                        }
                    }
                } else if method == "setStatus" {
                    let key = payload.get("statusKey").and_then(Value::as_str).unwrap_or("");
                    if key.starts_with("subagent") {
                        self.subagent_status_text = payload
                            .get("statusText")
                            .and_then(Value::as_str)
                            .map(String::from);
                    } else if key == "agy-link" {
                        self.agy_link_status_text = payload
                            .get("statusText")
                            .and_then(Value::as_str)
                            .filter(|s| !s.trim().is_empty())
                            .map(String::from);
                    }
                } else if method == "notify" {
                    if let Some(msg) = payload.get("message").and_then(Value::as_str) {
                        let notify_type = payload.get("notifyType").and_then(Value::as_str).unwrap_or("info");
                        let suppress_info_banner =
                            notify_type == "info" && (self.suppress_next_prompt_reload || self.agy_link_loading);
                        if !suppress_info_banner
                            && (notify_type == "error" || notify_type == "warning" || notify_type == "info")
                        {
                            let summary = msg.lines().take(3).collect::<Vec<_>>().join(" | ");
                            self.status_banner = Some(SharedString::from(summary));
                        }
                    }
                } else if let Ok(req) = serde_json::from_value::<ExtensionUiRequest>(payload) {
                    let n_title = crate::i18n::tr(self.locale, "notify.attentionTitle");
                    let n_body = req
                        .title
                        .as_deref()
                        .or(req.message.as_deref())
                        .unwrap_or("");
                    self.trigger_desktop_notification(n_title, n_body);
                    self.push_extension_dialog(req);
                }
            }
            AgentSseEvent::CompactionStart => {
                self.is_compacting = true;
                self.is_agent_running = true;
                let step = CompactionStep {
                    summary_markdown: String::new(),
                    tokens_before: None,
                    read_files: Vec::new(),
                    modified_files: Vec::new(),
                    is_running: true,
                };
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        if !last.content.trim().is_empty() {
                            let intermediate = std::mem::take(&mut last.content);
                            last.process_steps
                                .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                        }
                        last.is_streaming = true;
                        last.process_steps.push(ProcessStep::Compaction(step));
                        cx.notify();
                        return;
                    }
                }
                let id = self.controller.alloc_msg_id();
                self.messages.push(ChatMessage {
                    id,
                    entry_id: None,
                    role: MessageRole::System,
                    model_label: None,
                    timestamp: current_local_hhmm(),
                    thinking: SharedString::default(),
                    process_steps: vec![ProcessStep::Compaction(step)],
                    process_message_count: 1,
                    thinking_ms: None,
                    thinking_expanded: false,
                    content: String::new(),
                    tool_calls: Vec::new(),
                    attachments: Vec::new(),
                    is_streaming: true,
                    turn_duration_ms: None,
                    token_summary: None,
                    turn_usage: None,
                    stop_reason: None,
                    error_message: None,
                });
            }
            AgentSseEvent::CompactionEnd {
                summary,
                result,
                aborted,
                error_message,
            } => {
                self.is_compacting = false;
                let was_aborted = aborted.unwrap_or(false);
                if let Some(err) = error_message {
                    self.record_assistant_error(format!("Compaction failed: {err}"));
                }
                let raw_summary = summary
                    .or_else(|| {
                        result
                            .as_ref()
                            .and_then(|r| r.get("summary"))
                            .and_then(Value::as_str)
                            .map(String::from)
                    })
                    .unwrap_or_default();
                let tokens_before = result
                    .as_ref()
                    .and_then(|r| r.get("tokensBefore"))
                    .and_then(Value::as_u64);
                let (summary_markdown, read_files, modified_files) =
                    parse_compaction_summary(&raw_summary);

                let mut updated = false;
                for msg in self.messages.iter_mut().rev() {
                    for step in msg.process_steps.iter_mut().rev() {
                        if let ProcessStep::Compaction(c) = step {
                            if c.is_running {
                                c.is_running = false;
                                if !was_aborted {
                                    c.summary_markdown = summary_markdown.clone();
                                    c.tokens_before = tokens_before;
                                    c.read_files = read_files.clone();
                                    c.modified_files = modified_files.clone();
                                }
                                updated = true;
                                break;
                            }
                        }
                    }
                    if updated {
                        if msg.role == MessageRole::System {
                            msg.is_streaming = false;
                        }
                        break;
                    }
                }
                if !was_aborted {
                    self.stats.context_tokens = None;
                    self.stats.context_used_pct = 0.0;
                    if let Some(sid) = self.active_session_id.clone() {
                        self.sync_active_agent_state(sid, cx);
                    }
                }
            }
            AgentSseEvent::AutoRetryStart {
                attempt,
                max_attempts,
                error_message: _,
            } => {
                if let Some(last) = self.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        if let Some(prev_err) = last.error_message.take() {
                            last.process_steps.push(ProcessStep::Thinking(
                                SharedString::from(format!("⚠️ {prev_err}")),
                            ));
                            last.stop_reason = None;
                        }
                    }
                }
                let att = attempt.unwrap_or(1);
                let progress = match max_attempts {
                    Some(max) => format!("{att}/{max}"),
                    None => att.to_string(),
                };
                self.retry_status = Some(SharedString::from(progress));
            }
            AgentSseEvent::AutoRetryEnd {
                success,
                final_error,
                error_message,
            } => {
                self.retry_status = None;
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
                if success == Some(false) {
                    if let Some(err) = final_error.or(error_message).filter(|s| !s.trim().is_empty()) {
                        self.record_assistant_error(err);
                    }
                } else if success == Some(true) {
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == MessageRole::Assistant {
                            if let Some(prev_err) = last.error_message.take() {
                                last.process_steps.push(ProcessStep::Thinking(
                                    SharedString::from(format!("⚠️ {prev_err}")),
                                ));
                                last.stop_reason = None;
                            }
                        }
                    }
                }
            }
            AgentSseEvent::PromptError { error_message }
            | AgentSseEvent::StartupError { error_message } => {
                self.is_agent_running = false;
                self.retry_status = None;
        self.continuing_msg_id = None;
        self.continuing_prefix_text.clear();
                let err = error_message.unwrap_or_else(|| "Request failed".to_string());
                self.record_assistant_error(err);
            }
            AgentSseEvent::ExtensionError { error } => {
                let err = error.unwrap_or_else(|| "Extension error".to_string());
                self.record_assistant_error(err);
            }
            AgentSseEvent::Unknown => {}
        }
        cx.notify();
    }

    /// Route an `extension_ui_request` into app state. Extension dialogs are queued so
    /// the questionnaire renders one at a time, and an `ask_user_question` dialog is
    /// matched to its tool call so it submits the canonical answer envelope.
    ///
    /// Identical repeated questions or retried requests are automatically deduplicated.
    fn push_extension_dialog(&mut self, req: ExtensionUiRequest) {
        if req.method == "ask_user_question" {
            self.questionnaire_request_id = Some(req.request_id.clone());
        }

        let is_duplicate_id = self
            .pending_ui_request
            .as_ref()
            .map(|p| p.request_id == req.request_id)
            .unwrap_or(false)
            || self.dialog_queue.iter().any(|r| r.request_id == req.request_id);

        let is_duplicate_content = self.pending_ui_request.as_ref().is_some_and(|p| {
            p.method == req.method
                && p.message == req.message
                && p.questions.len() == req.questions.len()
                && p.questions
                    .iter()
                    .zip(req.questions.iter())
                    .all(|(a, b)| a.question.trim().eq_ignore_ascii_case(b.question.trim()))
        }) || self.dialog_queue.iter().any(|p| {
            p.method == req.method
                && p.message == req.message
                && p.questions.len() == req.questions.len()
                && p.questions
                    .iter()
                    .zip(req.questions.iter())
                    .all(|(a, b)| a.question.trim().eq_ignore_ascii_case(b.question.trim()))
        });

        if !is_duplicate_id && !is_duplicate_content {
            if self.pending_ui_request.is_none() {
                self.pending_ui_request = Some(req);
            } else {
                self.dialog_queue.push(req);
            }
        }
    }

    /// The tool name of the most recent unfinished `ask_user_question` call, if any.
    fn awaiting_ask_user_question(&self) -> bool {
        self.messages
            .iter()
            .rev()
            .find_map(|m| {
                m.tool_calls
                    .iter()
                    .rev()
                    .find(|tc| tc.name.as_ref() == "ask_user_question")
                    .map(|tc| tc.is_running)
            })
            .unwrap_or(false)
    }

    pub fn complete_pending_ui_request(&mut self, cx: &mut Context<Self>) {
        self.pending_ui_request = None;
        self.questionnaire_request_id = None;
        if !self.dialog_queue.is_empty() {
            let next = self.dialog_queue.remove(0);
            self.pending_ui_request = Some(next);
        }
        cx.notify();
    }

    pub fn open_agy_link_modal(&mut self, cx: &mut Context<Self>) {
        self.agy_link_modal_open = false;
        self.settings_initial_page = 6;
        self.settings_open_nonce = self.settings_open_nonce.wrapping_add(1);
        self.settings_open = true;
        cx.notify();
    }

    pub fn close_agy_link_modal(&mut self, cx: &mut Context<Self>) {
        self.agy_link_modal_open = false;
        self.agy_link_details_account_id = None;
        cx.notify();
    }

    pub fn copy_agy_auth_url(&mut self, cx: &mut Context<Self>) {
        self.copy_agy_command_auth_url("/agy auth-url", false, cx);
    }

    pub fn copy_agy_relogin_url(&mut self, account_id: &str, is_system_home: bool, cx: &mut Context<Self>) {
        let cmd = if is_system_home {
            "/agy primary-url".to_string()
        } else {
            format!("/agy reauth-url {account_id}")
        };
        self.copy_agy_command_auth_url(&cmd, true, cx);
    }

    pub fn copy_agy_command_auth_url(&mut self, cmd: &str, force_new: bool, cx: &mut Context<Self>) {
        if !force_new {
            if let Some(url) = self
                .agy_link_state
                .as_ref()
                .and_then(|s| s.pending_auth_url.as_ref())
                .filter(|u| !u.trim().is_empty())
                .cloned()
            {
                cx.write_to_clipboard(gpui_kit::gpui::ClipboardItem::new_string(url));
                self.agy_link_copy_auth_url_pending = false;
                self.agy_link_auth_copied = true;
                self.status_banner = Some(SharedString::from("Copied Google OAuth login link to clipboard."));
                cx.notify();
                return;
            }
        }
        self.agy_link_copy_auth_url_pending = true;
        self.agy_link_auth_copied = false;
        self.send_agy_command(cmd.to_string(), cx);
    }

    pub fn sync_agy_selected_model_visibility(&mut self, cx: &mut Context<Self>) {
        let Some(ref sel) = self.selected_model else {
            return;
        };
        if sel.provider != "agy-link" {
            return;
        }
        let is_hidden = self
            .agy_link_state
            .as_ref()
            .map(|st| st.settings.hidden_models.iter().any(|h| h == &sel.id))
            .unwrap_or(false);
        if !is_hidden {
            return;
        }
        if let Some(next_model) = self.visible_picker_models().into_iter().next() {
            self.selected_model = Some(next_model.clone());
            self.clamp_thinking_level_to_model();
            let clamped_lvl = self.thinking_level;
            if let Some(sid) = self.active_session_id.clone() {
                if self.sidecar_connected {
                    let client = self.client.clone();
                    cx.spawn(async move |_, _| {
                        let _ = client
                            .set_session_model(&sid, &next_model.provider, &next_model.id)
                            .await;
                        let _ = client.set_thinking_level(&sid, clamped_lvl).await;
                    })
                    .detach();
                }
            }
        }
    }

    pub fn refresh_available_models_async(&mut self, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            return;
        }
        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.get_models(Some(&cwd_str)).await {
                let _ = entity.update(cx, |this, cx| {
                    this.apply_models_response(&val);
                    this.enrich_thinking_levels_from_config();
                    this.sync_agy_selected_model_visibility(cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn send_agy_command(&mut self, cmd: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            self.status_banner = Some(SharedString::from("AGY commands require Pi sidecar to be connected."));
            cx.notify();
            return;
        }
        let Some(sid) = self.active_session_id.clone() else {
            self.status_banner = Some(SharedString::from("Open or start a session first to run AGY commands."));
            cx.notify();
            return;
        };
        let client = self.client.clone();
        self.agy_link_loading = true;
        self.suppress_next_prompt_reload = true;
        cx.notify();
        let app_weak = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let _ = client
                .send_agent_command(&sid, json!({ "type": "prompt", "message": cmd }))
                .await;
            smol::Timer::after(std::time::Duration::from_secs(12)).await;
            let _ = app_weak.update(cx, |s, cx| {
                s.suppress_next_prompt_reload = false;
                if s.agy_link_loading {
                    s.agy_link_loading = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn record_assistant_error(&mut self, err: String) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == MessageRole::Assistant && last.is_streaming {
                last.is_streaming = false;
                last.stop_reason = Some(SharedString::from("error"));
                last.error_message = Some(SharedString::from(err));
                if last.timestamp.as_ref() == "streaming" || last.timestamp.is_empty() {
                    last.timestamp = current_local_hhmm();
                }
                return;
            }
        }
        let id = self.controller.alloc_msg_id();
        let model_label = self.selected_model.as_ref().map(ModelInfo::display_label);
        self.messages.push(ChatMessage {
            id,
            entry_id: None,
            role: MessageRole::Assistant,
            model_label,
            timestamp: current_local_hhmm(),
            thinking: SharedString::default(),
            process_steps: Vec::new(),
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: String::new(),
            tool_calls: Vec::new(),
            attachments: Vec::new(),
            is_streaming: false,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: None,
            stop_reason: Some(SharedString::from("error")),
            error_message: Some(SharedString::from(err)),
        });
    }

    pub fn rename_session_by_id(&mut self, id: String, new_name: String, cx: &mut Context<Self>) {
        let trimmed = new_name.trim().to_string();
        if trimmed.is_empty() {
            self.renaming_session_id = None;
            cx.notify();
            return;
        }
        if let Some(sess) = self.sessions.iter_mut().find(|s| s.id == id) {
            sess.name = Some(trimmed.clone());
        }
        self.renaming_session_id = None;
        let _ = crate::client::local_store::rename_session_local(&id, &trimmed);
        if self.sidecar_connected {
            let client = self.client.clone();
            cx.spawn(async move |_, _| {
                let _ = client.rename_session(&id, &trimmed).await;
            })
            .detach();
        }
        cx.notify();
    }

    pub fn toggle_pin_session(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(sess) = self.sessions.iter_mut().find(|s| s.id == id) {
            sess.pinned = !sess.pinned;
        }
        cx.notify();
    }

    pub fn delete_session_by_id(&mut self, id: String, cx: &mut Context<Self>) {
        self.sessions.retain(|s| s.id != id);
        self.temp_chat_ids.remove(&id);
        crate::client::local_store::unregister_temp_chat(&id);
        if self.active_session_id.as_deref() == Some(id.as_str()) {
            self.start_new_composer(self.active_workspace_kind, cx);
        }
        let _ = crate::client::local_store::delete_session_local(&id);
        if self.sidecar_connected {
            let client = self.client.clone();
            cx.spawn(async move |_, _| {
                let _ = client.delete_session(&id).await;
            })
            .detach();
        }
        cx.notify();
    }

    pub fn fork_session_by_id(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            if let Ok((new_id, _path)) =
                crate::client::local_store::fork_session_local(&id, None)
            {
                let scanned = scan_local_sessions(&pi_sessions_root());
                if !scanned.is_empty() {
                    self.sessions = scanned;
                }
                self.select_session(new_id, cx);
            }
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            if let Ok(val) = client.fork_session(&id, "").await {
                let _ = entity.update(cx, |this, cx| {
                    this.refresh_from_sidecar(cx);
                    if let Some(new_id) = val.get("session").and_then(|s| s.get("id")).and_then(Value::as_str) {
                        this.select_session(new_id.to_string(), cx);
                    }
                });
            }
        })
        .detach();
    }

    #[allow(dead_code)]
    pub fn create_scheduled_task_payload(
        &mut self,
        payload: Value,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected {
            let _ = crate::client::local_store::create_scheduled_task_local(&payload);
            self.scheduled_tasks = crate::client::local_store::load_scheduled_tasks_local();
            cx.notify();
            return;
        }
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.create_scheduled_task_payload(payload).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_scheduled_tasks(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn set_thinking_level_choice(&mut self, level: ThinkingLevel, cx: &mut Context<Self>) {
        self.thinking_level = level;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client.set_thinking_level(&sid, level).await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_tool_preset_choice(&mut self, preset: ToolPreset, cx: &mut Context<Self>) {
        self.tool_preset = preset;
        let tools = preset.tool_names();
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client.set_session_tools(&sid, &tools).await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_auto_compaction(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.auto_compaction = enabled;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client
                        .send_agent_command(&sid, json!({ "type": "set_auto_compaction", "enabled": enabled }))
                        .await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_auto_retry(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.auto_retry = enabled;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client
                        .send_agent_command(&sid, json!({ "type": "set_auto_retry", "enabled": enabled }))
                        .await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_steering_mode(&mut self, mode: SteeringMode, cx: &mut Context<Self>) {
        self.steering_mode = mode;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                let m = mode.as_str();
                cx.spawn(async move |_, _| {
                    let _ = client
                        .send_agent_command(&sid, json!({ "type": "set_steering_mode", "mode": m }))
                        .await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn set_follow_up_mode(&mut self, mode: SteeringMode, cx: &mut Context<Self>) {
        self.follow_up_mode = mode;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                let m = mode.as_str();
                cx.spawn(async move |_, _| {
                    let _ = client
                        .send_agent_command(&sid, json!({ "type": "set_follow_up_mode", "mode": m }))
                        .await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn trigger_compact(&mut self, cx: &mut Context<Self>) {
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client.send_agent_command(&sid, json!({ "type": "compact" })).await;
                })
                .detach();
            }
            self.status_banner = Some(SharedString::from("Compacting session context..."));
            cx.notify();
        }
    }

    /// Built-in slash command dispatch aligned with hooks/useAgentSession.ts
    pub fn try_handle_slash_command(&mut self, raw_text: &str, cx: &mut Context<Self>) -> bool {
        let trimmed = raw_text.trim();
        if !trimmed.starts_with('/') {
            return false;
        }
        let mut parts = trimmed[1..].splitn(2, char::is_whitespace);
        let cmd = parts.next().unwrap_or("").to_lowercase();
        let args = parts.next().unwrap_or("").trim().to_string();

        match cmd.as_str() {
            "clear" | "new" => {
                self.start_new_composer(self.active_workspace_kind, cx);
                true
            }
            "session" => {
                self.open_right_tab(RightPanelTab::Inspector, cx);
                true
            }
            "mcp" if args.is_empty() => {
                self.open_settings(cx);
                true
            }
            "name" => {
                if let Some(sid) = self.active_session_id.clone() {
                    if !args.is_empty() {
                        self.rename_session_by_id(sid, args, cx);
                    }
                }
                true
            }
            "compact" => {
                if let Some(sid) = self.active_session_id.clone() {
                    if self.sidecar_connected {
                        let client = self.client.clone();
                        cx.spawn(async move |_, _| {
                            let _ = client.send_agent_command(&sid, json!({ "type": "compact" })).await;
                        })
                        .detach();
                    }
                    self.status_banner = Some(SharedString::from("Compacting session context..."));
                    cx.notify();
                }
                true
            }
            "reload" => {
                self.refresh_from_sidecar(cx);
                self.status_banner = Some(SharedString::from("Reloaded session resources"));
                cx.notify();
                true
            }
            _ => false,
        }
    }

    pub fn submit_prompt(&mut self, raw_text: String, cx: &mut Context<Self>) {
        let text = raw_text.trim().to_string();
        if text.is_empty() || self.controller.send_in_flight {
            return;
        }

        if self.try_handle_slash_command(&text, cx) {
            return;
        }

        if self.is_agent_running {
            let qid = self.controller.alloc_queue_id();
            self.outbox.push(QueuedPrompt {
                id: qid,
                text,
                attachments: Vec::new(),
            });
            cx.notify();
            return;
        }

        self.dispatch_single_prompt(text, cx);
    }

    fn dispatch_single_prompt(&mut self, text: String, cx: &mut Context<Self>) {
        if self.selected_model.is_none() {
            self.controller.send_in_flight = false;
            self.is_agent_running = false;
            self.record_assistant_error(
                "No model is configured yet. Open Settings → Models, pick a provider (for example OpenRouter or DeepSeek), paste its API key, and fetch the model list."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        self.controller.send_in_flight = true;

        let msg_id = self.controller.alloc_msg_id();
        self.messages.push(ChatMessage {
            id: msg_id,
            entry_id: None,
            role: MessageRole::User,
            model_label: None,
            timestamp: current_local_hhmm(),
            thinking: SharedString::default(),
            process_steps: Vec::new(),
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: text.clone(),
            tool_calls: Vec::new(),
            attachments: Vec::new(),
            is_streaming: false,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: None,
            stop_reason: None,
            error_message: None,
        });

        // If sidecar is offline, attempt native Rust LLM streaming + built-in tool execution
        // (§6.3.2), falling back to the local preview only when no provider key is configured.
        if !self.sidecar_connected {
            self.controller.send_in_flight = false;
            self.is_agent_running = true;
            let reply_id = self.controller.alloc_msg_id();
            let model_label = self.selected_model.as_ref().map(ModelInfo::display_label);

            let selected_provider_and_model = self
                .selected_model
                .as_ref()
                .map(|m| (m.provider.clone(), m.id.clone()));

            let native_resolved = selected_provider_and_model.and_then(|(prov, mid)| {
                crate::engine::llm_stream::resolve_provider_endpoint_and_key(&prov)
                    .ok()
                    .map(|(kind, base_url, api_key)| (prov, mid, kind, base_url, api_key))
            });

            if let Some((provider_id, model_id, api_kind, base_url, api_key)) = native_resolved {
                self.messages.push(ChatMessage {
                    id: reply_id,
                    entry_id: None,
                    role: MessageRole::Assistant,
                    model_label,
                    timestamp: SharedString::from("Streaming…"),
                    thinking: SharedString::default(),
                    process_steps: Vec::new(),
                    process_message_count: 1,
                    thinking_ms: None,
                    thinking_expanded: false,
                    content: String::new(),
                    tool_calls: Vec::new(),
                    attachments: Vec::new(),
                    is_streaming: true,
                    turn_duration_ms: None,
                    token_summary: None,
                    turn_usage: None,
                    stop_reason: None,
                    error_message: None,
                });

                let mut history: Vec<crate::engine::llm_stream::NativeChatMessage> = self
                    .messages
                    .iter()
                    .filter(|m| m.id != reply_id)
                    .filter_map(|m| match m.role {
                        MessageRole::User => Some(crate::engine::llm_stream::NativeChatMessage {
                            role: "user".to_string(),
                            content: m.content.clone(),
                            tool_call_id: None,
                            tool_calls: Vec::new(),
                        }),
                        MessageRole::Assistant if !m.content.trim().is_empty() => {
                            Some(crate::engine::llm_stream::NativeChatMessage {
                                role: "assistant".to_string(),
                                content: m.content.clone(),
                                tool_call_id: None,
                                tool_calls: Vec::new(),
                            })
                        }
                        _ => None,
                    })
                    .collect();

                let allowed_tools: Vec<String> = self
                    .tool_preset
                    .tool_names()
                    .into_iter()
                    .map(String::from)
                    .collect();
                let mut tool_defs: Vec<crate::engine::llm_stream::NativeToolDefinition> =
                    crate::engine::native_tools::builtin_tool_definitions()
                        .into_iter()
                        .filter(|t| allowed_tools.iter().any(|n| n == &t.name))
                        .collect();
                if allowed_tools.iter().any(|n| n == "codemode") {
                    tool_defs.push(crate::engine::llm_stream::NativeToolDefinition {
                        name: "codemode".to_string(),
                        description: "Execute JavaScript in the QuickJS/JavaScriptCore sandbox with pi.read, pi.write, and pi.edit.".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "script": { "type": "string", "description": "JavaScript source code to evaluate" }
                            },
                            "required": ["script"]
                        }),
                    });
                }

                let cwd = self.active_cwd.clone();
                let thinking_lvl = Some(self.thinking_level.as_str().to_string());
                let http_client = self.client.raw_http();
                let entity = cx.entity().clone();
                let started_at = std::time::Instant::now();

                cx.spawn(async move |_, cx| {
                    for _step in 0..8 {
                        let req = crate::engine::llm_stream::NativeLlmStreamRequest {
                            provider_id: provider_id.clone(),
                            model_id: model_id.clone(),
                            api_kind,
                            base_url: base_url.clone(),
                            api_key: api_key.clone(),
                            system_prompt: Some(format!(
                                "You are Pi Agent running natively in Rust gpui. Working directory: {}",
                                cwd.display()
                            )),
                            messages: history.clone(),
                            tools: tool_defs.clone(),
                            max_tokens: Some(4096),
                            thinking_level: thinking_lvl.clone(),
                        };

                        let mut rx = crate::engine::llm_stream::spawn_native_llm_stream(
                            http_client.clone(),
                            req,
                        );
                        let mut step_text = String::new();
                        let mut completed_calls = Vec::new();
                        let mut stream_err = None;

                        while let Some(ev) = rx.recv().await {
                            match ev {
                                crate::engine::llm_stream::NativeLlmStreamEvent::TextDelta(delta) => {
                                    step_text.push_str(&delta);
                                    let _ = entity.update(cx, |this, cx| {
                                        if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                            m.content.push_str(&delta);
                                        }
                                        cx.notify();
                                    });
                                }
                                crate::engine::llm_stream::NativeLlmStreamEvent::ThinkingDelta(delta) => {
                                    let _ = entity.update(cx, |this, cx| {
                                        if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                            let mut t = m.thinking.to_string();
                                            t.push_str(&delta);
                                            let shared = SharedString::from(t);
                                            m.thinking = shared.clone();
                                            if let Some(ProcessStep::Thinking(prev)) = m.process_steps.first_mut() {
                                                *prev = shared;
                                            } else {
                                                m.process_steps.insert(0, ProcessStep::Thinking(shared));
                                            }
                                        }
                                        cx.notify();
                                    });
                                }
                                crate::engine::llm_stream::NativeLlmStreamEvent::ToolCallComplete(call) => {
                                    completed_calls.push(call);
                                }
                                crate::engine::llm_stream::NativeLlmStreamEvent::Usage {
                                    input_tokens,
                                    output_tokens,
                                    cache_read_tokens,
                                } => {
                                    let _ = entity.update(cx, |this, cx| {
                                        if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                            m.turn_usage = Some(crate::client::types::TurnUsage {
                                                input_tokens,
                                                output_tokens,
                                                cache_read_tokens,
                                                cache_write_tokens: 0,
                                                total_tokens: input_tokens + output_tokens + cache_read_tokens,
                                                cost_usd: 0.0,
                                            });
                                        }
                                        cx.notify();
                                    });
                                }
                                crate::engine::llm_stream::NativeLlmStreamEvent::Error(e) => {
                                    stream_err = Some(e);
                                    break;
                                }
                                _ => {}
                            }
                        }

                        if let Some(err) = stream_err {
                            let _ = entity.update(cx, |this, cx| {
                                if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                    m.error_message = Some(SharedString::from(err));
                                    m.stop_reason = Some(SharedString::from("error"));
                                    m.is_streaming = false;
                                    m.timestamp = current_local_hhmm();
                                }
                                this.is_agent_running = false;
                                cx.notify();
                            });
                            break;
                        }

                        if completed_calls.is_empty() {
                            break;
                        }

                        history.push(crate::engine::llm_stream::NativeChatMessage {
                            role: "assistant".to_string(),
                            content: step_text,
                            tool_call_id: None,
                            tool_calls: completed_calls.clone(),
                        });

                        for call in completed_calls {
                            let args_vec: Vec<(SharedString, SharedString)> = call
                                .arguments
                                .as_object()
                                .map(|obj| {
                                    obj.iter()
                                        .map(|(k, v)| {
                                            let val_str = v
                                                .as_str()
                                                .map(String::from)
                                                .unwrap_or_else(|| v.to_string());
                                            (SharedString::from(k.clone()), SharedString::from(val_str))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();

                            let tc_id = SharedString::from(call.id.clone());
                            let tc_name = SharedString::from(call.name.clone());
                            let initial_item = crate::client::types::ToolCallItem {
                                id: tc_id.clone(),
                                name: tc_name.clone(),
                                arguments: args_vec.clone(),
                                output: SharedString::default(),
                                diff_patch: None,
                                subagent_session_id: None,
                                is_running: true,
                                is_error: false,
                                expanded: false,
                            };

                            let _ = entity.update(cx, |this, cx| {
                                if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                    m.tool_calls.push(initial_item.clone());
                                    m.process_steps.push(ProcessStep::ToolCall(initial_item));
                                }
                                cx.notify();
                            });

                            let exec_res = match call.name.as_str() {
                                "read" => crate::engine::native_tools::execute_read_tool(&cwd, &call.arguments),
                                "write" => crate::engine::native_tools::execute_write_tool(&cwd, &call.arguments),
                                "edit" => crate::engine::native_tools::execute_edit_tool(&cwd, &call.arguments),
                                "bash" => crate::engine::native_tools::execute_bash_tool(&cwd, &call.arguments).await,
                                "codemode" => {
                                    let script = call
                                        .arguments
                                        .get("script")
                                        .and_then(Value::as_str)
                                        .unwrap_or("");
                                    let eval = crate::engine::quickjs_sandbox::eval_codemode_script(&cwd, script);
                                    let res_text = if eval.is_error {
                                        eval.error_message.unwrap_or_else(|| eval.result.to_string())
                                    } else {
                                        eval.result.to_string()
                                    };
                                    let out = if eval.logs.is_empty() {
                                        res_text
                                    } else {
                                        format!("{}\n{}", eval.logs.join("\n"), res_text)
                                    };
                                    crate::engine::native_tools::NativeToolResult {
                                        output: out,
                                        is_error: eval.is_error,
                                        diff: None,
                                        details: None,
                                    }
                                }
                                other => crate::engine::native_tools::NativeToolResult {
                                    output: format!("Unsupported native tool: {other}"),
                                    is_error: true,
                                    diff: None,
                                    details: None,
                                },
                            };

                            let out_shared = SharedString::from(exec_res.output.clone());
                            let diff_shared = exec_res.diff.map(SharedString::from);
                            let is_err = exec_res.is_error;

                            let _ = entity.update(cx, |this, cx| {
                                if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                                    for tc in &mut m.tool_calls {
                                        if tc.id == tc_id {
                                            tc.output = out_shared.clone();
                                            tc.diff_patch = diff_shared.clone();
                                            tc.is_running = false;
                                            tc.is_error = is_err;
                                        }
                                    }
                                    for step in &mut m.process_steps {
                                        if let ProcessStep::ToolCall(tc) = step
                                            && tc.id == tc_id
                                        {
                                            tc.output = out_shared.clone();
                                            tc.diff_patch = diff_shared.clone();
                                            tc.is_running = false;
                                            tc.is_error = is_err;
                                        }
                                    }
                                }
                                cx.notify();
                            });

                            history.push(crate::engine::llm_stream::NativeChatMessage {
                                role: "tool".to_string(),
                                content: exec_res.output,
                                tool_call_id: Some(call.id),
                                tool_calls: Vec::new(),
                            });
                        }
                    }

                    let elapsed_ms = started_at.elapsed().as_millis() as u64;
                    let _ = entity.update(cx, |this, cx| {
                        if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                            m.is_streaming = false;
                            m.timestamp = current_local_hhmm();
                            m.turn_duration_ms = Some(elapsed_ms);
                        }
                        this.is_agent_running = false;
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
                return;
            }

            let provider_label = self
                .selected_model
                .as_ref()
                .map(ModelInfo::display_label)
                .unwrap_or_else(|| SharedString::from("the selected model"));
            let message = format!(
                "No usable credentials or endpoint for {provider_label}. Open Settings → Models, add a provider, and paste an API key. The desktop never fabricates an assistant reply."
            );
            self.record_assistant_error(message);
            self.is_agent_running = false;
            cx.notify();
            return;
        }

        let client = self.client.clone();
        let active_session = self.active_session_id.clone();
        if let Some(ref sid) = active_session {
            if self.controller.sse_task.is_none() {
                self.connect_session_sse(sid.clone(), cx);
            }
        }
        let is_temp_chat = self.active_is_temp_chat;
        let is_chat_workspace = self.active_workspace_kind == WorkspaceKind::Chat;
        if active_session.is_none() && is_chat_workspace {
            if is_temp_chat {
                let _ = std::fs::create_dir_all(crate::client::local_store::temp_chat_root_dir());
            } else {
                let _ = std::fs::create_dir_all(crate::client::local_store::standalone_chat_workspace_dir());
            }
        }
        let cwd = self.active_cwd.to_string_lossy().into_owned();
        let tool_names = self.tool_preset.tool_names();
        let provider = self.selected_model.as_ref().map(|m| m.provider.clone());
        let model_id = self.selected_model.as_ref().map(|m| m.id.clone());
        let thinking_lvl = Some(self.thinking_level.as_str().to_string());
        let entity = cx.entity().clone();

        cx.spawn(async move |_, cx| {
            let result = match active_session {
                Some(ref sid) => {
                    client
                        .send_agent_command(sid, json!({ "type": "prompt", "message": text }))
                        .await
                }
                None => {
                    let effective_cwd = if is_temp_chat {
                        client
                            .get_chat_workspace()
                            .await
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_else(|_| cwd.clone())
                    } else {
                        cwd.clone()
                    };
                    client
                        .start_new_session(
                            &effective_cwd,
                            &text,
                            &tool_names,
                            provider.as_deref(),
                            model_id.as_deref(),
                            thinking_lvl.as_deref(),
                        )
                        .await
                }
            };

            let _ = entity.update(cx, |this, cx| {
                this.controller.send_in_flight = false;
                match result {
                    Ok(val) => {
                        if this.active_session_id.is_none() {
                            if let Some(new_id) = val
                                .get("sessionId")
                                .or_else(|| val.get("data").and_then(|d| d.get("sessionId")))
                                .and_then(Value::as_str)
                            {
                                this.active_session_id = Some(new_id.to_string());
                                if is_temp_chat {
                                    this.temp_chat_ids.insert(new_id.to_string());
                                    crate::client::local_store::register_temp_chat(
                                        new_id,
                                        None,
                                        crate::client::local_store::TEMP_CHAT_TTL_MS,
                                    );
                                }
                                this.connect_session_sse(new_id.to_string(), cx);
                                this.refresh_from_sidecar(cx);
                            }
                        }
                        this.is_agent_running = true;
                    }
                    Err(err) => {
                        this.is_agent_running = false;
                        this.record_assistant_error(err.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn send_queued_now(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(pos) = self.outbox.iter().position(|q| q.id == id) else {
            return;
        };
        let item = self.outbox.remove(pos);
        if self.is_agent_running {
            if let Some(sid) = self.active_session_id.clone() {
                if self.sidecar_connected {
                    let client = self.client.clone();
                    let msg_text = item.text.clone();
                    cx.spawn(async move |_, _| {
                        let _ = client
                            .send_agent_command(
                                &sid,
                                json!({ "type": "steer", "message": msg_text }),
                            )
                            .await;
                    })
                    .detach();
                    cx.notify();
                    return;
                }
            }
        }
        self.dispatch_single_prompt(item.text, cx);
        cx.notify();
    }

    pub fn edit_queued_message(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(pos) = self.outbox.iter().position(|q| q.id == id) else {
            return;
        };
        let item = self.outbox.remove(pos);
        self.draft_restore_text = Some(item.text);
        cx.notify();
    }

    pub fn delete_queued_message(&mut self, id: u64, cx: &mut Context<Self>) {
        self.outbox.retain(|q| q.id != id);
        cx.notify();
    }

    fn flush_next_outbox(&mut self, cx: &mut Context<Self>) {
        if !self.is_agent_running && !self.outbox.is_empty() {
            let next = self.outbox.remove(0);
            self.dispatch_single_prompt(next.text, cx);
        }
    }

    pub fn stop_running_agent(&mut self, cx: &mut Context<Self>) {
        self.is_agent_running = false;
        if let Some(sid) = self.active_session_id.clone() {
            if self.sidecar_connected {
                let client = self.client.clone();
                cx.spawn(async move |_, _| {
                    let _ = client.abort_agent(&sid).await;
                })
                .detach();
            }
        }
        cx.notify();
    }

    pub fn run_file_fuzzy_search(&mut self, query: &str, cx: &mut Context<Self>) {
        let root = self.active_cwd.clone();
        let q = query.to_string();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let matches = smol::unblock(move || fuzzy_search_files(&root, &q, 18)).await;
            let _ = entity.update(cx, |this, cx| {
                this.file_search_results = matches;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn insert_composer_mention(&mut self, relative_path: &str, cx: &mut Context<Self>) {
        let mention = format!("@{relative_path}");
        match &mut self.pending_composer_mention {
            Some(existing) => {
                if !existing.is_empty() {
                    existing.push(' ');
                }
                existing.push_str(&mention);
            }
            slot => {
                *slot = Some(mention);
            }
        }
        cx.notify();
    }

    pub fn open_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_file_with_diff(path, None, cx);
    }

    pub fn open_file_with_diff(
        &mut self,
        path: PathBuf,
        diff_patch: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.open_file_path = Some(path.clone());
        self.open_file_diff_patch = diff_patch.clone();
        self.open_right_tab(RightPanelTab::FileViewer, cx);
        let entity = cx.entity().clone();
        let path_clone = path.clone();
        let cwd_clone = self.active_cwd.clone();
        cx.spawn(async move |_, cx| {
            let (content, patch_result) = smol::unblock(move || {
                let text = match std::fs::read(&path_clone) {
                    Ok(bytes) => {
                        if bytes.len() > 2 * 1024 * 1024 {
                            format!("File too large to preview ({} bytes)", bytes.len())
                        } else {
                            String::from_utf8(bytes)
                                .unwrap_or_else(|_| "<Binary file>".to_string())
                        }
                    }
                    Err(e) => format!("Failed to read file: {e}"),
                };

                let git_diff = if diff_patch.is_none() {
                    let out = crate::client::hidden_command("git")
                        .arg("-C")
                        .arg(&cwd_clone)
                        .args(["diff", "HEAD", "--"])
                        .arg(&path_clone)
                        .output()
                        .ok();
                    out.and_then(|o| {
                        if o.status.success() && !o.stdout.is_empty() {
                            let s = String::from_utf8_lossy(&o.stdout).to_string();
                            (!s.trim().is_empty()).then(|| SharedString::from(s))
                        } else {
                            None
                        }
                    })
                } else {
                    diff_patch
                };

                (text, git_diff)
            })
            .await;
            let _ = entity.update(cx, |this, cx| {
                this.open_file_content = SharedString::from(content);
                this.open_file_diff_patch = patch_result;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_initial_page = 0;
        self.settings_open_nonce = self.settings_open_nonce.wrapping_add(1);
        self.settings_open = true;
        self.refresh_settings_data(cx);
        cx.notify();
    }

    /// Opens Settings directly on the Models page (used by the no-model
    /// onboarding button in the empty chat state).
    pub fn open_model_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_initial_page = 1;
        self.settings_open_nonce = self.settings_open_nonce.wrapping_add(1);
        self.settings_open = true;
        self.refresh_settings_data(cx);
        cx.notify();
    }

    pub fn refresh_settings_data(&mut self, cx: &mut Context<Self>) {
        self.settings_loading = true;
        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let health_ok = client.health().await.is_ok();
            if !health_ok {
                let _ = entity.update(cx, |this, cx| {
                    this.sidecar_connected = false;
                    this.settings_loading = false;
                    this.load_local_store_snapshot();
                    cx.notify();
                });
                return;
            }
            let enabled_models_val = client.get_enabled_models(Some(&cwd_str)).await.ok();
            let auth_val = client.get_auth_all_providers().await.ok();
            let oauth_val = client.get_oauth_providers().await.ok();
            let mcp_val = client.get_mcp(Some(&cwd_str)).await.ok();
            let subagent_settings_val = client.get_subagent_settings().await.ok();
            let subagents_val = client.get_subagents(Some(&cwd_str)).await.ok();
            let skills_val = client.get_skills(Some(&cwd_str)).await.ok();
            let plugins_val = client.get_plugins(Some(&cwd_str)).await.ok();
            let proxy_val = client.get_network_proxy().await.ok();

            let _ = entity.update(cx, |this, cx| {
                this.sidecar_connected = true;
                if let Some(val) = enabled_models_val {
                    this.apply_enabled_models_response(&val);
                }
                if let Some(val) = auth_val {
                    this.apply_auth_providers_response(&val);
                }
                if let Some(val) = oauth_val {
                    this.apply_oauth_providers_response(&val);
                }
                if let Some(val) = mcp_val {
                    this.apply_mcp_response(&val);
                }
                if let Some(val) = subagent_settings_val {
                    if let Some(en) = val.get("enabled").and_then(Value::as_bool) {
                        this.subagent_built_in_enabled = en;
                    }
                    if let Some(mc) = val.get("maxConcurrent").and_then(Value::as_u64) {
                        this.subagent_max_concurrent = mc as usize;
                    }
                }
                if let Some(val) = subagents_val {
                    this.apply_subagents_response(&val);
                }
                if let Some(val) = skills_val {
                    this.apply_skills_response(&val);
                }
                if let Some(val) = plugins_val {
                    this.apply_plugins_response(&val);
                }
                this.settings_loading = false;
                if let Some(val) = proxy_val {
                    this.apply_network_proxy_response(&val);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn parse_thinking_level_str(raw: &str) -> Option<ThinkingLevel> {
    match raw.trim().to_lowercase().as_str() {
        "off" => Some(ThinkingLevel::Off),
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "xhigh" | "max" => Some(ThinkingLevel::XHigh),
        _ => None,
    }
}

fn resolve_git_branch(cwd: &Path) -> Option<SharedString> {
    let out = crate::client::hidden_command("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()?;

    if !out.status.success() || !out.stdout.starts_with(b"true") {
        return None;
    }

    if let Ok(branch_out) = crate::client::hidden_command("git")
        .arg("-C")
        .arg(cwd)
        .args(["branch", "--show-current"])
        .output()
    {
        if branch_out.status.success() {
            let b = String::from_utf8_lossy(&branch_out.stdout).trim().to_string();
            if !b.is_empty() {
                return Some(SharedString::from(b));
            }
        }
    }

    if let Ok(head_out) = crate::client::hidden_command("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if head_out.status.success() {
            let h = String::from_utf8_lossy(&head_out.stdout).trim().to_string();
            if !h.is_empty() {
                return Some(SharedString::from(h));
            }
        }
    }

    None
}

/// Inserts or updates a provider entry, preserving catalog metadata and the
/// strongest known "configured" state (`merge_configured`) or the exact state
/// reported by the sidecar (false, used for OAuth login/logout).
fn merge_auth_provider(
    list: &mut Vec<(String, String, bool)>,
    id: &str,
    name: &str,
    configured: bool,
    merge_configured: bool,
) {
    if let Some(existing) = list.iter_mut().find(|(i, _, _)| i == id) {
        if !name.is_empty() && (existing.1 == existing.0 || existing.1.is_empty()) {
            existing.1 = name.to_string();
        }
        existing.2 = if merge_configured {
            existing.2 || configured
        } else {
            configured
        };
    } else {
        list.push((
            id.to_string(),
            if name.is_empty() { id.to_string() } else { name.to_string() },
            configured,
        ));
    }
}

/// Sends a native macOS notification via `UNUserNotificationCenter` when running
/// inside a `.app` bundle (so macOS displays the app's own `icon.icns` instead of
/// `osascript`'s Script Editor icon). Returns `false` outside a `.app` bundle.
#[cfg(target_os = "macos")]
fn send_macos_bundle_notification(title: &str, body: &str, _with_sound: bool) -> bool {
    use std::ffi::{CString, c_char, c_void};
    use std::sync::{Once, OnceLock};

    #[link(name = "UserNotifications", kind = "framework")]
    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {
        static _NSConcreteGlobalBlock: [usize; 32];
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    #[repr(C)]
    struct BlockDescriptor {
        reserved: usize,
        size: usize,
    }
    #[repr(C)]
    struct GlobalBlock {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: *const c_void,
        descriptor: &'static BlockDescriptor,
    }
    unsafe impl Sync for GlobalBlock {}
    unsafe impl Send for GlobalBlock {}

    unsafe extern "C" fn noop_auth_cb(_block: *mut c_void, _granted: bool, _err: *mut c_void) {}
    unsafe extern "C" fn noop_add_cb(_block: *mut c_void, _err: *mut c_void) {}

    static DESCRIPTOR: BlockDescriptor = BlockDescriptor {
        reserved: 0,
        size: std::mem::size_of::<GlobalBlock>(),
    };
    static AUTH_BLOCK: OnceLock<GlobalBlock> = OnceLock::new();
    static ADD_BLOCK: OnceLock<GlobalBlock> = OnceLock::new();

    #[inline]
    unsafe fn sel(name: &[u8]) -> *mut c_void {
        unsafe { sel_registerName(name.as_ptr() as *const c_char) }
    }
    #[inline]
    unsafe fn cls(name: &[u8]) -> *mut c_void {
        unsafe { objc_getClass(name.as_ptr() as *const c_char) }
    }

    type Msg0 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type Msg1Ptr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    type Msg1Cstr = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char) -> *mut c_void;
    type Msg2UsizePtr =
        unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut c_void) -> *mut c_void;
    type Msg2PtrPtr =
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    type Msg3Ptr = unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *mut c_void,
        *mut c_void,
    ) -> *mut c_void;

    unsafe {
        let msg0: Msg0 = std::mem::transmute(objc_msgSend as *const c_void);
        let msg1_ptr: Msg1Ptr = std::mem::transmute(objc_msgSend as *const c_void);
        let msg1_cstr: Msg1Cstr = std::mem::transmute(objc_msgSend as *const c_void);
        let msg2_usize_ptr: Msg2UsizePtr = std::mem::transmute(objc_msgSend as *const c_void);
        let msg2_ptr_ptr: Msg2PtrPtr = std::mem::transmute(objc_msgSend as *const c_void);
        let msg3_ptr: Msg3Ptr = std::mem::transmute(objc_msgSend as *const c_void);

        let pool_cls = cls(b"NSAutoreleasePool\0");
        let pool = msg0(msg0(pool_cls, sel(b"alloc\0")), sel(b"init\0"));

        let bundle_cls = cls(b"NSBundle\0");
        let main_bundle = msg0(bundle_cls, sel(b"mainBundle\0"));
        let bundle_id = if main_bundle.is_null() {
            std::ptr::null_mut()
        } else {
            msg0(main_bundle, sel(b"bundleIdentifier\0"))
        };
        if bundle_id.is_null() {
            if !pool.is_null() {
                msg0(pool, sel(b"drain\0"));
            }
            return false;
        }

        let center_cls = cls(b"UNUserNotificationCenter\0");
        let content_cls = cls(b"UNMutableNotificationContent\0");
        let req_cls = cls(b"UNNotificationRequest\0");
        let nsstring_cls = cls(b"NSString\0");
        if center_cls.is_null() || content_cls.is_null() || req_cls.is_null() || nsstring_cls.is_null() {
            if !pool.is_null() {
                msg0(pool, sel(b"drain\0"));
            }
            return false;
        }

        let center = msg0(center_cls, sel(b"currentNotificationCenter\0"));
        if center.is_null() {
            if !pool.is_null() {
                msg0(pool, sel(b"drain\0"));
            }
            return false;
        }

        static AUTH_ONCE: Once = Once::new();
        AUTH_ONCE.call_once(|| {
            let block = AUTH_BLOCK.get_or_init(|| GlobalBlock {
                isa: (&raw const _NSConcreteGlobalBlock).cast::<c_void>(),
                flags: 1 << 28,
                reserved: 0,
                invoke: noop_auth_cb as *const c_void,
                descriptor: &DESCRIPTOR,
            });
            // UNAuthorizationOptionAlert (1 << 2) | UNAuthorizationOptionSound (1 << 1) = 6
            msg2_usize_ptr(
                center,
                sel(b"requestAuthorizationWithOptions:completionHandler:\0"),
                6,
                (block as *const GlobalBlock).cast_mut().cast::<c_void>(),
            );
        });

        let c_title = CString::new(title.replace('\0', "")).unwrap_or_default();
        let c_body = CString::new(body.replace('\0', "")).unwrap_or_default();
        let id_str = format!("picrust-{}", uuid::Uuid::new_v4());
        let c_id = CString::new(id_str).unwrap_or_default();

        let ns_title = msg1_cstr(nsstring_cls, sel(b"stringWithUTF8String:\0"), c_title.as_ptr());
        let ns_body = msg1_cstr(nsstring_cls, sel(b"stringWithUTF8String:\0"), c_body.as_ptr());
        let ns_id = msg1_cstr(nsstring_cls, sel(b"stringWithUTF8String:\0"), c_id.as_ptr());

        let content = msg0(msg0(content_cls, sel(b"alloc\0")), sel(b"init\0"));
        if !content.is_null() {
            msg1_ptr(content, sel(b"setTitle:\0"), ns_title);
            msg1_ptr(content, sel(b"setBody:\0"), ns_body);
            let req = msg3_ptr(
                req_cls,
                sel(b"requestWithIdentifier:content:trigger:\0"),
                ns_id,
                content,
                std::ptr::null_mut(),
            );
            if !req.is_null() {
                let block = ADD_BLOCK.get_or_init(|| GlobalBlock {
                    isa: (&raw const _NSConcreteGlobalBlock).cast::<c_void>(),
                    flags: 1 << 28,
                    reserved: 0,
                    invoke: noop_add_cb as *const c_void,
                    descriptor: &DESCRIPTOR,
                });
                msg2_ptr_ptr(
                    center,
                    sel(b"addNotificationRequest:withCompletionHandler:\0"),
                    req,
                    (block as *const GlobalBlock).cast_mut().cast::<c_void>(),
                );
            }
            msg0(content, sel(b"release\0"));
        }

        if !pool.is_null() {
            msg0(pool, sel(b"drain\0"));
        }
        true
    }
}

/// Whether a configured model should offer the strength ladder when pi reports
/// only `["off"]`: hidden only when models.json explicitly says non-reasoning.
fn should_offer_thinking_ladder(declared: &ConfiguredModelInfo) -> bool {
    declared.reasoning || !declared.thinking_level_map.is_empty() || !declared.reasoning_declared
}

#[cfg(test)]
mod thinking_enrichment_tests {
    use super::*;

    fn declared(reasoning: bool, reasoning_declared: bool) -> ConfiguredModelInfo {
        ConfiguredModelInfo {
            id: "m".into(),
            name: None,
            context_window: None,
            max_tokens: None,
            input: vec![],
            reasoning,
            reasoning_declared,
            thinking_level_map: Default::default(),
        }
    }

    #[test]
    fn absent_flag_offers_the_ladder() {
        // deepseek declares neither reasoning nor a map.
        assert!(should_offer_thinking_ladder(&declared(false, false)));
        // explicitly enabled reasoning
        assert!(should_offer_thinking_ladder(&declared(true, true)));
        // explicitly disabled reasoning stays hidden
        assert!(!should_offer_thinking_ladder(&declared(false, true)));
    }

    #[test]
    fn sanitize_thinking_levels_drops_aliases_and_nulled_entries() {
        let mut levels: Vec<String> = vec![
            "off".into(),
            "minimal".into(),
            "low".into(),
            "medium".into(),
            "high".into(),
            "xhigh".into(),
            "max".into(),
        ];
        let map: std::collections::HashMap<String, String> = [
            ("minimal", "low"),
            ("low", "low"),
            ("medium", "medium"),
            ("high", "high"),
            ("xhigh", "high"),
            ("max", "high"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let nulled: std::collections::HashSet<String> = ["off".to_string()].into_iter().collect();

        AppState::sanitize_thinking_levels(&mut levels, &map, &nulled);
        assert_eq!(levels, vec!["low", "medium", "high"]);
    }
}

