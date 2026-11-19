use crate::app_state::{AppState, RightPanelTab};
use crate::client::types::{
    MessageRole, ModelInfo, SendShortcut, SteeringMode, ThinkingLevel, ToolPreset, WorkspaceKind,
};
use crate::i18n::{AppLocale, format_commas, format_tokens_km, tr};
use crate::icons::{ShieldIcon, ZapIcon};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::progress::{Progress, ProgressCircle};
use gpui_kit::component::select::{
    SearchableVec, Select, SelectDelegate, SelectEvent, SelectGroup, SelectItem, SelectState,
};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{
    ActiveTheme as _, ElementExt as _, Icon, IconName, IndexPath, Sizable as _,
    StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    Anchor, AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable as _, Font,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, TextRun, Window, div, px,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Clone, Debug)]
struct FileBrowserEntry {
    name: String,
    relative_path: String,
    is_dir: bool,
}

fn list_workspace_directory(root: &Path, rel_dir: &str) -> Vec<FileBrowserEntry> {
    let target = if rel_dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel_dir)
    };
    let Ok(rd) = std::fs::read_dir(&target) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | ".next" | ".next-desktop" | "dist" | ".DS_Store"
        ) {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let relative_path = if rel_dir.is_empty() {
            name.clone()
        } else {
            format!("{rel_dir}/{name}")
        };
        let item = FileBrowserEntry {
            name,
            relative_path,
            is_dir,
        };
        if is_dir {
            dirs.push(item);
        } else {
            files.push(item);
        }
    }
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    dirs.extend(files);
    dirs
}

struct SlashCommandEntry {
    name: &'static str,
    desc_key: &'static str,
    execute_immediately: bool,
}

const BUILTIN_SLASH_COMMANDS: &[SlashCommandEntry] = &[
    SlashCommandEntry {
        name: "compact",
        desc_key: "slash.compact",
        execute_immediately: true,
    },
    SlashCommandEntry {
        name: "name",
        desc_key: "slash.name",
        execute_immediately: false,
    },
    SlashCommandEntry {
        name: "session",
        desc_key: "slash.session",
        execute_immediately: true,
    },
    SlashCommandEntry {
        name: "mcp",
        desc_key: "slash.mcp",
        execute_immediately: true,
    },
    SlashCommandEntry {
        name: "reload",
        desc_key: "slash.reload",
        execute_immediately: true,
    },
    SlashCommandEntry {
        name: "clear",
        desc_key: "slash.clear",
        execute_immediately: true,
    },
];

#[derive(Clone, Debug)]
struct ModelOptionItem {
    provider: String,
    id: String,
    provider_name: String,
    display_name: String,
    value: SharedString,
}

impl SelectItem for ModelOptionItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        SharedString::from(format!("{} ({})", self.display_name, self.provider))
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.display_name.to_lowercase().contains(&q)
            || self.id.to_lowercase().contains(&q)
            || self.provider.to_lowercase().contains(&q)
            || self.provider_name.to_lowercase().contains(&q)
    }
}

struct ModelGroupDelegate {
    inner: SearchableVec<SelectGroup<ModelOptionItem>>,
}

impl ModelGroupDelegate {
    fn from_models(models: &[ModelInfo]) -> Self {
        let mut order: Vec<String> = Vec::new();
        let mut by_provider: BTreeMap<String, (String, Vec<ModelOptionItem>)> = BTreeMap::new();
        for m in models {
            let p_title = m
                .provider_name
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| m.provider.clone());
            if !by_provider.contains_key(&m.provider) {
                order.push(m.provider.clone());
            }
            let entry = by_provider
                .entry(m.provider.clone())
                .or_insert_with(|| (p_title, Vec::new()));
            entry.1.push(ModelOptionItem {
                provider: m.provider.clone(),
                id: m.id.clone(),
                provider_name: entry.0.clone(),
                display_name: m.name.clone().unwrap_or_else(|| m.id.clone()),
                value: SharedString::from(format!("{}:{}", m.provider, m.id)),
            });
        }
        let groups: Vec<SelectGroup<ModelOptionItem>> = order
            .into_iter()
            .filter_map(|p| {
                let (title, items) = by_provider.remove(&p)?;
                Some(SelectGroup::new(title).items(items))
            })
            .collect();
        Self {
            inner: SearchableVec::new(groups),
        }
    }
}

impl SelectDelegate for ModelGroupDelegate {
    type Item = ModelOptionItem;

    fn sections_count(&self, cx: &App) -> usize {
        self.inner.sections_count(cx)
    }

    fn items_count(&self, section: usize) -> usize {
        self.inner.items_count(section)
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.inner.item(ix)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SelectItem<Value = V>,
        V: PartialEq,
    {
        self.inner.position(value)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, cx: &mut App) -> Task<()> {
        self.inner.perform_search(query, window, cx)
    }

    fn render_section_header(
        &self,
        section: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        #[allow(deprecated)]
        let title = self.inner.section(section)?;
        Some(
            div()
                .w_full()
                .pt_2()
                .pb_1()
                .px_2p5()
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .child(title)
                .into_any_element(),
        )
    }

    fn render_item(
        &self,
        _ix: IndexPath,
        item: &Self::Item,
        checked: bool,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let mut row = h_flex()
            .w_full()
            .min_h(px(36.))
            .py_2()
            .px_2p5()
            .gap_2()
            .items_center()
            .justify_between()
            .rounded_md()
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
                            .text_color(cx.theme().foreground)
                            .child(SharedString::from(item.display_name.clone())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(item.id.clone())),
                    ),
            );
        if checked {
            row = row.child(
                Icon::new(IconName::Check)
                    .small()
                    .text_color(cx.theme().primary),
            );
        }
        Some(row.into_any_element())
    }
}

pub struct ComposerView {
    focus_handle: FocusHandle,
    app_state: Entity<AppState>,
    textarea: Entity<TextareaState>,
    disabled_bottom_input: Entity<InputState>,
    model_select: Entity<SelectState<ModelGroupDelegate>>,
    synced_model_keys: Vec<String>,
    synced_selected_key: Option<(String, String)>,
    synced_send_shortcut: Option<SendShortcut>,
    synced_locale: AppLocale,
    bottom_slot_width: f32,
    is_promoted: bool,
    at_query: Option<String>,
    ignored_at_positions: std::collections::HashSet<usize>,
    file_picker_open: bool,
    file_picker_dir: String,
    file_picker_entries: Vec<FileBrowserEntry>,
    file_picker_selected_idx: usize,
    file_picker_scroll: ScrollHandle,
    slash_query: Option<String>,
    session_drafts: HashMap<Option<String>, String>,
    last_rendered_session_id: Option<String>,
}

impl gpui_kit::Render for ComposerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_composer(window, cx)
    }
}

/// Composer placeholder, composed from the configured send shortcut so the hint
/// follows Settings › Send shortcut (and the platform's modifier) instead of
/// being baked into one string.
fn composer_placeholder(locale: AppLocale, shortcut: SendShortcut) -> String {
    let key = match shortcut {
        SendShortcut::Enter => "Enter",
        SendShortcut::CmdEnter => {
            #[cfg(target_os = "macos")]
            {
                "Cmd+Enter"
            }
            #[cfg(not(target_os = "macos"))]
            {
                "Ctrl+Enter"
            }
        }
        SendShortcut::ShiftEnter => "Shift+Enter",
    };
    format!(
        "{} {}, {}",
        tr(locale, "chat.placeholderBase"),
        tr(locale, "chat.placeholderKeys"),
        tr(locale, "chat.placeholderSend").replace("{shortcut}", key)
    )
}

