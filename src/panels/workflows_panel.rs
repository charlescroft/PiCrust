use crate::app_state::AppState;
use crate::client::workspace_extras::{
    WorkflowDef, WorkflowScope, WorkflowStep, build_workflow_execution_prompt, delete_workflow,
    list_workflows, project_notes_available, sanitize_workflow_name, save_workflow,
    serialize_workflow_md,
};
use crate::i18n::tr;
use crate::icons::{PencilIcon, PlayIcon, TrashIcon};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{Select, SelectItem, SelectState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IndexPath, Selectable as _, Sizable as _,
    StyledExt as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use std::path::PathBuf;

#[derive(Clone)]
struct WorkflowSelectItem {
    name: String,
    label: SharedString,
}

impl SelectItem for WorkflowSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.name
    }
}

struct StepEditorRow {
    title: Entity<InputState>,
    instruction: Entity<TextareaState>,
}

pub struct WorkflowsPanelView {
    app_state: Entity<AppState>,
    scope: WorkflowScope,
    workflows: Vec<WorkflowDef>,
    selected_name: Option<String>,
    select_state: Entity<SelectState<Vec<WorkflowSelectItem>>>,
    args_input: Entity<InputState>,
    editing: bool,
    source_mode: bool,
    edit_name_input: Entity<InputState>,
    edit_desc_input: Entity<InputState>,
    edit_source_input: Entity<TextareaState>,
    edit_steps: Vec<StepEditorRow>,
    editing_existing_path: Option<PathBuf>,
    last_cwd: PathBuf,
}

