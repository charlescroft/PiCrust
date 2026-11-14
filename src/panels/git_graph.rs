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
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, ClipboardItem, Context, ElementId, Entity, FocusHandle, Focusable,
    Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, hsla, px,
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
    pub active_lanes: Vec<bool>,
    pub connections_down: Vec<(usize, usize)>,
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
    let mut active_lanes: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(raw_commits.len());

    for raw in raw_commits {
        let col = if let Some(existing_idx) = active_lanes
            .iter()
            .position(|lane| lane.as_deref() == Some(&raw.hash))
        {
            existing_idx
        } else if let Some(empty_idx) = active_lanes.iter().position(|lane| lane.is_none()) {
            active_lanes[empty_idx] = Some(raw.hash.clone());
            empty_idx
        } else {
            active_lanes.push(Some(raw.hash.clone()));
            active_lanes.len() - 1
        };

        let passing_lanes: Vec<bool> = active_lanes
            .iter()
            .enumerate()
            .map(|(i, lane)| i != col && lane.is_some())
            .collect();

        let mut connections_down = Vec::new();
        if raw.parents.is_empty() {
            active_lanes[col] = None;
        } else {
            active_lanes[col] = Some(raw.parents[0].clone());
            connections_down.push((col, col));

            for p in &raw.parents[1..] {
                let p_col = if let Some(pos) = active_lanes
                    .iter()
                    .position(|l| l.as_deref() == Some(p.as_str()))
                {
                    pos
                } else if let Some(empty_pos) = active_lanes.iter().position(|l| l.is_none()) {
                    active_lanes[empty_pos] = Some(p.clone());
                    empty_pos
                } else {
                    active_lanes.push(Some(p.clone()));
                    active_lanes.len() - 1
                };
                connections_down.push((col, p_col));
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

        let max_lanes = col.max(active_lanes.len()).max(1);

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
            color_idx: col % 8,
            max_lanes,
            active_lanes: passing_lanes,
            connections_down,
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
    is_loading: bool,
    is_fetching: bool,
    new_branch_input: Entity<InputState>,
    new_tag_input: Entity<InputState>,
    dialog_mode: Option<GitGraphDialog>,
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
            is_loading: false,
            is_fetching: false,
            new_branch_input,
            new_tag_input,
            dialog_mode: None,
        };

        view.reload_graph(cx);
        view
    }

    pub fn reload_graph(&mut self, cx: &mut Context<Self>) {
        let cwd = self.app_state.read(cx).active_cwd.clone();
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
        let is_git = self.app_state.read(cx).active_branch.is_some() || !self.commits.is_empty();

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
        let mut list_rows = v_flex()
            .id("git-graph-list")
            .size_full()
            .overflow_y_scroll()
            .min_h_0();

        for commit in filtered_commits {
            let hash_c = commit.hash.clone();
            let short_h = commit.short_hash.clone();
            let is_selected = self
                .selected_commit
                .as_ref()
                .is_some_and(|sc| sc.hash == commit.hash);
            let col = commit.col;
            let color = lane_color(commit.color_idx);
            let lane_w = px(16.);
            let graph_total_w = px((commit.max_lanes + 1) as f32 * 16.0);

            // Construct DAG Lane Element
            let mut graph_box = div()
                .relative()
                .h(px(32.))
                .w(graph_total_w.max(px(36.)))
                .flex_shrink_0();

            // Draw passing lines
            for (lane_ix, &is_active) in commit.active_lanes.iter().enumerate() {
                if is_active {
                    let l_color = lane_color(lane_ix);
                    graph_box = graph_box.child(
                        div()
                            .absolute()
                            .left(lane_w * lane_ix as f32 + px(7.))
                            .top_0()
                            .bottom_0()
                            .w(px(2.))
                            .bg(l_color.opacity(0.85)),
                    );
                }
            }

            // Draw connection lines downward
            for &(from_l, to_l) in &commit.connections_down {
                let conn_color = lane_color(from_l);
                if from_l == to_l {
                    graph_box = graph_box.child(
                        div()
                            .absolute()
                            .left(lane_w * from_l as f32 + px(7.))
                            .top(px(16.))
                            .bottom_0()
                            .w(px(2.))
                            .bg(conn_color),
                    );
                } else {
                    let left_col = from_l.min(to_l);
                    let right_col = from_l.max(to_l);
                    let line_left = lane_w * left_col as f32 + px(7.);
                    let line_w = lane_w * (right_col - left_col) as f32;
                    graph_box = graph_box.child(
                        div()
                            .absolute()
                            .left(line_left)
                            .top(px(16.))
                            .w(line_w)
                            .h(px(2.))
                            .bg(conn_color.opacity(0.75)),
                    );
                }
            }

            // Draw Commit Circle Node
            let is_head = commit.refs.iter().any(|r| r.is_head);
            graph_box = graph_box.child(
                div()
                    .absolute()
                    .left(lane_w * col as f32 + px(4.))
                    .top(px(12.))
                    .w(px(8.))
                    .h(px(8.))
                    .rounded_full()
                    .bg(color)
                    .when(is_head, |this| {
                        this.border_2().border_color(cx.theme().primary)
                    }),
            );

            // Ref Badges (HEAD, branch, tag, remote)
            let mut refs_stack = h_flex().gap_1().items_center().flex_shrink_0();
            for r in &commit.refs {
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
                refs_stack = refs_stack.child(badge_el);
            }

            let hash_copy = commit.hash.clone();
            let row_id: ElementId = SharedString::from(format!("git-row-{}", commit.hash)).into();
            let btn_hash_id: ElementId = SharedString::from(format!("git-btn-{}", commit.hash)).into();

            let commit_row = h_flex()
                .id(row_id)
                .w_full()
                .h(px(32.))
                .px_2()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .bg(if is_selected {
                    cx.theme().selection.opacity(0.35)
                } else {
                    cx.theme().transparent
                })
                .hover(|s| s.bg(cx.theme().secondary.opacity(0.5)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_commit(&hash_c, cx);
                }))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .items_center()
                        .child(graph_box)
                        .children((!commit.refs.is_empty()).then(|| refs_stack))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
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
                        .gap_3()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(commit.author_name.clone())),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground.opacity(0.75))
                                .child(SharedString::from(commit.date_str.clone())),
                        )
                        .child(
                            Button::new(btn_hash_id)
                                .ghost()
                                .xsmall()
                                .label(short_h)
                                .tooltip(tr(locale, "git.copyHash"))
                                .on_click(move |_, window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(hash_copy.clone()));
                                    window.push_notification(Notification::success(tr(locale, "common.copied")), cx);
                                }),
                        ),
                );

            list_rows = list_rows.child(commit_row);

            // If selected, render commit details card directly inline below row
            if is_selected {
                let ent_dlg_b = cx.entity().clone();
                let ent_dlg_t = cx.entity().clone();
                let ent_co = cx.entity().clone();
                let h_full = commit.hash.clone();
                let h_co = commit.hash.clone();
                let h_br = commit.hash.clone();
                let h_tag = commit.hash.clone();

                let actions_bar = h_flex()
                    .gap_2()
                    .items_center()
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
                    );

                let mut files_view = v_flex().gap_1().w_full();
                for (f_ix, f) in self.commit_files.iter().enumerate() {
                    let p_clone = f.path.clone();
                    let app_for_diff = self.app_state.clone();
                    let f_row_id: ElementId = SharedString::from(format!("git-file-{f_ix}-{}", f.path)).into();
                    files_view = files_view.child(
                        h_flex()
                            .id(f_row_id)
                            .w_full()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .justify_between()
                            .items_center()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                let p = p_clone.clone();
                                app_for_diff.update(cx, |s, cx| {
                                    let full_path = s.active_cwd.join(&p);
                                    s.open_file(full_path, cx);
                                });
                            })
                            .child(
                                h_flex()
                                    .gap_1p5()
                                    .items_center()
                                    .child(Icon::new(IconName::FileText).xsmall().text_color(cx.theme().muted_foreground))
                                    .child(div().text_xs().child(SharedString::from(f.path.clone()))),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(div().text_xs().text_color(cx.theme().success).child(format!("+{}", f.added)))
                                    .child(div().text_xs().text_color(cx.theme().danger).child(format!("-{}", f.deleted))),
                            ),
                    );
                }

                let details_panel = v_flex()
                    .w_full()
                    .p_3()
                    .gap_2p5()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover.opacity(0.85))
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_xs().font_semibold().child("Commit:"))
                                    .child(div().text_xs().font_family(cx.theme().font_family.clone()).child(SharedString::from(h_full))),
                            )
                            .child(actions_bar),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} <{}> · {}",
                                commit.author_name, commit.author_email, commit.date_str
                            )),
                    )
                    .children((!commit.body.is_empty()).then(|| {
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().secondary.opacity(0.5))
                            .text_xs()
                            .text_color(cx.theme().foreground)
                            .child(SharedString::from(commit.body.clone()))
                    }))
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
                    .child(files_view);

                list_rows = list_rows.child(details_panel);
            }
        }

        let mut root = v_flex().size_full().child(toolbar).child(list_rows);

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
