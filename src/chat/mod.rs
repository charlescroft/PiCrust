pub mod branch_nav;
pub mod composer;
pub mod find_bar;
pub mod message_row;
pub mod questionnaire;

use crate::actions::{SendPrompt, StopAgent};
use crate::app_state::AppState;
use crate::chat::branch_nav::render_branch_nav;
use crate::chat::composer::ComposerView;
use crate::chat::find_bar::ConversationCommandPalette;
use crate::chat::message_row::{CachedRowEntities, render_message_row};
use crate::chat::questionnaire::QuestionnaireView;
use crate::client::types::{ChatMessage, MessageRole, WorkspaceKind};
use crate::i18n::{format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Anchor, AppContext as _, Context, Entity, FocusHandle, Focusable, FollowMode,
    InteractiveElement as _, IntoElement, ListAlignment, ListOffset, ListState, ParentElement,
    Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, list, px,
    rems,
};
use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

const LIST_OVERDRAW: gpui_kit::Pixels = px(800.);

pub struct ConversationScrollerState {
    list_state: ListState,
    pub visible_top_ix: usize,
    pub is_scrolled_up: bool,
    last_scroll_notify_key: (bool, usize),
    last_scroll_key_cell: Rc<Cell<(bool, usize)>>,
    total_items_cell: Rc<Cell<usize>>,
}

impl ConversationScrollerState {
    fn build_list_state(
        item_count: usize,
        cx: &mut Context<Self>,
    ) -> (
        ListState,
        Rc<Cell<(bool, usize)>>,
        Rc<Cell<usize>>,
    ) {
        let list_state = ListState::new(item_count, ListAlignment::Bottom, LIST_OVERDRAW);
        list_state.set_follow_mode(FollowMode::Tail);

        let init_key = (false, item_count.saturating_sub(1));
        let last_key_cell = Rc::new(Cell::new(init_key));
        let total_items_cell = Rc::new(Cell::new(item_count));

        let weak_state = cx.weak_entity();
        let last_key_for_handler = last_key_cell.clone();
        let total_for_handler = total_items_cell.clone();

        list_state.set_scroll_handler(move |event, _, cx| {
            let is_following = event.is_following_tail;
            let top_ix = event.visible_range.start;
            let total = total_for_handler.get();
            let is_scrolled = !is_following && total > 1;
            let key = (is_scrolled, top_ix);
            if key != last_key_for_handler.get() {
                last_key_for_handler.set(key);
                let weak_state = weak_state.clone();
                cx.defer(move |cx| {
                    let _ = weak_state.update(cx, |this, cx| {
                        this.visible_top_ix = top_ix;
                        this.is_scrolled_up = is_scrolled;
                        this.last_scroll_notify_key = key;
                        cx.notify();
                    });
                });
            }
        });

        (list_state, last_key_cell, total_items_cell)
    }

    pub fn new(item_count: usize, cx: &mut Context<Self>) -> Self {
        let (list_state, last_scroll_key_cell, total_items_cell) =
            Self::build_list_state(item_count, cx);
        let last_ix = item_count.saturating_sub(1);
        Self {
            list_state,
            visible_top_ix: last_ix,
            is_scrolled_up: false,
            last_scroll_notify_key: (false, last_ix),
            last_scroll_key_cell,
            total_items_cell,
        }
    }

    #[inline]
    pub fn is_scrolled_up(&self) -> bool {
        self.is_scrolled_up
    }

    pub fn is_following_tail(&self) -> bool {
        self.list_state.is_following_tail()
    }

    pub fn logical_scroll_top(&self) -> ListOffset {
        self.list_state.logical_scroll_top()
    }

    pub fn reset(&mut self, item_count: usize, cx: &mut Context<Self>) {
        self.list_state.reset(item_count);
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.total_items_cell.set(item_count);
        let last_ix = item_count.saturating_sub(1);
        self.visible_top_ix = last_ix;
        self.is_scrolled_up = false;
        self.last_scroll_notify_key = (false, last_ix);
        self.last_scroll_key_cell.set((false, last_ix));
        cx.notify();
    }

    /// Replaces `list_state` with a fresh `ListState` instance instead of mutating
    /// the previous `Rc<RefCell<ListStateInner>>` in place. Required when switching
    /// sessions so previously cached `ListState` handles stay isolated.
    pub fn replace_fresh(&mut self, item_count: usize, cx: &mut Context<Self>) {
        let (list_state, last_key_cell, total_items_cell) =
            Self::build_list_state(item_count, cx);
        self.list_state = list_state;
        self.last_scroll_key_cell = last_key_cell;
        self.total_items_cell = total_items_cell;
        let last_ix = item_count.saturating_sub(1);
        self.visible_top_ix = last_ix;
        self.is_scrolled_up = false;
        self.last_scroll_notify_key = (false, last_ix);
        cx.notify();
    }

    pub fn splice(
        &mut self,
        old_range: Range<usize>,
        count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let item_count = self.list_state.item_count();
        if old_range.start > old_range.end || old_range.end > item_count {
            return false;
        }
        self.list_state.splice(old_range, count);
        self.total_items_cell.set(self.list_state.item_count());
        cx.notify();
        true
    }

