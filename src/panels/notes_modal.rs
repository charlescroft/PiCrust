use crate::app_state::AppState;
use crate::client::workspace_extras::{
    NoteEntry, NoteScope, delete_note, list_notes, notes_dir, project_notes_available, read_note,
    sanitize_note_filename, save_note,
};
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};
use std::path::PathBuf;

pub struct NotesModal {
    app_state: Entity<AppState>,
    scope: NoteScope,
    notes: Vec<NoteEntry>,
    selected_path: Option<PathBuf>,
    edit_mode: bool,
    title_input: Entity<InputState>,
    body_input: Entity<TextareaState>,
    last_cwd: PathBuf,
}

impl NotesModal {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = app_state.read(cx).locale;
        let title_ph = tr(locale, "notes.titlePlaceholder");
        let body_ph = tr(locale, "notes.bodyPlaceholder");
        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder(title_ph));
        let body_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(14)
                .placeholder(body_ph)
        });
        let last_cwd = app_state.read(cx).active_cwd.clone();
        let mut this = Self {
            app_state,
            scope: NoteScope::Global,
            notes: Vec::new(),
            selected_path: None,
            edit_mode: false,
            title_input,
            body_input,
            last_cwd,
        };
        this.reload_list(window, cx);
        this
    }

    fn reload_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.app_state.read(cx);
        let cwd = st.active_cwd.clone();
        let kind = st.active_workspace_kind;
        self.last_cwd = cwd.clone();
        self.notes = list_notes(self.scope, &cwd, kind);
        if let Some(first) = self.notes.first().cloned() {
            if self
                .selected_path
                .as_ref()
                .is_none_or(|p| !self.notes.iter().any(|n| &n.path == p))
            {
                self.select_note(&first, window, cx);
            }
        } else {
            self.selected_path = None;
            self.title_input
                .update(cx, |s, cx| s.set_value("", window, cx));
            self.body_input
                .update(cx, |s, cx| s.set_value("", window, cx));
        }
    }

    fn select_note(&mut self, entry: &NoteEntry, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_path = Some(entry.path.clone());
        let body = read_note(&entry.path).unwrap_or_default();
        self.title_input
            .update(cx, |s, cx| s.set_value(&entry.name, window, cx));
        self.body_input
            .update(cx, |s, cx| s.set_value(&body, window, cx));
    }

    fn create_new_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.app_state.read(cx);
        let Some(dir) = notes_dir(self.scope, &st.active_cwd, st.active_workspace_kind) else {
            return;
        };
        let mut idx = 1usize;
        let path = loop {
            let candidate = dir.join(format!("untitled-{idx}.md"));
            if !candidate.exists() {
                break candidate;
            }
            idx += 1;
        };
        let title = format!("untitled-{idx}");
        let initial = format!("# {title}\n\n");
        if save_note(&path, &initial).is_ok() {
            self.selected_path = Some(path.clone());
            self.edit_mode = true;
            self.reload_list(window, cx);
            self.title_input
                .update(cx, |s, cx| s.set_value(&title, window, cx));
            self.body_input
                .update(cx, |s, cx| s.set_value(&initial, window, cx));
        }
        cx.notify();
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = self.app_state.read(cx);
        let locale = st.locale;
        let Some(dir) = notes_dir(self.scope, &st.active_cwd, st.active_workspace_kind) else {
            return;
        };
        let raw_title = self.title_input.read(cx).value().to_string();
        let safe_name = sanitize_note_filename(&raw_title);
        let target_path = dir.join(&safe_name);
        let body = self.body_input.read(cx).value().to_string();
        match save_note(&target_path, &body) {
            Ok(()) => {
                if let Some(old_path) = self.selected_path.as_ref() {
                    if old_path != &target_path {
                        let _ = delete_note(old_path);
                    }
                }
                self.selected_path = Some(target_path);
                self.reload_list(window, cx);
                self.app_state.update(cx, |s, cx| {
                    s.show_success_toast(tr(locale, "notes.saved"), cx);
                });
            }
            Err(err) => {
                self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            }
        }
        cx.notify();
    }

    fn delete_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.app_state.read(cx).locale;
        let Some(path) = self.selected_path.take() else {
            return;
        };
        match delete_note(&path) {
            Ok(()) => {
                self.reload_list(window, cx);
                self.app_state.update(cx, |s, cx| {
                    s.show_success_toast(tr(locale, "notes.deleted"), cx);
                });
            }
            Err(err) => {
                self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            }
        }
        cx.notify();
    }

    fn insert_reference(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_mode {
            self.save_current(window, cx);
        }
        let Some(path) = self.selected_path.clone() else {
            return;
        };
        let st = self.app_state.read(cx);
        let mention = match self.scope {
            NoteScope::Project => {
                if let Ok(rel) = path.strip_prefix(&st.active_cwd) {
                    format!("@{} ", rel.to_string_lossy().replace('\\', "/"))
                } else {
                    format!("@{} ", path.display())
                }
            }
            NoteScope::Global => format!("@{} ", path.display()),
        };
        self.app_state.update(cx, |s, cx| {
            s.pending_composer_mention = Some(mention);
            s.close_notes_modal(cx);
        });
    }
}

