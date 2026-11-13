use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::chat::ConversationScrollerState;
use crate::i18n::{AppLocale, format_commas, tr};
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::{ActiveTheme as _, IconName};
use gpui_kit::{
    AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px,
};

/// One row of the conversation command palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    Search,
    Prev,
    Next,
    Settings,
    Tasks,
    Scheduled,
    Artifacts,
    ToggleRightPanel,
    Theme,
}

pub struct ConversationCommandPalette {
    app_state: Entity<AppState>,
    scroller: Entity<ConversationScrollerState>,
    state: Entity<CommandState>,
    matched_msg_indices: Vec<usize>,
    current_match: usize,
    last_locale: AppLocale,
    last_query: String,
    was_open: bool,
}

impl gpui_kit::Render for ConversationCommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_palette(window, cx)
    }
}

impl ConversationCommandPalette {
    pub fn new(
        app_state: Entity<AppState>,
        scroller: Entity<ConversationScrollerState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_locale = app_state.read(cx).locale;
        let state = cx.new(|cx| CommandState::new(window, cx));
        state.update(cx, |s, cx| s.focus(window, cx));

        Self {
            app_state,
            scroller,
            state,
            matched_msg_indices: Vec::new(),
            current_match: 0,
            last_locale: initial_locale,
            last_query: String::new(),
            was_open: false,
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.app_state.update(cx, |s, cx| {
            s.find_open = false;
            s.find_query = SharedString::default();
            cx.notify();
        });
        cx.notify();
    }

    fn recompute_matches(&mut self, cx: &mut Context<Self>) {
        let query = self.last_query.trim().to_lowercase();
        let messages = self.app_state.read(cx).messages.clone();
        self.matched_msg_indices.clear();
        if query.is_empty() {
            self.current_match = 0;
            return;
        }
        for (ix, m) in messages.iter().enumerate() {
            let mut hit = m.content.to_lowercase().contains(&query)
                || m.thinking.to_lowercase().contains(&query);
            if !hit {
                for tc in &m.tool_calls {
                    if tc.name.to_lowercase().contains(&query)
                        || tc.output.to_lowercase().contains(&query)
                    {
                        hit = true;
                        break;
                    }
                }
            }
            if hit {
                self.matched_msg_indices.push(ix);
            }
        }
        if self.current_match >= self.matched_msg_indices.len() {
            self.current_match = 0;
        }
    }

    fn jump_to_current(&self, cx: &mut Context<Self>) {
        if let Some(ix) = self.matched_msg_indices.get(self.current_match).copied() {
            self.scroller.update(cx, |s, cx| {
                let _ = s.scroll_to_item(ix, cx);
            });
        }
    }

    fn next_match(&mut self, cx: &mut Context<Self>) {
        if self.matched_msg_indices.is_empty() {
            return;
        }
        self.current_match = (self.current_match + 1) % self.matched_msg_indices.len();
        self.jump_to_current(cx);
        cx.notify();
    }

    fn prev_match(&mut self, cx: &mut Context<Self>) {
        if self.matched_msg_indices.is_empty() {
            return;
        }
        let len = self.matched_msg_indices.len();
        self.current_match = (self.current_match + len - 1) % len;
        self.jump_to_current(cx);
        cx.notify();
    }

    fn run_action(&mut self, action: PaletteAction, cx: &mut Context<Self>) {
        // Navigation rows keep the palette open so the query can be refined.
        match action {
            PaletteAction::Search => {
                self.jump_to_current(cx);
                return;
            }
            PaletteAction::Prev => {
                self.prev_match(cx);
                return;
            }
            PaletteAction::Next => {
                self.next_match(cx);
                return;
            }
            _ => {}
        }

        self.close(cx);
        match action {
            PaletteAction::Settings => self.app_state.update(cx, |s, cx| s.open_settings(cx)),
            PaletteAction::Tasks => self.app_state.update(cx, |s, cx| {
                s.active_page = WorkspacePage::Tasks;
                s.refresh_tasks_from_sidecar(cx);
            }),
            PaletteAction::Scheduled => self.app_state.update(cx, |s, cx| {
                s.active_page = WorkspacePage::Scheduled;
                s.refresh_scheduled_tasks(cx);
                cx.notify();
            }),
            PaletteAction::Artifacts => self.app_state.update(cx, |s, cx| {
                s.active_page = WorkspacePage::Artifacts;
                cx.notify();
            }),
            PaletteAction::ToggleRightPanel => self.app_state.update(cx, |s, cx| {
                if s.right_panel_open {
                    s.right_panel_open = false;
                    cx.notify();
                } else {
                    let tab = s
                        .open_right_tabs
                        .last()
                        .copied()
                        .unwrap_or(RightPanelTab::Inspector);
                    s.open_right_tab(tab, cx);
                }
            }),
            PaletteAction::Theme => self.app_state.update(cx, |s, cx| {
                s.dark_mode = !s.dark_mode;
                cx.notify();
            }),
            PaletteAction::Search | PaletteAction::Prev | PaletteAction::Next => {}
        }
    }

    /// Builds the visible rows for the current query: the conversation search
    /// action first, then match navigation, then app actions. Filtering happens
    /// here so the search row always survives a text query.
    fn rows(&self, locale: AppLocale, query: &str, matches: usize) -> Vec<(String, PaletteAction)> {
        let needle = query.trim().to_lowercase();
        let mut rows: Vec<(String, PaletteAction)> = Vec::new();

        let search_label = if needle.is_empty() {
            tr(locale, "chat.commandSearchLabel").to_string()
        } else {
            format!(
                "{} — {} ({})",
                tr(locale, "chat.commandSearchLabel"),
                query.trim(),
                format_commas(matches)
            )
        };
        rows.push((search_label, PaletteAction::Search));

        if !needle.is_empty() && matches > 0 {
            rows.push((tr(locale, "find.prev").to_string(), PaletteAction::Prev));
            rows.push((tr(locale, "find.next").to_string(), PaletteAction::Next));
        }

        let actions: [(PaletteAction, &'static str); 6] = [
            (PaletteAction::Settings, "settings.title"),
            (PaletteAction::Tasks, "sidebar.tasks"),
            (PaletteAction::Scheduled, "sidebar.scheduled"),
            (PaletteAction::Artifacts, "sidebar.artifacts"),
            (PaletteAction::ToggleRightPanel, "title.toggleRightPanel"),
            (PaletteAction::Theme, "title.toggleTheme"),
        ];
        for (action, key) in actions {
            let label = tr(locale, key);
            if needle.is_empty() || label.to_lowercase().contains(&needle) {
                let _ = &action;
                rows.push((label.to_string(), action));
            }
        }

        rows
    }

    pub fn render_palette(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.app_state.read(cx).find_open;
        if !is_open {
            self.was_open = false;
            return div().into_any_element();
        }
        // Focus (and reset the query) each time the palette is opened.
        if !self.was_open {
            self.was_open = true;
            self.state.update(cx, |s, cx| {
                s.set_query("", _window, cx);
                s.focus(_window, cx);
            });
        }
        let locale = self.app_state.read(cx).locale;
        if self.last_locale != locale {
            self.last_locale = locale;
        }
        // The palette's query is the source of truth for the highlight query.
        let typed = self.state.read(cx).query(cx).to_string();
        if typed != self.last_query {
            self.last_query = typed.clone();
            self.app_state.update(cx, |s, cx| {
                s.find_query = SharedString::from(typed);
                cx.notify();
            });
        }
        self.recompute_matches(cx);

        let total = self.matched_msg_indices.len();
        let current = if total == 0 { 0 } else { self.current_match + 1 };
        let query_empty = self.last_query.trim().is_empty();
        let query = self.last_query.clone();
        let rows = self.rows(locale, &query, total);

        let mut palette = Command::new(&self.state)
            .placeholder(tr(locale, "chat.commandPlaceholder"))
            .max_h(px(400.))
            .filterable(false);

        for (label, action) in &rows {
            let icon = match action {
                PaletteAction::Search => IconName::Search,
                PaletteAction::Prev => IconName::ChevronUp,
                PaletteAction::Next => IconName::ChevronDown,
                PaletteAction::Settings => IconName::Settings,
                PaletteAction::Tasks => IconName::CircleCheck,
                PaletteAction::Scheduled => IconName::Calendar,
                PaletteAction::Artifacts => IconName::LayoutDashboard,
                PaletteAction::ToggleRightPanel => IconName::PanelRightOpen,
                PaletteAction::Theme => IconName::Sun,
            };
            let mut item = CommandItem::new()
                .label(SharedString::from(label.clone()))
                .icon(icon);
            if matches!(action, PaletteAction::Search) && total > 0 {
                item = item.checked(true);
            }
            palette = palette.item(item);
        }

        palette = palette.on_confirm({
            let entity = cx.entity().clone();
            move |path, _window, app| {
                if path.section != 0 {
                    return;
                }
                let Some((_, action)) = rows.get(path.row) else {
                    return;
                };
                let action = action.clone();
                let entity = entity.clone();
                app.defer(move |cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.run_action(action.clone(), cx);
                    });
                });
            }
        });

        div()
            .id("command-palette-backdrop")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(96.))
            .bg(cx.theme().background.opacity(0.30))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.close(cx);
            }))
            .child(
                div()
                    .id("command-palette-card")
                    .occlude()
                    .w(px(620.))
                    .max_w_full()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_: &ClickEvent, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .w_full()
                            .shadow_lg()
                            .rounded_lg()
                            .child(palette),
                    )
                    .child(
                        div()
                            .w_full()
                            .px_3()
                            .py_1p5()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(if total > 0 {
                                format!(
                                    "{} {} / {}",
                                    tr(locale, "chat.searchMatchesCount"),
                                    format_commas(current),
                                    format_commas(total)
                                )
                            } else if query_empty {
                                tr(locale, "chat.commandPalette").to_string()
                            } else {
                                tr(locale, "find.noMatches").to_string()
                            })),
                    ),
            )
            .into_any_element()
    }
}