    pub fn append(&mut self, count: usize, cx: &mut Context<Self>) -> bool {
        let item_count = self.list_state.item_count();
        self.splice(item_count..item_count, count, cx)
    }

    pub fn remeasure(&mut self, cx: &mut Context<Self>) {
        self.list_state.remeasure();
        cx.notify();
    }

    pub fn remeasure_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> bool {
        let item_count = self.list_state.item_count();
        if range.start > range.end || range.end > item_count {
            return false;
        }
        self.list_state.remeasure_items(range);
        cx.notify();
        true
    }

    pub fn scroll_to_item(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let count = self.list_state.item_count();
        if index >= count {
            return false;
        }
        self.list_state.set_follow_mode(FollowMode::Normal);
        self.list_state.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.),
        });
        self.visible_top_ix = index;
        self.is_scrolled_up = count > 1 && index + 1 < count;
        let key = (self.is_scrolled_up, index);
        self.last_scroll_notify_key = key;
        self.last_scroll_key_cell.set(key);
        cx.notify();
        true
    }

    /// Restores an exact reading position without re-entering tail follow.
    pub fn scroll_to_offset(&mut self, offset: ListOffset, cx: &mut Context<Self>) {
        let count = self.list_state.item_count();
        self.list_state.set_follow_mode(FollowMode::Normal);
        self.list_state.scroll_to(offset);
        self.visible_top_ix = offset.item_ix;
        self.is_scrolled_up = count > 1 && offset.item_ix + 1 < count;
        let key = (self.is_scrolled_up, offset.item_ix);
        self.last_scroll_notify_key = key;
        self.last_scroll_key_cell.set(key);
        cx.notify();
    }

    pub fn scroll_to_end(&mut self, cx: &mut Context<Self>) {
        self.list_state.set_follow_mode(FollowMode::Tail);
        self.list_state.scroll_to_end();
        let last_ix = self.list_state.item_count().saturating_sub(1);
        self.visible_top_ix = last_ix;
        self.is_scrolled_up = false;
        self.last_scroll_notify_key = (false, last_ix);
        self.last_scroll_key_cell.set((false, last_ix));
        cx.notify();
    }
}

struct SessionPanelCache {
    row_cache: HashMap<u64, CachedRowEntities>,
    msg_ids: Vec<u64>,
    list_state: ListState,
    turns: Vec<TurnLocation>,
    tail_sig: Option<(u64, usize, usize, usize, bool)>,
    font_size: f32,
}

pub struct ChatPanel {
    focus_handle: FocusHandle,
    app_state: Entity<AppState>,
    pub(crate) scroller: Entity<ConversationScrollerState>,
    composer: Entity<ComposerView>,
    palette: Entity<ConversationCommandPalette>,
    questionnaire: Option<Entity<QuestionnaireView>>,
    row_cache: HashMap<u64, CachedRowEntities>,
    last_message_count: usize,
    last_session_id: Option<String>,
    last_font_size: f32,
    last_tail_sig: Option<(u64, usize, usize, usize, bool)>,
    hovered_turn_idx: Option<usize>,
    nav_window_start: usize,
    show_all_turns: bool,
    cached_turns: Vec<TurnLocation>,
    cached_msg_ids: Vec<u64>,
    session_caches: HashMap<String, SessionPanelCache>,
    session_cache_order: Vec<String>,
}

impl Focusable for ChatPanel {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ChatPanel {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (count, last_font_size, init_msg_ids) = {
            let st = app_state.read(cx);
            (
                st.messages.len(),
                st.chat_font_size,
                st.messages.iter().map(|m| m.id).collect::<Vec<_>>(),
            )
        };
        let scroller = cx.new(|cx| ConversationScrollerState::new(count, cx));
        cx.observe(&scroller, |_, _, cx| cx.notify()).detach();
        cx.observe(&app_state, |this, app, cx| {
            this.sync_scroller_with_messages(&app, cx);
            cx.notify();
        })
        .detach();

        let composer = cx.new(|cx| ComposerView::new(app_state.clone(), window, cx));
        let scroller_for_find = scroller.clone();
        let palette =
            cx.new(|cx| ConversationCommandPalette::new(app_state.clone(), scroller_for_find, window, cx));

        Self {
            focus_handle: cx.focus_handle(),
            app_state,
            scroller,
            composer,
            palette,
            questionnaire: None,
            row_cache: HashMap::new(),
            last_message_count: count,
            last_session_id: None,
            last_font_size,
            last_tail_sig: None,
            hovered_turn_idx: None,
            nav_window_start: 0,
            show_all_turns: false,
            cached_turns: Vec::new(),
            cached_msg_ids: init_msg_ids,
            session_caches: HashMap::new(),
            session_cache_order: Vec::new(),
        }
    }