impl WorkflowsPanelView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = app_state.read(cx).locale;
        let args_ph = tr(locale, "workflows.argsPlaceholder");
        let name_ph = tr(locale, "workflows.namePlaceholder");
        let desc_ph = tr(locale, "workflows.descPlaceholder");
        let src_ph = tr(locale, "workflows.sourcePlaceholder");
        let select_state = cx.new(|cx| SelectState::new(Vec::<WorkflowSelectItem>::new(), None, window, cx));
        let args_input = cx.new(|cx| InputState::new(window, cx).placeholder(args_ph));
        let edit_name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_ph));
        let edit_desc_input = cx.new(|cx| InputState::new(window, cx).placeholder(desc_ph));
        let edit_source_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(14)
                .placeholder(src_ph)
        });
        let last_cwd = app_state.read(cx).active_cwd.clone();

        let mut this = Self {
            app_state,
            scope: WorkflowScope::Global,
            workflows: Vec::new(),
            selected_name: None,
            select_state,
            args_input,
            editing: false,
            source_mode: false,
            edit_name_input,
            edit_desc_input,
            edit_source_input,
            edit_steps: Vec::new(),
            editing_existing_path: None,
            last_cwd,
        };
        this.reload(window, cx);
        this
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.app_state.read(cx);
        let cwd = st.active_cwd.clone();
        let kind = st.active_workspace_kind;
        self.last_cwd = cwd.clone();
        self.workflows = list_workflows(self.scope, &cwd, kind);

        let items: Vec<WorkflowSelectItem> = self
            .workflows
            .iter()
            .map(|w| WorkflowSelectItem {
                name: w.name.clone(),
                label: SharedString::from(w.name.clone()),
            })
            .collect();

        if self
            .selected_name
            .as_ref()
            .is_none_or(|n| !self.workflows.iter().any(|w| &w.name == n))
        {
            self.selected_name = self.workflows.first().map(|w| w.name.clone());
        }

        let sel_idx = self
            .selected_name
            .as_ref()
            .and_then(|n| items.iter().position(|it| &it.name == n))
            .map(|i| IndexPath::default().row(i));

        self.select_state.update(cx, |ss, cx| {
            ss.set_items(items, window, cx);
            ss.set_selected_index(sel_idx, window, cx);
        });
    }

    fn current_workflow(&self) -> Option<&WorkflowDef> {
        self.selected_name
            .as_ref()
            .and_then(|n| self.workflows.iter().find(|w| &w.name == n))
    }

    fn enter_new_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        self.source_mode = false;
        self.editing_existing_path = None;
        self.edit_name_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.edit_desc_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        let locale = self.app_state.read(cx).locale;
        let title_ph = tr(locale, "workflows.stepTitle");
        let inst_ph = tr(locale, "workflows.stepInstruction");
        let title = cx.new(|cx| InputState::new(window, cx).placeholder(title_ph));
        let instruction = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(3)
                .placeholder(inst_ph)
        });
        self.edit_steps = vec![StepEditorRow { title, instruction }];
        self.sync_source_from_steps(window, cx);
    }

    fn enter_edit_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wf) = self.current_workflow().cloned() else {
            return;
        };
        self.editing = true;
        self.source_mode = false;
        self.editing_existing_path = Some(wf.path.clone());
        self.edit_name_input
            .update(cx, |s, cx| s.set_value(&wf.name, window, cx));
        self.edit_desc_input
            .update(cx, |s, cx| s.set_value(&wf.description, window, cx));
        self.edit_source_input
            .update(cx, |s, cx| s.set_value(&wf.raw_content, window, cx));
        self.edit_steps = if wf.steps.is_empty() {
            let title = cx.new(|cx| InputState::new(window, cx).placeholder("Step title"));
            let instruction = cx.new(|cx| TextareaState::new(window, cx).rows(3));
            vec![StepEditorRow { title, instruction }]
        } else {
            wf.steps
                .iter()
                .map(|st| {
                    let t_val = st.title.clone();
                    let i_val = st.instruction.clone();
                    let title = cx.new(|cx| {
                        let mut s = InputState::new(window, cx);
                        s.set_value(&t_val, window, cx);
                        s
                    });
                    let instruction = cx.new(|cx| {
                        let mut s = TextareaState::new(window, cx).rows(3);
                        s.set_value(&i_val, window, cx);
                        s
                    });
                    StepEditorRow { title, instruction }
                })
                .collect()
        };
    }

    fn collect_steps(&self, cx: &gpui_kit::App) -> Vec<WorkflowStep> {
        self.edit_steps
            .iter()
            .filter_map(|row| {
                let t = row.title.read(cx).value().trim().to_string();
                let inst = row.instruction.read(cx).value().trim().to_string();
                if t.is_empty() && inst.is_empty() {
                    None
                } else {
                    Some(WorkflowStep {
                        title: if t.is_empty() { "Step".to_string() } else { t },
                        instruction: inst,
                    })
                }
            })
            .collect()
    }

    fn sync_source_from_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let desc = self.edit_desc_input.read(cx).value().to_string();
        let steps = self.collect_steps(cx);
        let md = serialize_workflow_md(&desc, &steps);
        self.edit_source_input
            .update(cx, |s, cx| s.set_value(&md, window, cx));
    }

    fn save_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name_raw = self.edit_name_input.read(cx).value().to_string();
        let safe_name = sanitize_workflow_name(&name_raw);
        let content = if self.source_mode {
            self.edit_source_input.read(cx).value().to_string()
        } else {
            let desc = self.edit_desc_input.read(cx).value().to_string();
            let steps = self.collect_steps(cx);
            serialize_workflow_md(&desc, &steps)
        };

        let st = self.app_state.read(cx);
        let locale = st.locale;
        let cwd = st.active_cwd.clone();
        let kind = st.active_workspace_kind;

        match save_workflow(self.scope, &cwd, kind, &safe_name, &content) {
            Ok(saved_path) => {
                if let Some(old_path) = self.editing_existing_path.take() {
                    if old_path != saved_path {
                        let _ = delete_workflow(&old_path);
                    }
                }
                self.selected_name = Some(safe_name);
                self.editing = false;
                self.reload(window, cx);
                self.app_state.update(cx, |s, cx| {
                    if s.sidecar_connected {
                        s.refresh_from_sidecar(cx);
                    }
                    s.show_success_toast(tr(locale, "workflows.saved"), cx);
                });
            }
            Err(err) => {
                self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            }
        }
        cx.notify();
    }

    fn delete_current(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(wf) = self.current_workflow().cloned() else {
            return;
        };
        if let Err(err) = delete_workflow(&wf.path) {
            self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            return;
        }
        self.selected_name = None;
        self.editing = false;
        self.reload(_window, cx);
        self.app_state.update(cx, |s, cx| {
            let msg = tr(s.locale, "toast.workflowDeleted");
            s.show_success_toast(msg, cx);
        });
    }

    fn run_current(&mut self, cx: &mut Context<Self>) {
        let Some(wf) = self.current_workflow().cloned() else {
            return;
        };
        let args = self.args_input.read(cx).value().to_string();
        let prompt = build_workflow_execution_prompt(&wf, &args);
        self.app_state.update(cx, |s, cx| {
            s.submit_prompt(prompt, cx);
            let msg = tr(s.locale, "toast.workflowStarted");
            s.show_success_toast(msg, cx);
        });
    }
}

