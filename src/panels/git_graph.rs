//! Native Git Graph panel (`pi-gpui/src/panels/git_graph.rs`).
//!
//! Full native migration of vscode-git-graph features:
//! - Interactive commit topological DAG graph (colored branch lanes, commit nodes, merge lines)
//! - Ref badges (HEAD pointer, local branches, remote branches, tags)
//! - Branch switcher / filter dropdown
//! - Commit search & message filtering
//! - Commit details expansion with changed files list & numstat (+added / -deleted)
//! - Direct Git actions: checkout, create branch, create tag, copy commit SHA

use std::path::Path;
use std::process::Command;

use chrono::{Local, TimeZone};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffState};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, ClipboardItem, Context, ElementId, Entity, FocusHandle, Focusable,
    Hsla, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, PathBuilder,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, canvas, div, hsla,
    point, px,
};

use crate::app_state::AppState;
use crate::i18n::{format_commas, tr};

pub fn lane_color(idx: usize) -> Hsla {
    let colors = [
        hsla(217.0 / 360.0, 0.91, 0.60, 1.0), // Blue #3b82f6
        hsla(160.0 / 360.0, 0.84, 0.39, 1.0), // Emerald #10b981
        hsla(38.0 / 360.0, 0.92, 0.50, 1.0),  // Amber #f59e0b
        hsla(262.0 / 360.0, 0.83, 0.58, 1.0), // Purple #8b5cf6
        hsla(330.0 / 360.0, 0.81, 0.60, 1.0), // Pink #ec4899
        hsla(189.0 / 360.0, 0.94, 0.43, 1.0), // Cyan #06b6d4
        hsla(25.0 / 360.0, 0.95, 0.53, 1.0),  // Orange #f97316
        hsla(84.0 / 360.0, 0.81, 0.44, 1.0),  // Lime #84cc16
    ];
    colors[idx % colors.len()]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRefBadge {
    pub name: String,
    pub is_head: bool,
    pub is_tag: bool,
    pub is_remote: bool,
}

#[derive(Debug, Clone)]
pub struct RawGitCommit {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64,
    pub subject: String,
    pub body: String,
    pub refs: Vec<GitRefBadge>,
}

#[derive(Debug, Clone)]
pub struct GitCommitRow {
    pub hash: String,
    pub short_hash: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64,
    pub date_str: String,
    pub subject: String,
    pub body: String,
    pub refs: Vec<GitRefBadge>,
    pub col: usize,
    pub color_idx: usize,
    pub max_lanes: usize,
    pub incoming_connections: Vec<(usize, usize, usize)>,
    pub passing_lanes: Vec<(usize, usize)>,
    pub outgoing_connections: Vec<(usize, usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct GitFileChange {
    pub path: String,
    pub added: usize,
    pub deleted: usize,
    pub is_binary: bool,
}

pub fn parse_git_decorations(decorations: &str) -> Vec<GitRefBadge> {
    let trimmed = decorations.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(trimmed);

    let mut refs = Vec::new();
    for part in inner.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if let Some(target) = p.strip_prefix("HEAD -> ") {
            refs.push(GitRefBadge {
                name: target.trim().to_string(),
                is_head: true,
                is_tag: false,
                is_remote: false,
            });
        } else if p == "HEAD" {
            refs.push(GitRefBadge {
                name: "HEAD".to_string(),
                is_head: true,
                is_tag: false,
                is_remote: false,
            });
        } else if let Some(tag_name) = p.strip_prefix("tag: ") {
            refs.push(GitRefBadge {
                name: tag_name.trim().to_string(),
                is_head: false,
                is_tag: true,
                is_remote: false,
            });
        } else {
            let is_remote = p.contains('/') && !p.starts_with("refs/");
            refs.push(GitRefBadge {
                name: p.to_string(),
                is_head: false,
                is_tag: false,
                is_remote,
            });
        }
    }
    refs
}

pub fn parse_raw_git_log(log_output: &str) -> Vec<RawGitCommit> {
    let mut commits = Vec::new();
    for record in log_output.split('\x1f') {
        let trimmed = record.trim();
        if trimmed.is_empty() {
            continue;
        }
        let fields: Vec<&str> = trimmed.split('\0').collect();
        if fields.len() < 7 {
            continue;
        }

        let hash = fields[0].trim().to_string();
        let short_hash = fields[1].trim().to_string();
        let parents: Vec<String> = fields[2]
            .split_whitespace()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let author_name = fields[3].trim().to_string();
        let author_email = fields[4].trim().to_string();
        let timestamp = fields[5].trim().parse::<i64>().unwrap_or(0);
        let subject = fields[6].trim().to_string();
        let decorations = fields.get(7).copied().unwrap_or("");
        let body = fields.get(8).copied().unwrap_or("").trim().to_string();
        let refs = parse_git_decorations(decorations);

        commits.push(RawGitCommit {
            hash,
            short_hash,
            parents,
            author_name,
            author_email,
            timestamp,
            subject,
            body,
            refs,
        });
    }
    commits
}

pub fn compute_git_graph_dag(raw_commits: Vec<RawGitCommit>) -> Vec<GitCommitRow> {
    // Each active lane holds Option<(expected_commit_hash, color_idx)>
    let mut active_lanes: Vec<Option<(String, usize)>> = Vec::new();
    let mut next_color_idx: usize = 0;
    let mut rows = Vec::with_capacity(raw_commits.len());

    for raw in raw_commits {
        // 1. Identify all lanes expecting this commit hash
        let mut matching_lanes = Vec::new();
        for (i, lane) in active_lanes.iter().enumerate() {
            if let Some((h, c_idx)) = lane {
                if h == &raw.hash {
                    matching_lanes.push((i, *c_idx));
                }
            }
        }

        let mut incoming_connections = Vec::new();
        let (col, color_idx) = if let Some(&(first_col, first_color)) = matching_lanes.first() {
            incoming_connections.push((first_col, first_col, first_color));
            // Merge any duplicate lanes waiting for this same commit smoothly into first_col!
            for &(dup_col, dup_color) in &matching_lanes[1..] {
                incoming_connections.push((dup_col, first_col, dup_color));
                active_lanes[dup_col] = None;
            }
            (first_col, first_color)
        } else {
            let c_idx = next_color_idx % 8;
            next_color_idx += 1;
            let assigned_col = if let Some(empty_idx) = active_lanes.iter().position(|l| l.is_none()) {
                active_lanes[empty_idx] = Some((raw.hash.clone(), c_idx));
                empty_idx
            } else {
                active_lanes.push(Some((raw.hash.clone(), c_idx)));
                active_lanes.len() - 1
            };
            (assigned_col, c_idx)
        };

        // 2. Snapshot passing lanes (all other active lanes running straight through this row)
        let mut passing_lanes = Vec::new();
        for (i, lane) in active_lanes.iter().enumerate() {
            if i != col {
                if let Some((_, c_idx)) = lane {
                    passing_lanes.push((i, *c_idx));
                }
            }
        }

        // 3. Process parents and build outgoing curves (deduplicating identical parallel tracks!)
        let mut outgoing_connections = Vec::new();
        active_lanes[col] = None;

        if !raw.parents.is_empty() {
            let p0 = &raw.parents[0];
            if let Some(existing_col) = active_lanes
                .iter()
                .position(|l| l.as_ref().is_some_and(|(h, _)| h == p0))
            {
                // Parent already tracked on existing_col -> merge directly into it!
                outgoing_connections.push((col, existing_col, color_idx));
            } else {
                active_lanes[col] = Some((p0.clone(), color_idx));
                outgoing_connections.push((col, col, color_idx));
            }

            for p in &raw.parents[1..] {
                if let Some(existing_col) = active_lanes
                    .iter()
                    .position(|l| l.as_ref().is_some_and(|(h, _)| h == p))
                {
                    let existing_color = active_lanes[existing_col]
                        .as_ref()
                        .map(|(_, c)| *c)
                        .unwrap_or(color_idx);
                    outgoing_connections.push((col, existing_col, existing_color));
                } else {
                    let branch_color = next_color_idx % 8;
                    next_color_idx += 1;
                    let p_col = if let Some(empty_pos) = active_lanes.iter().position(|l| l.is_none()) {
                        active_lanes[empty_pos] = Some((p.clone(), branch_color));
                        empty_pos
                    } else {
                        active_lanes.push(Some((p.clone(), branch_color)));
                        active_lanes.len() - 1
                    };
                    outgoing_connections.push((col, p_col, branch_color));
                }
            }
        }

        while active_lanes.last().is_some_and(|l| l.is_none()) {
            active_lanes.pop();
        }

        let date_str = Local
            .timestamp_opt(raw.timestamp, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Unknown date".to_string());

        let max_lanes = (col + 1).max(active_lanes.len()).max(1);

        rows.push(GitCommitRow {
            hash: raw.hash,
            short_hash: raw.short_hash,
            parents: raw.parents,
            author_name: raw.author_name,
            author_email: raw.author_email,
            timestamp: raw.timestamp,
            date_str,
            subject: raw.subject,
            body: raw.body,
            refs: raw.refs,
            col,
            color_idx,
            max_lanes,
            incoming_connections,
            passing_lanes,
            outgoing_connections,
        });
    }

    rows
}

pub fn load_git_graph_commits(
    cwd: &Path,
    branch_filter: Option<&str>,
    max_count: usize,
) -> Result<Vec<GitCommitRow>, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(cwd);
    cmd.arg("log");
    if let Some(b) = branch_filter {
        if b != "all" && !b.is_empty() {
            cmd.arg(b);
        } else {
            cmd.arg("--all");
        }
    } else {
        cmd.arg("--all");
    }
    cmd.arg("--date-order");
    cmd.arg(format!("-n{}", max_count.clamp(10, 1000)));
    cmd.arg("--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%at%x00%s%x00%d%x00%b%x1f");

    let out = cmd.output().map_err(|e| format!("Failed to run git log: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git log error: {err}"));
    }

    let raw_text = String::from_utf8_lossy(&out.stdout);
    let raw_commits = parse_raw_git_log(&raw_text);
    Ok(compute_git_graph_dag(raw_commits))
}

pub fn load_commit_files(cwd: &Path, hash: &str) -> Vec<GitFileChange> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["show", "--numstat", "--format=", hash])
        .output();

    let Ok(output) = out else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();

    for line in raw.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let added_str = parts[0].trim();
        let deleted_str = parts[1].trim();
        let path = parts[2].trim().to_string();

        let is_binary = added_str == "-" || deleted_str == "-";
        let added = added_str.parse::<usize>().unwrap_or(0);
        let deleted = deleted_str.parse::<usize>().unwrap_or(0);

        files.push(GitFileChange {
            path,
            added,
            deleted,
            is_binary,
        });
    }

    files
}

