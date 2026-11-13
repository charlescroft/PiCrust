use crate::app_state::AppState;
use crate::client::session_controller::format_compact_relative_time;
use crate::client::types::{ScheduledRunView, ScheduledTask, TaskScheduleSpec};
use crate::i18n::{AppLocale, format_commas, tr};
use chrono::{DateTime, Local};
use gpui_kit::component::badge::Badge;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, PathPromptOptions, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScheduledViewMode {
    List,
    Detail(String),
    Edit(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleKind {
    Daily,
    Weekdays,
    Weekly,
    Hourly,
    Once,
    Custom,
    Manual,
}

impl ScheduleKind {
    fn all() -> &'static [Self] {
        &[
            Self::Daily,
            Self::Weekdays,
            Self::Weekly,
            Self::Hourly,
            Self::Once,
            Self::Custom,
            Self::Manual,
        ]
    }

    fn i18n_key(&self) -> &'static str {
        match self {
            Self::Daily => "scheduled.schedule.daily",
            Self::Weekdays => "scheduled.schedule.weekdays",
            Self::Weekly => "scheduled.schedule.weekly",
            Self::Hourly => "scheduled.schedule.hourly",
            Self::Once => "scheduled.schedule.once",
            Self::Custom => "scheduled.schedule.custom",
            Self::Manual => "scheduled.schedule.manual",
        }
    }
}

pub struct ScheduledPageView {
    app_state: Entity<AppState>,
    mode: ScheduledViewMode,
    busy: bool,
    action_error: Option<String>,
    confirm_delete_id: Option<String>,

    // Run history state for detail view
    history_task_id: Option<String>,
    runs: Vec<ScheduledRunView>,
    runs_has_more: bool,
    runs_error: Option<String>,

    // Editor fields matching ScheduledTaskEditor.tsx
    editing_task_id: Option<String>,
    name_input: Entity<InputState>,
    desc_input: Entity<InputState>,
    prompt_input: Entity<TextareaState>,
    cwd_input: Entity<InputState>,
    time_input: Entity<InputState>,
    minute_input: Entity<InputState>,
    once_input: Entity<InputState>,
    cron_input: Entity<InputState>,
    max_duration_input: Entity<InputState>,
    schedule_kind: ScheduleKind,
    weekday: usize,
    model_key: Option<(String, String)>,
    thinking_level: Option<String>,
    tool_preset: String,
    acknowledged_writes: bool,
    cron_preview_runs: Vec<String>,
    cron_preview_error: Option<String>,
}

impl Render for ScheduledPageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_page(window, cx)
    }
}