impl ComposerView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_locale = app_state.read(cx).locale;
        let initial_shortcut = app_state.read(cx).send_shortcut;
        let initial_placeholder = composer_placeholder(initial_locale, initial_shortcut);
        let textarea = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .placeholder(initial_placeholder)
                .submit_on_enter(initial_shortcut == SendShortcut::Enter)
        });
        let disabled_bottom_input = cx.new(|cx| InputState::new(window, cx));

        let initial_visible_models = app_state.read(cx).visible_picker_models();
        let initial_delegate = ModelGroupDelegate::from_models(&initial_visible_models);
        let initial_selected_ix = app_state.read(cx).selected_model.as_ref().and_then(|sel| {
            let key = SharedString::from(format!("{}:{}", sel.provider, sel.id));
            initial_delegate.position(&key)
        });
        let model_select = cx.new(|cx| {
            SelectState::new(initial_delegate, initial_selected_ix, window, cx).searchable(true)
        });

        let app_sub = app_state.clone();
        cx.subscribe(&textarea, move |this, state, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = state.read(cx).value().to_string();

                let trimmed = text.trim_start();
                if let Some(after_slash) = trimmed.strip_prefix('/') {
                    if !after_slash.contains(char::is_whitespace) {
                        this.slash_query = Some(after_slash.to_lowercase());
                    } else {
                        this.slash_query = None;
                    }
                } else {
                    this.slash_query = None;
                }

                this.ignored_at_positions
                    .retain(|&pos| text.as_bytes().get(pos) == Some(&b'@'));

                let mut cursor_byte = state.read(cx).cursor().min(text.len());
                while cursor_byte > 0 && !text.is_char_boundary(cursor_byte) {
                    cursor_byte -= 1;
                }
                let before_cursor = &text[..cursor_byte];

                let active_at = before_cursor.rfind('@').and_then(|at_pos| {
                    if this.ignored_at_positions.contains(&at_pos) {
                        return None;
                    }
                    let preceded_by_boundary = at_pos == 0
                        || before_cursor[..at_pos]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_whitespace);
                    if !preceded_by_boundary {
                        return None;
                    }
                    let after = &before_cursor[at_pos + 1..];
                    if after.contains(char::is_whitespace) {
                        return None;
                    }
                    Some(after)
                });

                if let Some(after) = active_at {
                    this.at_query = Some(after.to_string());
                    if after.is_empty() {
                        this.file_picker_open = true;
                        this.file_picker_dir.clear();
                        this.reload_file_browser(cx);
                    } else {
                        this.file_picker_open = false;
                        let q = after.to_string();
                        app_sub.update(cx, |s, cx| {
                            s.run_file_fuzzy_search(&q, cx);
                        });
                    }
                } else {
                    if this.at_query.is_some() {
                        this.file_picker_open = false;
                    }
                    this.at_query = None;
                }
                cx.notify();
            }
        })
        .detach();

        let app_model = app_state.clone();
        cx.subscribe(
            &model_select,
            move |_, _, event: &SelectEvent<ModelGroupDelegate>, cx| {
                let SelectEvent::Confirm(Some(key_val)) = event else {
                    return;
                };
                let target_key = key_val.clone();
                app_model.update(cx, |s, cx| {
                    if let Some(m) = s
                        .visible_picker_models()
                        .into_iter()
                        .find(|m| format!("{}:{}", m.provider, m.id) == target_key.as_ref())
                    {
                        s.select_model(m, cx);
                    }
                });
            },
        )
        .detach();

        let synced_model_keys: Vec<String> = app_state
            .read(cx)
            .visible_picker_models()
            .iter()
            .map(|m| format!("{}:{}", m.provider, m.id))
            .collect();
        let synced_selected_key = app_state
            .read(cx)
            .selected_model
            .as_ref()
            .map(|m| (m.provider.clone(), m.id.clone()));

        cx.observe(&app_state, |_, _, cx| {
            cx.notify();
        })
        .detach();

        let weak_self = cx.weak_entity();
        cx.intercept_keystrokes(move |event, window, cx| {
            let key = event.keystroke.key.as_str();
            if !matches!(key, "up" | "down" | "left" | "right" | "enter" | "tab" | "escape") {
                return;
            }
            if event.keystroke.modifiers.control
                || event.keystroke.modifiers.alt
                || event.keystroke.modifiers.platform
                || event.keystroke.modifiers.function
            {
                return;
            }
            let Some(entity) = weak_self.upgrade() else {
                return;
            };
            let mut handled = false;
            entity.update(cx, |this, cx| {
                if !this.file_picker_open && this.at_query.is_none() {
                    return;
                }
                let (modal_open, is_chat_page, file_matches) = {
                    let s = this.app_state.read(cx);
                    (
                        s.settings_open || s.find_open,
                        s.active_page == crate::app_state::WorkspacePage::Chat,
                        s.file_search_results.clone(),
                    )
                };
                if modal_open || !is_chat_page {
                    return;
                }
                let is_dir_browser = this.is_dir_browser_mode();
                let len = if is_dir_browser {
                    this.file_picker_entries.len()
                } else {
                    file_matches.len().min(40)
                };
                match key {
                    "up" => {
                        handled = true;
                        if len > 0 {
                            this.file_picker_selected_idx =
                                this.file_picker_selected_idx.saturating_sub(1);
                            this.file_picker_scroll
                                .scroll_to_item(this.file_picker_selected_idx);
                            cx.notify();
                        }
                    }
                    "down" => {
                        handled = true;
                        if len > 0 {
                            this.file_picker_selected_idx =
                                (this.file_picker_selected_idx + 1).min(len - 1);
                            this.file_picker_scroll
                                .scroll_to_item(this.file_picker_selected_idx);
                            cx.notify();
                        }
                    }
                    "right" => {
                        handled = true;
                        if is_dir_browser {
                            if let Some(entry) = this
                                .file_picker_entries
                                .get(this.file_picker_selected_idx)
                                .cloned()
                            {
                                if entry.is_dir {
                                    this.enter_file_picker_dir(entry.relative_path, cx);
                                }
                            }
                        }
                    }
                    "left" => {
                        handled = true;
                        if is_dir_browser && !this.file_picker_dir.is_empty() {
                            this.exit_file_picker_dir(cx);
                        }
                    }
                    "enter" => {
                        handled = true;
                        if is_dir_browser {
                            if let Some(entry) = this
                                .file_picker_entries
                                .get(this.file_picker_selected_idx)
                                .cloned()
                            {
                                let target = if entry.is_dir {
                                    format!("{}/", entry.relative_path)
                                } else {
                                    entry.relative_path
                                };
                                this.insert_file_reference(&target, window, cx);
                                this.textarea.update(cx, |st, cx| st.focus(window, cx));
                            }
                        } else if let Some(m) = file_matches.get(this.file_picker_selected_idx) {
                            let rel = m.relative_path.clone();
                            this.insert_file_reference(&rel, window, cx);
                            this.textarea.update(cx, |st, cx| st.focus(window, cx));
                        }
                    }
                    "tab" if is_dir_browser => {
                        if let Some(entry) = this
                            .file_picker_entries
                            .get(this.file_picker_selected_idx)
                            .cloned()
                        {
                            handled = true;
                            let target = if entry.is_dir {
                                format!("{}/", entry.relative_path)
                            } else {
                                entry.relative_path
                            };
                            this.insert_file_reference(&target, window, cx);
                            this.textarea.update(cx, |st, cx| st.focus(window, cx));
                        }
                    }
                    "escape" => {
                        handled = true;
                        this.dismiss_file_picker(cx);
                        this.textarea.update(cx, |st, cx| st.focus(window, cx));
                    }
                    _ => {}
                }
            });
            if handled {
                window.prevent_default();
                cx.stop_propagation();
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            app_state,
            textarea,
            disabled_bottom_input,
            model_select,
            synced_model_keys,
            synced_selected_key,
            synced_send_shortcut: Some(initial_shortcut),
            synced_locale: initial_locale,
            bottom_slot_width: 560.0,
            is_promoted: false,
            at_query: None,
            ignored_at_positions: std::collections::HashSet::new(),
            file_picker_open: false,
            file_picker_dir: String::new(),
            file_picker_entries: Vec::new(),
            file_picker_selected_idx: 0,
            file_picker_scroll: ScrollHandle::new(),
            slash_query: None,
            session_drafts: HashMap::new(),
            last_rendered_session_id: None,
        }
    }

    pub fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.textarea.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.textarea.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.at_query = None;
        self.ignored_at_positions.clear();
        self.file_picker_open = false;
        self.slash_query = None;
        self.app_state.update(cx, |s, cx| {
            s.submit_prompt(text, cx);
        });
    }

    fn dismiss_file_picker(&mut self, cx: &mut Context<Self>) {
        if self.at_query.is_some() {
            let text = self.textarea.read(cx).value().to_string();
            let mut cursor_byte = self.textarea.read(cx).cursor().min(text.len());
            while cursor_byte > 0 && !text.is_char_boundary(cursor_byte) {
                cursor_byte -= 1;
            }
            if let Some(at_pos) = text[..cursor_byte].rfind('@') {
                self.ignored_at_positions.insert(at_pos);
            }
        }
        self.at_query = None;
        self.file_picker_open = false;
        cx.notify();
    }

    fn is_dir_browser_mode(&self) -> bool {
        self.file_picker_open && self.at_query.as_deref().map(str::is_empty).unwrap_or(true)
    }

    fn reload_file_browser(&mut self, cx: &gpui_kit::App) {
        let root = self.app_state.read(cx).active_cwd.clone();
        self.file_picker_entries = list_workspace_directory(&root, &self.file_picker_dir);
        self.file_picker_selected_idx = 0;
    }

    fn enter_file_picker_dir(&mut self, rel_dir: String, cx: &mut Context<Self>) {
        self.file_picker_dir = rel_dir;
        self.reload_file_browser(cx);
        self.file_picker_scroll.scroll_to_item(0);
        cx.notify();
    }

    fn exit_file_picker_dir(&mut self, cx: &mut Context<Self>) {
        if self.file_picker_dir.is_empty() {
            return;
        }
        let prev_dir = self.file_picker_dir.clone();
        if let Some(slash) = self.file_picker_dir.rfind('/') {
            self.file_picker_dir = self.file_picker_dir[..slash].to_string();
        } else {
            self.file_picker_dir.clear();
        }
        self.reload_file_browser(cx);
        if let Some(pos) = self
            .file_picker_entries
            .iter()
            .position(|e| e.relative_path == prev_dir)
        {
            self.file_picker_selected_idx = pos;
            self.file_picker_scroll.scroll_to_item(pos);
        }
        cx.notify();
    }

    fn insert_file_reference(&mut self, rel: &str, window: &mut Window, cx: &mut Context<Self>) {
        let cur = self.textarea.read(cx).value().to_string();
        let (updated, inserted_at_pos) = if self.at_query.is_some() {
            let mut cursor_byte = self.textarea.read(cx).cursor().min(cur.len());
            while cursor_byte > 0 && !cur.is_char_boundary(cursor_byte) {
                cursor_byte -= 1;
            }
            if let Some(pos) = cur[..cursor_byte].rfind('@') {
                (format!("{}@{} {}", &cur[..pos], rel, &cur[cursor_byte..]), pos)
            } else {
                let pos = cur.len();
                (format!("{cur}@{rel} "), pos)
            }
        } else {
            let sep = if cur.is_empty() || cur.ends_with(char::is_whitespace) {
                ""
            } else {
                " "
            };
            let pos = cur.len() + sep.len();
            (format!("{cur}{sep}@{rel} "), pos)
        };
        self.ignored_at_positions.insert(inserted_at_pos);
        self.at_query = None;
        self.file_picker_open = false;
        let line = updated.bytes().filter(|&b| b == b'\n').count() as u32;
        let col = updated
            .rsplit('\n')
            .next()
            .map(|s| s.chars().count() as u32)
            .unwrap_or(0);
        self.textarea.update(cx, |st, cx| {
            st.set_value(&updated, window, cx);
        });
        let textarea = self.textarea.clone();
        window.defer(cx, move |window, cx| {
            textarea.update(cx, |st, cx| {
                st.set_cursor_position(
                    gpui_kit::component::input::Position::new(line, col),
                    window,
                    cx,
                );
            });
        });
        cx.notify();
    }

    pub fn render_composer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Isolate composer draft & state per session: switching sessions immediately restores
        // that session's draft (or blank) so text and states are never shared across conversations.
        let active_sid = self.app_state.read(cx).active_session_id.clone();
        if active_sid != self.last_rendered_session_id {
            let current_text = self.textarea.read(cx).value().to_string();
            if !current_text.trim().is_empty() {
                self.session_drafts.insert(self.last_rendered_session_id.clone(), current_text);
            } else {
                self.session_drafts.remove(&self.last_rendered_session_id);
            }
            self.last_rendered_session_id = active_sid.clone();
            let saved_draft = self.session_drafts.get(&active_sid).cloned().unwrap_or_default();
            self.textarea.update(cx, |st, cx| {
                st.set_value(&saved_draft, window, cx);
            });
            self.is_promoted = false;
            self.at_query = None;
            self.file_picker_open = false;
            self.slash_query = None;
        }

        let restore = self
            .app_state
            .update(cx, |s, _| s.draft_restore_text.take());
        if let Some(restored) = restore {
            let line = restored.bytes().filter(|&b| b == b'\n').count() as u32;
            let col = restored
                .rsplit('\n')
                .next()
                .map(|s| s.chars().count() as u32)
                .unwrap_or(0);
            self.textarea.update(cx, |state, cx| {
                state.set_value(&restored, window, cx);
            });
            let textarea = self.textarea.clone();
            window.defer(cx, move |window, cx| {
                textarea.update(cx, |state, cx| {
                    state.set_cursor_position(
                        gpui_kit::component::input::Position::new(line, col),
                        window,
                        cx,
                    );
                });
            });
        }

        let pending_paste = self
            .app_state
            .update(cx, |s, _| s.pending_composer_paste.take());
        if let Some(paste_text) = pending_paste {
            let cur = self.textarea.read(cx).value().to_string();
            let sep = if cur.is_empty() || cur.ends_with('\n') || cur.ends_with(' ') {
                ""
            } else {
                "\n"
            };
            let updated = format!("{cur}{sep}{paste_text}");
            let line = updated.bytes().filter(|&b| b == b'\n').count() as u32;
            let col = updated
                .rsplit('\n')
                .next()
                .map(|s| s.chars().count() as u32)
                .unwrap_or(0);
            self.textarea.update(cx, |state, cx| {
                state.set_value(&updated, window, cx);
                state.focus(window, cx);
            });
            let textarea = self.textarea.clone();
            window.defer(cx, move |window, cx| {
                textarea.update(cx, |state, cx| {
                    state.set_cursor_position(
                        gpui_kit::component::input::Position::new(line, col),
                        window,
                        cx,
                    );
                });
            });
        }

        // Consume pending @file mention triggered from FileExplorer
        let pending_mention = self
            .app_state
            .update(cx, |s, _| s.pending_composer_mention.take());
        if let Some(mention) = pending_mention {
            let cur = self.textarea.read(cx).value().to_string();
            let sep = if cur.is_empty() || cur.ends_with(char::is_whitespace) {
                ""
            } else {
                " "
            };
            let updated = format!("{cur}{sep}{mention} ");
            let line = updated.bytes().filter(|&b| b == b'\n').count() as u32;
            let col = updated
                .rsplit('\n')
                .next()
                .map(|s| s.chars().count() as u32)
                .unwrap_or(0);
            self.textarea.update(cx, |state, cx| {
                state.set_value(&updated, window, cx);
            });
            let textarea = self.textarea.clone();
            window.defer(cx, move |window, cx| {
                textarea.update(cx, |state, cx| {
                    state.set_cursor_position(
                        gpui_kit::component::input::Position::new(line, col),
                        window,
                        cx,
                    );
                });
            });
        }

        // Sync grouped model_select items & active selection whenever sidecar models or session model update
        let (model_keys, models_snapshot, selected_key) = {
            let s = self.app_state.read(cx);
            let visible = s.visible_picker_models();
            let keys: Vec<String> = visible
                .iter()
                .map(|m| format!("{}:{}", m.provider, m.id))
                .collect();
            let sel_key = s
                .selected_model
                .as_ref()
                .map(|m| (m.provider.clone(), m.id.clone()));
            (keys, visible, sel_key)
        };
        if model_keys != self.synced_model_keys {
            self.synced_model_keys = model_keys;
            self.synced_selected_key = selected_key.clone();
            let delegate = ModelGroupDelegate::from_models(&models_snapshot);
            let sel_ix = selected_key.as_ref().and_then(|(p, id)| {
                let k = SharedString::from(format!("{p}:{id}"));
                delegate.position(&k)
            });
            self.model_select.update(cx, |sel, cx| {
                sel.set_items(delegate, window, cx);
                sel.set_selected_index(sel_ix, window, cx);
            });
        } else if selected_key != self.synced_selected_key {
            self.synced_selected_key = selected_key.clone();
            self.model_select.update(cx, |sel, cx| {
                if let Some((p, id)) = selected_key {
                    sel.set_selected_value(&SharedString::from(format!("{p}:{id}")), window, cx);
                } else {
                    sel.set_selected_index(None, window, cx);
                }
            });
        }

        let current_text = self.textarea.read(cx).value().to_string();
        let has_text = !current_text.trim().is_empty();

        // Measure whether the text exceeds the single-line bottom slot width or contains newlines
        let should_promote = if current_text.contains('\n') {
            true
        } else if current_text.is_empty() {
            false
        } else {
            let font = Font {
                family: cx.theme().font_family.clone(),
                ..Default::default()
            };
            let run = TextRun {
                len: current_text.len(),
                font,
                color: cx.theme().foreground,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let measured_w: f32 = window
                .text_system()
                .shape_line(SharedString::from(current_text.clone()), px(14.), &[run], None)
                .width
                .into();
            let avail = (self.bottom_slot_width - 20.0).max(200.0);
            if self.is_promoted {
                measured_w > (avail - 24.0).max(160.0)
            } else {
                measured_w > avail
            }
        };
        if should_promote != self.is_promoted {
            self.is_promoted = should_promote;
            let fh = self.textarea.read(cx).focus_handle(cx);
            fh.focus(window, cx);
        }

        let (send_shortcut, locale_now) = {
            let s = self.app_state.read(cx);
            (s.send_shortcut, s.locale)
        };
        let shortcut_changed = self.synced_send_shortcut != Some(send_shortcut);
        let locale_changed = self.synced_locale != locale_now;
        if shortcut_changed {
            self.synced_send_shortcut = Some(send_shortcut);
            self.textarea.update(cx, |st, cx| {
                st.set_submit_on_enter(send_shortcut == SendShortcut::Enter, cx);
            });
        }
        if shortcut_changed || locale_changed {
            self.synced_locale = locale_now;
            let placeholder = composer_placeholder(locale_now, send_shortcut);
            self.textarea.update(cx, |st, cx| {
                st.set_placeholder(placeholder, window, cx);
            });
        }

        let state = self.app_state.read(cx);
        let locale = state.locale;
        let is_running = state.is_agent_running;
        let activity_hint = compute_composer_activity_hint(state, locale);
        let thinking = state.thinking_level;
        let (supported_thinking, thinking_map): (Vec<String>, HashMap<String, String>) = state
            .selected_model
            .as_ref()
            .map(|m| (m.thinking_levels.clone(), m.thinking_level_map.clone()))
            .unwrap_or_default();
        let preset = state.tool_preset;
        let auto_compaction = state.auto_compaction;
        let auto_retry = state.auto_retry;
        let steering_mode = state.steering_mode;
        let follow_up_mode = state.follow_up_mode;
        let outbox = state.outbox.clone();
        let file_matches = state.file_search_results.clone();
        let skills = state.skills.clone();
        let todos = state.todos.clone();
        let todos_expanded = state.todos_expanded;
        let todos_hidden = state.todos_hidden;
        let has_ctx = state.stats.has_context_usage;
        let ctx_tokens = state.stats.context_tokens;
        let ctx_window = state.stats.context_window;
        let ctx_pct = state.stats.context_used_pct;
        let total_cost = state.stats.cost_usd;
        let is_workspace_project = !state.active_is_temp_chat
            && !state
                .active_session_id
                .as_ref()
                .is_some_and(|sid| state.temp_chat_ids.contains(sid))
            && state.active_workspace_kind != WorkspaceKind::Chat
            && !crate::client::local_store::is_chat_workspace_cwd(&state.active_cwd);
        let project_label = SharedString::from(
            state
                .active_cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Workspace")
                .to_string(),
        );

        let app_think = self.app_state.clone();
        let app_preset = self.app_state.clone();
        let app_auto = self.app_state.clone();
        let app_stop = self.app_state.clone();
        let app_proj = self.app_state.clone();
        let app_ctx = self.app_state.clone();

        let mut wrapper = v_flex()
            .w_full()
            .max_w(px(state.chat_content_width))
            .mx_auto()
            .gap_2()
            .track_focus(&self.focus_handle);

        // Live activity status hint in the gap above composer/todos, strictly <= half composer width
        if let Some(hint) = activity_hint {
            wrapper = wrapper.child(
                h_flex()
                    .w_full()
                    .px_2()
                    .pb_0p5()
                    .child(
                        div()
                            .max_w(px(400.))
                            .truncate()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                ShimmerText::new(hint)
                                    .id("composer-activity-shimmer")
                                    .duration(std::time::Duration::from_millis(1400))
                                    .text_xs(),
                            ),
                    ),
            );
        }

        // 0. rpiv-todos collapsible rounded progress panel above the composer
        if !todos.is_empty() && !todos_hidden {
            let done_count = todos.iter().filter(|t| t.done).count();
            let total_count = todos.len();
            let pct = (done_count as f32 / total_count as f32) * 100.0;
            let app_toggle_todos = self.app_state.clone();

            let todo_header = h_flex()
                .id("rpiv-todos-header")
                .w_full()
                .px_3()
                .py_2()
                .justify_between()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().background.opacity(0.45)))
                .on_click(move |_, _, cx| {
                    app_toggle_todos.update(cx, |s, cx| {
                        s.todos_expanded = !s.todos_expanded;
                        cx.notify();
                    });
                })
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::CircleCheck).xsmall())
                        .child(
                            div()
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child("rpiv-todos"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{}/{} {}",
                                    format_commas(done_count),
                                    format_commas(total_count),
                                    tr(locale, "chat.todosCompleted")
                                )),
                        ),
                )
                .child(
                    // Progress, collapse chevron and dismiss share one right-aligned group so
                    // the header's justify_between keeps them flush with the card edge.
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1p5()
                        .child(div().w(px(72.)).child(Progress::new("rpiv-todos-prog").value(pct)))
                        .child(
                            Icon::new(if todos_expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            })
                            .xsmall(),
                        )
                        .children({
                            let app_dismiss_todos = self.app_state.clone();
                            Some(
                                div()
                                    .id("rpiv-todos-dismiss")
                                    .flex_shrink_0()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                    .on_click(|_, _, cx| cx.stop_propagation())
                                    .child(
                                        Button::new("rpiv-todos-dismiss-btn")
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Close)
                                            .tooltip(tr(locale, "chat.dismissTodos"))
                                            .on_click(move |_, _, cx| {
                                                cx.stop_propagation();
                                                app_dismiss_todos.update(cx, |s, cx| {
                                                    s.dismiss_todos(cx);
                                                });
                                            }),
                                    ),
                            )
                        }),
                );

            let mut todo_card = v_flex()
                .w_full()
                .overflow_hidden()
                .rounded_xl()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(todo_header);

            if todos_expanded {
                let mut list = v_flex()
                    .id("rpiv-todos-scroll")
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .gap_1p5()
                    .border_t_1()
                    .border_color(cx.theme().border);

                for (idx, item) in todos.into_iter().enumerate() {
                    let _ = &item.id;
                    list = list.child(
                        h_flex()
                            .id(("todo-item", idx))
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(if item.done {
                                    IconName::CircleCheck
                                } else {
                                    IconName::Dash
                                })
                                .xsmall(),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .text_color(if item.done {
                                        cx.theme().muted_foreground
                                    } else {
                                        cx.theme().foreground
                                    })
                                    .child(item.content),
                            ),
                    );
                }
                todo_card = todo_card.child(list);
            }

            wrapper = wrapper.child(todo_card);
        }

        // 1. Outbox queue while agent is running
        if !outbox.is_empty() {
            let mut box_el = v_flex()
                .w_full()
                .p_2()
                .gap_1()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} ({})", tr(locale, "chat.queued"), format_commas(outbox.len()))),
                );

            for item in outbox {
                let qid = item.id;
                let qtext = item.text.clone();
                let app_send = self.app_state.clone();
                let app_edit = self.app_state.clone();
                let app_edit_txt = self.app_state.clone();
                let app_rm = self.app_state.clone();
                box_el = box_el.child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .id(("edit-queue-text", qid))
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .truncate()
                                .cursor_pointer()
                                .hover(|s| s.text_color(cx.theme().foreground))
                                .on_click(move |_, _, cx| {
                                    app_edit_txt.update(cx, |s, cx| {
                                        s.edit_queued_message(qid, cx);
                                    });
                                })
                                .child(SharedString::from(qtext)),
                        )
                        .child(
                            h_flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new(("send-queue-now", qid))
                                        .ghost()
                                        .xsmall()
                                        .h(px(24.))
                                        .icon(IconName::ArrowUp)
                                        .label(tr(locale, "chat.sendQueuedNow"))
                                        .tooltip(tr(locale, "chat.sendQueuedNow"))
                                        .on_click(move |_, _, cx| {
                                            app_send.update(cx, |s, cx| {
                                                s.send_queued_now(qid, cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new(("edit-queue", qid))
                                        .ghost()
                                        .w(px(24.))
                                        .h(px(24.))
                                        .icon(IconName::Undo2)
                                        .tooltip(tr(locale, "chat.editQueuedMessage"))
                                        .on_click(move |_, _, cx| {
                                            app_edit.update(cx, |s, cx| {
                                                s.edit_queued_message(qid, cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new(("rm-queue", qid))
                                        .ghost()
                                        .w(px(24.))
                                        .h(px(24.))
                                        .icon(Icon::new(crate::icons::TrashIcon))
                                        .tooltip(tr(locale, "chat.deleteQueuedMessage"))
                                        .on_click(move |_, _, cx| {
                                            app_rm.update(cx, |s, cx| {
                                                s.delete_queued_message(qid, cx);
                                            });
                                        }),
                                ),
                        ),
                );
            }
            wrapper = wrapper.child(box_el);
        }

        // 2. Slash command (`/`) autocomplete popup
        if let Some(ref sq) = self.slash_query {
            let mut items: Vec<(String, String, bool)> = BUILTIN_SLASH_COMMANDS
                .iter()
                .filter(|c| sq.is_empty() || c.name.contains(sq.as_str()))
                .map(|c| {
                    (
                        format!("/{}", c.name),
                        tr(locale, c.desc_key).to_string(),
                        c.execute_immediately,
                    )
                })
                .collect();

            for sk in &skills {
                if sk.enabled && (sq.is_empty() || sk.name.to_lowercase().contains(sq.as_str())) {
                    items.push((
                        format!("/skill:{}", sk.name),
                        sk.description.clone(),
                        false,
                    ));
                }
            }

            let mut popup = v_flex()
                .w_full()
                .max_h(px(220.))
                .overflow_hidden()
                .p_1p5()
                .gap_0p5()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .child(
                    h_flex()
                        .px_2p5()
                        .py_1()
                        .justify_between()
                        .items_center()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .text_xs()
                                .font_medium()
                                .text_color(cx.theme().muted_foreground)
                                .child(tr(locale, "chat.slashCommands")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} {}", format_commas(items.len()), tr(locale, "chat.available"))),
                        ),
                );

            if items.is_empty() {
                popup = popup.child(
                    div()
                        .px_2p5()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(locale, "chat.noCommands")),
                );
            } else {
                for (idx, (cmd_name, desc, exec_now)) in items.into_iter().take(10).enumerate() {
                    let cmd_fill = cmd_name.clone();
                    popup = popup.child(
                        h_flex()
                            .id(("slash-cmd-row", idx))
                            .w_full()
                            .min_w_0()
                            .px_2p5()
                            .py_1p5()
                            .gap_3()
                            .items_center()
                            .justify_start()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if exec_now {
                                    this.textarea.update(cx, |st, cx| {
                                        st.set_value("", window, cx);
                                    });
                                    this.slash_query = None;
                                    this.app_state.update(cx, |s, cx| {
                                        s.submit_prompt(cmd_fill.clone(), cx);
                                    });
                                } else {
                                    let val = format!("{cmd_fill} ");
                                    this.textarea.update(cx, |st, cx| {
                                        st.set_value(&val, window, cx);
                                    });
                                    this.slash_query = None;
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(140.))
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .truncate()
                                    .child(SharedString::from(cmd_name)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(SharedString::from(desc)),
                            ),
                    );
                }
            }

            wrapper = wrapper.child(popup);
        }

        // 3. Native @file fuzzy popup / "+" root-first directory browser with full keyboard nav
        if self.at_query.is_some() || self.file_picker_open {
            let is_dir_browser = self.is_dir_browser_mode();
            let list_len = if is_dir_browser {
                self.file_picker_entries.len()
            } else {
                file_matches.len().min(40)
            };
            if list_len > 0 && self.file_picker_selected_idx >= list_len {
                self.file_picker_selected_idx = list_len - 1;
            }

            let mut list_scroll = v_flex()
                .id("at-file-scroll")
                .track_scroll(&self.file_picker_scroll)
                .w_full()
                .max_h(px(210.))
                .overflow_y_scroll()
                .cursor_pointer();

            if list_len == 0 {
                list_scroll = list_scroll.child(
                    div()
                        .px_2p5()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(locale, "chat.noFiles")),
                );
            } else if is_dir_browser {
                if !self.file_picker_dir.is_empty() {
                    list_scroll = list_scroll.child(
                        h_flex()
                            .id("at-file-parent-dir")
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .px_2p5()
                            .py_1p5()
                            .gap_2()
                            .items_center()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.exit_file_picker_dir(cx);
                            }))
                            .child(
                                Icon::new(IconName::FolderOpen)
                                    .xsmall()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("..."),
                            ),
                    );
                }
                let entries = self.file_picker_entries.clone();
                let selected_ix = self.file_picker_selected_idx;
                for (idx, entry) in entries.into_iter().enumerate() {
                    let is_selected = idx == selected_ix;
                    let rel = entry.relative_path.clone();
                    let rel_pick_dir = entry.relative_path.clone();
                    let is_dir = entry.is_dir;
                    list_scroll = list_scroll.child(
                        h_flex()
                            .id(("at-file-row", idx))
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .px_2p5()
                            .py_1p5()
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(if is_selected {
                                cx.theme().accent.opacity(0.18)
                            } else {
                                cx.theme().transparent
                            })
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if is_dir {
                                    this.enter_file_picker_dir(rel.clone(), cx);
                                } else {
                                    this.insert_file_reference(&rel, window, cx);
                                }
                            }))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Icon::new(if is_dir {
                                            IconName::Folder
                                        } else {
                                            IconName::FileText
                                        })
                                        .xsmall()
                                        .text_color(if is_dir {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().muted_foreground
                                        }),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .font_medium()
                                            .text_color(cx.theme().foreground)
                                            .truncate()
                                            .child(SharedString::from(entry.name)),
                                    ),
                            )
                            .children(is_dir.then(|| {
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new(("pick-dir-ref", idx))
                                            .ghost()
                                            .xsmall()
                                            .icon(crate::icons::AtSignIcon)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.insert_file_reference(
                                                    &format!("{rel_pick_dir}/"),
                                                    window,
                                                    cx,
                                                );
                                                this.textarea
                                                    .update(cx, |st, cx| st.focus(window, cx));
                                            })),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronRight)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                            })),
                    );
                }
            } else {
                let selected_ix = self.file_picker_selected_idx;
                for (idx, m) in file_matches.iter().take(40).enumerate() {
                    let is_selected = idx == selected_ix;
                    let rel = m.relative_path.clone();
                    list_scroll = list_scroll.child(
                        h_flex()
                            .id(("at-file-row", idx))
                            .w_full()
                            .min_w_0()
                            .flex_shrink_0()
                            .px_2p5()
                            .py_1p5()
                            .gap_2()
                            .items_center()
                            .justify_start()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(if is_selected {
                                cx.theme().accent.opacity(0.18)
                            } else {
                                cx.theme().transparent
                            })
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.insert_file_reference(&rel, window, cx);
                            }))
                            .child(Icon::new(IconName::FileText).xsmall())
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(cx.theme().foreground)
                                    .truncate()
                                    .child(SharedString::from(m.relative_path.clone())),
                            ),
                    );
                }
            }

            let header_title = if is_dir_browser {
                if self.file_picker_dir.is_empty() {
                    "/".to_string()
                } else {
                    format!("/{}", self.file_picker_dir)
                }
            } else {
                tr(locale, "chat.projectFiles").to_string()
            };

            let popup = v_flex()
                .w_full()
                .overflow_hidden()
                .p_1p5()
                .gap_1()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .child(
                    h_flex()
                        .px_2()
                        .py_1()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .items_center()
                                .gap_1p5()
                                .children((is_dir_browser && !self.file_picker_dir.is_empty()).then(|| {
                                    Button::new("file-picker-back")
                                        .ghost()
                                        .small()
                                        .h(px(26.))
                                        .w(px(26.))
                                        .icon(IconName::ChevronLeft)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.exit_file_picker_dir(cx);
                                        }))
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_xs()
                                        .font_medium()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(SharedString::from(header_title)),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground.opacity(0.75))
                                        .child(if is_dir_browser {
                                            "↑↓ · ←/→ · Enter · Tab · Esc"
                                        } else {
                                            "↑↓ · Enter · Esc"
                                        }),
                                )
                                .child(
                                    Button::new("close-at-popup")
                                        .ghost()
                                        .small()
                                        .h(px(28.))
                                        .w(px(28.))
                                        .icon(IconName::Close)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.dismiss_file_picker(cx);
                                            this.textarea.update(cx, |st, cx| st.focus(window, cx));
                                        })),
                                ),
                        ),
                )
                .child(list_scroll);
            wrapper = wrapper.child(popup);
        }

        // 4. Unified composer card matching original ChatInput.tsx + native-theme.css:
        //    36px circular buttons, 10px inset, 29px outer corner radius, single-row auto-grow
        let send_action_btn = if is_running && !has_text {
            Button::new("composer-stop-btn")
                .danger()
                .w(px(36.))
                .h(px(36.))
                .rounded_full()
                .flex_shrink_0()
                .icon(IconName::Square)
                .tooltip(tr(locale, "chat.stopAgent"))
                .on_click(move |_, _, cx| {
                    app_stop.update(cx, |s, cx| s.stop_running_agent(cx));
                })
                .into_any_element()
        } else {
            let send_tooltip = if is_running {
                tr(locale, "chat.queueMessage").to_string()
            } else {
                match send_shortcut {
                    SendShortcut::Enter => tr(locale, "chat.sendEnter").to_string(),
                    SendShortcut::CmdEnter => {
                        #[cfg(target_os = "macos")]
                        {
                            tr(locale, "chat.sendCmdEnter").to_string()
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            tr(locale, "chat.sendCtrlEnter").to_string()
                        }
                    }
                    SendShortcut::ShiftEnter => tr(locale, "chat.sendShiftEnter").to_string(),
                }
            };

            Button::new("composer-send-btn")
                .primary()
                .w(px(36.))
                .h(px(36.))
                .rounded_full()
                .flex_shrink_0()
                .icon(IconName::ArrowUp)
                .tooltip(send_tooltip)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.submit(window, cx);
                }))
                .into_any_element()
        };

        let build_active_textarea = || {
            div()
                .w_full()
                .min_w_0()
                .capture_key_down(cx.listener(move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();

                    // File picker navigation is handled once by cx.intercept_keystrokes;
                    // only block Enter submission while the popup is open.
                    if this.file_picker_open || this.at_query.is_some() {
                        if matches!(key, "up" | "down" | "left" | "right" | "enter" | "tab" | "escape") {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        return;
                    }

                    if key == "enter" {
                        let sc = this.app_state.read(cx).send_shortcut;
                        let sec = event.keystroke.modifiers.secondary(cx);
                        let shift = event.keystroke.modifiers.shift;

                        let should_send = match sc {
                            SendShortcut::Enter => !shift && !sec,
                            SendShortcut::CmdEnter => sec && !shift,
                            SendShortcut::ShiftEnter => shift && !sec,
                        };

                        if should_send {
                            cx.stop_propagation();
                            this.submit(window, cx);
                        }
                    }
                }))
                .child(Textarea::new(&self.textarea).appearance(false))
        };

        let entity_for_bounds = cx.entity().clone();
        let promoted_top = self.is_promoted.then(|| {
            div()
                .w_full()
                .px_4()
                .pt_2p5()
                .pb_0p5()
                .child(build_active_textarea())
        });

        let bottom_middle_slot = if self.is_promoted {
            div()
                .flex_1()
                .min_w_0()
                .on_prepaint(move |bounds, _, cx| {
                    let w: f32 = bounds.size.width.into();
                    entity_for_bounds.update(cx, |this, _| {
                        if w > 80.0 {
                            this.bottom_slot_width = w;
                        }
                    });
                })
                .child(
                    Input::new(&self.disabled_bottom_input)
                        .appearance(false)
                        .disabled(true),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .on_prepaint(move |bounds, _, cx| {
                    let w: f32 = bounds.size.width.into();
                    entity_for_bounds.update(cx, |this, _| {
                        if w > 80.0 {
                            this.bottom_slot_width = w;
                        }
                    });
                })
                .child(build_active_textarea())
                .into_any_element()
        };

        let bottom_row = h_flex()
            .w_full()
            .items_center()
            .gap_2p5()
            .px_3()
            .py_2()
            .child(
                Button::new("composer-attach-btn")
                    .ghost()
                    .w(px(36.))
                    .h(px(36.))
                    .rounded_full()
                    .flex_shrink_0()
                    .icon(IconName::Plus)
                    .tooltip(tr(locale, "chat.referenceFiles"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.file_picker_open = !this.file_picker_open;
                        if this.file_picker_open {
                            this.at_query = None;
                            this.file_picker_dir.clear();
                            this.reload_file_browser(cx);
                            window.focus(&this.focus_handle, cx);
                        } else {
                            this.textarea.update(cx, |st, cx| st.focus(window, cx));
                        }
                        cx.notify();
                    })),
            )
            .child(bottom_middle_slot)
            .when_clear_btn(has_text, locale, cx.listener(|this, _, window, cx| {
                this.textarea.update(cx, |st, cx| st.set_value("", window, cx));
                this.at_query = None;
                this.ignored_at_positions.clear();
                this.file_picker_open = false;
                this.slash_query = None;
                cx.notify();
            }))
            .when_running_stop(is_running && has_text, locale, self.app_state.clone())
            .child(send_action_btn);

        let composer_card = v_flex()
            .w_full()
            .rounded(if self.is_promoted { px(22.) } else { px(28.) })
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .children(promoted_top)
            .child(bottom_row);

        // 5. Session controls row below the composer card:
        //    - Left: Project chip, Model selector, Context usage
        //    - Right: Icon-only upward menus for Thinking level, Tools preset, and Session automation
        // Monochrome foreground color matching adjacent text height (~13.5px), danger >= 90%, warning >= 75%
        let ctx_ring_color = if !has_ctx || ctx_tokens.is_none() {
            cx.theme().muted_foreground
        } else if ctx_pct >= 90.0 {
            cx.theme().danger
        } else if ctx_pct >= 75.0 {
            cx.theme().warning
        } else {
            cx.theme().foreground
        };
        let ctx_label = if has_ctx && ctx_tokens.is_some() {
            format!("{ctx_pct:.0}%")
        } else {
            "—".to_string()
        };
        let ctx_tooltip = {
            let mut parts = Vec::new();
            if has_ctx && ctx_tokens.is_some() {
                parts.push(format!("{ctx_pct:.1}%"));
                if let (Some(tok), Some(win)) = (ctx_tokens, ctx_window) {
                    parts.push(format!(
                        "{} / {}",
                        format_tokens_km(tok),
                        format_tokens_km(win)
                    ));
                }
            } else if let Some(win) = ctx_window {
                parts.push(format!("— / {}", format_tokens_km(win)));
            } else {
                parts.push(tr(locale, "chat.contextUsage").to_string());
            }
            if total_cost > 0.0 {
                parts.push(format!("${total_cost:.3}"));
            }
            parts.join(" · ")
        };

        let controls_row = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2p5()
                    .children(is_workspace_project.then(|| {
                        Button::new("composer-project-chip")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .icon(IconName::Folder)
                            .label(project_label)
                            .tooltip(tr(locale, "panel.files"))
                            .on_click(move |_, _, cx| {
                                app_proj.update(cx, |s, cx| {
                                    s.open_right_tab(RightPanelTab::Files, cx);
                                });
                            })
                    }))
                    .child(
                        div().w(px(210.)).child(
                            Select::new(&self.model_select)
                                .appearance(false)
                                .small()
                                .h(px(32.))
                                .menu_width(px(320.))
                                .menu_max_h(px(280.)),
                        ),
                    )
                    .child((|| {
                        // Only the levels the selected model actually advertises are shown;
                        // a model without extended reasoning gets no control at all.
                        let mut levels: Vec<String> = supported_thinking
                            .iter()
                            .filter(|l| parse_thinking_level_str_local(l).is_some())
                            .cloned()
                            .collect();
                        crate::app_state::AppState::sanitize_thinking_levels(
                            &mut levels,
                            &thinking_map,
                            &Default::default(),
                        );
                        // Hidden only for models that advertise no thinking levels at
                        // all; a model whose only level is `off` still shows the chip.
                        if levels.is_empty() {
                            return div().into_any_element();
                        }


                        // Keep the label on a supported level even when the stored level
                        // came from another model.
                        let effective_level = levels
                            .iter()
                            .find(|l| parse_thinking_level_str_local(l) == Some(thinking))
                            .cloned()
                            .unwrap_or_else(|| {
                                levels
                                    .iter()
                                    .filter(|l| l.as_str() != "off")
                                    .find(|l| parse_thinking_level_str_local(l) == Some(ThinkingLevel::Medium))
                                    .cloned()
                                    .or_else(|| levels.iter().find(|l| l.as_str() != "off").cloned())
                                    .unwrap_or_else(|| "off".to_string())
                            });
                        let active_think_label: String = thinking_map
                            .get(&effective_level)
                            .cloned()
                            .unwrap_or_else(|| effective_level.clone());

                        let thinking_btn = Button::new("thinking-menu-btn")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .label(active_think_label.clone())
                            .tooltip(format!(
                                "{}: {}",
                                tr(locale, "chat.reasoningHeading"),
                                active_think_label
                            ));

                        if levels.len() <= 1 {
                            // Only `off` is available: show it, but do not open a menu.
                            return gpui_kit::component::Disableable::disabled(thinking_btn, true)
                                .into_any_element();
                        }

                        let levels_for_menu = levels.clone();
                        let map_for_menu = thinking_map.clone();
                        thinking_btn
                            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                                menu = menu.label(tr(locale, "chat.reasoningHeading"));
                                for lvl_str in &levels_for_menu {
                                    let Some(lvl) = parse_thinking_level_str_local(lvl_str) else {
                                        continue;
                                    };
                                    let app_t = app_think.clone();
                                    let is_active = parse_thinking_level_str_local(lvl_str) == Some(thinking);
                                    let display_name = map_for_menu
                                        .get(lvl_str.as_str())
                                        .cloned()
                                        .unwrap_or_else(|| lvl_str.clone());
                                    let label = if display_name != *lvl_str {
                                        format!(
                                            "{display_name} ({lvl_str}) — {}",
                                            tr(locale, lvl.description_key())
                                        )
                                    } else {
                                        format!(
                                            "{display_name} — {}",
                                            tr(locale, lvl.description_key())
                                        )
                                    };
                                    menu = menu.item(
                                        PopupMenuItem::new(label)
                                            .checked(is_active)
                                            .on_click(move |_, _, cx| {
                                                app_t.update(cx, |s, cx| {
                                                    s.set_thinking_level_choice(lvl, cx);
                                                });
                                            }),
                                    );
                                }
                                menu
                            })
                            .into_any_element()
                    })())
                    .child({
                        let can_compact = state.active_session_id.is_some()
                            && state.sidecar_connected
                            && !state.is_compacting
                            && !state.is_agent_running;
                        let is_compacting = state.is_compacting;
                        let app_compact = app_ctx.clone();
                        let app_details = app_ctx.clone();
                        Button::new("composer-ctx-usage")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .px_2p5()
                            .tooltip(ctx_tooltip.clone())
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        ProgressCircle::new("composer-ctx-circle")
                                            .value(if has_ctx && ctx_tokens.is_some() {
                                                ctx_pct.max(4.0)
                                            } else {
                                                0.0
                                            })
                                            .size(px(15.))
                                            .color(ctx_ring_color),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_medium()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(ctx_label),
                                    ),
                            )
                            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |menu, _, _| {
                                let app_c = app_compact.clone();
                                let app_d = app_details.clone();
                                menu.item(
                                    PopupMenuItem::new(if is_compacting {
                                        tr(locale, "chat.compacting")
                                    } else {
                                        tr(locale, "chat.compactContext")
                                    })
                                    .icon(if is_compacting {
                                        IconName::LoaderCircle
                                    } else {
                                        IconName::Minimize
                                    })
                                    .disabled(!can_compact)
                                    .on_click(move |_, _, cx| {
                                        app_c.update(cx, |s, cx| {
                                            s.trigger_compact(cx);
                                        });
                                    }),
                                )
                                .item(
                                    PopupMenuItem::new(tr(locale, "chat.viewSessionDetails"))
                                        .icon(IconName::Inspector)
                                        .on_click(move |_, _, cx| {
                                            app_d.update(cx, |s, cx| {
                                                s.open_right_tab(RightPanelTab::Inspector, cx);
                                            });
                                        }),
                                )
                            })
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2p5()
                    .child(
                        Button::new("tools-menu-btn")
                            .ghost()
                            .small()
                            .h(px(32.))
                            .icon(Icon::new(ShieldIcon))
                            .label(tr(locale, preset.label_key()))
                            .tooltip(format!(
                                "{}: {} — {}",
                                tr(locale, "chat.toolsHeading"),
                                tr(locale, preset.label_key()),
                                tr(locale, preset.description_key())
                            ))
                            .dropdown_menu_with_anchor(Anchor::BottomRight, move |mut menu, _, _| {
                                menu = menu.label(tr(locale, "chat.toolsHeading"));
                                for &p in ToolPreset::all() {
                                    let app_p = app_preset.clone();
                                    let is_active = p == preset;
                                    menu = menu.item(
                                        PopupMenuItem::new(format!(
                                            "{} — {}",
                                            tr(locale, p.label_key()),
                                            tr(locale, p.description_key())
                                        ))
                                        .checked(is_active)
                                        .on_click(move |_, _, cx| {
                                            app_p.update(cx, |s, cx| {
                                                s.set_tool_preset_choice(p, cx);
                                            });
                                        }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Button::new("automation-menu-btn")
                            .ghost()
                            .w(px(32.))
                            .h(px(32.))
                            .icon(Icon::new(ZapIcon))
                            .tooltip(tr(locale, "chat.automationTooltip"))
                            .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                                let app_cmp = app_auto.clone();
                                let app_ac = app_auto.clone();
                                let app_ar = app_auto.clone();
                                let app_st_all = app_auto.clone();
                                let app_st_one = app_auto.clone();
                                let app_fu_all = app_auto.clone();
                                let app_fu_one = app_auto.clone();

                                menu.label(tr(locale, "chat.automationHeading"))
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.compactNow"))
                                            .icon(IconName::Minimize)
                                            .on_click(move |_, _, cx| {
                                                app_cmp.update(cx, |s, cx| s.trigger_compact(cx));
                                            }),
                                    )
                                    .separator()
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.autoCompaction"))
                                            .checked(auto_compaction)
                                            .on_click(move |_, _, cx| {
                                                app_ac.update(cx, |s, cx| {
                                                    s.set_auto_compaction(!auto_compaction, cx);
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.autoRetry"))
                                            .checked(auto_retry)
                                            .on_click(move |_, _, cx| {
                                                app_ar.update(cx, |s, cx| {
                                                    s.set_auto_retry(!auto_retry, cx);
                                                });
                                            }),
                                    )
                                    .separator()
                                    .label(tr(locale, "chat.steeringMode"))
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.steeringAll"))
                                            .checked(steering_mode == SteeringMode::All)
                                            .on_click(move |_, _, cx| {
                                                app_st_all.update(cx, |s, cx| {
                                                    s.set_steering_mode(SteeringMode::All, cx);
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.steeringOne"))
                                            .checked(steering_mode == SteeringMode::OneAtATime)
                                            .on_click(move |_, _, cx| {
                                                app_st_one.update(cx, |s, cx| {
                                                    s.set_steering_mode(SteeringMode::OneAtATime, cx);
                                                });
                                            }),
                                    )
                                    .separator()
                                    .label(tr(locale, "chat.followUpMode"))
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.followUpAll"))
                                            .checked(follow_up_mode == SteeringMode::All)
                                            .on_click(move |_, _, cx| {
                                                app_fu_all.update(cx, |s, cx| {
                                                    s.set_follow_up_mode(SteeringMode::All, cx);
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.followUpOne"))
                                            .checked(follow_up_mode == SteeringMode::OneAtATime)
                                            .on_click(move |_, _, cx| {
                                                app_fu_one.update(cx, |s, cx| {
                                                    s.set_follow_up_mode(SteeringMode::OneAtATime, cx);
                                                });
                                            }),
                                    )
                            }),
                    ),
            );

        div()
            .w_full()
            .px_6()
            .pb_3()
            .pt_2()
            .bg(cx.theme().background)
            .child(wrapper.child(composer_card).child(controls_row))
    }
}

fn truncate_chars_ellipsis(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn compute_composer_activity_hint(state: &AppState, locale: AppLocale) -> Option<String> {
    if state.is_compacting {
        return Some(truncate_chars_ellipsis(tr(locale, "chat.compacting"), 32));
    }
    if let Some(ref retry) = state.retry_status {
        let msg = format!("{} ({retry})...", tr(locale, "chat.phaseRetrying"));
        return Some(truncate_chars_ellipsis(&msg, 32));
    }
    if !state.is_agent_running {
        return None;
    }
    if let Some(last) = state.messages.last() {
        if last.role == MessageRole::Assistant {
            if let Some(tc) = last.tool_calls.iter().rev().find(|t| t.is_running) {
                let name = tc.name.as_ref();
                let target = tc
                    .arguments
                    .iter()
                    .find(|(k, _)| matches!(k.as_ref(), "path" | "file_path" | "command"))
                    .map(|(_, v)| {
                        let s = v.as_ref().trim();
                        if name == "bash" || name == "command" {
                            s.lines().next().unwrap_or(s).to_string()
                        } else {
                            std::path::Path::new(s)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(s)
                                .to_string()
                        }
                    });
                let verb = match name {
                    "read" => tr(locale, "chat.phaseReading"),
                    "write" => tr(locale, "chat.phaseWriting"),
                    "edit" => tr(locale, "chat.phaseEditing"),
                    _ => tr(locale, "chat.phaseRunning"),
                };
                let raw = match target {
                    Some(t) if !t.is_empty() => format!("{verb} {t}"),
                    _ => format!("{verb} {name}"),
                };
                return Some(truncate_chars_ellipsis(&raw, 32));
            }
            if last.content.trim().is_empty() {
                return Some(truncate_chars_ellipsis(
                    tr(locale, "chat.phaseThinking"),
                    32,
                ));
            }
            return Some(truncate_chars_ellipsis(
                tr(locale, "chat.phaseGenerating"),
                32,
            ));
        }
    }
    Some(truncate_chars_ellipsis(
        tr(locale, "chat.phaseThinking"),
        32,
    ))
}

fn parse_thinking_level_str_local(raw: &str) -> Option<ThinkingLevel> {
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

trait ComposerExt {
    fn when_clear_btn(
        self,
        show: bool,
        locale: AppLocale,
        on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    ) -> Self;
    fn when_running_stop(self, show: bool, locale: AppLocale, app: Entity<AppState>) -> Self;
}

impl<E: ParentElement> ComposerExt for E {
    fn when_clear_btn(
        self,
        show: bool,
        locale: AppLocale,
        on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    ) -> Self {
        if !show {
            return self;
        }
        self.child(
            Button::new("composer-clear-btn")
                .ghost()
                .w(px(36.))
                .h(px(36.))
                .rounded_full()
                .flex_shrink_0()
                .icon(Icon::new(crate::icons::TrashIcon))
                .tooltip(tr(locale, "chat.clearDraft"))
                .on_click(on_click),
        )
    }

    fn when_running_stop(self, show: bool, locale: AppLocale, app: Entity<AppState>) -> Self {
        if !show {
            return self;
        }
        self.child(
            Button::new("composer-stop-inline-btn")
                .danger()
                .w(px(36.))
                .h(px(36.))
                .rounded_full()
                .flex_shrink_0()
                .icon(IconName::Square)
                .tooltip(tr(locale, "chat.stopAgent"))
                .on_click(move |_, _, cx| {
                    app.update(cx, |s, cx| s.stop_running_agent(cx));
                }),
        )
    }
}
