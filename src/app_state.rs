use crate::client::PiDaemonClient;
use crate::client::native_index::{
    FileMatch, fuzzy_search_files, pi_sessions_root, scan_local_sessions, user_home,
};
use crate::client::session_controller::{
    SessionController, apply_optimistic_todo_args, apply_todo_tool_output_text,
    build_diff_from_tool, compute_context_tokens_from_payload, compute_stats_from_jsonl,
    current_local_hhmm, extract_artifacts_from_markdown, extract_model_and_thinking_from_jsonl, extract_todos_from_jsonl,
    extract_todos_from_payload, extract_turn_usage, flatten_content_text, flatten_tool_args,
    merge_turn_usage, parse_compaction_summary, parse_message_content, parse_session_jsonl_file,
    parse_session_messages, parse_subagent_async_json_widget, parse_todo_tool_details,
    parse_todo_widget_lines,
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
}

pub struct AppState {
    pub client: PiDaemonClient,
    pub _sidecar: Option<SidecarProcess>,
    pub sidecar_connected: bool,
    pub controller: SessionController,

    pub active_page: WorkspacePage,
    pub sidebar_collapsed: bool,
    pub right_panel_open: bool,
    pub right_panel_tab: RightPanelTab,
    pub open_right_tabs: Vec<RightPanelTab>,
    pub right_panel_width: Pixels,
    pub settings_open: bool,
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
    pub chat_content_width: f32,
    pub chat_font_slider: Entity<SliderState>,
    pub ui_font_slider: Entity<SliderState>,
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
    pub worktrees: Vec<WorktreeInfo>,
    pub active_branch: Option<SharedString>,

    pub sessions: Vec<SessionSummary>,
    pub running_session_ids: HashSet<String>,
    pub unread_session_ids: HashSet<String>,
    pub collapsed_projects: HashSet<String>,
    pub archived_projects: HashSet<String>,
    pub archived_session_ids: HashSet<String>,
    pub extra_projects: Vec<PathBuf>,
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
    pub retry_status: Option<SharedString>,
    pub draft_restore_text: Option<String>,
    pub pending_composer_mention: Option<String>,
    #[allow(dead_code)]
    pub status_banner: Option<SharedString>,
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
        // bundled sidecar address; auto mode keeps the sidecar, which is itself the
        // detected local core.
        let settings_snapshot = crate::app_settings::load_app_settings();
        let configured_core = settings_snapshot
            .core
            .url
            .trim()
            .trim_end_matches('/')
            .to_string();
        let core_url = if configured_core.is_empty() {
            None
        } else if settings_snapshot.core.mode == "manual" {
            Some(configured_core)
        } else {
            // Auto mode: a chosen service still wins over the default port.
            Some(configured_core)
        };
        let client = match core_url {
            Some(base) if base != base_url => {
                PiDaemonClient::new(base, &client.token, &client.instance_id)
            }
            _ => client,
        };

        let mut cwd = std::env::current_dir().unwrap_or_else(|_| user_home());
        let mut sessions = scan_local_sessions(&pi_sessions_root());
        let mut ctrl = SessionController::default();

        // Seed rich Phase 1 demo state if no local sessions exist yet so the UI
        // immediately showcases Markdown, Math, Mermaid, Diff, and Artifacts.
        let mut messages = Vec::new();
        let mut artifacts = Vec::new();
        let mut todos = Vec::new();
        let active_session_id;
        let mut active_workspace_kind = WorkspaceKind::Project;

        let mut initial_stats = SessionStats::default();
        let mut models = Vec::new();