    pub fn toggle_process_group(&mut self, msg_id: u64, default_open: bool, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry.process_group_open.unwrap_or(default_open);
            entry.process_group_open = Some(!cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_thinking_step(&mut self, msg_id: u64, step_idx: usize, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry
                .expanded_thinking_steps
                .get(&step_idx)
                .copied()
                .unwrap_or(false);
            entry.expanded_thinking_steps.insert(step_idx, !cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_compaction_step(
        &mut self,
        msg_id: u64,
        step_idx: usize,
        default_open: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry
                .expanded_compactions
                .get(&step_idx)
                .copied()
                .unwrap_or(default_open);
            entry.expanded_compactions.insert(step_idx, !cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_compaction_files(
        &mut self,
        msg_id: u64,
        step_idx: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry
                .compaction_files_open
                .get(&step_idx)
                .copied()
                .unwrap_or(false);
            entry.compaction_files_open.insert(step_idx, !cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_tool(&mut self, msg_id: u64, tool_id: SharedString, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry.expanded_tools.get(&tool_id).copied().unwrap_or(false);
            entry.expanded_tools.insert(tool_id, !cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_modified_files(&mut self, msg_id: u64, default_open: bool, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry.modified_files_open.unwrap_or(default_open);
            entry.modified_files_open = Some(!cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_error_expanded(&mut self, msg_id: u64, default_open: bool, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry.error_expanded.unwrap_or(default_open);
            entry.error_expanded = Some(!cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_user_expanded(&mut self, msg_id: u64, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            entry.user_expanded = !entry.user_expanded;
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    pub fn toggle_subagent_notice_expanded(&mut self, msg_id: u64, cx: &mut Context<Self>) {
        if let Some(entry) = self.row_cache.get_mut(&msg_id) {
            let cur = entry.subagent_notice_expanded.unwrap_or(true);
            entry.subagent_notice_expanded = Some(!cur);
        }
        self.scroller.update(cx, |s, cx| s.remeasure(cx));
    }

    fn sync_scroller_with_messages(&mut self, app: &Entity<AppState>, cx: &mut Context<Self>) {
        let (new_count, sid, font_sz, tail_sig, keep_ids, next_turns, next_msg_ids) = {
            let state = app.read(cx);
            let new_count = state.messages.len();
            let sid = state.active_session_id.clone();
            let font_sz = state.chat_font_size;
            let tail_sig = state.messages.last().map(|m| {
                (
                    m.id,
                    m.content.len(),
                    m.process_steps.len(),
                    m.thinking.len(),
                    m.is_streaming || state.is_agent_running || state.is_compacting,
                )
            });

            let top_ix = self.scroller.read(cx).visible_top_ix;
            let keep_ids: Option<std::collections::HashSet<u64>> = (self.row_cache.len() > 300).then(|| {
                let mut ids = std::collections::HashSet::new();
                let tail_from = state.messages.len().saturating_sub(60);
                for m in &state.messages[tail_from..] {
                    ids.insert(m.id);
                }
                let win_from = top_ix.saturating_sub(50);
                let win_to = (top_ix + 70).min(state.messages.len());
                for m in &state.messages[win_from..win_to] {
                    ids.insert(m.id);
                }
                ids
            });
            let structure_changed =
                sid != self.last_session_id || new_count != self.last_message_count;
            let next_turns = structure_changed.then(|| collect_turn_locations(&state.messages));
            let next_msg_ids =
                structure_changed.then(|| state.messages.iter().map(|m| m.id).collect::<Vec<_>>());
            (
                new_count,
                sid,
                font_sz,
                tail_sig,
                keep_ids,
                next_turns,
                next_msg_ids,
            )
        };

        if font_sz != self.last_font_size {
            self.last_font_size = font_sz;
            self.scroller.update(cx, |s, cx| s.remeasure(cx));
        }

        if sid != self.last_session_id {
            // Save outgoing session's row_cache + measured ListState (bounded to 6 recent sessions)
            if let Some(prev_sid) = self.last_session_id.take() {
                let prev_list_state = self.scroller.read(cx).list_state.clone();
                let prev_rows = std::mem::take(&mut self.row_cache);
                let prev_msg_ids = std::mem::take(&mut self.cached_msg_ids);
                let prev_turns = std::mem::take(&mut self.cached_turns);
                self.session_caches.insert(
                    prev_sid.clone(),
                    SessionPanelCache {
                        row_cache: prev_rows,
                        msg_ids: prev_msg_ids,
                        list_state: prev_list_state,
                        turns: prev_turns,
                        tail_sig: self.last_tail_sig,
                        font_size: self.last_font_size,
                    },
                );
                self.session_cache_order.retain(|s| s != &prev_sid);
                self.session_cache_order.push(prev_sid);
                while self.session_cache_order.len() > 6 {
                    let evicted = self.session_cache_order.remove(0);
                    self.session_caches.remove(&evicted);
                }
            }

            self.last_session_id = sid.clone();
            self.nav_window_start = usize::MAX;
            self.show_all_turns = false;
            let msg_ids = next_msg_ids.unwrap_or_default();

            // Restore if previously visited and message count + ListState match, for 0ms jitter-free switch!
            if let Some(ref next_sid) = sid {
                if let Some(mut cached) = self.session_caches.remove(next_sid) {
                    if cached.msg_ids.len() == new_count
                        && cached.list_state.item_count() == new_count
                    {
                        let mut remapped_rows = HashMap::with_capacity(cached.row_cache.len());
                        for (old_id, new_id) in cached.msg_ids.iter().zip(msg_ids.iter()) {
                            if let Some(entry) = cached.row_cache.remove(old_id) {
                                remapped_rows.insert(*new_id, entry);
                            }
                        }
                        self.row_cache = remapped_rows;
                        self.cached_msg_ids = msg_ids;
                        self.cached_turns = cached.turns;
                        self.last_message_count = new_count;
                        self.last_tail_sig = cached.tail_sig;
                        self.scroller.update(cx, |s, cx| {
                            s.list_state = cached.list_state;
                            let count = s.list_state.item_count();
                            s.total_items_cell.set(count);
                            s.visible_top_ix = count.saturating_sub(1);
                            s.is_scrolled_up = false;
                            s.last_scroll_notify_key = (false, s.visible_top_ix);
                            s.last_scroll_key_cell.set((false, s.visible_top_ix));
                            if cached.font_size != font_sz {
                                s.remeasure(cx);
                            } else {
                                cx.notify();
                            }
                        });
                        return;
                    }
                }
            }

            self.row_cache.clear();
            self.cached_msg_ids = msg_ids;
            self.last_message_count = new_count;
            self.last_tail_sig = tail_sig;
            if let Some(t) = next_turns {
                self.cached_turns = t;
            }
            self.scroller.update(cx, |s, cx| {
                s.replace_fresh(new_count, cx);
            });
            return;
        }

        if new_count > self.last_message_count {
            let added = new_count - self.last_message_count;
            self.last_message_count = new_count;
            self.last_tail_sig = tail_sig;
            if let Some(t) = next_turns {
                self.cached_turns = t;
            }
            if let Some(ids) = next_msg_ids {
                self.cached_msg_ids = ids;
            }
            let is_new_turn_start = app.read(cx).messages.last().is_some_and(|m| {
                m.role == MessageRole::User
                    || (m.role == MessageRole::Assistant
                        && m.is_streaming
                        && m.content.is_empty()
                        && m.thinking.is_empty())
            });
            self.scroller.update(cx, |s, cx| {
                let _ = s.append(added, cx);
                if is_new_turn_start || !s.is_scrolled_up() {
                    s.scroll_to_end(cx);
                }
            });
        } else if new_count < self.last_message_count {
            let old_count = self.last_message_count;
            self.last_message_count = new_count;
            self.last_tail_sig = tail_sig;
            if let Some(t) = next_turns {
                self.cached_turns = t;
            }
            if let Some(ids) = next_msg_ids {
                self.cached_msg_ids = ids;
            }
            self.scroller.update(cx, |s, cx| {
                if s.is_scrolled_up() {
                    let _ = s.splice(new_count..old_count, 0, cx);
                    if new_count > 0 {
                        let _ = s.remeasure_items(new_count - 1..new_count, cx);
                    }
                } else {
                    s.reset(new_count, cx);
                }
            });
        } else if new_count > 0 && tail_sig != self.last_tail_sig {
            self.last_tail_sig = tail_sig;
            let idx = new_count - 1;
            self.scroller.update(cx, |s, cx| {
                let _ = s.remeasure_items(idx..idx + 1, cx);
            });
        }

        // Bound row_cache memory on long sessions by keeping recent/visible message caches
        if let Some(ids) = keep_ids {
            self.row_cache.retain(|id, _| ids.contains(id));
        }
    }
}

impl Render for ChatPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_for_sync = self.app_state.clone();
        self.sync_scroller_with_messages(&app_for_sync, cx);

        // Sync pending ExtensionUiRequest -> QuestionnaireView
        let pending_req = self.app_state.read(cx).pending_ui_request.clone();
        match (pending_req, &self.questionnaire) {
            (Some(req), Some(existing))
                if existing.read(cx).request_id() == req.request_id
                    || existing.read(cx).has_same_content(&req) => {}
            (Some(req), _) => {
                let app = self.app_state.clone();
                self.questionnaire =
                    Some(cx.new(|cx| QuestionnaireView::new(app, req, window, cx)));
            }
            (None, Some(_)) => {
                self.questionnaire = None;
            }
            (None, None) => {}
        }

        let find_query = self.app_state.read(cx).find_query.clone();

        let panel_entity = cx.entity().clone();
        let app_state = self.app_state.clone();
        let q_for_list = find_query.clone();

        // Items 1, 2, 3, 6: ConversationNavigator ticks on far left, vertically centered.
        // Hidden ONLY when center chat column is too narrow and ticks would overlap the 840px text body.
        let state_snap = self.app_state.read(cx);
        let locale = state_snap.locale;
        let messages_empty = state_snap.messages.is_empty();
        let has_selected_model = state_snap.selected_model.is_some();
        let right_open = state_snap.right_panel_open;
        let right_w = if right_open {
            state_snap.right_panel_width
        } else {
            px(0.)
        };
        let sidebar_w = if state_snap.sidebar_collapsed {
            px(0.)
        } else {
            px(268.)
        };
        let chat_col_w = window.viewport_size().width - sidebar_w - right_w;
        // Centered 840px content leaves ((chat_col_w - 840) / 2) on the left; rail takes ~44px.
        let chat_width = state_snap.chat_content_width;
        let has_gutter_space = chat_col_w >= px(chat_width + 88.);

        let turns = if self.cached_turns.is_empty() && !messages_empty {
            collect_turn_locations(&state_snap.messages)
        } else {
            self.cached_turns.clone()
        };
        let top_item_ix = self.scroller.read(cx).visible_top_ix;
        let is_at_tail = !self.scroller.read(cx).is_scrolled_up;
        let active_turn = if is_at_tail && !turns.is_empty() {
            turns.len() - 1
        } else {
            turns
                .iter()
                .rposition(|t| t.msg_index <= top_item_ix)
                .unwrap_or(0)
        };

        // When reaching the bottom / last turn, restore concise 8 turns mode!
        if is_at_tail || active_turn + 1 >= turns.len() {
            self.show_all_turns = false;
        }

        let total_turns = turns.len();
        let (nav_start, nav_end) = if self.show_all_turns || total_turns <= MAX_NAV_TICKS {
            (0, total_turns)
        } else {
            let initial_start = total_turns.saturating_sub(MAX_NAV_TICKS);
            if active_turn < initial_start {
                self.show_all_turns = true;
                (0, total_turns)
            } else {
                (initial_start, total_turns)
            }
        };
        let visible_count = nav_end - nav_start;

        // Strictly bounded rail height <= 260px, perfectly vertically centered
        let max_rail_h = 260.0f32;
        let (tick_h, gap_px) = if visible_count <= 8 {
            (8.0f32, 8.0f32)
        } else {
            let t = ((visible_count - 8) as f32 / 32.0).clamp(0.0, 1.0);
            let th = 8.0 - 4.5 * t; // scales down from 8px to 3.5px
            let needed = visible_count as f32 * th;
            let rem_space = (max_rail_h - needed).max(0.0);
            let gap = if visible_count > 1 {
                (rem_space / (visible_count - 1) as f32).clamp(1.5, 8.0)
            } else {
                8.0
            };
            (th, gap)
        };
        let actual_rail_h = visible_count as f32 * tick_h + visible_count.saturating_sub(1) as f32 * gap_px;

        let hovered_turn = self
            .hovered_turn_idx
            .filter(|&ix| ix >= nav_start && ix < nav_end);

        let navigator_gutter = if has_gutter_space && turns.len() >= 2 {
            let mut ticks = v_flex()
                .w(px(32.))
                .h(px(actual_rail_h))
                .gap(px(gap_px))
                .justify_center()
                .items_start();

            for (t_idx, turn) in turns.iter().enumerate().skip(nav_start).take(nav_end - nav_start) {
                let is_active = t_idx == active_turn;
                let is_hovered = hovered_turn == Some(t_idx);
                let msg_idx = turn.msg_index;
                let is_first = t_idx == 0;
                let is_last = t_idx + 1 == total_turns;
                let scroller_ref = self.scroller.clone();

                ticks = ticks.child(
                    div()
                        .id(("turn-tick", t_idx))
                        .w(px(30.))
                        .h(px(tick_h))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_hover(cx.listener(move |panel, &hovered, _, cx| {
                            if hovered {
                                panel.hovered_turn_idx = Some(t_idx);
                            } else if panel.hovered_turn_idx == Some(t_idx) {
                                panel.hovered_turn_idx = None;
                            }
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            scroller_ref.update(cx, |s, cx| {
                                if is_last {
                                    s.scroll_to_end(cx);
                                } else if is_first {
                                    let _ = s.scroll_to_item(0, cx);
                                } else {
                                    let _ = s.scroll_to_item(msg_idx, cx);
                                }
                            });
                            cx.notify();
                        }))
                        .child(
                            div()
                                .h(if is_active || is_hovered { px((tick_h * 0.55).max(3.0)) } else { px((tick_h * 0.35).max(2.0)) })
                                .w(if is_active {
                                    px(24.)
                                } else if is_hovered {
                                    px(18.)
                                } else {
                                    px(12.)
                                })
                                .rounded_full()
                                .bg(if is_active || is_hovered {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().foreground.opacity(0.28)
                                }),
                        ),
                );
            }

            let preview_card = hovered_turn.and_then(|t_idx| turns.get(t_idx).map(|t| (t_idx, t))).map(|(t_idx, turn)| {
                v_flex()
                    .w(px(340.))
                    .p_3()
                    .gap_1p5()
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(SharedString::from(turn.question.clone())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(SharedString::from(
                                turn.answer
                                    .clone()
                                    .unwrap_or_else(|| "…".to_string()),
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground.opacity(0.75))
                            .child(format!(
                                "{} {} / {}",
                                tr(locale, "chat.clickJumpTurn"),
                                format_commas(t_idx + 1),
                                format_commas(total_turns)
                            )),
                    )
            });

            Some(
                h_flex()
                    .absolute()
                    .left(px(10.))
                    .top_0()
                    .bottom_0()
                    .items_center()
                    .gap_3()
                    .child(ticks)
                    .children(preview_card),
            )
        } else {
            None
        };

        let body_area = if messages_empty {
            let app_empty = self.app_state.clone();
            let mut empty = Empty::new().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(IconName::Bot)))
                    .title(EmptyTitle::new().child(if has_selected_model {
                        tr(locale, "chat.emptyTitle")
                    } else {
                        tr(locale, "chat.noModelTitle")
                    }))
                    .description(EmptyDescription::new().child(if has_selected_model {
                        tr(locale, "chat.emptyDesc")
                    } else {
                        tr(locale, "chat.noModelDesc")
                    })),
            );
            if !has_selected_model {
                empty = empty.child(
                    Button::new("empty-open-model-settings")
                        .primary()
                        .small()
                        .h(px(30.))
                        .icon(IconName::Settings)
                        .label(tr(locale, "chat.configureModel"))
                        .on_click(move |_, _, cx| {
                            app_empty.update(cx, |s, cx| s.open_model_settings(cx));
                        }),
                );
            }
            empty.into_any_element()
        } else {
            let (list_state, scrolled_up) = {
                let s = self.scroller.read(cx);
                (s.list_state.clone(), s.is_scrolled_up())
            };
            let item_count = list_state.item_count();
            let list_el = list(list_state.clone(), move |index, window, cx| {
                let Some(msg_clone) = app_state.read(cx).messages.get(index).cloned() else {
                    return div().into_any_element();
                };
                let q_str = q_for_list.to_string();
                let app = app_state.clone();
                let row = panel_entity.update(cx, |this, cx| {
                    render_message_row(
                        &msg_clone,
                        &mut this.row_cache,
                        &q_str,
                        &app,
                        window,
                        cx,
                    )
                });
                let has_earlier = app.read(cx).has_earlier_messages;
                let is_loading_earlier = app.read(cx).is_loading_earlier;
                let earlier_banner = if index == 0 && has_earlier {
                    let app_load = app.clone();
                    Some(
                        div()
                            .id("chat-load-earlier-banner")
                            .w_full()
                            .pb_3()
                            .flex()
                            .justify_center()
                            .child(
                                Button::new("btn-load-earlier-top")
                                    .ghost()
                                    .small()
                                    .h(px(26.))
                                    .icon(if is_loading_earlier {
                                        IconName::LoaderCircle
                                    } else {
                                        IconName::RotateCw
                                    })
                                    .label(tr(locale, "chat.loadEarlier"))
                                    .disabled(is_loading_earlier)
                                    .on_click(move |_, _, cx| {
                                        app_load.update(cx, |s, cx| {
                                            s.load_earlier_messages(cx);
                                        });
                                    }),
                            ),
                    )
                } else {
                    None
                };

                div()
                    .id(("chat-row", msg_clone.id))
                    .w_full()
                    .min_w_0()
                    .pt(if index == 0 { px(12.) } else { px(0.) })
                    .pb(if index + 1 == item_count { px(12.) } else { px(16.) })
                    .children(earlier_banner)
                    .child(row)
                    .into_any_element()
            })
            .size_full()
            .min_h_0();

            let scroller_for_jump = self.scroller.clone();
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .id("conversation-viewport")
                        .size_full()
                        .min_h_0()
                        .child(list_el)
                        .vertical_scrollbar(&list_state),
                )
                .children(scrolled_up.then(|| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(rems(0.75))
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("conv-jump-latest")
                                .secondary()
                                .icon(IconName::ArrowDown)
                                .tooltip(tr(locale, "chat.jumpLatest"))
                                .rounded_full()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().background)
                                .on_click(move |_, _, cx| {
                                    scroller_for_jump.update(cx, |s, cx| s.scroll_to_end(cx));
                                }),
                        )
                }))
                .children(navigator_gutter)
                .into_any_element()
        };

        let q_overlay = self.questionnaire.as_ref().map(|view| {
            div()
                .w_full()
                .px_4()
                .pb_2()
                .child(
                    div()
                        .w_full()
                        .max_w(px(chat_width))
                        .mx_auto()
                        .child(view.clone()),
                )
        });

        let is_temp_chat = self.app_state.read(cx).is_active_temp_chat();

        let session_title = {
            let state = self.app_state.read(cx);
            state
                .active_session_id
                .as_ref()
                .and_then(|sid| state.sessions.iter().find(|s| &s.id == sid))
                .map(|s| s.display_title())
                .unwrap_or_else(|| {
                    if is_temp_chat {
                        SharedString::from(tr(locale, "status.tempChatWorkspace"))
                    } else if state.active_workspace_kind == WorkspaceKind::Chat {
                        SharedString::from(tr(locale, "sidebar.newStandaloneChat"))
                    } else {
                        SharedString::from(tr(locale, "chat.newSession"))
                    }
                })
        };

        let full_session_title = session_title.clone();

        // Subagents dropdown placed to the left of the conversation title
        let subagent_runs = state_snap.active_subagent_runs();
        let active_subagents = subagent_runs.iter().filter(|r| r.is_active()).count();
        let subagent_status_text = state_snap.subagent_status_text.clone();
        let app_for_subagents = self.app_state.clone();

        let subagent_btn_label = if active_subagents > 0 {
            format!(
                "{} · {} {}",
                tr(locale, "subagents.label"),
                format_commas(active_subagents),
                tr(locale, "subagents.running")
            )
        } else if !subagent_runs.is_empty() {
            format!(
                "{} ({})",
                tr(locale, "subagents.label"),
                format_commas(subagent_runs.len())
            )
        } else {
            format!("{} (0)", tr(locale, "subagents.label"))
        };

        let mut subagent_btn = Button::new("conv-subagents-btn")
            .ghost()
            .small()
            .h(px(26.))
            .label(subagent_btn_label)
            .tooltip(tr(locale, "subagents.label"));
        if active_subagents > 0 {
            subagent_btn = subagent_btn.child(Spinner::new().xsmall().color(cx.theme().primary));
        } else {
            subagent_btn = subagent_btn.icon(IconName::Bot);
        }

        let subagent_dropdown = subagent_btn.dropdown_menu_with_anchor(
            Anchor::TopLeft,
            move |mut menu, _, _| {
                menu = menu.min_w(px(280.)).max_w(px(380.));
                let header = if active_subagents > 0 {
                    format!(
                        "{} · {} {}",
                        tr(locale, "subagents.label"),
                        format_commas(active_subagents),
                        tr(locale, "subagents.running")
                    )
                } else {
                    format!(
                        "{} · {}",
                        tr(locale, "subagents.label"),
                        tr(locale, "subagents.idle")
                    )
                };
                menu = menu.label(header);
                if let Some(ref status_line) = subagent_status_text {
                    if !status_line.trim().is_empty() {
                        let status_text = status_line.clone();
                        menu = menu.item(
                            PopupMenuItem::element(move |_, cx| {
                                div()
                                    .max_w(px(340.))
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(status_text.clone())
                            })
                            .disabled(true),
                        );
                    }
                }
                if subagent_runs.is_empty() {
                    menu = menu.item(
                        PopupMenuItem::new(tr(locale, "subagents.noneRunning")).disabled(true),
                    );
                } else {
                    for run in &subagent_runs {
                        let glyph = match run.state.as_str() {
                            "running" => "●",
                            "queued" | "pending" => "◦",
                            "complete" | "completed" => "✓",
                            "failed" | "rejected" => "✗",
                            _ => "■",
                        };
                        let mut details = Vec::new();
                        if let Some(ref m) = run.model {
                            if let Some(ref th) = run.thinking {
                                details.push(format!("{m} ({th})"));
                            } else {
                                details.push(m.clone());
                            }
                        }
                        if let Some(ref t) = run.current_tool {
                            details.push(format!("tool: {t}"));
                        }
                        if let Some(tc) = run.tool_count {
                            details.push(format!("{} tools", format_commas(tc)));
                        }
                        if let Some(turns) = run.turn_count {
                            details.push(format!("{} turns", format_commas(turns)));
                        }
                        if details.is_empty() {
                            if let Some(ref p) = run.progress_text {
                                details.push(p.clone());
                            } else {
                                details.push(run.state.clone());
                            }
                        }
                        let line = format!(
                            "{glyph} {} [{}] — {}",
                            run.label,
                            run.kind,
                            details.join(" · ")
                        );
                        if let Some(child_sid) = run.session_id.clone() {
                            let app_open = app_for_subagents.clone();
                            menu = menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    div()
                                        .max_w(px(316.))
                                        .min_w_0()
                                        .truncate()
                                        .child(line.clone())
                                })
                                .icon(IconName::ExternalLink)
                                .on_click(move |_, _, cx| {
                                    app_open.update(cx, |s, cx| {
                                        s.select_session(child_sid.clone(), cx);
                                    });
                                }),
                            );
                        } else {
                            menu = menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    div()
                                        .max_w(px(340.))
                                        .min_w_0()
                                        .truncate()
                                        .child(line.clone())
                                })
                                .disabled(true),
                            );
                        }
                        for child in &run.children {
                            let c_glyph = match child.state.as_str() {
                                "running" => "●",
                                "queued" | "pending" => "◦",
                                "complete" | "completed" => "✓",
                                "failed" | "rejected" => "✗",
                                _ => "■",
                            };
                            let c_name = child.agent.as_deref().unwrap_or(&child.label);
                            let c_extra = child
                                .current_tool
                                .as_ref()
                                .map(|t| format!(" · tool: {t}"))
                                .or_else(|| child.progress_text.as_ref().map(|p| format!(" · {p}")))
                                .unwrap_or_else(|| format!(" · {}", child.state));
                            let c_line = format!("   {c_glyph} {c_name}{c_extra}");
                            if let Some(child_sid) = child.session_id.clone() {
                                let app_open = app_for_subagents.clone();
                                menu = menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        div()
                                            .max_w(px(316.))
                                            .min_w_0()
                                            .truncate()
                                            .child(c_line.clone())
                                    })
                                    .icon(IconName::ExternalLink)
                                    .on_click(move |_, _, cx| {
                                        app_open.update(cx, |s, cx| {
                                            s.select_session(child_sid.clone(), cx);
                                        });
                                    }),
                                );
                            } else {
                                menu = menu.item(
                                    PopupMenuItem::element(move |_, _| {
                                        div()
                                            .max_w(px(340.))
                                            .min_w_0()
                                            .truncate()
                                            .child(c_line.clone())
                                    })
                                    .disabled(true),
                                );
                            }
                        }
                    }
                }
                let app_settings = app_for_subagents.clone();
                menu.separator().item(
                    PopupMenuItem::new(tr(locale, "subagents.manageProfiles"))
                        .icon(IconName::Settings)
                        .on_click(move |_, _, cx| {
                            app_settings.update(cx, |s, cx| {
                                s.open_settings(cx);
                            });
                        }),
                )
            },
        );

        let conversation_header = h_flex()
            .w_full()
            .h(px(36.))
            .px_4()
            .items_center()
            .justify_between()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(div().flex_shrink_0().child(subagent_dropdown))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .children(is_temp_chat.then(|| {
                                div()
                                    .id("conv-header-temp-clock")
                                    .flex_shrink_0()
                                    .cursor_default()
                                    .tooltip(move |w, cx| {
                                        Tooltip::new(tr(locale, "status.tempChatWorkspace"))
                                            .build(w, cx)
                                    })
                                    .child(
                                        Icon::new(crate::icons::ClockIcon)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                            }))
                            .child(
                                div()
                                    .id("conv-header-title")
                                    .max_w(px(580.))
                                    .truncate()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(cx.theme().muted_foreground)
                                    .tooltip(move |w, cx| {
                                        Tooltip::new(full_session_title.clone())
                                            .max_w(px(480.))
                                            .build(w, cx)
                                    })
                                    .child(session_title),
                            ),
                    ),
            )
            .child(div().w(px(90.)).flex_shrink_0());

        v_flex()
            .id("chat-panel-root")
            .relative()
            .key_context("ChatPanel")
            .track_focus(&self.focus_handle)
            .size_full()
            .font_family(cx.theme().font_family.clone())
            .on_action(cx.listener(|this, _: &StopAgent, _, cx| {
                this.app_state.update(cx, |s, cx| s.stop_running_agent(cx));
            }))
            .on_action(cx.listener(|this, _: &SendPrompt, window, cx| {
                this.composer.update(cx, |c, cx| c.submit(window, cx));
            }))
            .child(conversation_header)
            .child(render_branch_nav(&self.app_state, window, cx))
            .child(body_area)
            .children(q_overlay)
            .child(
                div()
                    .key_context("Composer")
                    .child(self.composer.clone()),
            )
            .child(self.palette.clone())
    }
}