impl ScheduledPageView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_cwd = app_state.read(cx).active_cwd.to_string_lossy().into_owned();
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Daily repo summary"));
        let desc_input = cx.new(|cx| InputState::new(window, cx));
        let prompt_input = cx.new(|cx| TextareaState::new(window, cx).rows(6));
        let cwd_input = cx.new(|cx| InputState::new(window, cx).default_value(default_cwd));
        let time_input = cx.new(|cx| InputState::new(window, cx).default_value("09:00"));
        let minute_input = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let once_input = cx.new(|cx| InputState::new(window, cx).placeholder("2026-05-01T09:00"));
        let cron_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0 9 * * 1-5")
                .default_value("0 9 * * 1-5")
        });
        let max_duration_input = cx.new(|cx| InputState::new(window, cx).default_value("30"));

        Self {
            app_state,
            mode: ScheduledViewMode::List,
            busy: false,
            action_error: None,
            confirm_delete_id: None,
            history_task_id: None,
            runs: Vec::new(),
            runs_has_more: false,
            runs_error: None,
            editing_task_id: None,
            name_input,
            desc_input,
            prompt_input,
            cwd_input,
            time_input,
            minute_input,
            once_input,
            cron_input,
            max_duration_input,
            schedule_kind: ScheduleKind::Daily,
            weekday: 1,
            model_key: None,
            thinking_level: None,
            tool_preset: "read-only".to_string(),
            acknowledged_writes: false,
            cron_preview_runs: Vec::new(),
            cron_preview_error: None,
        }
    }

    fn open_editor(&mut self, task: Option<&ScheduledTask>, window: &mut Window, cx: &mut Context<Self>) {
        let default_cwd = self
            .app_state
            .read(cx)
            .active_cwd
            .to_string_lossy()
            .into_owned();
        self.action_error = None;
        self.cron_preview_runs.clear();
        self.cron_preview_error = None;

        if let Some(t) = task {
            self.editing_task_id = Some(t.id.clone());
            self.name_input
                .update(cx, |s, cx| s.set_value(&t.name, window, cx));
            self.desc_input.update(cx, |s, cx| {
                s.set_value(t.description.as_deref().unwrap_or(""), window, cx)
            });
            self.prompt_input
                .update(cx, |s, cx| s.set_value(&t.prompt, window, cx));
            self.cwd_input
                .update(cx, |s, cx| s.set_value(&t.cwd, window, cx));
            self.max_duration_input.update(cx, |s, cx| {
                s.set_value(&t.max_duration_min.to_string(), window, cx)
            });
            self.model_key = t
                .model
                .as_ref()
                .map(|m| (m.provider.clone(), m.model_id.clone()));
            self.thinking_level = t.thinking_level.clone();
            self.tool_preset = t.tool_preset.clone();
            self.acknowledged_writes = false;

            match &t.schedule {
                TaskScheduleSpec::Manual => {
                    self.schedule_kind = ScheduleKind::Manual;
                }
                TaskScheduleSpec::Once { at } => {
                    self.schedule_kind = ScheduleKind::Once;
                    self.once_input.update(cx, |s, cx| s.set_value(at, window, cx));
                }
                TaskScheduleSpec::Cron { expr, .. } => {
                    self.cron_input
                        .update(cx, |s, cx| s.set_value(expr, window, cx));
                    if let Some((kind, time_str, min_str, wd)) = parse_preset_from_cron(expr) {
                        self.schedule_kind = kind;
                        self.time_input
                            .update(cx, |s, cx| s.set_value(&time_str, window, cx));
                        self.minute_input
                            .update(cx, |s, cx| s.set_value(&min_str, window, cx));
                        self.weekday = wd;
                    } else {
                        self.schedule_kind = ScheduleKind::Custom;
                    }
                }
            }
            self.mode = ScheduledViewMode::Edit(Some(t.id.clone()));
        } else {
            self.editing_task_id = None;
            self.name_input.update(cx, |s, cx| s.set_value("", window, cx));
            self.desc_input.update(cx, |s, cx| s.set_value("", window, cx));
            self.prompt_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.cwd_input
                .update(cx, |s, cx| s.set_value(&default_cwd, window, cx));
            self.time_input
                .update(cx, |s, cx| s.set_value("09:00", window, cx));
            self.minute_input
                .update(cx, |s, cx| s.set_value("0", window, cx));
            self.once_input.update(cx, |s, cx| s.set_value("", window, cx));
            self.cron_input
                .update(cx, |s, cx| s.set_value("0 9 * * 1-5", window, cx));
            self.max_duration_input
                .update(cx, |s, cx| s.set_value("30", window, cx));
            self.schedule_kind = ScheduleKind::Daily;
            self.weekday = 1;
            self.model_key = None;
            self.thinking_level = None;
            self.tool_preset = "read-only".to_string();
            self.acknowledged_writes = false;
            self.mode = ScheduledViewMode::Edit(None);
        }
        self.preview_cron_now(cx);
        cx.notify();
    }

    fn current_cron_expr(&self, cx: &gpui_kit::App) -> Option<String> {
        match self.schedule_kind {
            ScheduleKind::Manual | ScheduleKind::Once => None,
            ScheduleKind::Custom => {
                let s = self.cron_input.read(cx).value().trim().to_string();
                (!s.is_empty()).then_some(s)
            }
            ScheduleKind::Hourly => {
                let m = self
                    .minute_input
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(0)
                    .min(59);
                Some(format!("{m} * * * *"))
            }
            ScheduleKind::Daily | ScheduleKind::Weekdays | ScheduleKind::Weekly => {
                let time_val = self.time_input.read(cx).value().to_string();
                let (hh, mm) = parse_hhmm(&time_val).unwrap_or((9, 0));
                match self.schedule_kind {
                    ScheduleKind::Daily => Some(format!("{mm} {hh} * * *")),
                    ScheduleKind::Weekdays => Some(format!("{mm} {hh} * * 1-5")),
                    ScheduleKind::Weekly => Some(format!("{mm} {hh} * * {}", self.weekday)),
                    _ => None,
                }
            }
        }
    }

    fn preview_cron_now(&mut self, cx: &mut Context<Self>) {
        let Some(expr) = self.current_cron_expr(cx) else {
            self.cron_preview_runs.clear();
            self.cron_preview_error = None;
            return;
        };
        if !self.app_state.read(cx).sidecar_connected {
            return;
        }
        let client = self.app_state.read(cx).client.clone();
        let tz = local_timezone_name();
        cx.spawn(async move |this, cx| {
            if let Ok(val) = client.preview_scheduled_cron(&expr, &tz).await {
                let _ = this.update(cx, |view, cx| {
                    if val.get("valid").and_then(Value::as_bool) == Some(true) {
                        view.cron_preview_error = None;
                        view.cron_preview_runs = val
                            .get("nextRuns")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(Value::as_str)
                                    .map(String::from)
                                    .collect()
                            })
                            .unwrap_or_default();
                    } else {
                        view.cron_preview_runs.clear();
                        view.cron_preview_error = val
                            .get("error")
                            .and_then(Value::as_str)
                            .map(String::from);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn submit_editor(&mut self, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let desc = self.desc_input.read(cx).value().trim().to_string();
        let prompt = self.prompt_input.read(cx).value().to_string();
        let cwd = self.cwd_input.read(cx).value().trim().to_string();
        if name.is_empty() || prompt.trim().is_empty() || cwd.is_empty() {
            return;
        }
        let max_dur = self
            .max_duration_input
            .read(cx)
            .value()
            .trim()
            .parse::<u64>()
            .unwrap_or(30)
            .clamp(1, 24 * 60);

        let schedule_val = match self.schedule_kind {
            ScheduleKind::Manual => json!({ "kind": "manual" }),
            ScheduleKind::Once => {
                let raw = self.once_input.read(cx).value().trim().to_string();
                let iso = parse_once_to_iso(&raw).unwrap_or(raw);
                json!({ "kind": "once", "at": iso })
            }
            _ => {
                let expr = self
                    .current_cron_expr(cx)
                    .unwrap_or_else(|| "0 9 * * *".to_string());
                json!({ "kind": "cron", "expr": expr, "timezone": local_timezone_name() })
            }
        };

        let model_val = self
            .model_key
            .as_ref()
            .map(|(p, id)| json!({ "provider": p, "modelId": id }))
            .unwrap_or(Value::Null);
        let thinking_val = self
            .thinking_level
            .as_ref()
            .map(|t| Value::String(t.clone()))
            .unwrap_or(Value::Null);

        let mut payload = json!({
            "name": name,
            "description": if desc.is_empty() { Value::Null } else { Value::String(desc) },
            "prompt": prompt,
            "cwd": cwd,
            "schedule": schedule_val,
            "model": model_val,
            "thinkingLevel": thinking_val,
            "toolPreset": self.tool_preset,
            "maxDurationMin": max_dur,
        });
        if matches!(self.tool_preset.as_str(), "default" | "full") {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("acknowledgeUnattendedWrites".into(), json!(self.acknowledged_writes));
            }
        }

        let edit_id = self.editing_task_id.clone();
        let client = self.app_state.read(cx).client.clone();
        let app_state = self.app_state.clone();
        self.busy = true;
        self.action_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = match edit_id {
                Some(ref id) => client.update_scheduled_task(id, payload).await,
                None => client.create_scheduled_task_payload(payload).await,
            };
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match res {
                    Ok(val) => {
                        let new_id = val
                            .get("task")
                            .and_then(|t| t.get("id"))
                            .and_then(Value::as_str)
                            .map(String::from)
                            .or(edit_id);
                        app_state.update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                        if let Some(tid) = new_id {
                            view.load_run_history(tid.clone(), None, cx);
                            view.mode = ScheduledViewMode::Detail(tid);
                        } else {
                            view.mode = ScheduledViewMode::List;
                        }
                    }
                    Err(e) => {
                        view.action_error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_run_history(&mut self, task_id: String, before: Option<String>, cx: &mut Context<Self>) {
        if before.is_none() {
            self.history_task_id = Some(task_id.clone());
            self.runs.clear();
            self.runs_has_more = false;
            self.runs_error = None;
        }
        if !self.app_state.read(cx).sidecar_connected {
            return;
        }
        let client = self.app_state.read(cx).client.clone();
        cx.spawn(async move |this, cx| {
            let res = client.get_scheduled_runs(&task_id, 20, before.as_deref()).await;
            let _ = this.update(cx, |view, cx| {
                if view.history_task_id.as_deref() != Some(task_id.as_str()) {
                    return;
                }
                match res {
                    Ok(val) => {
                        let page: Vec<ScheduledRunView> = val
                            .get("runs")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                    .collect()
                            })
                            .unwrap_or_default();
                        if before.is_some() {
                            view.runs.extend(page);
                        } else {
                            view.runs = page;
                        }
                        view.runs_has_more = val
                            .get("hasMore")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        view.runs_error = None;
                    }
                    Err(e) => {
                        view.runs_error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn render_page(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (tasks, locale, running_ids, owner_status, models) = {
            let s = self.app_state.read(cx);
            (
                s.scheduled_tasks.clone(),
                s.locale,
                s.scheduled_running_ids.clone(),
                s.scheduler_owner.clone(),
                s.models.clone(),
            )
        };

        let selected_task = match &self.mode {
            ScheduledViewMode::Detail(id) => tasks.iter().find(|t| &t.id == id).cloned(),
            ScheduledViewMode::Edit(Some(id)) => tasks.iter().find(|t| &t.id == id).cloned(),
            _ => None,
        };
        if matches!(self.mode, ScheduledViewMode::Detail(_)) && selected_task.is_none() {
            self.mode = ScheduledViewMode::List;
        }
        if let ScheduledViewMode::Detail(ref id) = self.mode {
            if self.history_task_id.as_deref() != Some(id.as_str()) {
                self.load_run_history(id.clone(), None, cx);
            }
        }

        let owner_banner = (!owner_status.owner).then(|| {
            let app_takeover = self.app_state.clone();
            h_flex()
                .w_full()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().warning.opacity(0.5))
                .bg(cx.theme().secondary)
                .justify_between()
                .items_center()
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::Info).small())
                        .child(div().text_xs().child(tr(locale, "scheduled.ownerNotice"))),
                )
                .child(
                    Button::new("sched-takeover")
                        .outline()
                        .small()
                        .h(px(28.))
                        .label(tr(locale, "scheduled.takeOver"))
                        .on_click(move |_, _, cx| {
                            app_takeover.update(cx, |s, cx| {
                                let client = s.client.clone();
                                let ent = cx.entity().clone();
                                cx.spawn(async move |_, cx| {
                                    let _ = client.takeover_scheduler().await;
                                    let _ = ent.update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                                })
                                .detach();
                            });
                        }),
                )
        });

        let mut shell = v_flex()
            .id("scheduled-scroll-shell")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_4();

        match &self.mode {
            ScheduledViewMode::List => {
                let app_refresh = self.app_state.clone();
                shell = shell
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_semibold()
                                            .child(tr(locale, "page.scheduledTitle")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "scheduled.subtitle")),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("sched-refresh")
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .icon(IconName::RefreshCw)
                                            .label(tr(locale, "page.refresh"))
                                            .on_click(move |_, _, cx| {
                                                app_refresh
                                                    .update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                                            }),
                                    )
                                    .child(
                                        Button::new("sched-new")
                                            .primary()
                                            .small()
                                            .h(px(28.))
                                            .icon(IconName::Plus)
                                            .label(tr(locale, "scheduled.new"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_editor(None, window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .children(owner_banner);

                if tasks.is_empty() {
                    shell = shell.child(
                        div().flex_1().flex().items_center().justify_center().child(
                            Empty::new().header(
                                EmptyHeader::new()
                                    .media(EmptyMedia::new().child(Icon::new(IconName::Calendar)))
                                    .title(EmptyTitle::new().child(tr(locale, "page.noScheduled")))
                                    .description(
                                        EmptyDescription::new().child(tr(locale, "page.noScheduledDesc")),
                                    ),
                            ),
                        ),
                    );
                } else {
                    let mut list = v_flex().gap_2();
                    for (idx, t) in tasks.iter().enumerate() {
                        let tid = t.id.clone();
                        let status = task_status(t, &running_ids);
                        let dot_color = status_color(status, cx);
                        let status_text = tr(locale, status_i18n_key(status));
                        let sched_desc = describe_schedule(&t.schedule, locale);
                        let time_meta = if let Some(ref next) = t.next_run_at {
                            format!("{}: {}", tr(locale, "scheduled.nextRun"), format_local_dt(next))
                        } else {
                            let last = t
                                .last_run
                                .as_ref()
                                .and_then(|r| r.started_at.as_deref().or(r.scheduled_for.as_deref()));
                            let rel = last
                                .map(|s| format_compact_relative_time(Some(s)).to_string())
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| tr(locale, "scheduled.never").to_string());
                            format!("{}: {rel}", tr(locale, "scheduled.lastRun"))
                        };

                        list = list.child(
                            h_flex()
                                .id(("sched-card", idx))
                                .w_full()
                                .p_3()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .cursor_pointer()
                                .hover(|s| s.bg(cx.theme().secondary.opacity(0.85)))
                                .justify_between()
                                .items_center()
                                .gap_3()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.action_error = None;
                                    this.load_run_history(tid.clone(), None, cx);
                                    this.mode = ScheduledViewMode::Detail(tid.clone());
                                    cx.notify();
                                }))
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .items_center()
                                        .gap_2p5()
                                        .child(Badge::new().dot().color(dot_color))
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w_0()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_medium()
                                                        .truncate()
                                                        .child(SharedString::from(t.name.clone())),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .truncate()
                                                        .child(SharedString::from(sched_desc)),
                                                ),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_2p5()
                                        .child(Tag::secondary().small().child(status_text))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(time_meta)),
                                        )
                                        .children((t.unread_runs > 0).then(|| {
                                            Tag::primary()
                                                .small()
                                                .child(SharedString::from(format_commas(t.unread_runs)))
                                        })),
                                ),
                        );
                    }
                    shell = shell.child(list);
                }
            }

            ScheduledViewMode::Detail(task_id) => {
                let task = selected_task.unwrap();
                let status = task_status(&task, &running_ids);
                let is_running = running_ids.contains(&task.id)
                    || task.last_run.as_ref().is_some_and(|r| r.status == "running");
                let tid_run = task.id.clone();
                let tid_toggle = task.id.clone();
                let tid_del = task.id.clone();
                let next_enabled = !task.enabled;
                let task_for_edit = task.clone();
                let is_manual = matches!(task.schedule, TaskScheduleSpec::Manual);
                let is_done = status == "done";

                shell = shell
                    .child(
                        h_flex().child(
                            Button::new("sched-back-list")
                                .ghost()
                                .small()
                                .h(px(28.))
                                .icon(IconName::ArrowLeft)
                                .label(tr(locale, "scheduled.back"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.mode = ScheduledViewMode::List;
                                    this.action_error = None;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_start()
                            .gap_3()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_semibold()
                                            .child(SharedString::from(task.name.clone())),
                                    )
                                    .children(task.description.as_ref().filter(|d| !d.is_empty()).map(
                                        |d| {
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(d.clone()))
                                        },
                                    )),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("sched-detail-run")
                                            .primary()
                                            .small()
                                            .h(px(28.))
                                            .disabled(self.busy || is_running)
                                            .label(tr(locale, "scheduled.runNow"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.run_task_now(tid_run.clone(), cx);
                                            })),
                                    )
                                    .children((!is_manual && !is_done).then(|| {
                                        Button::new("sched-detail-toggle")
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .disabled(self.busy)
                                            .label(if task.enabled {
                                                tr(locale, "scheduled.pause")
                                            } else {
                                                tr(locale, "scheduled.resume")
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.toggle_task_enabled(
                                                    tid_toggle.clone(),
                                                    next_enabled,
                                                    cx,
                                                );
                                            }))
                                    }))
                                    .child(
                                        Button::new("sched-detail-edit")
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "chat.edit"))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_editor(Some(&task_for_edit), window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("sched-detail-delete")
                                            .danger()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "scheduled.delete"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.confirm_delete_id = Some(tid_del.clone());
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .children(self.action_error.as_ref().map(|err| {
                        div()
                            .p_2p5()
                            .rounded_md()
                            .bg(cx.theme().danger.opacity(0.12))
                            .border_1()
                            .border_color(cx.theme().danger)
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(SharedString::from(err.clone()))
                    }))
                    .children(task.auto_paused_reason.as_ref().map(|reason| {
                        div()
                            .p_2p5()
                            .rounded_md()
                            .bg(cx.theme().warning.opacity(0.12))
                            .border_1()
                            .border_color(cx.theme().warning)
                            .text_xs()
                            .child(SharedString::from(reason.clone()))
                    }))
                    .children(owner_banner);

                let last_run_str = task
                    .last_run
                    .as_ref()
                    .and_then(|r| r.started_at.as_deref().or(r.scheduled_for.as_deref()))
                    .map(|s| format_compact_relative_time(Some(s)).to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| tr(locale, "scheduled.never").to_string());
                let next_run_str = task
                    .next_run_at
                    .as_deref()
                    .map(format_local_dt)
                    .unwrap_or_else(|| "—".to_string());
                let model_str = match (&task.model, &task.thinking_level) {
                    (Some(m), Some(th)) => format!("{}/{} · {th}", m.provider, m.model_id),
                    (Some(m), None) => format!("{}/{}", m.provider, m.model_id),
                    (None, Some(th)) => format!("{} · {th}", tr(locale, "scheduled.model.default")),
                    (None, None) => tr(locale, "scheduled.model.default").to_string(),
                };

                shell = shell
                    .child(
                        v_flex()
                            .p_3()
                            .gap_2()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(fact_row(
                                tr(locale, "scheduled.field.schedule"),
                                SharedString::from(describe_schedule(&task.schedule, locale)),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.nextRun"),
                                SharedString::from(next_run_str),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.lastRun"),
                                SharedString::from(last_run_str),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.field.folder"),
                                SharedString::from(task.cwd.clone()),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.field.tools"),
                                SharedString::from(task.tool_preset.clone()),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.field.model"),
                                SharedString::from(model_str),
                                cx,
                            ))
                            .child(fact_row(
                                tr(locale, "scheduled.field.timeLimit"),
                                SharedString::from(format!(
                                    "{} {}",
                                    format_commas(task.max_duration_min),
                                    tr(locale, "scheduled.field.timeLimitMinutes")
                                )),
                                cx,
                            )),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "scheduled.field.prompt")),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .p_3()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary)
                                    .text_xs()
                                    .child(SharedString::from(task.prompt.clone())),
                            ),
                    );

                // Run history section
                let mut hist = v_flex().gap_2().child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(locale, "scheduled.history.title")),
                );
                if let Some(ref rerr) = self.runs_error {
                    hist = hist.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(SharedString::from(rerr.clone())),
                    );
                } else if self.runs.is_empty() {
                    hist = hist.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "scheduled.history.empty")),
                    );
                } else {
                    for (r_idx, run) in self.runs.iter().enumerate() {
                        let dot_col = match run.status.as_str() {
                            "succeeded" => cx.theme().success,
                            "running" => cx.theme().primary,
                            "failed" | "aborted" => cx.theme().danger,
                            _ => cx.theme().muted_foreground,
                        };
                        let ts = run
                            .started_at
                            .as_deref()
                            .or(run.scheduled_for.as_deref())
                            .map(format_local_dt)
                            .unwrap_or_default();
                        let app_open = self.app_state.clone();
                        let run_sid = run.session_id.clone();
                        let tid_seen = task_id.clone();
                        let rid_seen = run.run_id.clone();

                        hist = hist.child(
                            h_flex()
                                .p_2p5()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .justify_between()
                                .items_center()
                                .gap_2()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(Badge::new().dot().color(dot_col))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_semibold()
                                                        .child(SharedString::from(run.status.clone())),
                                                )
                                                .child(
                                                    Tag::secondary()
                                                        .small()
                                                        .child(SharedString::from(run.trigger.clone())),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(SharedString::from(ts)),
                                                ),
                                        )
                                        .children(run.error.as_ref().map(|e| {
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().danger)
                                                .child(SharedString::from(e.clone()))
                                        })),
                                )
                                .children(run_sid.map(|sid| {
                                    Button::new(("sched-open-run", r_idx))
                                        .outline()
                                        .small()
                                        .h(px(26.))
                                        .label(tr(locale, "scheduled.history.open"))
                                        .on_click(move |_, _, cx| {
                                            let client = app_open.read(cx).client.clone();
                                            let t_id = tid_seen.clone();
                                            let r_id = rid_seen.clone();
                                            cx.spawn(async move |_| {
                                                let _ = client.mark_scheduled_run_seen(&t_id, &r_id).await;
                                            })
                                            .detach();
                                            app_open.update(cx, |s, cx| {
                                                s.select_session(sid.clone(), cx);
                                            });
                                        })
                                })),
                        );
                    }
                    if self.runs_has_more {
                        if let Some(last_id) = self.runs.last().map(|r| r.run_id.clone()) {
                            let tid_more = task_id.clone();
                            hist = hist.child(
                                Button::new("sched-load-more")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .label(tr(locale, "scheduled.history.loadMore"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.load_run_history(
                                            tid_more.clone(),
                                            Some(last_id.clone()),
                                            cx,
                                        );
                                    })),
                            );
                        }
                    }
                }
                shell = shell.child(hist);
            }

            ScheduledViewMode::Edit(edit_opt) => {
                let back_label = selected_task
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| tr(locale, "scheduled.back").to_string());
                let back_target = edit_opt
                    .clone()
                    .map(ScheduledViewMode::Detail)
                    .unwrap_or(ScheduledViewMode::List);
                let is_edit = edit_opt.is_some();

                let mut kind_chips = h_flex().flex_wrap().gap_1p5();
                for (k_idx, &kind) in ScheduleKind::all().iter().enumerate() {
                    kind_chips = kind_chips.child(
                        Button::new(("sched-kind-chip", k_idx))
                            .outline()
                            .small()
                            .h(px(28.))
                            .selected(self.schedule_kind == kind)
                            .label(tr(locale, kind.i18n_key()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.schedule_kind = kind;
                                this.preview_cron_now(cx);
                                cx.notify();
                            })),
                    );
                }

                let mut sched_inputs = h_flex().flex_wrap().items_center().gap_3();
                match self.schedule_kind {
                    ScheduleKind::Weekly => {
                        let weekdays = if locale == AppLocale::ZhCn {
                            ["周日", "周一", "周二", "周三", "周四", "周五", "周六"]
                        } else {
                            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                        };
                        let wd_label = weekdays.get(self.weekday).copied().unwrap_or("Mon");
                        let entity_wd = cx.entity().clone();
                        sched_inputs = sched_inputs
                            .child(
                                Button::new("sched-wd-btn")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .label(format!("{}: {wd_label}", tr(locale, "scheduled.schedule.day")))
                                    .dropdown_menu(move |mut menu, _, _| {
                                        for (w_idx, w_lbl) in weekdays.iter().enumerate() {
                                            let e_wd = entity_wd.clone();
                                            menu = menu.item(PopupMenuItem::new(*w_lbl).on_click(
                                                move |_, _, cx| {
                                                    e_wd.update(cx, |v, cx| {
                                                        v.weekday = w_idx;
                                                        v.preview_cron_now(cx);
                                                        cx.notify();
                                                    });
                                                },
                                            ));
                                        }
                                        menu
                                    }),
                            )
                            .child(div().w(px(120.)).child(Input::new(&self.time_input).small()));
                    }
                    ScheduleKind::Daily | ScheduleKind::Weekdays => {
                        sched_inputs = sched_inputs.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_xs().child(tr(locale, "scheduled.schedule.time")))
                                .child(div().w(px(120.)).child(Input::new(&self.time_input).small())),
                        );
                    }
                    ScheduleKind::Hourly => {
                        sched_inputs = sched_inputs.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_xs().child(tr(locale, "scheduled.schedule.minute")))
                                .child(div().w(px(90.)).child(Input::new(&self.minute_input).small())),
                        );
                    }
                    ScheduleKind::Once => {
                        sched_inputs = sched_inputs.child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_xs().child(tr(locale, "scheduled.schedule.at")))
                                .child(div().w(px(220.)).child(Input::new(&self.once_input).small())),
                        );
                    }
                    ScheduleKind::Custom => {
                        sched_inputs = sched_inputs
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_xs().child(tr(locale, "scheduled.schedule.cron")))
                                    .child(div().w(px(200.)).child(Input::new(&self.cron_input).small())),
                            )
                            .child(
                                Button::new("sched-cron-preview-btn")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .label(tr(locale, "scheduled.preview.title"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.preview_cron_now(cx);
                                        cx.notify();
                                    })),
                            );
                    }
                    ScheduleKind::Manual => {}
                }

                let model_label = match &self.model_key {
                    Some((p, id)) => format!("{id} ({p})"),
                    None => tr(locale, "scheduled.model.default").to_string(),
                };
                let thinking_label = self
                    .thinking_level
                    .clone()
                    .unwrap_or_else(|| tr(locale, "scheduled.thinking.default").to_string());
                let entity_model = cx.entity().clone();
                let entity_think = cx.entity().clone();
                let models_for_menu = models.clone();

                let can_write = matches!(self.tool_preset.as_str(), "default" | "full");
                let preset_choices = ["read-only", "none", "default", "full"];
                let mut preset_row = h_flex().flex_wrap().gap_2();
                for (p_idx, preset_name) in preset_choices.iter().enumerate() {
                    let pname = (*preset_name).to_string();
                    preset_row = preset_row.child(
                        Button::new(("sched-preset-btn", p_idx))
                            .outline()
                            .small()
                            .h(px(28.))
                            .selected(self.tool_preset == pname)
                            .label(pname.clone())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tool_preset = pname.clone();
                                cx.notify();
                            })),
                    );
                }

                let missing_required = self.name_input.read(cx).value().trim().is_empty()
                    || self.prompt_input.read(cx).value().trim().is_empty()
                    || self.cwd_input.read(cx).value().trim().is_empty()
                    || (can_write && !is_edit && !self.acknowledged_writes);

                let back_t1 = back_target.clone();
                let back_t2 = back_target;

                shell = shell
                    .child(
                        h_flex().child(
                            Button::new("sched-edit-back")
                                .ghost()
                                .small()
                                .h(px(28.))
                                .icon(IconName::ArrowLeft)
                                .label(back_label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.mode = back_t1.clone();
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(div().text_lg().font_semibold().child(if is_edit {
                        tr(locale, "scheduled.edit")
                    } else {
                        tr(locale, "scheduled.new")
                    }))
                    .child(
                        v_flex()
                            .gap_3()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(form_field(
                                tr(locale, "scheduled.field.name"),
                                Input::new(&self.name_input).small(),
                                cx,
                            ))
                            .child(form_field(
                                tr(locale, "scheduled.field.description"),
                                Input::new(&self.desc_input).small(),
                                cx,
                            ))
                            .child(form_field(
                                tr(locale, "scheduled.field.prompt"),
                                Textarea::new(&self.prompt_input),
                                cx,
                            ))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "scheduled.field.folder")),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(div().flex_1().child(Input::new(&self.cwd_input).small()))
                                            .child(
                                                Button::new("sched-browse-cwd")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .label(tr(locale, "scheduled.field.browse"))
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        let rx = cx.prompt_for_paths(PathPromptOptions {
                                                            files: false,
                                                            directories: true,
                                                            multiple: false,
                                                            prompt: Some("Select Working Directory".into()),
                                                        });
                                                        let cwd_in = this.cwd_input.clone();
                                                        cx.spawn_in(window, async move |_, cx| {
                                                            if let Ok(Ok(Some(paths))) = rx.await {
                                                                if let Some(dir) = paths.into_iter().next() {
                                                                    let _ = cx.update(|win, cx| {
                                                                        cwd_in.update(cx, |s, cx| {
                                                                            s.set_value(
                                                                                dir.to_string_lossy().as_ref(),
                                                                                win,
                                                                                cx,
                                                                            );
                                                                        });
                                                                    });
                                                                }
                                                            }
                                                        })
                                                        .detach();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "scheduled.field.schedule")),
                                    )
                                    .child(kind_chips)
                                    .child(sched_inputs)
                                    .children(self.cron_preview_error.as_ref().map(|e| {
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().danger)
                                            .child(SharedString::from(e.clone()))
                                    }))
                                    .children((!self.cron_preview_runs.is_empty()).then(|| {
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .child(tr(locale, "scheduled.preview.title")),
                                            )
                                            .children(self.cron_preview_runs.iter().take(3).map(|r| {
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(SharedString::from(format_local_dt(r)))
                                            }))
                                    })),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "scheduled.field.model")),
                                            )
                                            .child(
                                                Button::new("sched-model-btn")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .label(model_label)
                                                    .dropdown_menu(move |mut menu, _, _| {
                                                        let e_def = entity_model.clone();
                                                        menu = menu.item(
                                                            PopupMenuItem::new(tr(
                                                                locale,
                                                                "scheduled.model.default",
                                                            ))
                                                            .on_click(move |_, _, cx| {
                                                                e_def.update(cx, |v, cx| {
                                                                    v.model_key = None;
                                                                    cx.notify();
                                                                });
                                                            }),
                                                        );
                                                        for m in &models_for_menu {
                                                            let e_m = entity_model.clone();
                                                            let p = m.provider.clone();
                                                            let id = m.id.clone();
                                                            let lbl = m.display_label();
                                                            menu = menu.item(
                                                                PopupMenuItem::new(lbl).on_click(
                                                                    move |_, _, cx| {
                                                                        e_m.update(cx, |v, cx| {
                                                                            v.model_key = Some((
                                                                                p.clone(),
                                                                                id.clone(),
                                                                            ));
                                                                            cx.notify();
                                                                        });
                                                                    },
                                                                ),
                                                            );
                                                        }
                                                        menu
                                                    }),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "scheduled.field.thinking")),
                                            )
                                            .child(
                                                Button::new("sched-think-btn")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .label(thinking_label)
                                                    .dropdown_menu(move |mut menu, _, _| {
                                                        let e_def = entity_think.clone();
                                                        menu = menu.item(
                                                            PopupMenuItem::new(tr(
                                                                locale,
                                                                "scheduled.thinking.default",
                                                            ))
                                                            .on_click(move |_, _, cx| {
                                                                e_def.update(cx, |v, cx| {
                                                                    v.thinking_level = None;
                                                                    cx.notify();
                                                                });
                                                            }),
                                                        );
                                                        for lvl in [
                                                            "off", "minimal", "low", "medium",
                                                            "high", "xhigh",
                                                        ] {
                                                            let e_l = entity_think.clone();
                                                            menu = menu.item(
                                                                PopupMenuItem::new(lvl).on_click(
                                                                    move |_, _, cx| {
                                                                        e_l.update(cx, |v, cx| {
                                                                            v.thinking_level = Some(
                                                                                lvl.to_string(),
                                                                            );
                                                                            cx.notify();
                                                                        });
                                                                    },
                                                                ),
                                                            );
                                                        }
                                                        menu
                                                    }),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .w(px(150.))
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "scheduled.field.timeLimit")),
                                            )
                                            .child(Input::new(&self.max_duration_input).small()),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "scheduled.field.tools")),
                                    )
                                    .child(preset_row)
                                    .children(can_write.then(|| {
                                        v_flex()
                                            .p_2p5()
                                            .gap_2()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().warning.opacity(0.5))
                                            .bg(cx.theme().warning.opacity(0.08))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .child(tr(locale, "scheduled.tools.writeWarning")),
                                            )
                                            .children((!is_edit).then(|| {
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        Switch::new("sched-ack-writes")
                                                            .checked(self.acknowledged_writes)
                                                            .on_click(cx.listener(|this, checked, _, cx| {
                                                                this.acknowledged_writes = *checked;
                                                                cx.notify();
                                                            })),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .child(tr(locale, "scheduled.tools.writeAck")),
                                                    )
                                            }))
                                    })),
                            )
                            .children(self.action_error.as_ref().map(|e| {
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().danger)
                                    .child(SharedString::from(e.clone()))
                            }))
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("sched-cancel-btn")
                                            .ghost()
                                            .small()
                                            .h(px(28.))
                                            .disabled(self.busy)
                                            .label(tr(locale, "scheduled.cancel"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.mode = back_t2.clone();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sched-save-btn")
                                            .primary()
                                            .small()
                                            .h(px(28.))
                                            .disabled(self.busy || missing_required)
                                            .label(if is_edit {
                                                tr(locale, "scheduled.save")
                                            } else {
                                                tr(locale, "scheduled.create")
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.submit_editor(cx);
                                            })),
                                    ),
                            ),
                    );
            }
        }

        let mut root = div().relative().size_full().child(shell);

        // Delete confirmation dialog matching ScheduledView.tsx
        if let Some(del_id) = self.confirm_delete_id.clone() {
            root = root.child(
                div()
                    .id("sched-del-backdrop")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().background.opacity(0.75))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        v_flex()
                            .w(px(380.))
                            .p_4()
                            .gap_3()
                            .rounded_xl()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().popover)
                            .shadow_lg()
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .child(tr(locale, "scheduled.confirmDeleteTitle")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "scheduled.confirmDeleteBody")),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("sched-del-cancel")
                                            .ghost()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "scheduled.cancel"))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.confirm_delete_id = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sched-del-confirm")
                                            .danger()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "scheduled.delete"))
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let id = del_id.clone();
                                                this.confirm_delete_id = None;
                                                this.delete_task_now(id, cx);
                                            })),
                                    ),
                            ),
                    ),
            );
        }

        root
    }

    fn run_task_now(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.app_state.read(cx).sidecar_connected {
            return;
        }
        self.busy = true;
        self.action_error = None;
        let client = self.app_state.read(cx).client.clone();
        let app_state = self.app_state.clone();
        cx.spawn(async move |this, cx| {
            let res = client.run_scheduled_task(&id).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                if let Err(e) = res {
                    view.action_error = Some(e.to_string());
                } else {
                    app_state.update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                    view.load_run_history(id, None, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_task_enabled(&mut self, id: String, enabled: bool, cx: &mut Context<Self>) {
        if !self.app_state.read(cx).sidecar_connected {
            return;
        }
        self.busy = true;
        let client = self.app_state.read(cx).client.clone();
        let app_state = self.app_state.clone();
        cx.spawn(async move |this, cx| {
            let res = client
                .update_scheduled_task(&id, json!({ "enabled": enabled }))
                .await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                if let Err(e) = res {
                    view.action_error = Some(e.to_string());
                } else {
                    app_state.update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn delete_task_now(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.app_state.read(cx).sidecar_connected {
            return;
        }
        self.busy = true;
        let client = self.app_state.read(cx).client.clone();
        let app_state = self.app_state.clone();
        cx.spawn(async move |this, cx| {
            let res = client.delete_scheduled_task(&id).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                if let Err(e) = res {
                    view.action_error = Some(e.to_string());
                } else {
                    app_state.update(cx, |s, cx| s.refresh_scheduled_tasks(cx));
                    view.mode = ScheduledViewMode::List;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn form_field(
    label: &'static str,
    control: impl IntoElement,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(control)
}

fn fact_row(
    label: &'static str,
    value: SharedString,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .gap_3()
        .text_xs()
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(div().flex_1().min_w_0().truncate().text_right().child(value))
}

fn task_status(task: &ScheduledTask, running: &std::collections::HashSet<String>) -> &'static str {
    if running.contains(&task.id)
        || task.last_run.as_ref().is_some_and(|r| r.status == "running")
    {
        return "running";
    }
    if task.auto_paused_reason.is_some() {
        return "attention";
    }
    if !task.enabled {
        return if matches!(task.schedule, TaskScheduleSpec::Once { .. }) && task.last_run.is_some()
        {
            "done"
        } else {
            "paused"
        };
    }
    if matches!(task.schedule, TaskScheduleSpec::Manual) {
        return "manual";
    }
    "active"
}

fn status_i18n_key(status: &str) -> &'static str {
    match status {
        "running" => "scheduled.status.running",
        "active" => "scheduled.status.active",
        "paused" => "scheduled.status.paused",
        "attention" => "scheduled.status.attention",
        "manual" => "scheduled.status.manual",
        _ => "scheduled.status.done",
    }
}

fn status_color(status: &str, cx: &gpui_kit::App) -> gpui_kit::Hsla {
    match status {
        "running" => cx.theme().primary,
        "active" => cx.theme().success,
        "attention" => cx.theme().warning,
        _ => cx.theme().muted_foreground,
    }
}

fn describe_schedule(sched: &TaskScheduleSpec, locale: AppLocale) -> String {
    match sched {
        TaskScheduleSpec::Manual => tr(locale, "scheduled.schedule.manual").to_string(),
        TaskScheduleSpec::Once { at } => format!("{} · {}", tr(locale, "scheduled.schedule.once"), format_local_dt(at)),
        TaskScheduleSpec::Cron { expr, .. } => {
            if let Some((kind, time_str, min_str, wd)) = parse_preset_from_cron(expr) {
                match kind {
                    ScheduleKind::Hourly => format!("{} (:{min_str})", tr(locale, "scheduled.schedule.hourly")),
                    ScheduleKind::Daily => format!("{} {time_str}", tr(locale, "scheduled.schedule.daily")),
                    ScheduleKind::Weekdays => format!("{} {time_str}", tr(locale, "scheduled.schedule.weekdays")),
                    ScheduleKind::Weekly => {
                        let days = if locale == AppLocale::ZhCn {
                            ["周日", "周一", "周二", "周三", "周四", "周五", "周六"]
                        } else {
                            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                        };
                        let d = days.get(wd).copied().unwrap_or("Mon");
                        format!("{} {d} {time_str}", tr(locale, "scheduled.schedule.weekly"))
                    }
                    _ => expr.clone(),
                }
            } else {
                expr.clone()
            }
        }
    }
}

fn parse_preset_from_cron(expr: &str) -> Option<(ScheduleKind, String, String, usize)> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }
    let min: u32 = parts[0].parse().ok()?;
    if parts[1] == "*" && parts[2] == "*" && parts[3] == "*" && parts[4] == "*" {
        return Some((
            ScheduleKind::Hourly,
            "09:00".to_string(),
            format!("{min:02}"),
            1,
        ));
    }
    let hour: u32 = parts[1].parse().ok()?;
    let time_str = format!("{hour:02}:{min:02}");
    if parts[2] == "*" && parts[3] == "*" {
        if parts[4] == "*" {
            return Some((ScheduleKind::Daily, time_str, format!("{min:02}"), 1));
        }
        if parts[4] == "1-5" {
            return Some((ScheduleKind::Weekdays, time_str, format!("{min:02}"), 1));
        }
        if let Ok(wd) = parts[4].parse::<usize>() {
            if wd < 7 {
                return Some((ScheduleKind::Weekly, time_str, format!("{min:02}"), wd));
            }
        }
    }
    None
}

fn parse_hhmm(raw: &str) -> Option<(u32, u32)> {
    let mut parts = raw.trim().split(':');
    let hh = parts.next()?.parse::<u32>().ok()?;
    let mm = parts.next()?.parse::<u32>().ok()?;
    if hh < 24 && mm < 60 {
        Some((hh, mm))
    } else {
        None
    }
}

fn parse_once_to_iso(raw: &str) -> Option<String> {
    if DateTime::parse_from_rfc3339(raw).is_ok() {
        return Some(raw.to_string());
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M") {
        if let Some(dt) = naive.and_local_timezone(Local).single() {
            return Some(dt.to_rfc3339());
        }
    }
    None
}

fn format_local_dt(iso: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return dt
            .with_timezone(&Local)
            .format("%b %d %H:%M")
            .to_string();
    }
    iso.to_string()
}

fn local_timezone_name() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let t = tz.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(target) = std::fs::read_link("/etc/localtime") {
        let s = target.to_string_lossy();
        if let Some(pos) = s.find("zoneinfo/") {
            let zone = &s[pos + "zoneinfo/".len()..];
            if !zone.is_empty() {
                return zone.to_string();
            }
        }
    }
    "UTC".to_string()
}