        if sessions.is_empty() {
            let demo_id = "demo-welcome-session".to_string();
            sessions.push(SessionSummary {
                id: demo_id.clone(),
                name: Some("Architecture Overview".to_string()),
                first_message: Some("Summarize project structure and build workflow".to_string()),
                cwd: Some(cwd.to_string_lossy().into_owned()),
                project_name: cwd
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from),
                updated_at: Some(current_local_hhmm().to_string()),
                created_at: None,
                parent_session: None,
                message_count: 4,
                is_running: false,
                awaiting_input: false,
                pinned: false,
                archived: false,
                workspace_kind: Some(WorkspaceKind::Project),
                path: None,
            });
            active_session_id = Some(demo_id.clone());
            messages = build_demo_messages(&mut ctrl);
            for msg in &messages {
                artifacts.extend(extract_artifacts_from_markdown(&msg.content, Some(&demo_id)));
            }
            todos = vec![
                TodoWidgetItem {
                    id: SharedString::from("t1"),
                    content: SharedString::from("Inspect workspace layout and session scanner"),
                    done: true,
                },
                TodoWidgetItem {
                    id: SharedString::from("t2"),
                    content: SharedString::from("Verify tool call diff & bash output cards"),
                    done: true,
                },
                TodoWidgetItem {
                    id: SharedString::from("t3"),
                    content: SharedString::from("Run cargo test & release bundle"),
                    done: false,
                },
            ];
            models = vec![
                ModelInfo {
                    provider: "anthropic".to_string(),
                    id: "claude-sonnet-4-6".to_string(),
                    name: Some("Claude Sonnet 4.6".to_string()),
                    provider_name: Some("Anthropic".to_string()),
                    reasoning: true,
                    context_window: Some(200_000),
                    enabled: Some(true),
                    thinking_levels: vec![
                        "off".into(),
                        "low".into(),
                        "medium".into(),
                        "high".into(),
                    ],
                    thinking_level_map: HashMap::new(),
                },
            ];
        } else {
            active_session_id = sessions.first().map(|s| s.id.clone());
            if let Some(first) = sessions.first() {
                if let Some(ref first_cwd) = first.cwd {
                    cwd = PathBuf::from(first_cwd);
                }
                active_workspace_kind = first.workspace_kind.unwrap_or(WorkspaceKind::Project);
                if let Some(ref first_path) = first.path {
                    let p = PathBuf::from(first_path);
                    messages = parse_session_jsonl_file(&p, &mut ctrl.next_local_msg_id);
                    initial_stats = compute_stats_from_jsonl(&p, 200_000);
                    todos = extract_todos_from_jsonl(&p);
                    let sid_ref = active_session_id.as_deref();
                    for msg in &messages {
                        artifacts.extend(extract_artifacts_from_markdown(&msg.content, sid_ref));
                    }
                }
            }
        }

        let initial_branch = resolve_git_branch(&cwd);

        let active_artifact_id = artifacts.first().map(|a| a.identifier.clone());

        let appearance = crate::themes::load_appearance_prefs();
        let initial_chat_font_size = appearance.chat_font_size.clamp(9.0, 22.0);
        let initial_ui_font_size = appearance.ui_font_size.clamp(5.0, 21.0);
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

        let app_cws = cx.entity().clone();
        cx.subscribe(&chat_width_slider, move |this, _, ev: &SliderEvent, cx| {
            if let SliderEvent::Change(val) | SliderEvent::Release(val) = ev {
                this.set_chat_content_width(val.start(), cx);
            }
        }).detach();

        let mut state = Self {
            client,
            _sidecar: sidecar,
            sidecar_connected: false,
            controller: ctrl,
            active_page: WorkspacePage::Chat,
            sidebar_collapsed: false,
            right_panel_open: false,
            right_panel_tab: RightPanelTab::Inspector,
            open_right_tabs: Vec::new(),
            right_panel_width: px(440.),
            settings_open: false,
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
            chat_content_width: initial_chat_width,
            chat_font_slider,
            ui_font_slider,
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
            worktrees: Vec::new(),
            active_branch: initial_branch,
            sessions,
            running_session_ids: HashSet::new(),
            unread_session_ids: HashSet::new(),
            collapsed_projects: HashSet::new(),
            archived_projects: HashSet::new(),
            archived_session_ids: HashSet::new(),
            extra_projects: Vec::new(),
            projects_section_collapsed: false,
            chats_section_collapsed: false,
            renaming_session_id: None,
            active_session_id,
            messages,
            branch_siblings: Vec::new(),
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
            todos_hidden: appearance.todos_hidden,
            todos_hidden_session: None,
            retry_status: None,
            draft_restore_text: None,
            pending_composer_mention: None,
            status_banner: None,
        };

        // Kick off initial sidecar probe & refresh + lightweight external session watcher
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            smol::Timer::after(std::time::Duration::from_millis(250)).await;
            let _ = entity.update(cx, |this, cx| {
                this.refresh_from_sidecar(cx);
                this.start_external_jsonl_watcher(cx);
                if this.core_mode == "auto" {
                    this.probe_core_candidates(cx);
                }
            });
        })
        .detach();

        let _ = &mut state;
        state
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

    pub fn close_right_tab(&mut self, tab: RightPanelTab, cx: &mut Context<Self>) {
        self.open_right_tabs.retain(|t| *t != tab);
        if self.open_right_tabs.is_empty() {
            self.right_panel_open = false;
        } else if self.right_panel_tab == tab {
            self.right_panel_tab = *self.open_right_tabs.last().unwrap();
        }
        cx.notify();
    }

    fn apply_running_ids(&mut self, next_running: HashSet<String>) {
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
        std::thread::spawn(move || {
            let _ = std::process::Command::new("git")
                .arg("-C")
                .arg(&cwd)
                .args(&["checkout", &target])
                .output();
        });
        self.active_branch = Some(SharedString::from(branch));
        cx.notify();
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
            todos_hidden: self.todos_hidden,
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
        self.persist_appearance_only();
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

    pub fn set_ui_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.ui_font_size = size.clamp(5.0, 21.0);
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

    pub fn edit_user_message(
        &mut self,
        text: String,
        entry_id: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.draft_restore_text = Some(text);
        if let Some(eid) = entry_id {
            if let Some(sid) = self.active_session_id.clone() {
                if self.sidecar_connected {
                    let client = self.client.clone();
                    let target = eid.to_string();
                    cx.spawn(async move |_, _| {
                        let _ = client.navigate_tree(&sid, &target).await;
                    })
                    .detach();
                }
            }
        }
        cx.notify();
    }

    pub fn active_subagent_runs(&self) -> Vec<SubagentRunInfo> {
        let mut merged = self.subagent_runs.clone();
        for tool_run in &self.tool_subagent_runs {
            if !merged.iter().any(|r| r.id == tool_run.id) {
                merged.push(tool_run.clone());
            }
        }
        merged
    }

    #[allow(dead_code)]
    pub fn active_subagent_count(&self) -> usize {
        self.active_subagent_runs()
            .iter()
            .filter(|r| r.is_active())
            .count()
    }

    pub fn refresh_from_sidecar(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let active_sid = self.active_session_id.clone();
        let entity = cx.entity().clone();

        cx.spawn(async move |_, cx| {
            let health_ok = client.health().await.is_ok();
            if !health_ok {
                let _ = entity.update(cx, |this, cx| {
                    this.sidecar_connected = false;
                    cx.notify();
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
                Some(ref sid) if sid != "demo-welcome-session" => {
                    client.get_agent_state(sid).await.ok()
                }
                _ => None,
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
                        if !parsed.is_empty() {
                            this.sessions = parsed;
                        }
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
                if let Some(sid) = this.active_session_id.clone() {
                    if sid != "demo-welcome-session" {
                        if this.messages.is_empty() {
                            this.select_session(sid, cx);
                        } else if this.controller.sse_task.is_none() {
                            this.connect_session_sse(sid, cx);
                        }
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
        if !self.sidecar_connected || session_id == "demo-welcome-session" {
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
                if let Some(map_obj) = maps_obj.and_then(|o| o.get(&key)).and_then(Value::as_object) {
                    for (k, v) in map_obj {
                        if let Some(lbl) = v.as_str() {
                            m.thinking_level_map.insert(k.clone(), lbl.to_string());
                        }
                    }
                }
            }
            if !parsed.is_empty() {
                self.models = parsed;
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
            let mut out = Vec::new();
            for p in arr {
                let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
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
                if !id.is_empty() {
                    out.push((id, name, configured));
                }
            }
            self.auth_providers = out;
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
            self.oauth_providers = out;
        }
        if self.auth_providers.is_empty() {
            if let Some(arr) = val.get("apiKeyProviders").and_then(Value::as_array) {
                let mut out = Vec::new();
                for p in arr {
                    let id = p.get("id").and_then(Value::as_str).unwrap_or("").to_string();
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
                    if !id.is_empty() {
                        out.push((id, name, configured));
                    }
                }
                self.auth_providers = out;
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

            let content = path_opt
                .and_then(|p| std::fs::read_to_string(p).ok())
                .or_else(|| v.get("content").and_then(Value::as_str).map(String::from))
                .unwrap_or_default();

            parsed.push(ArtifactRecord {
                identifier: id,
                title,
                kind,
                language: Some(ext.to_string()),
                content,
                previous_content: None,
                version,
                session_id,
                updated_at,
                favorite,
            });
        }
        if !parsed.is_empty() {
            self.artifacts = parsed;
        }
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
        if !self.sidecar_connected {
            return;
        }
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
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
        if !self.sidecar_connected {
            return;
        }
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn delete_configured_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected {
            return;
        }
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
        if let Some(provs) = config.get_mut("providers").and_then(Value::as_object_mut) {
            provs.remove(&provider_id);
        }
        self.expanded_configured_providers.remove(&provider_id);
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn delete_model_from_configured_provider(
        &mut self,
        provider_id: String,
        model_id: String,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected {
            return;
        }
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn add_models_to_configured_provider(
        &mut self,
        provider_id: String,
        models: Vec<ConfiguredModelInfo>,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected || models.is_empty() {
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn add_model_to_configured_provider(
        &mut self,
        provider_id: String,
        model: ConfiguredModelInfo,
        cx: &mut Context<Self>,
    ) {
        if !self.sidecar_connected {
            return;
        }
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let _ = client.put_models_config(config).await;
            let _ = entity.update(cx, |s, cx| {
                s.refresh_from_sidecar(cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn discover_models_for_provider(&mut self, provider_id: String, cx: &mut Context<Self>) {
        if !self.sidecar_connected || self.discovering_provider_id.is_some() {
            return;
        }
        let provider_raw = self
            .configured_providers_raw
            .get("providers")
            .and_then(|p| p.get(&provider_id))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut config = if self.configured_providers_raw.is_object() {
            self.configured_providers_raw.clone()
        } else {
            json!({ "providers": {} })
        };
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
            let mut last_mtime: Option<(String, std::time::SystemTime)> = None;
            let mut last_root_mtime: Option<std::time::SystemTime> = None;
            loop {
                smol::Timer::after(std::time::Duration::from_millis(1500)).await;
                let mut check_path: Option<(String, PathBuf)> = None;
                let mut is_running = false;
                let mut alive = false;
                entity.update(cx, |this, _| {
                    alive = true;
                    is_running = this.is_agent_running;
                    if let Some(sid) = this.active_session_id.as_deref() {
                        if let Some(sess) = this.sessions.iter().find(|s| s.id == sid) {
                            if let Some(p) = &sess.path {
                                check_path = Some((sid.to_string(), PathBuf::from(p)));
                            }
                        }
                    }
                });
                if !alive {
                    break;
                }
                let root = pi_sessions_root();
                if let Ok(meta) = std::fs::metadata(&root) {
                    if let Ok(mt) = meta.modified() {
                        if last_root_mtime != Some(mt) {
                            let prev = last_root_mtime;
                            last_root_mtime = Some(mt);
                            if prev.is_some() {
                                let _ = entity.update(cx, |this, cx| {
                                    let scanned = scan_local_sessions(&root);
                                    if !scanned.is_empty() && this.sessions.len() != scanned.len() {
                                        this.sessions = scanned;
                                        cx.notify();
                                    }
                                });
                            }
                        }
                    }
                }
                if is_running {
                    continue;
                }
                if let Some((sid, path)) = check_path {
                    if let Ok(meta) = std::fs::metadata(&path) {
                        if let Ok(mt) = meta.modified() {
                            let changed = last_mtime
                                .as_ref()
                                .map(|(id, prev)| id != &sid || *prev != mt)
                                .unwrap_or(false);
                            let same_sid = last_mtime
                                .as_ref()
                                .map(|(id, _)| id == &sid)
                                .unwrap_or(false);
                            last_mtime = Some((sid.clone(), mt));
                            if changed && same_sid {
                                let _ = entity.update(cx, |this, cx| {
                                    if this.active_session_id.as_deref() == Some(sid.as_str())
                                        && !this.is_agent_running
                                    {
                                        let parsed = parse_session_jsonl_file(
                                            &path,
                                            &mut this.controller.next_local_msg_id,
                                        );
                                        if !parsed.is_empty()
                                            && (parsed.len() != this.messages.len()
                                                || parsed.last().map(|m| &m.content)
                                                    != this.messages.last().map(|m| &m.content))
                                        {
                                            this.messages = parsed;
                                            let cw = this
                                                .selected_model
                                                .as_ref()
                                                .and_then(|m| m.context_window)
                                                .unwrap_or(200_000);
                                            this.stats = compute_stats_from_jsonl(&path, cw);
                                            this.set_todos(extract_todos_from_jsonl(&path));
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        }
                    }
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
            self.is_agent_running = streaming;
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
                    self.set_todos(parse_todo_widget_lines(&lines));
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        let url = client.running_events_url();
        let http = client.raw_http();
        let token = client.token.clone();
        let instance_id = client.instance_id.clone();

        cx.spawn(async move |_, cx| {
            loop {
                let res = crate::client::sse::consume_json_sse(
                    http.clone(),
                    url.clone(),
                    token.clone(),
                    instance_id.clone(),
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
                                // session: it both starts and *ends* a run. Only setting
                                // it here left `is_agent_running` pinned true, which hid a
                                // settled turn's footer and modified-files summary.
                                if let Some(sid) = this.active_session_id.clone() {
                                    let running = this.running_session_ids.contains(&sid);
                                    this.is_agent_running = running;
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

                let mut keep_going = false;
                let _ = entity.update(cx, |this, _| {
                    keep_going = this.sidecar_connected;
                    if !keep_going {
                        this.running_events_subscribed = false;
                    }
                });
                if !keep_going {
                    break;
                }
                let delay_ms = if res.is_err() { 1500 } else { 800 };
                smol::Timer::after(std::time::Duration::from_millis(delay_ms)).await;
            }
        })
        .detach();
    }

    pub fn start_new_composer(&mut self, workspace_kind: WorkspaceKind, cx: &mut Context<Self>) {
        self.controller.next_load_id();
        self.controller.stop_sse();
        self.active_session_id = None;
        self.active_workspace_kind = workspace_kind;
        self.active_page = WorkspacePage::Chat;
        self.messages.clear();
        self.branch_siblings.clear();
        self.is_agent_running = false;
        self.is_compacting = false;
        self.retry_status = None;
        self.pending_ui_request = None;
        self.dialog_queue.clear();
        self.questionnaire_request_id = None;
        self.todos.clear();
        self.todos_expanded = false;
        self.dismissed_todo_ids.clear();
        
        
        self.stats = SessionStats::default();
        self.subagent_runs.clear();
        self.tool_subagent_runs.clear();
        self.subagent_status_text = None;
        cx.notify();
    }

    /// Monotonic request_id guarded session selection (per AGENTS.md hard rule).
    pub fn select_session(&mut self, session_id: String, cx: &mut Context<Self>) {
        let req_id = self.controller.next_load_id();
        self.controller.stop_sse();
        self.active_session_id = Some(session_id.clone());
        self.unread_session_ids.remove(&session_id);
        self.active_page = WorkspacePage::Chat;
        self.pending_ui_request = None;
        self.dialog_queue.clear();
        self.questionnaire_request_id = None;
        self.is_agent_running = self.running_session_ids.contains(&session_id);
        self.is_compacting = false;
        self.retry_status = None;
        self.subagent_runs.clear();
        self.tool_subagent_runs.clear();
        self.subagent_status_text = None;
        self.todos.clear();
        self.todos_expanded = false;
        self.stats = SessionStats::default();

        let mut local_jsonl_path: Option<PathBuf> = None;
        if let Some(summary) = self.sessions.iter().find(|s| s.id == session_id) {
            if let Some(cwd) = &summary.cwd {
                self.active_cwd = PathBuf::from(cwd);
                self.active_branch = resolve_git_branch(&self.active_cwd);
            }
            if let Some(p) = &summary.path {
                local_jsonl_path = Some(PathBuf::from(p));
            }
            self.active_workspace_kind = summary.workspace_kind.unwrap_or(WorkspaceKind::Project);
        }

        if session_id == "demo-welcome-session" {
            self.messages = build_demo_messages(&mut self.controller);
            cx.notify();
            return;
        }

        let ctx_win = self
            .selected_model
            .as_ref()
            .and_then(|m| m.context_window)
            .unwrap_or(200_000);

        // Fast local JSONL load so switching sessions is instant and works offline
        if let Some(ref p) = local_jsonl_path {
            let parsed = parse_session_jsonl_file(p, &mut self.controller.next_local_msg_id);
            if !parsed.is_empty() {
                self.messages = parsed;
            }
            self.stats = compute_stats_from_jsonl(p, ctx_win);
            self.set_todos(extract_todos_from_jsonl(p));

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
        }

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
                    let parsed =
                        parse_session_messages(&payload, &mut this.controller.next_local_msg_id);
                    if !parsed.is_empty() {
                        this.messages = parsed;
                    }
                    if let Some(payload_todos) = extract_todos_from_payload(&payload) {
                        this.set_todos(payload_todos);
                    }
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
                    this.artifacts.clear();
                    for msg in &this.messages {
                        this.artifacts.extend(extract_artifacts_from_markdown(
                            &msg.content,
                            Some(&session_id),
                        ));
                    }
                    if this.active_artifact_id.is_none() {
                        this.active_artifact_id =
                            this.artifacts.first().map(|a| a.identifier.clone());
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
        let client = self.client.clone();
        let entity = cx.entity().clone();
        let url = client.agent_events_url(&session_id);
        let http = client.raw_http();
        let token = client.token.clone();
        let instance_id = client.instance_id.clone();

        let task = cx.spawn(async move |_, cx| {
            let sid = session_id.clone();
            loop {
                let res = consume_agent_sse(
                    http.clone(),
                    url.clone(),
                    token.clone(),
                    instance_id.clone(),
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

                let mut should_retry = false;
                let _ = entity.update(cx, |this, _| {
                    should_retry = this.sidecar_connected
                        && this.active_session_id.as_deref() == Some(sid.as_str());
                });
                if !should_retry {
                    break;
                }
                let delay_ms = if res.is_err() { 1200 } else { 800 };
                smol::Timer::after(std::time::Duration::from_millis(delay_ms)).await;
            }
        });
        self.controller.sse_task = Some(task);
    }

    fn apply_sse_event(&mut self, event: AgentSseEvent, cx: &mut Context<Self>) {
        match event {
            AgentSseEvent::Connected { is_streaming } => {
                if is_streaming {
                    self.is_agent_running = true;
                    if let Some(sid) = self.active_session_id.clone() {
                        self.sync_active_agent_state(sid, cx);
                    }
                }
            }
            AgentSseEvent::AgentStart | AgentSseEvent::TurnStart => {
                self.is_agent_running = true;
            }
            AgentSseEvent::AgentEnd { duration_ms }
            | AgentSseEvent::TurnEnd { duration_ms, .. } => {
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
                let was_running = self.is_agent_running;
                self.is_agent_running = false;
                self.is_compacting = false;
                self.retry_status = None;
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
                                    let parsed = parse_session_messages(
                                        &payload,
                                        &mut this.controller.next_local_msg_id,
                                    );
                                    if !parsed.is_empty() {
                                        this.messages = parsed;
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
                        if !user_text.trim().is_empty() {
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
                        let was_streaming = last.is_streaming;
                        if !last.content.trim().is_empty() && !was_streaming {
                            let intermediate = std::mem::take(&mut last.content);
                            last.process_steps
                                .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                        }
                        if !snap_text.is_empty() {
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
                            last.content.clear();
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
                                last.content = full.to_string();
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
                        let tool_name = msg_val
                            .get("toolName")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let is_error = msg_val
                            .get("isError")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let output_text = flatten_content_text(msg_val.get("content"));

                        if let Some(last) = self.messages.last_mut() {
                            for tc in last.tool_calls.iter_mut().rev() {
                                if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                                    tc.is_running = false;
                                    tc.is_error = is_error;
                                    if !output_text.is_empty() {
                                        tc.output = SharedString::from(output_text.clone());
                                    }
                                    break;
                                }
                            }
                            for step in last.process_steps.iter_mut().rev() {
                                if let ProcessStep::ToolCall(tc) = step {
                                    if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                                        tc.is_running = false;
                                        tc.is_error = is_error;
                                        if !output_text.is_empty() {
                                            tc.output = SharedString::from(output_text.clone());
                                        }
                                        break;
                                    }
                                }
                            }
                        }

                        if !is_error && (tool_name == "todo" || msg_val.get("details").and_then(|d| d.get("tasks")).is_some()) {
                            let updated_from_details = msg_val
                                .get("details")
                                .and_then(parse_todo_tool_details)
                                .map(|items| {
                                    self.set_todos(items);
                                })
                                .is_some();
                            if !updated_from_details && !output_text.is_empty() {
                                apply_todo_tool_output_text(&mut self.todos, &output_text);
                            }
                            if let Some(sid) = self.active_session_id.clone() {
                                self.sync_active_agent_state(sid, cx);
                            }
                        }
                    }
                    cx.notify();
                    return;
                }
                if role == "user" {
                    if let Some(msg_val) = message.as_ref() {
                        let user_text = flatten_content_text(msg_val.get("content"));
                        if !user_text.trim().is_empty() {
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
                            if let Some(sr) = msg_val.get("stopReason").and_then(Value::as_str) {
                                last.stop_reason = Some(SharedString::from(sr.to_string()));
                            }
                            if let Some(em) = msg_val
                                .get("errorMessage")
                                .or_else(|| msg_val.get("error"))
                                .and_then(Value::as_str)
                                .filter(|s| !s.trim().is_empty())
                            {
                                last.error_message = Some(SharedString::from(em.to_string()));
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
                                    last.content = completed.content;
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
                    if let Some(ref a) = args {
                        apply_optimistic_todo_args(&mut self.todos, a);
                        self.prune_dismissed_todos();
                    }
                }
                if tool_name == "subagent" || tool_name == "subagent_launch" {
                    let agent_lbl = args
                        .as_ref()
                        .and_then(|a| {
                            a.get("agent")
                                .and_then(Value::as_str)
                                .or_else(|| a.get("key").and_then(Value::as_str))
                        })
                        .unwrap_or("subagent")
                        .to_string();
                    let task_desc = args
                        .as_ref()
                        .and_then(|a| a.get("task").and_then(Value::as_str))
                        .map(|t| t.chars().take(80).collect::<String>());
                    let model = args
                        .as_ref()
                        .and_then(|a| a.get("model").and_then(Value::as_str))
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
                        session_id: None,
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
                if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
                    if let Some(details) = partial_result.as_ref().and_then(|v| v.get("details")) {
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
                    let out_str = partial_result.map(|v| v.to_string()).unwrap_or_default();
                    if let Some(tc) = last.tool_calls.iter_mut().find(|t| t.id.as_ref() == tool_call_id) {
                        tc.output = SharedString::from(out_str.clone());
                    }
                    for step in &mut last.process_steps {
                        if let ProcessStep::ToolCall(tc) = step {
                            if tc.id.as_ref() == tool_call_id {
                                tc.output = SharedString::from(out_str.clone());
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
                    if let Some(details) = result.as_ref().and_then(|v| v.get("details")) {
                        if let Some(parsed) = parse_todo_tool_details(details) {
                            self.set_todos(parsed);
                        }
                    }
                    if let Some(sid) = self.active_session_id.clone() {
                        self.sync_active_agent_state(sid, cx);
                    }
                }
                if let Some(run) = self.tool_subagent_runs.iter_mut().find(|r| r.id == tool_call_id) {
                    run.state = if is_error { "failed" } else { "complete" }.to_string();
                    run.current_tool = None;
                }
                if let Some(last) = self.messages.last_mut() {
                    let details_val = result.as_ref().and_then(|v| v.get("details"));
                    let extra_diff = if tool_name == "edit" {
                        build_diff_from_tool(&tool_name, None, details_val)
                    } else {
                        None
                    };
                    let text = result
                        .map(|val| {
                            val.get("content")
                                .and_then(Value::as_array)
                                .and_then(|a| a.first())
                                .and_then(|c| c.get("text"))
                                .and_then(Value::as_str)
                                .map(String::from)
                                .unwrap_or_else(|| val.to_string())
                        })
                        .unwrap_or_default();
                    if let Some(tc) = last.tool_calls.iter_mut().find(|t| t.id.as_ref() == tool_call_id) {
                        tc.is_running = false;
                        tc.is_error = is_error;
                        if tc.diff_patch.is_none() {
                            tc.diff_patch = extra_diff.clone();
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
                    } else if key.contains("todo") {
                        self.set_todos(parse_todo_widget_lines(&lines));
                    }
                } else if method == "setStatus" {
                    let key = payload.get("statusKey").and_then(Value::as_str).unwrap_or("");
                    if key.starts_with("subagent") {
                        self.subagent_status_text = payload
                            .get("statusText")
                            .and_then(Value::as_str)
                            .map(String::from);
                    }
                } else if method == "notify" {
                    if let Some(msg) = payload.get("message").and_then(Value::as_str) {
                        let notify_type = payload.get("notifyType").and_then(Value::as_str).unwrap_or("info");
                        if notify_type == "error" || notify_type == "warning" {
                            self.status_banner = Some(SharedString::from(msg.to_string()));
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
                    let is_blocking_dialog = req.method == "select"
                        || req.method == "input"
                        || req.method == "confirm"
                        || req.method == "ask_user_question";
                    if is_blocking_dialog && self.awaiting_ask_user_question() {
                        self.questionnaire_request_id = Some(req.request_id.clone());
                    }
                    if req.method == "ask_user_question" {
                        self.pending_ui_request = Some(req);
                    } else {
                        self.push_extension_dialog(req);
                    }
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
                if success == Some(false) {
                    if let Some(err) = final_error.or(error_message).filter(|s| !s.trim().is_empty()) {
                        self.record_assistant_error(err);
                    }
                }
            }
            AgentSseEvent::PromptError { error_message }
            | AgentSseEvent::StartupError { error_message } => {
                self.is_agent_running = false;
                self.retry_status = None;
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
    fn push_extension_dialog(&mut self, req: ExtensionUiRequest) {
        if !self
            .pending_ui_request
            .as_ref()
            .map(|p| p.request_id == req.request_id)
            .unwrap_or(false)
            && !self.dialog_queue.iter().any(|r| r.request_id == req.request_id)
        {
            self.dialog_queue.push(req);
        }
        if self.pending_ui_request.is_none() && !self.dialog_queue.is_empty() {
            self.pending_ui_request = Some(self.dialog_queue.remove(0));
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

    fn record_assistant_error(&mut self, err: String) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == MessageRole::Assistant {
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
        if self.active_session_id.as_deref() == Some(id.as_str()) {
            self.start_new_composer(self.active_workspace_kind, cx);
        }
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
            self.status_banner = Some(SharedString::from("Fork requires active sidecar connection."));
            cx.notify();
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
        if self.sidecar_connected {
            let client = self.client.clone();
            let entity = cx.entity().clone();
            cx.spawn(async move |_, cx| {
                let _ = client.create_scheduled_task_payload(payload).await;
                let _ = entity.update(cx, |s, cx| {
                    s.refresh_scheduled_tasks(cx);
                });
            })
            .detach();
        }
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

        // If sidecar is offline, simulate a local mock streaming reply so Phase 1 UI
        // can be exercised interactively without Node.
        if !self.sidecar_connected {
            self.controller.send_in_flight = false;
            self.is_agent_running = true;
            let reply_id = self.controller.alloc_msg_id();
            let model_label = self.selected_model.as_ref().map(ModelInfo::display_label);
            let think_str = SharedString::from("Preparing response preview (sidecar not connected)...");
            self.messages.push(ChatMessage {
                id: reply_id,
                entry_id: None,
                role: MessageRole::Assistant,
                model_label,
                timestamp: SharedString::from("Offline Preview"),
                thinking: think_str.clone(),
                process_steps: vec![ProcessStep::Thinking(think_str)],
                process_message_count: 1,
                thinking_ms: Some(120),
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

            let entity = cx.entity().clone();
            let reply_text = format!(
                "Received (`{}`):\n\n> {}\n\nSidecar is not running yet. Build and start it via:\n```bash\nnode pi-gpui/sidecar/build-sidecar.mjs\nnode pi-gpui/sidecar/api-sidecar.mjs\n```",
                self.active_cwd.display(),
                text.replace('\n', "\n> ")
            );
            cx.spawn(async move |_, cx| {
                for chunk in reply_text.as_bytes().chunks(18) {
                    smol::Timer::after(std::time::Duration::from_millis(24)).await;
                    let s = String::from_utf8_lossy(chunk).into_owned();
                    let _ = entity.update(cx, |this, cx| {
                        if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                            m.content.push_str(&s);
                        }
                        cx.notify();
                    });
                }
                let _ = entity.update(cx, |this, cx| {
                    if let Some(m) = this.messages.iter_mut().find(|m| m.id == reply_id) {
                        m.is_streaming = false;
                        m.timestamp = current_local_hhmm();
                        m.turn_duration_ms = Some(640);
                    }
                    this.is_agent_running = false;
                    cx.notify();
                });
            })
            .detach();
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
                    client
                        .start_new_session(
                            &cwd,
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
                                this.connect_session_sse(new_id.to_string(), cx);
                                this.refresh_from_sidecar(cx);
                            }
                        }
                        this.is_agent_running = true;
                    }
                    Err(err) => {
                        // Restore text into composer on send failure & show clean inline error
                        if this.messages.last().map(|m| m.id) == Some(msg_id) {
                            this.messages.pop();
                        }
                        this.draft_restore_text = Some(text);
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
        self.open_file_diff_patch = diff_patch;
        self.open_right_tab(RightPanelTab::FileViewer, cx);
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let content = smol::unblock(move || {
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        if bytes.len() > 2 * 1024 * 1024 {
                            format!("File too large to preview ({} bytes)", bytes.len())
                        } else {
                            String::from_utf8(bytes)
                                .unwrap_or_else(|_| "<Binary file>".to_string())
                        }
                    }
                    Err(e) => format!("Failed to read file: {e}"),
                }
            })
            .await;
            let _ = entity.update(cx, |this, cx| {
                this.open_file_content = SharedString::from(content);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.refresh_settings_data(cx);
        cx.notify();
    }

    pub fn refresh_settings_data(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        let cwd_str = self.active_cwd.to_string_lossy().into_owned();
        let entity = cx.entity().clone();
        cx.spawn(async move |_, cx| {
            let health_ok = client.health().await.is_ok();
            if !health_ok {
                let _ = entity.update(cx, |this, cx| {
                    this.sidecar_connected = false;
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
    let mut dir = cwd;
    loop {
        let head = dir.join(".git").join("HEAD");
        if let Ok(raw) = std::fs::read_to_string(&head) {
            let trimmed = raw.trim();
            if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
                return Some(SharedString::from(branch.to_string()));
            }
            if !trimmed.is_empty() {
                let short: String = trimmed.chars().take(7).collect();
                return Some(SharedString::from(short));
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    None
}

fn build_demo_messages(ctrl: &mut SessionController) -> Vec<ChatMessage> {
    let id1 = ctrl.alloc_msg_id();
    let id2 = ctrl.alloc_msg_id();
    let id3 = ctrl.alloc_msg_id();
    let id4 = ctrl.alloc_msg_id();
    let tc_bash = ToolCallItem {
        id: SharedString::from("tc-demo-bash"),
        name: SharedString::from("bash"),
        arguments: vec![(
            SharedString::from("command"),
            SharedString::from("cargo test --quiet"),
        )],
        output: SharedString::from("test result: ok. 4 passed; 0 failed"),
        diff_patch: None,
        is_running: false,
        is_error: false,
        expanded: false,
    };
    let tc_edit = ToolCallItem {
        id: SharedString::from("tc-demo-edit"),
        name: SharedString::from("edit"),
        arguments: vec![(
            SharedString::from("path"),
            SharedString::from("src/main.rs"),
        )],
        output: SharedString::from("Updated configuration"),
        diff_patch: Some(SharedString::from(
            "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n fn main() {\n-    start_default();\n+    start_agent_session();\n }\n",
        )),
        is_running: false,
        is_error: false,
        expanded: false,
    };
    let tc_write = ToolCallItem {
        id: SharedString::from("tc-demo-write"),
        name: SharedString::from("write"),
        arguments: vec![
            (
                SharedString::from("path"),
                SharedString::from("config.toml"),
            ),
            (
                SharedString::from("content"),
                SharedString::from("[agent]\nmodel = \"claude-sonnet-4-6\"\nthinking = \"medium\"\n"),
            ),
        ],
        output: SharedString::from("Wrote 3 lines to config.toml"),
        diff_patch: None,
        is_running: false,
        is_error: false,
        expanded: false,
    };
    vec![
        ChatMessage {
            id: id1,
            entry_id: Some(SharedString::from("demo-e1")),
            role: MessageRole::User,
            model_label: None,
            timestamp: SharedString::from("09:58"),
            thinking: SharedString::default(),
            process_steps: Vec::new(),
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: "Summarize the agent architecture and check the sidecar setup.".to_string(),
            tool_calls: Vec::new(),
            attachments: vec![AttachmentItem {
                name: SharedString::from("README.md"),
                mime: SharedString::from("text/markdown"),
                size_bytes: 14_949,
            }],
            is_streaming: false,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: None,
            stop_reason: None,
            error_message: None,
        },
        ChatMessage {
            id: id2,
            entry_id: Some(SharedString::from("demo-e2")),
            role: MessageRole::Assistant,
            model_label: Some(SharedString::from("Claude Sonnet 4.6")),
            timestamp: SharedString::from("09:59"),
            thinking: SharedString::from("Reviewing session lifecycle, tool execution pipeline, and sidecar endpoints."),
            process_steps: vec![
                ProcessStep::Thinking(SharedString::from(
                    "Reviewing session lifecycle, tool execution pipeline, and sidecar endpoints.",
                )),
                ProcessStep::ToolCall(tc_bash.clone()),
            ],
            process_message_count: 2,
            thinking_ms: Some(420),
            thinking_expanded: false,
            content: r##"### Pi Agent Architecture

Session events stream over SSE while context usage tracks $U = \frac{T_{\text{used}}}{T_{\text{window}}}$.

```rust
fn start_agent_session(cwd: &str) {
    println!("Session started in {cwd}");
}
```

```mermaid
flowchart LR
  UI[Pi Desktop] -->|HTTP + SSE| Sidecar[API Sidecar]
  Sidecar --> SDK[Pi Coding Agent SDK]
  UI --> PTY[Local Terminal]
  UI -->|Lazy Preview| Sandbox[HTML/React Sandbox]
```

:::artifact{identifier="arch-summary-svg" type="svg" title="Memory Budget Overview"}
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 360 140">
  <rect width="360" height="140" rx="12" fill="#18181b"/>
  <rect x="20" y="24" width="145" height="92" rx="8" fill="#27272a" stroke="#3f3f46"/>
  <text x="36" y="54" fill="#a1a1aa" font-family="sans-serif" font-size="12">Desktop Client</text>
  <text x="36" y="88" fill="#4ade80" font-family="monospace" font-size="24" font-weight="bold">&lt; 120 MB</text>
  <rect x="195" y="24" width="145" height="92" rx="8" fill="#27272a" stroke="#3f3f46"/>
  <text x="211" y="54" fill="#a1a1aa" font-family="sans-serif" font-size="12">API Sidecar</text>
  <text x="211" y="88" fill="#60a5fa" font-family="monospace" font-size="24" font-weight="bold">~ 55 MB</text>
</svg>
```
:::
"##.to_string(),
            tool_calls: vec![tc_bash],
            attachments: Vec::new(),
            is_streaming: false,
            turn_duration_ms: Some(1_140),
            token_summary: Some(SharedString::from("1.4k in / 320 out · $0.008")),
            turn_usage: Some(TurnUsage {
                input_tokens: 1_400,
                output_tokens: 320,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 1_720,
                cost_usd: 0.008,
            }),
            stop_reason: None,
            error_message: None,
        },
        ChatMessage {
            id: id3,
            entry_id: Some(SharedString::from("demo-e3")),
            role: MessageRole::User,
            model_label: None,
            timestamp: SharedString::from("10:02"),
            thinking: SharedString::default(),
            process_steps: Vec::new(),
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: "Update main.rs and add a default config note.".to_string(),
            tool_calls: Vec::new(),
            attachments: Vec::new(),
            is_streaming: false,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: None,
            stop_reason: None,
            error_message: None,
        },
        ChatMessage {
            id: id4,
            entry_id: Some(SharedString::from("demo-e4")),
            role: MessageRole::Assistant,
            model_label: Some(SharedString::from("Claude Sonnet 4.6")),
            timestamp: SharedString::from("10:03"),
            thinking: SharedString::from("Checking src/main.rs and config.toml before applying edits."),
            process_steps: vec![
                ProcessStep::Thinking(SharedString::from(
                    "Checking src/main.rs and config.toml before applying edits.",
                )),
                ProcessStep::ToolCall(tc_edit.clone()),
                ProcessStep::ToolCall(tc_write.clone()),
            ],
            process_message_count: 3,
            thinking_ms: Some(310),
            thinking_expanded: false,
            content: "Applied the update to `src/main.rs` and created `config.toml`.".to_string(),
            tool_calls: vec![tc_edit, tc_write],
            attachments: Vec::new(),
            is_streaming: false,
            turn_duration_ms: Some(890),
            token_summary: Some(SharedString::from("980 in / 140 out · $0.004")),
            turn_usage: Some(TurnUsage {
                input_tokens: 980,
                output_tokens: 140,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 1_120,
                cost_usd: 0.004,
            }),
            stop_reason: None,
            error_message: None,
        },
    ]
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
}