#[derive(Clone)]
struct TurnLocation {
    msg_index: usize,
    question: String,
    answer: Option<String>,
}

fn collect_turn_locations(messages: &[ChatMessage]) -> Vec<TurnLocation> {
    let mut turns = Vec::new();
    for (idx, m) in messages.iter().enumerate() {
        if m.role != MessageRole::User {
            continue;
        }
        let preview: String = m
            .content
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .chars()
            .take(80)
            .collect();
        let mut answer: Option<String> = None;
        for next_msg in &messages[idx + 1..] {
            if next_msg.role == MessageRole::User {
                break;
            }
            if next_msg.role == MessageRole::Assistant {
                let ans_line: String = next_msg
                    .content
                    .lines()
                    .map(str::trim)
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect();
                if !ans_line.is_empty() {
                    answer = Some(ans_line);
                }
            }
        }
        turns.push(TurnLocation {
            msg_index: idx,
            question: if preview.is_empty() {
                format!("Turn {}", turns.len() + 1)
            } else {
                preview
            },
            answer,
        });
    }
    turns
}

const MAX_NAV_TICKS: usize = 8;

/// Sliding 8-tick window for ConversationNavigator:
/// - Shows at most 8 ticks
/// - When scrolling up to/past the top tick (or down to the 8th tick), shifts the window
///   so the active turn becomes the 8th (last) tick and earlier turns load above it.
#[allow(dead_code)]
fn compute_nav_window(total: usize, active_turn: usize, prev_start: usize) -> (usize, usize) {
    if total <= MAX_NAV_TICKS {
        return (0, total);
    }
    let max_start = total - MAX_NAV_TICKS;
    let start = if prev_start == usize::MAX {
        max_start
    } else {
        prev_start.min(max_start)
    };
    if active_turn <= start && start > 0 {
        let end = (active_turn + 1).max(MAX_NAV_TICKS).min(total);
        (end - MAX_NAV_TICKS, end)
    } else if active_turn >= start + MAX_NAV_TICKS - 1 {
        let end = (active_turn + 1).max(MAX_NAV_TICKS).min(total);
        (end - MAX_NAV_TICKS, end)
    } else {
        (start, start + MAX_NAV_TICKS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_window_caps_at_eight_and_loads_earlier_on_scroll() {
        assert_eq!(compute_nav_window(5, 4, 0), (0, 5));
        assert_eq!(compute_nav_window(20, 19, usize::MAX), (12, 20));
        // Inside window stays stable
        assert_eq!(compute_nav_window(20, 15, 12), (12, 20));
        // Reaching top tick (12) places 12 as 8th (last) tick and loads 5..13
        assert_eq!(compute_nav_window(20, 12, 12), (5, 13));
    }
}
