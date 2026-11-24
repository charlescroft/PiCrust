use gpui_kit::SharedString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    XHigh,
}

impl ThinkingLevel {
    #[allow(dead_code)]
    pub fn all() -> &'static [Self] {
        &[
            Self::Off,
            Self::Minimal,
            Self::Low,
            Self::Medium,
            Self::High,
            Self::XHigh,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" => Some(Self::Off),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" | "max" => Some(Self::XHigh),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "XHigh",
        }
    }

    pub fn description_key(&self) -> &'static str {
        match self {
            Self::Off => "thinking.offDesc",
            Self::Minimal => "thinking.minimalDesc",
            Self::Low => "thinking.lowDesc",
            Self::Medium => "thinking.mediumDesc",
            Self::High => "thinking.highDesc",
            Self::XHigh => "thinking.xhighDesc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolPreset {
    None,
    ReadOnly,
    #[default]
    Default,
    Full,
}

impl ToolPreset {
    pub fn all() -> &'static [Self] {
        &[Self::None, Self::ReadOnly, Self::Default, Self::Full]
    }

    pub fn label_key(&self) -> &'static str {
        match self {
            Self::None => "tools.noneLabel",
            Self::ReadOnly => "tools.readonlyLabel",
            Self::Default => "tools.defaultLabel",
            Self::Full => "tools.fullLabel",
        }
    }

    pub fn description_key(&self) -> &'static str {
        match self {
            Self::None => "tools.noneDesc",
            Self::ReadOnly => "tools.readonlyDesc",
            Self::Default => "tools.defaultDesc",
            Self::Full => "tools.fullDesc",
        }
    }

    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "Chat only",
            Self::ReadOnly => "Read-only",
            Self::Default => "Default",
            Self::Full => "Full",
        }
    }

    pub fn tool_names(&self) -> Vec<&'static str> {
        match self {
            Self::None => vec![],
            Self::ReadOnly => vec!["read", "grep", "find", "ls"],
            Self::Default => vec!["read", "bash", "edit", "write", "grep", "find", "ls"],
            Self::Full => vec![
                "read", "bash", "edit", "write", "grep", "find", "ls", "subagent", "codemode",
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendShortcut {
    #[default]
    Enter,
    CmdEnter,
    ShiftEnter,
}

impl SendShortcut {
    pub fn all() -> &'static [Self] {
        &[Self::Enter, Self::CmdEnter, Self::ShiftEnter]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::CmdEnter => "cmd_enter",
            Self::ShiftEnter => "shift_enter",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "cmd_enter" | "ctrl_enter" => Self::CmdEnter,
            "shift_enter" => Self::ShiftEnter,
            _ => Self::Enter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SteeringMode {
    #[default]
    All,
    OneAtATime,
}

impl SteeringMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::OneAtATime => "one-at-a-time",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceKind {
    #[default]
    Project,
    Chat,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRelation {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub first_message: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default, alias = "modified")]
    pub updated_at: Option<String>,
    #[serde(default, alias = "created")]
    pub created_at: Option<String>,
    #[serde(default)]
    pub parent_session: Option<String>,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub is_running: bool,
    #[serde(default)]
    pub awaiting_input: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub workspace_kind: Option<WorkspaceKind>,
    #[serde(default)]
    pub relation: Option<SessionRelation>,
    #[serde(default)]
    pub path: Option<String>,
}

impl SessionSummary {
    pub fn is_subagent(&self) -> bool {
        self.relation
            .as_ref()
            .and_then(|r| r.kind.as_deref())
            == Some("subagent")
    }
    pub fn display_title(&self) -> SharedString {
        if let Some(name) = &self.name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return SharedString::from(trimmed.to_string());
            }
        }
        if let Some(first) = &self.first_message {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                let single_line: String = trimmed
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(72)
                    .collect();
                if !single_line.is_empty() {
                    return SharedString::from(single_line);
                }
            }
        }
        SharedString::from(format!("Session {}", &self.id.chars().take(8).collect::<String>()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Notice,
}

#[derive(Debug, Clone)]
pub struct ToolCallItem {
    pub id: SharedString,
    pub name: SharedString,
    pub arguments: Vec<(SharedString, SharedString)>,
    pub output: SharedString,
    pub diff_patch: Option<SharedString>,
    pub subagent_session_id: Option<SharedString>,
    pub is_running: bool,
    pub is_error: bool,
    pub expanded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AttachmentItem {
    pub name: SharedString,
    pub mime: SharedString,
    pub size_bytes: usize,
    #[serde(default)]
    pub local_path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub base64_data: Option<String>,
}

impl AttachmentItem {
    pub fn is_image(&self) -> bool {
        self.mime.starts_with("image/")
    }
}

#[derive(Debug, Clone)]
pub struct CompactionStep {
    pub summary_markdown: String,
    pub tokens_before: Option<u64>,
    pub read_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub is_running: bool,
}

#[derive(Debug, Clone)]
pub enum ProcessStep {
    Thinking(SharedString),
    ToolCall(ToolCallItem),
    Compaction(CompactionStep),
}

#[derive(Debug, Clone, Default)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: u64,
    pub entry_id: Option<SharedString>,
    pub role: MessageRole,
    pub model_label: Option<SharedString>,
    pub timestamp: SharedString,
    pub thinking: SharedString,
    pub process_steps: Vec<ProcessStep>,
    pub process_message_count: usize,
    #[allow(dead_code)]
    pub thinking_ms: Option<u64>,
    #[allow(dead_code)]
    pub thinking_expanded: bool,
    pub content: String,
    pub tool_calls: Vec<ToolCallItem>,
    pub attachments: Vec<AttachmentItem>,
    pub is_streaming: bool,
    pub turn_duration_ms: Option<u64>,
    pub token_summary: Option<SharedString>,
    pub turn_usage: Option<TurnUsage>,
    pub stop_reason: Option<SharedString>,
    pub error_message: Option<SharedString>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub provider: String,
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider_name: Option<String>,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub thinking_levels: Vec<String>,
    #[serde(default)]
    pub thinking_level_map: HashMap<String, String>,
}

impl ModelInfo {
    pub fn display_label(&self) -> SharedString {
        let name = self.name.as_deref().unwrap_or(&self.id);
        SharedString::from(format!("{name} ({})", self.provider))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub is_main: bool,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSibling {
    pub entry_id: String,
    pub label: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedPrompt {
    pub id: u64,
    pub text: String,
    pub attachments: Vec<AttachmentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionChoice {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionDefinition {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default)]
    pub options: Vec<QuestionChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionUiRequest {
    #[serde(alias = "id")]
    pub request_id: String,
    pub method: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub prefill: Option<String>,
    #[serde(default)]
    pub questions: Vec<QuestionDefinition>,
    /// `select` choices, already rendered as labels by the pi host.
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    Html,
    React,
    Svg,
    Mermaid,
    Markdown,
    Code,
}

impl ArtifactKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Html => "HTML",
            Self::React => "React",
            Self::Svg => "SVG",
            Self::Mermaid => "Mermaid",
            Self::Markdown => "Markdown",
            Self::Code => "Code",
        }
    }

    #[allow(dead_code)]
    pub fn uses_webview(&self) -> bool {
        matches!(self, Self::Html | Self::React)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    pub identifier: String,
    pub title: String,
    pub kind: ArtifactKind,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub file_path: Option<String>,
    pub content: String,
    /// Previous revision, kept for the panel's Diff tab.
    #[serde(default)]
    pub previous_content: Option<String>,
    pub version: usize,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub session_id: String,
    pub title: String,
    pub project: String,
    pub status: String, // "waiting" | "running" | "recent"
    #[serde(default)]
    pub current_step: Option<String>,
    #[serde(default)]
    pub run_started_at: Option<u64>,
    #[serde(default)]
    pub waiting_prompts: Vec<String>,
    #[serde(default)]
    pub last_run_status: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TaskScheduleSpec {
    Manual,
    Cron {
        expr: String,
        #[serde(default)]
        timezone: String,
    },
    Once {
        at: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledModelRef {
    pub provider: String,
    pub model_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledRunView {
    pub run_id: String,
    pub task_id: String,
    pub trigger: String, // "schedule" | "catch-up" | "manual"
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    pub status: String, // "running" | "succeeded" | "failed" | "aborted" | "skipped"
    #[serde(default)]
    pub skip_reason: Option<String>,
    #[serde(default)]
    pub skipped_slots: Option<u64>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub prompt: String,
    pub cwd: String,
    pub schedule: TaskScheduleSpec,
    #[serde(default)]
    pub model: Option<ScheduledModelRef>,
    #[serde(default)]
    pub thinking_level: Option<String>,
    #[serde(default = "default_scheduled_preset")]
    pub tool_preset: String, // "none" | "read-only" | "default" | "full"
    #[serde(default = "default_execution_target", alias = "execution_target")]
    pub execution_target: String, // "new" | "reuse"
    #[serde(default = "default_max_duration_min")]
    pub max_duration_min: u64,
    pub enabled: bool,
    #[serde(default)]
    pub auto_paused_reason: Option<String>,
    #[serde(default)]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub last_run: Option<ScheduledRunView>,
    #[serde(default)]
    pub unread_runs: u64,
}

fn default_scheduled_preset() -> String {
    "read-only".to_string()
}

fn default_execution_target() -> String {
    "new".to_string()
}

fn default_max_duration_min() -> u64 {
    30
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SchedulerOwnerStatus {
    pub owner: bool,
    pub pid: Option<u64>,
    pub heartbeat_at: Option<String>,
    pub started_at: Option<String>,
    pub alive: bool,
    pub fresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerRow {
    pub name: String,
    pub scope: String,
    pub transport: String,
    pub status: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tools_count: usize,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub needs_auth: bool,
    #[serde(default)]
    pub error: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRow {
    pub name: String,
    pub description: String,
    pub source: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub disable_model_invocation: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRow {
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub scope: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub latest_version: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub resource_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentProfileRow {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub scope: String,
    pub built_in: bool,
    pub enabled: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct EnabledModelItem {
    pub id: String,
    pub name: String,
    pub model_ref: String,
    pub enabled: bool,
    pub context_window: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct EnabledProviderGroup {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub kind: String,
    pub enabled_count: usize,
    pub models: Vec<EnabledModelItem>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct EnabledModelsOverview {
    pub all_enabled: bool,
    pub enabled_total: usize,
    pub available_total: usize,
    pub scope: String,
    pub settings_path: String,
    pub editable: bool,
    pub providers: Vec<EnabledProviderGroup>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct NetworkProxyState {
    pub enabled: bool,
    pub url: String,
    pub no_proxy: String,
    pub effective_url: Option<String>,
    pub effective_source: Option<String>,
    pub local: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SubagentChildInfo {
    pub id: String,
    pub label: String,
    pub agent: Option<String>,
    pub state: String,
    pub model: Option<String>,
    pub current_tool: Option<String>,
    pub progress_text: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SubagentRunInfo {
    pub id: String,
    pub kind: String, // "subagent" | "workflow"
    pub label: String,
    pub state: String, // "queued" | "running" | "complete" | "failed" | "partial" | "paused" | "stopped"
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub current_tool: Option<String>,
    pub turn_count: Option<u64>,
    pub tool_count: Option<u64>,
    pub progress_text: Option<String>,
    pub session_id: Option<String>,
    pub children: Vec<SubagentChildInfo>,
}

impl SubagentRunInfo {
    pub fn is_active(&self) -> bool {
        matches!(self.state.as_str(), "running" | "queued" | "pending" | "starting")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
    pub context_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub context_used_pct: f32,
    pub has_context_usage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatFontSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ChatFontSize {
    pub fn all() -> &'static [Self] {
        &[Self::Small, Self::Medium, Self::Large]
    }

    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    pub fn px_value(&self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 13.0,
            Self::Large => 14.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TodoWidgetItem {
    pub id: SharedString,
    pub content: SharedString,
    pub done: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExtensionStatusItem {
    pub key: SharedString,
    pub text: SharedString,
    pub command: Option<SharedString>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfiguredModelInfo {
    pub id: String,
    pub name: Option<String>,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    pub input: Vec<String>,
    pub reasoning: bool,
    /// Whether models.json states `reasoning` at all. Absent means "inherit",
    /// so the composer still offers the strength ladder (e.g. deepseek).
    pub reasoning_declared: bool,
    /// `thinkingLevelMap` from models.json — a `null` value marks a level the
    /// model does not offer, so the composer can list the real strengths.
    pub thinking_level_map: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfiguredProviderInfo {
    pub id: String,
    pub display_name: Option<String>,
    pub base_url: Option<String>,
    pub api: Option<String>,
    pub has_api_key: bool,
    pub models: Vec<ConfiguredModelInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkModelQuota {
    pub name: String,
    pub percentage: i32,
    #[serde(default)]
    pub reset_time: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkAccount {
    pub id: String,
    pub alias: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub plan_tier_override: Option<String>,
    #[serde(default)]
    pub priority: usize,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub system_home: bool,
    #[serde(default)]
    pub proxy_url: Option<String>,
    #[serde(default)]
    pub auth_required: bool,
    #[serde(default)]
    pub auth_error: Option<String>,
    #[serde(default)]
    pub cooldown_until: Option<u64>,
    #[serde(default)]
    pub cooldown_reason: Option<String>,
    #[serde(default)]
    pub five_hour_pct: Option<i32>,
    #[serde(default)]
    pub five_hour_reset: Option<String>,
    #[serde(default)]
    pub weekly_pct: Option<i32>,
    #[serde(default)]
    pub weekly_reset: Option<String>,
    #[serde(default)]
    pub argon_pct: Option<i32>,
    #[serde(default)]
    pub argon_reset: Option<String>,
    #[serde(default)]
    pub models: Vec<AgyLinkModelQuota>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkCacheBucket {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub turns: u64,
    #[serde(default)]
    pub cold_turns: u64,
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub hit_percent: Option<f64>,
    #[serde(default)]
    pub reasons: std::collections::HashMap<String, u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkCacheLastSample {
    #[serde(default)]
    pub at: u64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub hit_percent: Option<f64>,
    #[serde(default)]
    pub context_tokens: u64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkCacheStats {
    #[serde(default)]
    pub today: AgyLinkCacheBucket,
    #[serde(default)]
    pub last: Option<AgyLinkCacheLastSample>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkSettings {
    #[serde(default)]
    pub agy_plugins_enabled: Vec<String>,
    #[serde(default = "default_agy_mode")]
    pub mode: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default = "default_agy_permission")]
    pub permission_mode: String,
    #[serde(default)]
    pub default_effort: String,
    #[serde(default)]
    pub orchestrator_planner_model: String,
    #[serde(default)]
    pub orchestrator_planner_effort: String,
    #[serde(default)]
    pub orchestrator_executor_model: String,
    #[serde(default)]
    pub orchestrator_executor_effort: String,
    #[serde(default)]
    pub parallel_same_account: bool,
    #[serde(default)]
    pub max_concurrent: u32,
    #[serde(default)]
    pub dispatch_timeout_minutes: u32,
    #[serde(default)]
    pub dispatch_retries: u32,
    #[serde(default)]
    pub suppress_agy_auto_update: bool,
    #[serde(default)]
    pub pool_wide_catalog: bool,
    #[serde(default)]
    pub auto_fallback_model: bool,
    #[serde(default)]
    pub hidden_models: Vec<String>,
    #[serde(default)]
    pub selected_models: Vec<String>,
    #[serde(default)]
    pub auto_warmup: bool,
    #[serde(default = "default_agy_warmup_windows")]
    pub warmup_windows: String,
    #[serde(default)]
    pub warmup_daily_time: String,
    #[serde(default)]
    pub warmup_timezone: String,
    #[serde(default)]
    pub warmup_interval_hours: u32,
    #[serde(default = "default_agy_warmup_max_probes")]
    pub warmup_max_probes_per_run: u32,
    #[serde(default)]
    pub image_model: String,
    #[serde(default = "default_agy_image_effort")]
    pub image_effort: String,
    #[serde(default = "default_agy_image_timeout_ms")]
    pub image_timeout_ms: u64,
    #[serde(default = "default_agy_image_retries")]
    pub image_retries: u32,
    #[serde(default)]
    pub image_output_dir: String,
    #[serde(default)]
    pub image_tool: bool,
    #[serde(default)]
    pub auto_compact_override: u64,
    #[serde(default = "default_agy_effective_compact_limit")]
    pub effective_compact_limit: u64,
    #[serde(default = "default_agy_model_api_max_retries")]
    pub model_api_max_retries: u32,
    #[serde(default = "default_agy_slim_prompt_rules")]
    pub slim_prompt_rules: bool,
    #[serde(default = "default_agy_transport")]
    pub transport: String,
    #[serde(default)]
    pub acp_active_processes: u32,
    #[serde(default)]
    pub acp_sessions: u32,
    #[serde(default)]
    pub acp_draining_processes: u32,
    #[serde(default)]
    pub acp_stray_processes: u32,
    #[serde(default)]
    pub acp_slot_overwrite_detected_count: u32,
    #[serde(default)]
    pub cache_stats: Option<AgyLinkCacheStats>,
    #[serde(default)]
    pub sandbox: bool,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub agy_settings: String,
    #[serde(default)]
    pub sync_account_customizations: bool,
}

fn default_agy_mode() -> String {
    "sequential".to_string()
}

fn default_agy_permission() -> String {
    "skip".to_string()
}

fn default_agy_warmup_windows() -> String {
    "weekly".to_string()
}

fn default_agy_warmup_max_probes() -> u32 {
    12
}

fn default_agy_image_effort() -> String {
    "low".to_string()
}

fn default_agy_image_timeout_ms() -> u64 {
    240_000
}

fn default_agy_image_retries() -> u32 {
    1
}

fn default_agy_effective_compact_limit() -> u64 {
    256_000
}

fn default_agy_model_api_max_retries() -> u32 {
    3
}

fn default_agy_slim_prompt_rules() -> bool {
    true
}

fn default_agy_transport() -> String {
    "acp".to_string()
}

impl Default for AgyLinkSettings {
    fn default() -> Self {
        Self {
            agy_plugins_enabled: Vec::new(),
            mode: default_agy_mode(),
            proxy_url: String::new(),
            permission_mode: default_agy_permission(),
            default_effort: String::new(),
            orchestrator_planner_model: String::new(),
            orchestrator_planner_effort: String::new(),
            orchestrator_executor_model: String::new(),
            orchestrator_executor_effort: String::new(),
            parallel_same_account: false,
            max_concurrent: 0,
            dispatch_timeout_minutes: 0,
            dispatch_retries: 0,
            suppress_agy_auto_update: false,
            pool_wide_catalog: false,
            auto_fallback_model: false,
            hidden_models: Vec::new(),
            selected_models: Vec::new(),
            auto_warmup: false,
            warmup_windows: default_agy_warmup_windows(),
            warmup_daily_time: String::new(),
            warmup_timezone: String::new(),
            warmup_interval_hours: 0,
            warmup_max_probes_per_run: default_agy_warmup_max_probes(),
            image_model: String::new(),
            image_effort: default_agy_image_effort(),
            image_timeout_ms: default_agy_image_timeout_ms(),
            image_retries: default_agy_image_retries(),
            image_output_dir: String::new(),
            image_tool: false,
            auto_compact_override: 0,
            effective_compact_limit: default_agy_effective_compact_limit(),
            model_api_max_retries: default_agy_model_api_max_retries(),
            slim_prompt_rules: default_agy_slim_prompt_rules(),
            transport: default_agy_transport(),
            acp_active_processes: 0,
            acp_sessions: 0,
            acp_draining_processes: 0,
            acp_stray_processes: 0,
            acp_slot_overwrite_detected_count: 0,
            cache_stats: None,
            sandbox: false,
            agent: String::new(),
            agy_settings: String::new(),
            sync_account_customizations: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyLinkState {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub dormant_reason: Option<String>,
    #[serde(default)]
    pub active_account_id: Option<String>,
    #[serde(default)]
    pub pending_auth_url: Option<String>,
    #[serde(default)]
    pub accounts: Vec<AgyLinkAccount>,
    #[serde(default)]
    pub available_models: Vec<String>,
    #[serde(default)]
    pub settings: AgyLinkSettings,
    #[serde(default)]
    pub updated_at: u64,
}