pub fn load_commit_file_diff(cwd: &Path, hash: &str, file_path: &str) -> String {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["show", "--format=", hash, "--", file_path])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(o) => String::from_utf8_lossy(&o.stderr).to_string(),
        Err(e) => format!("Failed to load diff: {e}"),
    }
}

pub fn git_checkout_target(cwd: &Path, target: &str) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["checkout", target])
        .output()
        .map_err(|e| format!("Failed to checkout: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stderr).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

pub fn git_create_branch(cwd: &Path, branch_name: &str, target_hash: &str) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["branch", branch_name, target_hash])
        .output()
        .map_err(|e| format!("Failed to create branch: {e}"))?;
    if out.status.success() {
        Ok(format!("Created branch {branch_name}"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

pub fn git_create_tag(cwd: &Path, tag_name: &str, target_hash: &str) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["tag", tag_name, target_hash])
        .output()
        .map_err(|e| format!("Failed to create tag: {e}"))?;
    if out.status.success() {
        Ok(format!("Created tag {tag_name}"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

pub fn git_fetch_all(cwd: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["fetch", "--all", "--prune"])
        .output()
        .map_err(|e| format!("Failed to fetch: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stderr).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

pub struct CommitFilePreview {
    pub commit_hash: String,
    pub file_path: String,
    pub patch: String,
    pub diff_state: Option<Entity<DiffState>>,
    pub diff_mode: DiffMode,
}

pub struct GitGraphView {
    focus_handle: FocusHandle,
    app_state: Entity<AppState>,
    commits: Vec<GitCommitRow>,
    branches: Vec<String>,
    selected_branch: String,
    search_input: Entity<InputState>,
    search_query: String,
    selected_commit: Option<GitCommitRow>,
    commit_files: Vec<GitFileChange>,
    file_preview: Option<CommitFilePreview>,
    is_loading: bool,
    is_fetching: bool,
    new_branch_input: Entity<InputState>,
    new_tag_input: Entity<InputState>,
    dialog_mode: Option<GitGraphDialog>,
    last_loaded_cwd: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitGraphDialog {
    CreateBranch(String),
    CreateTag(String),
}

impl Focusable for GitGraphView {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl GitGraphView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Filter commits (hash, author, message)...")
        });
        let new_branch_input = cx.new(|cx| InputState::new(window, cx).placeholder("Branch name"));
        let new_tag_input = cx.new(|cx| InputState::new(window, cx).placeholder("Tag name (e.g. v1.0.0)"));

        let search_sync = search_input.clone();
        cx.subscribe(
            &search_input,
            move |this, _, ev: &gpui_kit::component::input::InputEvent, cx| {
                if matches!(ev, gpui_kit::component::input::InputEvent::Change) {
                    this.search_query = search_sync.read(cx).value().trim().to_lowercase();
                    cx.notify();
                }
            },
        )
        .detach();

        let init_cwd = app_state.read(cx).active_cwd.clone();

        let app_obs = app_state.clone();
        cx.observe(&app_state, move |this, _, cx| {
            let state = app_obs.read(cx);
            let cur_cwd = state.active_cwd.clone();
            let has_git = state.active_branch.is_some();
            if this.last_loaded_cwd.as_ref() != Some(&cur_cwd) {
                this.last_loaded_cwd = Some(cur_cwd);
                this.selected_commit = None;
                this.commit_files.clear();
                this.file_preview = None;
                this.reload_graph(cx);
            } else if !has_git && !this.commits.is_empty() {
                this.commits.clear();
                this.branches.clear();
                this.selected_commit = None;
                this.commit_files.clear();
                this.file_preview = None;
                cx.notify();
            }
        })
        .detach();

        let mut view = Self {
            focus_handle: cx.focus_handle(),
            app_state,
            commits: Vec::new(),
            branches: Vec::new(),
            selected_branch: "all".to_string(),
            search_input,
            search_query: String::new(),
            selected_commit: None,
            commit_files: Vec::new(),
            file_preview: None,
            is_loading: false,
            is_fetching: false,
            new_branch_input,
            new_tag_input,
            dialog_mode: None,
            last_loaded_cwd: Some(init_cwd),
        };

        view.reload_graph(cx);
        view
    }

    pub fn reload_graph(&mut self, cx: &mut Context<Self>) {
        let (cwd, is_git) = {
            let s = self.app_state.read(cx);
            (s.active_cwd.clone(), s.active_branch.is_some())
        };
        if !is_git {
            self.commits.clear();
            self.branches.clear();
            self.selected_commit = None;
            self.commit_files.clear();
            self.file_preview = None;
            self.is_loading = false;
            cx.notify();
            return;
        }

        let branch_filter = if self.selected_branch == "all" {
            None
        } else {
            Some(self.selected_branch.clone())
        };

        self.is_loading = true;
        self.branches = crate::workspace::status_bar::get_local_git_branches(&cwd);

        cx.spawn(async move |this, cx| {
            let res = load_git_graph_commits(&cwd, branch_filter.as_deref(), 300);
            let _ = this.update(cx, |this, cx| {
                this.is_loading = false;
                if let Ok(list) = res {
                    this.commits = list;
                    if let Some(ref sel) = this.selected_commit.clone() {
                        this.select_commit(&sel.hash, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        if self.selected_commit.as_ref().is_some_and(|sc| sc.hash == hash) {
            self.selected_commit = None;
            self.commit_files.clear();
            self.file_preview = None;
            cx.notify();
            return;
        }
        self.select_commit(hash, cx);
    }

    pub fn open_file_preview(&mut self, hash: &str, file_path: &str, cx: &mut Context<Self>) {
        let cwd = self.app_state.read(cx).active_cwd.clone();
        let patch = load_commit_file_diff(&cwd, hash, file_path);
        let diff_state = DiffFile::parse(&patch).ok().map(|files| {
            cx.new(|cx| {
                let mut state = DiffState::new(files, cx);
                state.set_mode(DiffMode::Unified, cx);
                state
            })
        });
        self.file_preview = Some(CommitFilePreview {
            commit_hash: hash.to_string(),
            file_path: file_path.to_string(),
            patch,
            diff_state,
            diff_mode: DiffMode::Unified,
        });
        cx.notify();
    }

    pub fn select_commit(&mut self, hash: &str, cx: &mut Context<Self>) {
        let Some(c) = self.commits.iter().find(|c| c.hash == hash).cloned() else {
            self.selected_commit = None;
            self.commit_files.clear();
            return;
        };
        self.selected_commit = Some(c);
        let cwd = self.app_state.read(cx).active_cwd.clone();
        let hash_s = hash.to_string();
        cx.spawn(async move |this, cx| {
            let files = load_commit_files(&cwd, &hash_s);
            let _ = this.update(cx, |this, cx| {
                this.commit_files = files;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn fetch_all(&mut self, cx: &mut Context<Self>) {
        let cwd = self.app_state.read(cx).active_cwd.clone();
        self.is_fetching = true;
        cx.spawn(async move |this, cx| {
            let _ = git_fetch_all(&cwd);
            let _ = this.update(cx, |this, cx| {
                this.is_fetching = false;
                this.reload_graph(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn checkout_ref(&mut self, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.app_state.read(cx).active_cwd.clone();
        let locale = self.app_state.read(cx).locale;
        match git_checkout_target(&cwd, target) {
            Ok(_) => {
                window.push_notification(
                    Notification::success(format!("{}: {target}", tr(locale, "git.checkedOut"))),
                    cx,
                );
                self.reload_graph(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(e), cx);
            }
        }
        cx.notify();
    }

    pub fn submit_create_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(GitGraphDialog::CreateBranch(hash)) = self.dialog_mode.take() else {
            return;
        };
        let branch_name = self.new_branch_input.read(cx).value().trim().to_string();
        if branch_name.is_empty() {
            return;
        }
        let cwd = self.app_state.read(cx).active_cwd.clone();
        match git_create_branch(&cwd, &branch_name, &hash) {
            Ok(msg) => {
                window.push_notification(Notification::success(msg), cx);
                self.reload_graph(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(e), cx);
            }
        }
        cx.notify();
    }

    pub fn submit_create_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(GitGraphDialog::CreateTag(hash)) = self.dialog_mode.take() else {
            return;
        };
        let tag_name = self.new_tag_input.read(cx).value().trim().to_string();
        if tag_name.is_empty() {
            return;
        }
        let cwd = self.app_state.read(cx).active_cwd.clone();
        match git_create_tag(&cwd, &tag_name, &hash) {
            Ok(msg) => {
                window.push_notification(Notification::success(msg), cx);
                self.reload_graph(cx);
            }
            Err(e) => {
                window.push_notification(Notification::error(e), cx);
            }
        }
        cx.notify();
    }
}

impl Render for GitGraphView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.app_state.read(cx).locale;
        let is_git = self.app_state.read(cx).active_branch.is_some();

        let filtered_commits: Vec<&GitCommitRow> = self
            .commits
            .iter()
            .filter(|c| {
                if self.search_query.is_empty() {
                    return true;
                }
                c.subject.to_lowercase().contains(&self.search_query)
                    || c.short_hash.to_lowercase().contains(&self.search_query)
                    || c.hash.to_lowercase().contains(&self.search_query)
                    || c.author_name.to_lowercase().contains(&self.search_query)
            })
            .collect();

        // 1. Toolbar Bar (Branch filter, search, fetch, refresh)
        let branch_label = if self.selected_branch == "all" {
            tr(locale, "git.allBranches").to_string()
        } else {
            self.selected_branch.clone()
        };

        let entity_branch = cx.entity().clone();
        let cur_branch = self.selected_branch.clone();
        let branches = self.branches.clone();

        let branch_dropdown = Button::new("git-graph-branch-btn")
            .outline()
            .small()
            .h(px(28.))
            .icon(Icon::new(crate::icons::GitBranchIcon))
            .dropdown_caret(true)
            .label(branch_label)
            .dropdown_menu(move |mut menu, _, _| {
                let e_all = entity_branch.clone();
                menu = menu.item(
                    PopupMenuItem::new(tr(locale, "git.allBranches"))
                        .checked(cur_branch == "all")
                        .on_click(move |_, _, cx| {
                            e_all.update(cx, |this, cx| {
                                this.selected_branch = "all".to_string();
                                this.reload_graph(cx);
                            });
                        }),
                );
                for b in &branches {
                    let e_b = entity_branch.clone();
                    let b_name = b.clone();
                    let checked = cur_branch == *b;
                    menu = menu.item(
                        PopupMenuItem::new(b.clone())
                            .checked(checked)
                            .on_click(move |_, _, cx| {
                                e_b.update(cx, |this, cx| {
                                    this.selected_branch = b_name.clone();
                                    this.reload_graph(cx);
                                });
                            }),
                    );
                }
                menu
            });

        let entity_fetch = cx.entity().clone();
        let is_fetching = self.is_fetching;
        let fetch_button = Button::new("git-graph-fetch-btn")
            .outline()
            .small()
            .h(px(28.))
            .icon(if is_fetching {
                IconName::LoaderCircle
            } else {
                IconName::ArrowDown
            })
            .disabled(is_fetching)
            .label(tr(locale, "git.fetch"))
            .tooltip(tr(locale, "git.fetchTooltip"))
            .on_click(move |_, _, cx| {
                entity_fetch.update(cx, |this, cx| {
                    this.fetch_all(cx);
                });
            });

        let entity_reload = cx.entity().clone();
        let is_loading = self.is_loading;
        let refresh_button = Button::new("git-graph-refresh-btn")
            .outline()
            .small()
            .h(px(28.))
            .icon(if is_loading {
                IconName::LoaderCircle
            } else {
                IconName::RotateCw
            })
            .disabled(is_loading)
            .tooltip(tr(locale, "git.refresh"))
            .on_click(move |_, _, cx| {
                entity_reload.update(cx, |this, cx| {
                    this.reload_graph(cx);
                });
            });

        let toolbar = h_flex()
            .w_full()
            .px_3()
            .py_2()
            .gap_2()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(branch_dropdown)
                    .child(
                        div()
                            .w(px(220.))
                            .child(Input::new(&self.search_input).small().h(px(28.))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(Tag::secondary().small().child(format!(
                        "{} {}",
                        format_commas(filtered_commits.len()),
                        tr(locale, "git.commits")
                    )))
                    .child(fetch_button)
                    .child(refresh_button),
            );

        if !is_git {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .child(
                    Empty::new().header(
                        EmptyHeader::new()
                            .media(EmptyMedia::new().child(Icon::new(crate::icons::GitBranchIcon)))
                            .title(EmptyTitle::new().child(tr(locale, "git.notRepo")))
                            .description(EmptyDescription::new().child(tr(locale, "git.notRepoDesc"))),
                    ),
                )
                .into_any_element();
        }

        // 2. Commit List with Visual DAG tracks
        let global_max_lanes = filtered_commits
            .iter()
            .map(|c| c.max_lanes)
            .max()
            .unwrap_or(1)
            .clamp(2, 20);
        let graph_col_w = px(global_max_lanes as f32 * 16.0 + 12.0);

        let mut list_inner = v_flex().w_full();

        for commit in filtered_commits {
            let hash_c = commit.hash.clone();
            let short_h = commit.short_hash.clone();
            let is_selected = self
                .selected_commit
                .as_ref()
                .is_some_and(|sc| sc.hash == commit.hash);
            let node_col = commit.col;
            let node_color = lane_color(commit.color_idx);
            let is_head = commit.refs.iter().any(|r| r.is_head);
            let bg_color = cx.theme().background;
            let primary_color = cx.theme().primary;

            let incoming = commit.incoming_connections.clone();
            let passing = commit.passing_lanes.clone();
            let outgoing = commit.outgoing_connections.clone();

            let graph_canvas = canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let lane_w = 16.0f32;
                    let top = bounds.top();
                    let bottom = bounds.bottom();
                    let mid_y = top + px(16.0);
                    let left = bounds.left();

                    // 1. Passing vertical lanes (0px -> 32px)
                    for (lane_col, c_idx) in &passing {
                        let x = left + px(*lane_col as f32 * lane_w + 8.0);
                        let mut pb = PathBuilder::stroke(px(2.0));
                        pb.move_to(point(x, top));
                        pb.line_to(point(x, bottom));
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, lane_color(*c_idx).opacity(0.85));
                        }
                    }

                    // 2. Incoming connections (0px -> 16px) with smooth Bezier curves
                    for (from_l, to_l, c_idx) in &incoming {
                        let x1 = left + px(*from_l as f32 * lane_w + 8.0);
                        let x2 = left + px(*to_l as f32 * lane_w + 8.0);
                        let mut pb = PathBuilder::stroke(px(2.0));
                        pb.move_to(point(x1, top));
                        if from_l == to_l {
                            pb.line_to(point(x2, mid_y));
                        } else {
                            pb.cubic_bezier_to(
                                point(x2, mid_y),
                                point(x1, top + px(8.0)),
                                point(x2, mid_y - px(8.0)),
                            );
                        }
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, lane_color(*c_idx));
                        }
                    }

                    // 3. Outgoing connections (16px -> 32px) with smooth Bezier curves
                    for (from_l, to_l, c_idx) in &outgoing {
                        let x1 = left + px(*from_l as f32 * lane_w + 8.0);
                        let x2 = left + px(*to_l as f32 * lane_w + 8.0);
                        let mut pb = PathBuilder::stroke(px(2.0));
                        pb.move_to(point(x1, mid_y));
                        if from_l == to_l {
                            pb.line_to(point(x2, bottom));
                        } else {
                            pb.cubic_bezier_to(
                                point(x2, bottom),
                                point(x1, mid_y + px(8.0)),
                                point(x2, bottom - px(8.0)),
                            );
                        }
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, lane_color(*c_idx));
                        }
                    }
                },
            )
            .size_full();

            // Construct DAG Lane Element (overflow_hidden so branch lines never collide with text)
            let graph_box = div()
                .relative()
                .h(px(32.))
                .w(graph_col_w)
                .flex_shrink_0()
                .overflow_hidden()
                .child(graph_canvas)
                .child(
                    div()
                        .absolute()
                        .left(px(node_col as f32 * 16.0 + 3.0))
                        .top(px(11.0))
                        .w(px(10.0))
                        .h(px(10.0))
                        .rounded_full()
                        .border_2()
                        .border_color(if is_head { primary_color } else { bg_color })
                        .bg(node_color),
                );

            // Ref Badges (HEAD, branch, tag, remote) - show at most 2 badges inline to prevent overflow
            let mut refs_stack = h_flex().gap_1().items_center().flex_shrink_0();
            for r in commit.refs.iter().take(2) {
                let badge_el = if r.is_head {
                    Tag::primary()
                        .small()
                        .child(format!("HEAD: {}", r.name))
                } else if r.is_tag {
                    Tag::warning()
                        .small()
                        .child(format!("tag: {}", r.name))
                } else if r.is_remote {
                    Tag::secondary()
                        .small()
                        .child(r.name.clone())
                } else {
                    Tag::success()
                        .small()
                        .child(r.name.clone())
                };
                refs_stack = refs_stack.child(div().flex_shrink_0().child(badge_el));
            }
            if commit.refs.len() > 2 {
                refs_stack = refs_stack.child(
                    div().flex_shrink_0().child(
                        Tag::secondary()
                            .small()
                            .child(format!("+{}", commit.refs.len() - 2)),
                    ),
                );
            }

            let hash_copy = commit.hash.clone();
            let row_id: ElementId = SharedString::from(format!("git-row-{}", commit.hash)).into();
            let btn_hash_id: ElementId = SharedString::from(format!("git-btn-{}", commit.hash)).into();

            let commit_row = h_flex()
                .id(row_id)
                .w_full()
                .h(px(32.))
                .flex_shrink_0()
                .px_2()
                .gap_2()
                .items_center()
                .justify_between()
                .overflow_hidden()
                .cursor_pointer()
                .bg(if is_selected {
                    cx.theme().selection.opacity(0.35)
                } else {
                    cx.theme().transparent
                })
                .hover(|s| s.bg(cx.theme().secondary.opacity(0.5)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_commit(&hash_c, cx);
                }))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .overflow_hidden()
                        .child(graph_box)
                        .children((!commit.refs.is_empty()).then(|| refs_stack))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .text_color(if is_selected {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().foreground.opacity(0.9)
                                })
                                .child(SharedString::from(commit.subject.clone())),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            div()
                                .max_w(px(72.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(commit.author_name.clone())),
                        )
                        .child(
                            Button::new(btn_hash_id)
                                .ghost()
                                .xsmall()
                                .label(short_h)
                                .tooltip(tr(locale, "git.copyHash"))
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    cx.write_to_clipboard(ClipboardItem::new_string(hash_copy.clone()));
                                    window.push_notification(Notification::success(tr(locale, "common.copied")), cx);
                                }),
                        ),
                );

            list_inner = list_inner.child(commit_row);
        }

        let list_scroll = div()
            .id("git-graph-list")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .child(list_inner);

        let mut root = v_flex().relative().size_full().child(toolbar).child(list_scroll);

        // Full-panel overlay Commit Details Modal
        if let Some(ref commit) = self.selected_commit.clone() {
            let ent_dlg_b = cx.entity().clone();
            let ent_dlg_t = cx.entity().clone();
            let ent_co = cx.entity().clone();
            let h_full = commit.hash.clone();
            let h_copy_modal = commit.hash.clone();
            let h_co = commit.hash.clone();
            let h_br = commit.hash.clone();
            let h_tag = commit.hash.clone();

            let mut modal_refs = h_flex().gap_1().flex_wrap().items_center();
            for r in &commit.refs {
                let badge = if r.is_head {
                    Tag::primary().small().child(format!("HEAD: {}", r.name))
                } else if r.is_tag {
                    Tag::warning().small().child(format!("tag: {}", r.name))
                } else if r.is_remote {
                    Tag::secondary().small().child(r.name.clone())
                } else {
                    Tag::success().small().child(r.name.clone())
                };
                modal_refs = modal_refs.child(badge);
            }

            let actions_bar = h_flex()
                .gap_2()
                .items_center()
                .flex_wrap()
                .child(
                    Button::new("btn-co-commit")
                        .outline()
                        .small()
                        .icon(IconName::Check)
                        .label(tr(locale, "git.checkout"))
                        .on_click(move |_, window, cx| {
                            ent_co.update(cx, |this, cx| {
                                this.checkout_ref(&h_co, window, cx);
                            });
                        }),
                )
                .child(
                    Button::new("btn-new-branch")
                        .outline()
                        .small()
                        .icon(Icon::new(crate::icons::GitBranchIcon))
                        .label(tr(locale, "git.createBranch"))
                        .on_click(move |_, _, cx| {
                            ent_dlg_b.update(cx, |this, cx| {
                                this.dialog_mode = Some(GitGraphDialog::CreateBranch(h_br.clone()));
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new("btn-new-tag")
                        .outline()
                        .small()
                        .icon(IconName::Info)
                        .label(tr(locale, "git.createTag"))
                        .on_click(move |_, _, cx| {
                            ent_dlg_t.update(cx, |this, cx| {
                                this.dialog_mode = Some(GitGraphDialog::CreateTag(h_tag.clone()));
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new("btn-copy-commit-sha-modal")
                        .outline()
                        .small()
                        .icon(IconName::Copy)
                        .label(tr(locale, "chat.copy"))
                        .on_click(move |_, window, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(h_copy_modal.clone()));
                            window.push_notification(Notification::success(tr(locale, "common.copied")), cx);
                        }),
                );

            let mut files_list_inner = v_flex().w_full().gap_1();
            for (f_ix, f) in self.commit_files.iter().enumerate() {
                let p_clone = f.path.clone();
                let h_for_preview = commit.hash.clone();
                let f_row_id: ElementId = SharedString::from(format!("git-file-{f_ix}-{}", f.path)).into();
                files_list_inner = files_list_inner.child(
                    h_flex()
                        .id(f_row_id)
                        .w_full()
                        .h(px(30.))
                        .flex_shrink_0()
                        .px_2p5()
                        .rounded_md()
                        .justify_between()
                        .items_center()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_file_preview(&h_for_preview, &p_clone, cx);
                        }))
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(IconName::FileText).xsmall().text_color(cx.theme().muted_foreground))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_xs()
                                        .child(SharedString::from(f.path.clone())),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1p5()
                                .flex_shrink_0()
                                .child(div().text_xs().text_color(cx.theme().success).child(format!("+{}", f.added)))
                                .child(div().text_xs().text_color(cx.theme().danger).child(format!("-{}", f.deleted))),
                        ),
                );
            }

            // Fixed-height scrollable changed files box (~6 visible rows = 200px)
            let files_scroll_box = div()
                .id("git-commit-files-scroll")
                .w_full()
                .h(px(200.))
                .flex_shrink_0()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background.opacity(0.6))
                .p_1p5()
                .overflow_y_scroll()
                .child(files_list_inner);

            let modal_card = v_flex()
                .id("commit-details-modal")
                .w_full()
                .max_w(px(660.))
                .max_h_full()
                .rounded_xl()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .shadow_lg()
                .p_4()
                .gap_3()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Header with SHA + Close
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(crate::icons::GitBranchIcon).small().text_color(cx.theme().primary))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .flex_shrink_0()
                                        .child(SharedString::from(commit.short_hash.clone())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .font_family(cx.theme().font_family.clone())
                                        .child(SharedString::from(h_full)),
                                ),
                        )
                        .child(
                            Button::new("btn-close-commit-modal")
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected_commit = None;
                                    this.commit_files.clear();
                                    this.file_preview = None;
                                    cx.notify();
                                })),
                        ),
                )
                .children((!commit.refs.is_empty()).then(|| modal_refs))
                // Subject & Body
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(SharedString::from(commit.subject.clone())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} <{}> · {}",
                                    commit.author_name, commit.author_email, commit.date_str
                                )),
                        ),
                )
                .children((!commit.body.is_empty()).then(|| {
                    div()
                        .id("commit-body-scroll")
                        .w_full()
                        .max_h(px(120.))
                        .overflow_y_scroll()
                        .p_2p5()
                        .rounded_md()
                        .bg(cx.theme().secondary.opacity(0.45))
                        .text_xs()
                        .text_color(cx.theme().foreground)
                        .child(SharedString::from(commit.body.clone()))
                }))
                .child(actions_bar)
                // Bottom Changed Files section with fixed visible height
                .child(
                    v_flex()
                        .gap_1p5()
                        .w_full()
                        .child(
                            div()
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} ({})",
                                    tr(locale, "git.changedFiles"),
                                    self.commit_files.len()
                                )),
                        )
                        .child(files_scroll_box),
                );

            root = root.child(
                div()
                    .id("commit-modal-backdrop")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_5()
                    .bg(cx.theme().background.opacity(0.75))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.selected_commit = None;
                            this.commit_files.clear();
                            this.file_preview = None;
                            cx.notify();
                        }),
                    )
                    .child(modal_card),
            );
        }

        // Dedicated File Diff Preview Window Modal (opened when clicking a changed file)
        if let Some(ref preview) = self.file_preview {
            let patch_for_copy = preview.patch.clone();
            let file_path_title = preview.file_path.clone();
            let short_sha: String = preview.commit_hash.chars().take(8).collect();
            let diff_mode = preview.diff_mode;

            let diff_body = if let Some(ref ds) = preview.diff_state {
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        Diff::new(ds)
                            .header_visible(false)
                            .line_number(true)
                            .size_full(),
                    )
                    .into_any_element()
            } else {
                div()
                    .id("git-raw-patch-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .p_3()
                    .text_xs()
                    .font_family(cx.theme().font_family.clone())
                    .child(SharedString::from(preview.patch.clone()))
                    .into_any_element()
            };

            let preview_card = v_flex()
                .id("git-file-preview-modal")
                .size_full()
                .max_w(px(900.))
                .max_h(px(680.))
                .rounded_xl()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .shadow_lg()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .w_full()
                        .px_4()
                        .py_2p5()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(IconName::FileText).small().text_color(cx.theme().primary))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_sm()
                                        .font_semibold()
                                        .child(SharedString::from(file_path_title)),
                                )
                                .child(Tag::secondary().small().child(short_sha)),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .flex_shrink_0()
                                .children(preview.diff_state.is_some().then(|| {
                                    Button::new("btn-preview-diff-mode")
                                        .outline()
                                        .small()
                                        .label(if diff_mode == DiffMode::Unified {
                                            tr(locale, "chat.splitView")
                                        } else {
                                            tr(locale, "chat.unifiedView")
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(ref mut pv) = this.file_preview {
                                                let next = if pv.diff_mode == DiffMode::Unified {
                                                    DiffMode::Split
                                                } else {
                                                    DiffMode::Unified
                                                };
                                                pv.diff_mode = next;
                                                if let Some(ref ds) = pv.diff_state {
                                                    ds.update(cx, |d, cx| d.set_mode(next, cx));
                                                }
                                            }
                                            cx.notify();
                                        }))
                                }))
                                .child(
                                    Button::new("btn-copy-file-patch")
                                        .outline()
                                        .small()
                                        .icon(IconName::Copy)
                                        .label(tr(locale, "chat.copy"))
                                        .on_click(move |_, window, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(patch_for_copy.clone()));
                                            window.push_notification(Notification::success(tr(locale, "common.copied")), cx);
                                        }),
                                )
                                .child(
                                    Button::new("btn-close-file-preview")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Close)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.file_preview = None;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .child(diff_body);

            root = root.child(
                div()
                    .id("git-file-preview-backdrop")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .bg(cx.theme().background.opacity(0.8))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.file_preview = None;
                            cx.notify();
                        }),
                    )
                    .child(preview_card),
            );
        }

        // Dialog for Create Branch
        if let Some(GitGraphDialog::CreateBranch(ref hash)) = self.dialog_mode.clone() {
            let ent_cancel = cx.entity().clone();
            let ent_ok = cx.entity().clone();
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background.opacity(0.75))
                    .child(
                        v_flex()
                            .w(px(360.))
                            .p_4()
                            .gap_3()
                            .rounded_xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().popover)
                            .shadow_lg()
                            .child(div().text_sm().font_semibold().child(tr(locale, "git.createBranch")))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Create new branch from {hash}")),
                            )
                            .child(Input::new(&self.new_branch_input).h(px(32.)))
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("btn-cancel-create-branch")
                                            .outline()
                                            .small()
                                            .label(tr(locale, "common.cancel"))
                                            .on_click(move |_, _, cx| {
                                                ent_cancel.update(cx, |this, cx| {
                                                    this.dialog_mode = None;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("btn-confirm-create-branch")
                                            .primary()
                                            .small()
                                            .label(tr(locale, "common.create"))
                                            .on_click(move |_, window, cx| {
                                                ent_ok.update(cx, |this, cx| {
                                                    this.submit_create_branch(window, cx);
                                                });
                                            }),
                                    ),
                            ),
                    ),
            );
        }

        // Dialog for Create Tag
        if let Some(GitGraphDialog::CreateTag(ref hash)) = self.dialog_mode.clone() {
            let ent_cancel = cx.entity().clone();
            let ent_ok = cx.entity().clone();
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background.opacity(0.75))
                    .child(
                        v_flex()
                            .w(px(360.))
                            .p_4()
                            .gap_3()
                            .rounded_xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().popover)
                            .shadow_lg()
                            .child(div().text_sm().font_semibold().child(tr(locale, "git.createTag")))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("Create new tag on {hash}")),
                            )
                            .child(Input::new(&self.new_tag_input).h(px(32.)))
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("btn-cancel-create-tag")
                                            .outline()
                                            .small()
                                            .label(tr(locale, "common.cancel"))
                                            .on_click(move |_, _, cx| {
                                                ent_cancel.update(cx, |this, cx| {
                                                    this.dialog_mode = None;
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("btn-confirm-create-tag")
                                            .primary()
                                            .small()
                                            .label(tr(locale, "common.create"))
                                            .on_click(move |_, window, cx| {
                                                ent_ok.update(cx, |this, cx| {
                                                    this.submit_create_tag(window, cx);
                                                });
                                            }),
                                    ),
                            ),
                    ),
            );
        }

        root.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_log_output_and_computes_lanes() {
        let fake_log = "c1\0c1short\0\0Alice\0alice@example.com\01700000000\0Initial commit\0(HEAD -> main, tag: v1.0)\0First commit body\x1f\
                        c2\0c2short\0c1\0Bob\0bob@example.com\01700001000\0Feature A\0(feature/a)\0\x1f\
                        c3\0c3short\0c1 c2\0Charlie\0charlie@example.com\01700002000\0Merge branch feature/a\0\0Merge body\x1f";

        let raw = parse_raw_git_log(fake_log);
        assert_eq!(raw.len(), 3);
        assert_eq!(raw[0].hash, "c1");
        assert_eq!(raw[0].refs.len(), 2);
        assert!(raw[0].refs[0].is_head);
        assert_eq!(raw[0].refs[0].name, "main");
        assert!(raw[0].refs[1].is_tag);
        assert_eq!(raw[0].refs[1].name, "v1.0");

        let graph = compute_git_graph_dag(raw);
        assert_eq!(graph.len(), 3);
        assert_eq!(graph[0].col, 0);
    }
}