impl Render for NotesModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let st = self.app_state.read(cx);
        let locale = st.locale;
        let proj_ok = project_notes_available(st.active_workspace_kind, &st.active_cwd);
        if st.active_cwd != self.last_cwd {
            self.reload_list(window, cx);
        }
        if self.scope == NoteScope::Project && !proj_ok {
            self.scope = NoteScope::Global;
            self.reload_list(window, cx);
        }

        let app_close = self.app_state.clone();
        let active_idx = if self.scope == NoteScope::Global { 0 } else { 1 };
        let body_md = self.body_input.read(cx).value().to_string();
        let selected_path_str = self
            .selected_path
            .as_ref()
            .map(|p| {
                if self.scope == NoteScope::Project {
                    p.strip_prefix(&self.last_cwd)
                        .map(|r| r.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| p.to_string_lossy().into_owned())
                } else {
                    p.to_string_lossy().into_owned()
                }
            })
            .unwrap_or_default();

        div()
            .id("notes-modal-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .bg(cx.theme().background.opacity(0.65))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                app_close.update(cx, |s, cx| s.close_notes_modal(cx));
            })
            .child(
                v_flex()
                    .id("notes-modal-card")
                    .w(px(760.))
                    .h(px(520.))
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_2p5()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .child(tr(locale, "notes.title")),
                                    )
                                    .child(
                                        TabBar::new("notes-scope-tabs")
                                            .segmented()
                                            .small()
                                            .selected_index(active_idx)
                                            .on_click(cx.listener(|this, idx: &usize, window, cx| {
                                                let next = if *idx == 1 {
                                                    NoteScope::Project
                                                } else {
                                                    NoteScope::Global
                                                };
                                                if next == NoteScope::Project {
                                                    let st = this.app_state.read(cx);
                                                    if !project_notes_available(
                                                        st.active_workspace_kind,
                                                        &st.active_cwd,
                                                    ) {
                                                        window.push_notification(
                                                            Notification::info(tr(
                                                                st.locale,
                                                                "notes.noProject",
                                                            )),
                                                            cx,
                                                        );
                                                        return;
                                                    }
                                                }
                                                this.scope = next;
                                                this.edit_mode = false;
                                                this.reload_list(window, cx);
                                                cx.notify();
                                            }))
                                            .child(Tab::new().label(tr(locale, "notes.globalTab")))
                                            .child(
                                                Tab::new()
                                                    .label(tr(locale, "notes.projectTab"))
                                                    .disabled(!proj_ok),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new("notes-close-btn")
                                    .ghost()
                                    .small()
                                    .h(px(26.))
                                    .w(px(26.))
                                    .icon(IconName::Close)
                                    .on_click({
                                        let app = self.app_state.clone();
                                        move |_, _, cx| {
                                            app.update(cx, |s, cx| s.close_notes_modal(cx));
                                        }
                                    }),
                            ),
                    )
                    // Body: left list + right single content area
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .child(
                                v_flex()
                                    .w(px(200.))
                                    .h_full()
                                    .flex_shrink_0()
                                    .border_r_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary.opacity(0.25))
                                    .child(
                                        div().p_2().child(
                                            Button::new("notes-new-btn")
                                                .outline()
                                                .small()
                                                .w_full()
                                                .h(px(28.))
                                                .label(tr(locale, "notes.new"))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.create_new_note(window, cx);
                                                })),
                                        ),
                                    )
                                    .child(
                                        v_flex()
                                            .id("notes-list-scroll")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_y_scroll()
                                            .px_2()
                                            .pb_2()
                                            .gap_1()
                                            .children(if self.notes.is_empty() {
                                                vec![div()
                                                    .px_2()
                                                    .py_3()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "notes.empty"))
                                                    .into_any_element()]
                                            } else {
                                                self.notes
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(idx, note)| {
                                                        let is_sel = self.selected_path.as_ref()
                                                            == Some(&note.path);
                                                        let n_clone = note.clone();
                                                        div()
                                                            .id(("note-item", idx))
                                                            .px_2p5()
                                                            .py_1p5()
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .text_xs()
                                                            .truncate()
                                                            .bg(if is_sel {
                                                                cx.theme().accent
                                                            } else {
                                                                cx.theme().background.opacity(0.0)
                                                            })
                                                            .text_color(if is_sel {
                                                                cx.theme().accent_foreground
                                                            } else {
                                                                cx.theme().foreground
                                                            })
                                                            .hover(|s| {
                                                                s.bg(cx.theme().accent.opacity(0.6))
                                                            })
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.select_note(
                                                                        &n_clone, window, cx,
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ))
                                                            .child(SharedString::from(
                                                                note.name.clone(),
                                                            ))
                                                            .into_any_element()
                                                    })
                                                    .collect()
                                            }),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .px_3()
                                            .py_2()
                                            .border_b_1()
                                            .border_color(cx.theme().border)
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                div().flex_1().min_w_0().child(if self.edit_mode {
                                                    Input::new(&self.title_input)
                                                        .small()
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .text_sm()
                                                        .font_medium()
                                                        .truncate()
                                                        .text_color(cx.theme().foreground)
                                                        .child(SharedString::from(
                                                            self.title_input
                                                                .read(cx)
                                                                .value()
                                                                .to_string(),
                                                        ))
                                                        .into_any_element()
                                                }),
                                            )
                                            .child(
                                                TabBar::new("notes-mode-tabs")
                                                    .segmented()
                                                    .small()
                                                    .selected_index(if self.edit_mode { 1 } else { 0 })
                                                    .on_click(cx.listener(
                                                        |this, idx: &usize, window, cx| {
                                                            let want_edit = *idx == 1;
                                                            if !want_edit && this.edit_mode {
                                                                this.save_current(window, cx);
                                                            }
                                                            this.edit_mode = want_edit;
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(Tab::new().label(tr(locale, "notes.preview")))
                                                    .child(Tab::new().label(tr(locale, "notes.edit"))),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("notes-content-scroll")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_y_scroll()
                                            .p_3()
                                            .child(if self.edit_mode {
                                                Textarea::new(&self.body_input)
                                                    .h(px(330.))
                                                    .into_any_element()
                                            } else if body_md.trim().is_empty() {
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "notes.empty"))
                                                    .into_any_element()
                                            } else {
                                                TextView::markdown("notes-md-preview", body_md)
                                                    .selectable(true)
                                                    .into_any_element()
                                            }),
                                    ),
                            ),
                    )
                    // Footer
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_2p5()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(selected_path_str)),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .flex_shrink_0()
                                    .child(
                                        Button::new("notes-delete-btn")
                                            .danger()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "notes.delete"))
                                            .disabled(self.selected_path.is_none())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.delete_current(window, cx);
                                            })),
                                    )
                                    .when(self.edit_mode, |el| {
                                        el.child(
                                            Button::new("notes-save-btn")
                                                .outline()
                                                .small()
                                                .h(px(28.))
                                                .label(tr(locale, "notes.save"))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.save_current(window, cx);
                                                    this.edit_mode = false;
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("notes-insert-btn")
                                            .primary()
                                            .small()
                                            .h(px(28.))
                                            .label(tr(locale, "notes.insertRef"))
                                            .disabled(self.selected_path.is_none())
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.insert_reference(window, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}