impl Render for WorkflowsPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.app_state.read(cx);
        let locale = st.locale;
        let proj_ok = project_notes_available(st.active_workspace_kind, &st.active_cwd);
        if st.active_cwd != self.last_cwd {
            self.reload(window, cx);
        }
        if self.scope == WorkflowScope::Project && !proj_ok {
            self.scope = WorkflowScope::Global;
            self.reload(window, cx);
        }

        // Sync select_state choice
        if let Some(val) = self.select_state.read(cx).selected_value().cloned() {
            if self.selected_name.as_ref() != Some(&val) {
                self.selected_name = Some(val);
            }
        }

        let scope_idx = if self.scope == WorkflowScope::Global { 0 } else { 1 };

        if self.editing {
            return v_flex()
                .size_full()
                .p_3()
                .gap_3()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(tr(locale, "workflows.edit")),
                        )
                        .child(
                            TabBar::new("wf-edit-mode-tabs")
                                .segmented()
                                .small()
                                .selected_index(if self.source_mode { 1 } else { 0 })
                                .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                    let want_source = *idx == 1;
                                    if want_source && !this.source_mode {
                                        this.sync_source_from_steps(window, cx);
                                    }
                                    this.source_mode = want_source;
                                    cx.notify();
                                }))
                                .child(Tab::new().label(tr(locale, "workflows.stepsMode")))
                                .child(Tab::new().label(tr(locale, "workflows.sourceMode"))),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(tr(locale, "workflows.name")),
                                )
                                .child(Input::new(&self.edit_name_input).small()),
                        )
                        .when(!self.source_mode, |el| {
                            el.child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "workflows.description")),
                                    )
                                    .child(Input::new(&self.edit_desc_input).small()),
                            )
                        }),
                )
                .child(
                    div()
                        .id("wf-edit-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(if self.source_mode {
                            Textarea::new(&self.edit_source_input)
                                .h(px(320.))
                                .into_any_element()
                        } else {
                            let step_count = self.edit_steps.len();
                            v_flex()
                                .gap_2()
                                .children(self.edit_steps.iter().enumerate().map(|(idx, row)| {
                                    v_flex()
                                        .p_2p5()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().secondary.opacity(0.3))
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_medium()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(format!(
                                                            "{} {}",
                                                            tr(locale, "workflows.stepLabel"),
                                                            idx + 1
                                                        )),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_1()
                                                        .child(
                                                            Button::new(("wf-step-up", idx))
                                                                .ghost()
                                                                .small()
                                                                .h(px(22.))
                                                                .w(px(22.))
                                                                .icon(IconName::ArrowUp)
                                                                .disabled(idx == 0)
                                                                .on_click(cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        if idx > 0 {
                                                                            this.edit_steps
                                                                                .swap(idx, idx - 1);
                                                                            cx.notify();
                                                                        }
                                                                    },
                                                                )),
                                                        )
                                                        .child(
                                                            Button::new(("wf-step-dn", idx))
                                                                .ghost()
                                                                .small()
                                                                .h(px(22.))
                                                                .w(px(22.))
                                                                .icon(IconName::ArrowDown)
                                                                .disabled(idx + 1 >= step_count)
                                                                .on_click(cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        if idx + 1
                                                                            < this.edit_steps.len()
                                                                        {
                                                                            this.edit_steps
                                                                                .swap(idx, idx + 1);
                                                                            cx.notify();
                                                                        }
                                                                    },
                                                                )),
                                                        )
                                                        .child(
                                                            Button::new(("wf-step-del", idx))
                                                                .danger()
                                                                .small()
                                                                .h(px(22.))
                                                                .w(px(22.))
                                                                .icon(Icon::new(TrashIcon))
                                                                .disabled(step_count <= 1)
                                                                .on_click(cx.listener(
                                                                    move |this, _, _, cx| {
                                                                        if this.edit_steps.len() > 1
                                                                        {
                                                                            this.edit_steps
                                                                                .remove(idx);
                                                                            cx.notify();
                                                                        }
                                                                    },
                                                                )),
                                                        ),
                                                ),
                                        )
                                        .child(Input::new(&row.title).small())
                                        .child(Textarea::new(&row.instruction).h(px(76.)))
                                        .into_any_element()
                                }))
                                .child(
                                    Button::new("wf-add-step")
                                        .outline()
                                        .small()
                                        .h(px(28.))
                                        .icon(IconName::Plus)
                                        .label(tr(locale, "workflows.addStep"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let loc = this.app_state.read(cx).locale;
                                            let title_ph = tr(loc, "workflows.stepTitle");
                                            let inst_ph = tr(loc, "workflows.stepInstruction");
                                            let title = cx.new(|cx| {
                                                InputState::new(window, cx).placeholder(title_ph)
                                            });
                                            let instruction = cx.new(|cx| {
                                                TextareaState::new(window, cx)
                                                    .rows(3)
                                                    .placeholder(inst_ph)
                                            });
                                            this.edit_steps
                                                .push(StepEditorRow { title, instruction });
                                            cx.notify();
                                        })),
                                )
                                .into_any_element()
                        }),
                )
                .child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .pt_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Button::new("wf-edit-cancel")
                                .outline()
                                .small()
                                .h(px(28.))
                                .label(tr(locale, "workflows.discard"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.editing = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("wf-edit-save")
                                .primary()
                                .small()
                                .h(px(28.))
                                .label(tr(locale, "workflows.save"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_edit(window, cx);
                                })),
                        ),
                )
                .into_any_element();
        }

        let current = self.current_workflow().cloned();
        let has_current = current.is_some();
        let has_args = current.as_ref().is_some_and(|w| w.has_args);
        let is_running = self.app_state.read(cx).is_agent_running;

        v_flex()
            .size_full()
            .p_3()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(tr(locale, "workflows.title")),
                    )
                    .child(
                        TabBar::new("wf-scope-tabs")
                            .segmented()
                            .small()
                            .selected_index(scope_idx)
                            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                let next = if *idx == 1 {
                                    WorkflowScope::Project
                                } else {
                                    WorkflowScope::Global
                                };
                                if next == WorkflowScope::Project {
                                    let st = this.app_state.read(cx);
                                    if !project_notes_available(
                                        st.active_workspace_kind,
                                        &st.active_cwd,
                                    ) {
                                        return;
                                    }
                                }
                                this.scope = next;
                                this.reload(window, cx);
                                cx.notify();
                            }))
                            .child(Tab::new().label(tr(locale, "workflows.global")))
                            .child(
                                Tab::new()
                                    .label(tr(locale, "workflows.project"))
                                    .disabled(!proj_ok),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Select::new(&self.select_state).small()),
                    )
                    .child(
                        Button::new("wf-new-btn")
                            .outline()
                            .small()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.enter_new_mode(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("wf-edit-btn")
                            .outline()
                            .small()
                            .icon(Icon::new(PencilIcon))
                            .disabled(!has_current)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.enter_edit_mode(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("wf-del-btn")
                            .danger()
                            .small()
                            .icon(Icon::new(TrashIcon))
                            .disabled(!has_current)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.delete_current(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("wf-steps-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(match current {
                        None => div()
                            .py_6()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "workflows.empty"))
                            .into_any_element(),
                        Some(wf) => {
                            let step_count = wf.steps.len();
                            v_flex()
                                .gap_2()
                                .when(!wf.description.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(wf.description.clone())),
                                    )
                                })
                                .children(wf.steps.iter().enumerate().map(|(idx, step)| {
                                    v_flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            v_flex()
                                                .w_full()
                                                .p_2p5()
                                                .rounded_md()
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .bg(cx.theme().secondary.opacity(0.35))
                                                .gap_1()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .px_1p5()
                                                                .py_0p5()
                                                                .rounded_sm()
                                                                .bg(cx.theme().primary.opacity(0.15))
                                                                .text_xs()
                                                                .font_semibold()
                                                                .text_color(cx.theme().primary)
                                                                .child(format!("{}", idx + 1)),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_medium()
                                                                .text_color(cx.theme().foreground)
                                                                .child(SharedString::from(
                                                                    step.title.clone(),
                                                                )),
                                                        ),
                                                )
                                                .when(!step.instruction.is_empty(), |el| {
                                                    el.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(SharedString::from(
                                                                step.instruction.clone(),
                                                            )),
                                                    )
                                                }),
                                        )
                                        .when(idx + 1 < step_count, |el| {
                                            el.child(
                                                Icon::new(IconName::ArrowDown)
                                                    .xsmall()
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                        })
                                        .into_any_element()
                                }))
                                .into_any_element()
                        }
                    }),
            )
            .when(has_args, |el| {
                el.child(
                    Input::new(&self.args_input)
                        .small(),
                )
            })
            .child(
                Button::new("wf-run-btn")
                    .primary()
                    .small()
                    .w_full()
                    .h(px(30.))
                    .icon(Icon::new(PlayIcon))
                    .label(tr(locale, "workflows.run"))
                    .disabled(!has_current || is_running)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run_current(cx);
                    })),
            )
            .into_any_element()
    }
}
