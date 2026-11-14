use crate::app_state::{AppState, RightPanelTab};
use crate::client::session_controller::extract_artifacts_from_markdown;
use crate::client::types::{ChatMessage, CompactionStep, MessageRole, ProcessStep, ToolCallItem};
use crate::i18n::{AppLocale, format_commas, format_tokens_km, tr};
use crate::icons::DatabaseIcon;
use std::time::Duration;
use gpui_kit::component::attachment::{Attachment, AttachmentContent, AttachmentTitle};
use gpui_kit::component::bubble::{Bubble, BubbleVariant};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::message::{Message, MessageAlignment, MessageContent, MessageFooter};
use gpui_kit::component::shimmer::ShimmerText;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::text::{RangeHighlight, TextView, TextViewState, TextViewStyle};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Anchor, AnyElement, AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px, rems,
};
use gpui_kit_markdown_extras::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct CachedRowEntities {
    pub text_state: Entity<TextViewState>,
    pub last_text_len: usize,
    pub last_text_snapshot: String,
    pub diff_states: HashMap<SharedString, Entity<DiffState>>,
    pub compaction_states: HashMap<usize, (String, Entity<TextViewState>)>,
    pub thinking_states: HashMap<usize, (String, Entity<TextViewState>)>,
    pub compaction_files_open: HashMap<usize, bool>,
    pub tool_output_editors: HashMap<SharedString, (String, Entity<EditorState>)>,
    pub process_group_open: Option<bool>,
    pub expanded_thinking_steps: HashMap<usize, bool>,
    pub expanded_compactions: HashMap<usize, bool>,
    pub expanded_tools: HashMap<SharedString, bool>,
    pub modified_files_open: Option<bool>,
    pub error_expanded: bool,
    pub error_dismissed: bool,
    pub user_expanded: bool,
}

#[derive(Clone, Debug)]
struct ModifiedFileSummary {
    path: String,
    added: usize,
    removed: usize,
    patch: Option<SharedString>,
}

pub fn render_message_row(
    msg: &ChatMessage,
    cache: &mut HashMap<u64, CachedRowEntities>,
    find_query: &str,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    let msg_id = msg.id;
    let mut newly_created = false;
    let row_cache = cache.entry(msg_id).or_insert_with(|| {
        newly_created = true;
        let initial_text = strip_artifact_blocks(&msg.content);
        let text_state = cx.new(|cx| TextViewState::markdown(&initial_text, cx).selectable(true));
        CachedRowEntities {
            text_state,
            last_text_len: initial_text.len(),
            last_text_snapshot: initial_text,
            diff_states: HashMap::new(),
            compaction_states: HashMap::new(),
            thinking_states: HashMap::new(),
            compaction_files_open: HashMap::new(),
            tool_output_editors: HashMap::new(),
            process_group_open: None,
            expanded_thinking_steps: HashMap::new(),
            expanded_compactions: HashMap::new(),
            expanded_tools: HashMap::new(),
            modified_files_open: None,
            error_expanded: false,
            error_dismissed: false,
            user_expanded: false,
        }
    });
    if newly_created && row_cache.text_state.read(cx).rendered_text().source().is_empty() && !row_cache.last_text_snapshot.trim().is_empty() {
        let tv = row_cache.text_state.clone();
        let app_for_obs = app_state.clone();
        cx.observe(&tv, move |panel, _, cx| {
            let idx_opt = app_for_obs.read(cx).messages.iter().position(|m| m.id == msg_id);
            if let Some(idx) = idx_opt {
                panel.scroller.update(cx, |s, cx| {
                    let _ = s.remeasure_items(idx..idx + 1, cx);
                });
            }
            cx.notify();
        })
        .detach();
    }

    // Incremental update of retained TextViewState (fast path when no artifacts & unchanged)
    if !newly_created && (msg.content.len() != row_cache.last_text_len || msg.content.contains(":::artifact")) {
        let display_md = if msg.content.contains(":::artifact") {
            strip_artifact_blocks(&msg.content)
        } else {
            msg.content.clone()
        };
        if display_md != row_cache.last_text_snapshot {
            let prev = &row_cache.last_text_snapshot;
            let is_append = display_md.starts_with(prev.as_str());
            if is_append {
                let delta = &display_md[prev.len()..];
                if !delta.is_empty() {
                    row_cache.text_state.update(cx, |state, cx| {
                        state.push_str(delta, cx);
                    });
                }
            } else {
                row_cache.text_state.update(cx, |state, cx| {
                    state.set_text(&display_md, cx);
                });
            }
            row_cache.last_text_len = display_md.len();
            row_cache.last_text_snapshot = display_md;
        }
    }

    // Apply ConversationFind highlights only if query is active
    if !find_query.is_empty() {
        let warn_bg = cx.theme().warning.opacity(0.35);
        row_cache.text_state.update(cx, |state, cx| {
            let rendered = state.rendered_text();
            let lower = rendered.as_str().to_lowercase();
            let q_lower = find_query.to_lowercase();
            let ranges: Vec<RangeHighlight> = lower
                .match_indices(&q_lower)
                .map(|(start, found)| RangeHighlight::new(start..start + found.len(), warn_bg))
                .collect();
            let _ = state.set_range_highlights(ranges, cx);
        });
    }

    let (is_turn_live, is_last_turn, font_px, locale, user_collapse_chars) = {
        let state = app_state.read(cx);
        let is_last = state.messages.last().map(|m| m.id) == Some(msg.id);
        (
            msg.is_streaming || (is_last && (state.is_agent_running || state.is_compacting)),
            is_last,
            state.chat_font_size,
            state.locale,
            state.user_collapse_chars,
        )
    };

    let inner = match msg.role {
        MessageRole::User => render_user_row(
            msg,
            row_cache,
            user_collapse_chars,
            font_px,
            locale,
            app_state,
            cx,
        ),
        MessageRole::System | MessageRole::Notice => {
            render_notice_row(msg, row_cache, font_px, locale, cx)
        }
        MessageRole::Assistant => {
            render_assistant_row(msg, row_cache, font_px, is_turn_live, is_last_turn, locale, app_state, window, cx)
        }
    };

    // Exact same max_w(840px) + px_6() container as ComposerView, explicitly inheriting system font
    div()
        .w_full()
        .px_6()
        .py_1p5()
        .font_family(cx.theme().font_family.clone())
        .child(div().w_full().max_w(px(app_state.read(cx).chat_content_width)).mx_auto().child(inner))
        .into_any_element()
}

fn render_user_row(
    msg: &ChatMessage,
    row_cache: &CachedRowEntities,
    user_collapse_chars: usize,
    font_px: f32,
    locale: AppLocale,
    app_state: &Entity<AppState>,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    let mut stack = v_flex().gap_2().min_w_0();

    if !msg.attachments.is_empty() {
        let mut att_row = h_flex().gap_2().flex_wrap();
        for (i, att) in msg.attachments.iter().enumerate() {
            att_row = att_row.child(
                Attachment::new()
                    .id(("att", msg.id * 100 + i as u64))
                    .content(AttachmentContent::new().title(AttachmentTitle::new(att.name.clone()))),
            );
        }
        stack = stack.child(att_row);
    }

    let total_chars = msg.content.chars().count();
    let can_collapse = user_collapse_chars > 0 && total_chars > user_collapse_chars;
    let is_expanded = row_cache.user_expanded;
    let shown_text = if can_collapse && !is_expanded {
        let mut s: String = msg.content.chars().take(user_collapse_chars).collect();
        s.push('…');
        s
    } else {
        msg.content.clone()
    };

    stack = stack.child(
        div()
            .text_size(px(font_px))
            .line_height(rems(1.65))
            .font_family(cx.theme().font_family.clone())
            .text_color(cx.theme().foreground)
            .child(SharedString::from(shown_text)),
    );

    if can_collapse {
        let msg_id = msg.id;
        let btn_label = if is_expanded {
            tr(locale, "chat.collapseUserMsg").to_string()
        } else {
            format!(
                "{} ({} {})",
                tr(locale, "chat.expandUserMsg"),
                format_commas(total_chars),
                tr(locale, "chat.charsSuffix")
            )
        };
        stack = stack.child(
            h_flex().justify_end().child(
                Button::new(("user-collapse", msg.id))
                    .ghost()
                    .xsmall()
                    .icon(if is_expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .label(btn_label)
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.toggle_user_expanded(msg_id, cx);
                    })),
            ),
        );
    }

    let copy_text = msg.content.clone();
    let edit_text = msg.content.clone();
    let entry_id_edit = msg.entry_id.clone();
    let entry_id_fork = msg.entry_id.clone();
    let app_edit = app_state.clone();
    let app_fork = app_state.clone();
    let mut footer_row = h_flex()
        .items_center()
        .gap_1p5()
        .child(
            Button::new(("copy-user", msg.id))
                .ghost()
                .small()
                .h(px(24.))
                .icon(IconName::Copy)
                .label(tr(locale, "chat.copy"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
                }),
        )
        .child(
            Button::new(("edit-user", msg.id))
                .ghost()
                .small()
                .h(px(24.))
                .icon(IconName::Undo2)
                .label(tr(locale, "chat.edit"))
                .tooltip(tr(locale, "chat.editResendTooltip"))
                .on_click(move |_, _, cx| {
                    let txt = edit_text.clone();
                    let eid = entry_id_edit.clone();
                    app_edit.update(cx, |s, cx| {
                        s.edit_user_message(txt, eid, cx);
                    });
                }),
        )
        .child(
            Button::new(("fork-user", msg.id))
                .ghost()
                .small()
                .h(px(24.))
                .icon(IconName::Redo2)
                .label(tr(locale, "chat.fork"))
                .tooltip(tr(locale, "chat.forkFromHereTooltip"))
                .on_click(move |_, _, cx| {
                    let eid = entry_id_fork.clone();
                    app_fork.update(cx, |s, cx| {
                        if let Some(sid) = s.active_session_id.clone() {
                            if let Some(target_eid) = eid {
                                let client = s.client.clone();
                                let target = target_eid.to_string();
                                cx.spawn(async move |_, _| {
                                    let _ = client.fork_session(&sid, &target).await;
                                })
                                .detach();
                            } else {
                                s.fork_session_by_id(sid, cx);
                            }
                        }
                    });
                }),
        );

    if !msg.timestamp.is_empty() {
        footer_row = footer_row.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(msg.timestamp.clone()),
        );
    }

    let footer = MessageFooter::new().content_inset(false).child(footer_row);

    // Item 2: No user avatar and no user name header
    Message::new()
        .id(("msg", msg.id))
        .alignment(MessageAlignment::End)
        .content(
            MessageContent::new()
                .bubble(Bubble::new().with_variant(BubbleVariant::Secondary).child(stack)),
        )
        .footer(footer)
        .into_any_element()
}

fn render_notice_row(
    msg: &ChatMessage,
    row_cache: &mut CachedRowEntities,
    font_px: f32,
    locale: AppLocale,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    if let Some((step_idx, ProcessStep::Compaction(c))) = msg
        .process_steps
        .iter()
        .enumerate()
        .find(|(_, s)| matches!(s, ProcessStep::Compaction(_)))
    {
        return div()
            .id(("compaction-notice", msg.id))
            .w_full()
            .py_1()
            .child(render_compaction_step_card(
                msg.id,
                step_idx,
                c,
                row_cache,
                font_px,
                locale,
                false,
                cx,
            ))
            .into_any_element();
    }

    div()
        .id(("notice", msg.id))
        .w_full()
        .py_1()
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(msg.content.clone())),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_compaction_step_card(
    msg_id: u64,
    step_idx: usize,
    c: &CompactionStep,
    row_cache: &mut CachedRowEntities,
    font_px: f32,
    locale: AppLocale,
    default_open: bool,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    let is_open = row_cache
        .expanded_compactions
        .get(&step_idx)
        .copied()
        .unwrap_or(default_open);

    // Lazy creation: only parse/construct TextViewState when the compaction card is opened
    if is_open && !c.summary_markdown.trim().is_empty() {
        let md = c.summary_markdown.clone();
        match row_cache.compaction_states.get_mut(&step_idx) {
            Some((prev_md, tv)) if *prev_md != md => {
                tv.update(cx, |state, cx| state.set_text(&md, cx));
                *prev_md = md;
            }
            None => {
                let tv = cx.new(|cx| TextViewState::markdown(&md, cx).selectable(true));
                row_cache.compaction_states.insert(step_idx, (md, tv));
            }
            _ => {}
        }
    }

    let title_label = if c.is_running {
        tr(locale, "chat.compacting")
    } else {
        tr(locale, "chat.compaction")
    };

    let tok_badge = c.tokens_before.map(|t| format!("{} tok", format_tokens_km(t)));

    let header = h_flex()
        .id(("compact-hdr", msg_id * 1000 + step_idx as u64))
        .w_full()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2p5()
        .py_1p5()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().secondary))
        .on_click(cx.listener(move |panel, _, _, cx| {
            panel.toggle_compaction_step(msg_id, step_idx, default_open, cx);
            cx.notify();
        }))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(16.))
                        .h(px(16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if c.is_running {
                            Spinner::new().xsmall().into_any_element()
                        } else {
                            Icon::new(IconName::Minimize).xsmall().into_any_element()
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .line_height(px(16.))
                        .text_color(cx.theme().foreground)
                        .child(title_label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(tr(locale, "chat.compactionHint")),
                ),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap_1p5()
                .children(tok_badge.map(|b| Tag::secondary().small().child(SharedString::from(b))))
                .child(
                    div()
                        .w(px(16.))
                        .h(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(if is_open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .xsmall(),
                        ),
                ),
        );

    let mut card = v_flex()
        .w_full()
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().primary.opacity(0.35))
        .bg(cx.theme().background)
        .child(header);

    if is_open && !c.summary_markdown.trim().is_empty() {
        let mut body = v_flex()
            .w_full()
            .min_w_0()
            .px_3()
            .py_2p5()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border);

        if let Some((_, tv)) = row_cache.compaction_states.get(&step_idx) {
            let mut tv_style = TextViewStyle::default();
            tv_style.heading_base_font_size = px((font_px - 1.0).max(11.0));
            tv_style.paragraph_gap = rems(0.75);
            tv_style.highlight_theme = cx.theme().highlight_theme.clone();
            let view = with_plugins(TextView::new(tv).style(tv_style).selectable(true));
            body = body.child(
                div()
                    .w_full()
                    .min_w_0()
                    .font_family(cx.theme().font_family.clone())
                    .text_size(px((font_px - 1.0).max(11.0)))
                    .line_height(rems(1.55))
                    .text_color(cx.theme().muted_foreground)
                    .child(view),
            );
        }

        if !c.read_files.is_empty() || !c.modified_files.is_empty() {
            let files_open = row_cache
                .compaction_files_open
                .get(&step_idx)
                .copied()
                .unwrap_or(false);

            let mut meta_parts = Vec::new();
            if !c.modified_files.is_empty() {
                meta_parts.push(format!("{} modified", c.modified_files.len()));
            }
            if !c.read_files.is_empty() {
                meta_parts.push(format!("{} read", c.read_files.len()));
            }
            let files_summary = meta_parts.join(", ");

            let mut files_section = v_flex()
                .w_full()
                .pt_2()
                .border_t_1()
                .border_color(cx.theme().border.opacity(0.6))
                .child(
                    h_flex()
                        .id(("compact-files-hdr", msg_id * 1000 + step_idx as u64))
                        .w_full()
                        .items_center()
                        .justify_between()
                        .cursor_pointer()
                        .p_1()
                        .rounded_sm()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .on_click(cx.listener(move |panel, _, _, cx| {
                            panel.toggle_compaction_files(msg_id, step_idx, cx);
                            cx.notify();
                        }))
                        .child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(Icon::new(IconName::Folder).xsmall().text_color(cx.theme().muted_foreground))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_medium()
                                        .text_color(cx.theme().foreground)
                                        .child(format!("{} ({})", tr(locale, "chat.fileContext"), files_summary)),
                                ),
                        )
                        .child(
                            Icon::new(if files_open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .xsmall()
                            .text_color(cx.theme().muted_foreground),
                        ),
                );

            if files_open {
                let mut files_body = v_flex().w_full().gap_2().pt_1().pl_4();

                if !c.modified_files.is_empty() {
                    let mut mod_col = v_flex().gap_1();
                    mod_col = mod_col.child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(format!("{} ({})", tr(locale, "chat.modifiedFiles"), c.modified_files.len())),
                    );
                    for f in &c.modified_files {
                        let f_path = f.clone();
                        mod_col = mod_col.child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(Icon::new(IconName::File).xsmall().text_color(cx.theme().primary))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(cx.theme().muted_foreground)
                                        .child(SharedString::from(f_path)),
                                ),
                        );
                    }
                    files_body = files_body.child(mod_col);
                }

                if !c.read_files.is_empty() {
                    let mut read_col = v_flex().gap_1();
                    read_col = read_col.child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().foreground)
                            .child(format!("{} ({})", tr(locale, "chat.readFiles"), c.read_files.len())),
                    );
                    for f in &c.read_files {
                        let f_path = f.clone();
                        read_col = read_col.child(
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .child(Icon::new(IconName::FileText).xsmall().text_color(cx.theme().muted_foreground))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .text_color(cx.theme().muted_foreground)
                                        .child(SharedString::from(f_path)),
                                ),
                        );
                    }
                    files_body = files_body.child(read_col);
                }

                files_section = files_section.child(files_body);
            }

            body = body.child(files_section);
        }

        card = card.child(body);
    }

    card.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_assistant_row(
    msg: &ChatMessage,
    row_cache: &mut CachedRowEntities,
    font_px: f32,
    is_turn_live: bool,
    is_last_turn: bool,
    locale: AppLocale,
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    let mut stack = v_flex().gap_3().w_full().min_w_0();
    let has_answer_text = !row_cache.last_text_snapshot.trim().is_empty();
    let (active_cwd, thinking_expanded_default) = {
        let s = app_state.read(cx);
        (s.active_cwd.clone(), s.thinking_expanded_default)
    };

    // Model name displayed ONCE at the top of the assistant turn
    let header_label = msg
        .model_label
        .clone()
        .unwrap_or_else(|| SharedString::from(tr(locale, "chat.assistantLabel")));
    stack = stack.child(
        div()
            .text_xs()
            .font_medium()
            .text_color(cx.theme().muted_foreground)
            .child(header_label),
    );

    // 1. ProcessDetailsGroup + ThinkingBlock + ToolCallBlock matching ChatWindow.tsx & MessageView.tsx:
    //    - All intermediate thinking blocks & tool calls of a turn live inside ProcessDetailsGroup
    //    - When the final answer has arrived (`has_answer_text && !msg.is_streaming`), ProcessDetailsGroup
    //      is COLLAPSED BY DEFAULT unless `thinking_expanded_default` is enabled
    //    - Clicking the ProcessDetailsGroup trigger expands/collapses the entire multi-step process at once!
    let has_process =
        !msg.process_steps.is_empty() || !msg.thinking.is_empty() || !msg.tool_calls.is_empty() || is_turn_live;
    if has_process {
        let default_process_open = thinking_expanded_default || is_turn_live;
        let process_open = row_cache
            .process_group_open
            .unwrap_or(default_process_open);
        let msg_id = msg.id;
        let msg_count = msg.process_message_count.max(1);
        let tool_count = msg.tool_calls.len();
        let compaction_count = msg
            .process_steps
            .iter()
            .filter(|s| matches!(s, ProcessStep::Compaction(_)))
            .count();

        let mut summary_parts = vec![
            tr(locale, "chat.processDetails").to_string(),
            if locale == AppLocale::ZhCn {
                format!("{} {}", format_commas(msg_count), tr(locale, "chat.messagesUnit"))
            } else if msg_count == 1 {
                "1 message".to_string()
            } else {
                format!("{} messages", format_commas(msg_count))
            },
        ];
        if tool_count > 0 {
            summary_parts.push(if locale == AppLocale::ZhCn {
                format!("{} {}", format_commas(tool_count), tr(locale, "chat.toolCallsUnit"))
            } else if tool_count == 1 {
                "1 tool call".to_string()
            } else {
                format!("{} tool calls", format_commas(tool_count))
            });
        }
        if compaction_count > 0 {
            summary_parts.push(if locale == AppLocale::ZhCn {
                format!("{compaction_count} {}", tr(locale, "chat.compactionsSuffix"))
            } else if compaction_count == 1 {
                "1 compaction".to_string()
            } else {
                format!("{compaction_count} compactions")
            });
        }
        let process_label = summary_parts.join(" · ");

        let process_trigger = h_flex()
            .id(("process-group-trigger", msg_id))
            .w_full()
            .h(px(26.))
            .gap_2()
            .items_center()
            .cursor_pointer()
            .on_click(cx.listener(move |panel, _, _, cx| {
                panel.toggle_process_group(msg_id, default_process_open, cx);
                cx.notify();
            }))
            .child(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(if process_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .xsmall(),
                    ),
            )
            .child(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Bot).xsmall()),
            )
            .child(
                if is_turn_live {
                    ShimmerText::new(process_label)
                        .id(("proc-shimmer", msg_id))
                        .text_color(cx.theme().muted_foreground)
                        .highlight_color(cx.theme().primary)
                        .duration(Duration::from_millis(1400))
                        .spread(0.4)
                        .text_xs()
                        .into_any_element()
                } else {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(process_label))
                        .into_any_element()
                },
            );

        let mut process_container = v_flex().w_full().gap_2().child(process_trigger);

        if process_open {
            let mut steps_stack = v_flex().w_full().gap_2().pl_3().border_l_1().border_color(cx.theme().border);

            let steps_to_render: Vec<ProcessStep> = if !msg.process_steps.is_empty() {
                msg.process_steps.clone()
            } else {
                let mut fallback = Vec::new();
                if !msg.thinking.is_empty() {
                    fallback.push(ProcessStep::Thinking(msg.thinking.clone()));
                }
                for tc in &msg.tool_calls {
                    fallback.push(ProcessStep::ToolCall(tc.clone()));
                }
                fallback
            };

            let last_compaction_idx = steps_to_render
                .iter()
                .rposition(|s| matches!(s, ProcessStep::Compaction(_)));

            for (step_idx, step) in steps_to_render.into_iter().enumerate() {
                let is_pre_compaction = last_compaction_idx
                    .map(|c_idx| step_idx < c_idx)
                    .unwrap_or(false);
                match step {
                    ProcessStep::Thinking(think_text) => {
                        let step_open = row_cache
                            .expanded_thinking_steps
                            .get(&step_idx)
                            .copied()
                            .unwrap_or(thinking_expanded_default);

                        let non_empty_lines: Vec<&str> = think_text
                            .lines()
                            .map(str::trim)
                            .filter(|l| !l.is_empty())
                            .collect();
                        let first_line = non_empty_lines.first().copied().unwrap_or("");
                        let mut chars = first_line.chars();
                        let head: String = chars.by_ref().take(80).collect();
                        let is_truncated = chars.next().is_some() || non_empty_lines.len() > 1;
                        let think_preview = if head.is_empty() {
                            tr(locale, "chat.thinkingLabel").to_string()
                        } else if is_truncated {
                            format!("{head}…")
                        } else {
                            head
                        };

                        let header_bar = h_flex()
                            .id(("think-step", msg_id * 1000 + step_idx as u64))
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_2p5()
                            .py_1p5()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().secondary))
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.toggle_thinking_step(msg_id, step_idx, cx);
                                cx.notify();
                            }))
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(16.))
                                            .h(px(16.))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconName::Bot).xsmall()),
                                    )
                                    .child(if step_open {
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .line_height(px(16.))
                                            .text_color(cx.theme().foreground)
                                            .child(tr(locale, "chat.thinkingLabel"))
                                            .into_any_element()
                                    } else {
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .line_height(px(16.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(think_preview))
                                            .into_any_element()
                                    }),
                            )
                            .child(
                                Icon::new(if step_open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .xsmall()
                                .text_color(cx.theme().muted_foreground),
                            );

                        let mut think_card = v_flex()
                            .w_full()
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(header_bar);

                        if step_open {
                            let md = think_text.to_string();
                            let tv = match row_cache.thinking_states.get_mut(&step_idx) {
                                Some((prev_md, tv)) if *prev_md != md => {
                                    tv.update(cx, |state, cx| state.set_text(&md, cx));
                                    *prev_md = md.clone();
                                    tv.clone()
                                }
                                Some((_, tv)) => tv.clone(),
                                None => {
                                    let tv = cx.new(|cx| TextViewState::markdown(&md, cx).selectable(true));
                                    row_cache.thinking_states.insert(step_idx, (md.clone(), tv.clone()));
                                    tv
                                }
                            };
                            let mut tv_style = TextViewStyle::default();
                            tv_style.heading_base_font_size = px((font_px - 1.0).max(11.0));
                            tv_style.paragraph_gap = rems(0.5);
                            tv_style.highlight_theme = cx.theme().highlight_theme.clone();
                            let markdown_view = with_plugins(TextView::new(&tv).style(tv_style).selectable(true));

                            think_card = think_card.child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .px_3()
                                    .py_2()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .font_family(cx.theme().font_family.clone())
                                    .text_size(px((font_px - 1.0).max(11.0)))
                                    .line_height(rems(1.5))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(markdown_view),
                            );
                        }

                        let think_el = think_card;
                        steps_stack = steps_stack.child(if is_pre_compaction {
                            div().w_full().opacity(0.75).child(think_el).into_any_element()
                        } else {
                            think_el.into_any_element()
                        });
                    }
                    ProcessStep::Compaction(c) => {
                        let card = render_compaction_step_card(
                            msg_id,
                            step_idx,
                            &c,
                            row_cache,
                            font_px,
                            locale,
                            false,
                            cx,
                        );
                        steps_stack = steps_stack.child(card);
                    }
                    ProcessStep::ToolCall(tc) => {
                        let card =
                            render_tool_call_card(msg.id, step_idx, &tc, row_cache, locale, window, cx);
                        steps_stack = steps_stack.child(if is_pre_compaction {
                            div().w_full().opacity(0.75).child(card).into_any_element()
                        } else {
                            card
                        });
                    }
                }
            }

            if let Some(err) = &msg.error_message {
                let (_, clean_summary) = parse_error_summary(err.as_ref());
                let err_copy = clean_summary.clone();
                steps_stack = steps_stack.child(
                    h_flex()
                        .w_full()
                        .px_2p5()
                        .py_1p5()
                        .gap_2()
                        .items_center()
                        .justify_between()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().danger.opacity(0.45))
                        .bg(cx.theme().danger.opacity(0.12))
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(IconName::TriangleAlert).xsmall().text_color(cx.theme().danger))
                                .child(div().flex_1().min_w_0().child(SharedString::from(clean_summary))),
                        )
                        .child(
                            Button::new(("copy-proc-err", msg_id))
                                .ghost()
                                .xsmall()
                                .h(px(22.))
                                .icon(IconName::Copy)
                                .label(tr(locale, "chat.copy"))
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    cx.write_to_clipboard(ClipboardItem::new_string(err_copy.clone()));
                                }),
                        ),
                );
            }

            process_container = process_container.child(steps_stack);
        }

        stack = stack.child(process_container);
    }

    // 2. Markdown response with comfortable paragraph/line spacing, configurable font size,
    //    stream fade, math/mermaid, AND fenced code block Copy button
    if has_answer_text {
        let mut tv_style = TextViewStyle::default();
        tv_style.heading_base_font_size = px(font_px);
        tv_style.paragraph_gap = rems(1.05);
        tv_style.highlight_theme = cx.theme().highlight_theme.clone();

        let view = TextView::new(&row_cache.text_state)
            .style(tv_style)
            .stream_fade(msg.is_streaming)
            .selectable(true)
            .code_block_actions(move |code_block, _, cx| {
                let code_str = code_block.code().to_string();
                let lang_label = code_block.lang().unwrap_or_else(|| SharedString::from("code"));
                let btn_id = SharedString::from(format!(
                    "copy-code-{}-{}",
                    code_block.span.as_ref().map(|s| s.start).unwrap_or(0),
                    code_str.len()
                ));
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(lang_label),
                    )
                    .child(
                        Button::new(btn_id)
                            .outline()
                            .xsmall()
                            .icon(IconName::Copy)
                            .label("Copy")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(code_str.clone()));
                            }),
                    )
            });
        let view = with_plugins(view);
        let parse_pending = row_cache.text_state.read(cx).rendered_text().source().is_empty()
            && !row_cache.last_text_snapshot.trim().is_empty();
        let body_content = if parse_pending {
            div()
                .w_full()
                .whitespace_normal()
                .child(SharedString::from(row_cache.last_text_snapshot.clone()))
                .into_any_element()
        } else {
            view.into_any_element()
        };
        stack = stack.child(
            div()
                .w_full()
                .min_w_0()
                .font_family(cx.theme().font_family.clone())
                .text_size(px(font_px))
                .line_height(rems(1.65))
                .child(body_content),
        );
    }

    // 2b. Standalone provider error alert: ONLY if there was no process group to house it!
    let err_text = (!has_process).then(|| {
        msg.error_message.clone().or_else(|| {
            if msg.stop_reason.as_deref() == Some("error") {
                Some(SharedString::from("Model request failed"))
            } else {
                None
            }
        })
    }).flatten();
    let _old_err_text = {
        None::<SharedString>
    };
    if let Some(err) = err_text.filter(|_| !row_cache.error_dismissed) {
        let (code_opt, clean_summary) = parse_error_summary(err.as_ref());
        let err_copy = clean_summary.clone();
        let msg_id = msg.id;
        let is_err_open = row_cache.error_expanded;
        let header_title = match &code_opt {
            Some(code) => format!("{} {code}", tr(locale, "chat.errorPrefix")),
            None => tr(locale, "chat.errorPrefix").to_string(),
        };

        let mut err_card = v_flex()
            .w_full()
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().danger.opacity(0.45))
            .bg(cx.theme().danger.opacity(0.1))
            .child(
                h_flex()
                    .id(("err-toggle-hdr", msg_id))
                    .w_full()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.toggle_error_expanded(msg_id, cx);
                        cx.notify();
                    }))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::TriangleAlert)
                                    .small()
                                    .text_color(cx.theme().danger),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().danger)
                                    .child(SharedString::from(header_title)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().danger.opacity(0.8))
                                    .child(tr(locale, "chat.errorDetailsToggle")),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new(("copy-err", msg.id))
                                    .ghost()
                                    .xsmall()
                                    .h(px(24.))
                                    .icon(IconName::Copy)
                                    .label(tr(locale, "chat.copy"))
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            err_copy.clone(),
                                        ));
                                    }),
                            )
                            .child(
                                Button::new(("dismiss-err", msg.id))
                                    .ghost()
                                    .xsmall()
                                    .h(px(24.))
                                    .icon(IconName::Close)
                                    .tooltip(tr(locale, "chat.dismissError"))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        cx.stop_propagation();
                                        panel.dismiss_error(msg_id, cx);
                                    })),
                            )
                            .child(
                                Icon::new(if is_err_open {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .xsmall()
                                .text_color(cx.theme().danger),
                            ),
                    ),
            );

        if is_err_open {
            err_card = err_card.child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().danger.opacity(0.25))
                    .text_xs()
                    .line_height(rems(1.5))
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(clean_summary)),
            );
        }

        stack = stack.child(err_card);
    } else if msg.stop_reason.as_deref() == Some("length") {
        stack = stack.child(
            h_flex()
                .w_full()
                .p_2p5()
                .gap_2()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().warning.opacity(0.45))
                .bg(cx.theme().warning.opacity(0.1))
                .text_xs()
                .text_color(cx.theme().warning)
                .child(Icon::new(IconName::TriangleAlert).xsmall())
                .child(tr(locale, "chat.truncatedNotice")),
        );
    } else if !is_turn_live && !has_answer_text && msg.stop_reason.as_deref() == Some("aborted") {
        stack = stack.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(tr(locale, "chat.stoppedNotice")),
        );
    }

    // 3. Inline Artifact cards (skip regex scan when no artifact markers exist)
    let artifacts = if msg.content.contains(":::artifact") {
        extract_artifacts_from_markdown(&msg.content, None)
    } else {
        Vec::new()
    };
    for (idx, art) in artifacts.into_iter().enumerate() {
        let app_open = app_state.clone();
        let app_open_menu = app_state.clone();
        let art_id = art.identifier.clone();
        let art_id_menu = art.identifier.clone();
        let art_title = art.title.clone();
        let art_title_copy = art.title.clone();
        let kind_label = art.kind.label();
        let cwd_for_reveal = active_cwd.clone();

        stack = stack.child(
            h_flex()
                .w_full()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2p5()
                        .items_center()
                        .child(Icon::new(IconName::Frame).small())
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().font_medium().truncate().child(art_title))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{kind_label} Artifact")),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2p5()
                        .child(
                            Button::new(("open-art", msg.id * 100 + idx as u64))
                                .outline()
                                .small()
                                .icon(IconName::Eye)
                                .label(tr(locale, "chat.preview"))
                                .on_click(move |_, _, cx| {
                                    app_open.update(cx, |s, cx| {
                                        s.active_artifact_id = Some(art_id.clone());
                                        s.open_right_tab(RightPanelTab::Artifact, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new(("art-actions", msg.id * 100 + idx as u64))
                                .ghost()
                                .small()
                                .icon(IconName::Ellipsis)
                                .tooltip(tr(locale, "chat.artifactActions"))
                                .dropdown_menu(move |menu, _, _| {
                                    let app_p = app_open_menu.clone();
                                    let aid = art_id_menu.clone();
                                    let title_cp = art_title_copy.clone();
                                    let cwd_rev = cwd_for_reveal.clone();

                                    menu.item(
                                        PopupMenuItem::new(tr(locale, "chat.openPreview"))
                                            .icon(IconName::Eye)
                                            .on_click(move |_, _, cx| {
                                                app_p.update(cx, |s, cx| {
                                                    s.active_artifact_id = Some(aid.clone());
                                                    s.open_right_tab(RightPanelTab::Artifact, cx);
                                                });
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.openFileLocation"))
                                            .icon(IconName::FolderOpen)
                                            .on_click(move |_, _, _| {
                                                reveal_path_in_finder(&cwd_rev);
                                            }),
                                    )
                                    .item(
                                        PopupMenuItem::new(tr(locale, "chat.copyTitle"))
                                            .icon(IconName::Copy)
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    title_cp.clone(),
                                                ));
                                            }),
                                    )
                                }),
                        ),
                ),
        );
    }

    // 5. Modified files summary card:
    //    - Shown ONLY when the entire turn has settled (!is_turn_live)
    //    - Expanded by default ONLY for the latest turn (is_last_turn); all earlier turns stay collapsed
    //    - When collapsed, does NOT parse diffs or allocate patch strings; reads lazily only on expand!
    let mf_count = if !is_turn_live {
        count_modified_file_paths(&msg.tool_calls)
    } else {
        0
    };
    if mf_count > 0 {
        let mf_open = row_cache.modified_files_open.unwrap_or(is_last_turn);
        let msg_id = msg.id;

        let (count, total_added, total_removed, modified_files_opt) = if mf_open {
            let files = collect_modified_files(&msg.tool_calls);
            let added: usize = files.iter().map(|f| f.added).sum();
            let removed: usize = files.iter().map(|f| f.removed).sum();
            (files.len(), added, removed, Some(files))
        } else {
            (mf_count, 0, 0, None)
        };

        let mut header_left = h_flex()
            .gap_2()
            .items_center()
            .child(Icon::new(IconName::FileText).xsmall())
            .child(
                div()
                    .text_xs()
                    .font_medium()
                    .child(if count == 1 {
                        tr(locale, "chat.fileModifiedOne").to_string()
                    } else {
                        format!("{} {}", format_commas(count), tr(locale, "chat.filesModified"))
                    }),
            );

        if mf_open {
            header_left = header_left
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().success)
                        .child(format!("+{}", format_commas(total_added))),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(format!("-{}", format_commas(total_removed))),
                );
        }

        let mf_header = h_flex()
            .id(("mod-files-hdr", msg_id))
            .w_full()
            .px_3()
            .py_2()
            .justify_between()
            .items_center()
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().background.opacity(0.45)))
            .on_click(cx.listener(move |panel, _, _, cx| {
                panel.toggle_modified_files(msg_id, is_last_turn, cx);
                cx.notify();
            }))
            .child(header_left)
            .child(
                Icon::new(if mf_open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .xsmall(),
            );

        let mut mf_card = v_flex()
            .w_full()
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(mf_header);

        if let Some(modified_files) = modified_files_opt {
            let mut list = v_flex().border_t_1().border_color(cx.theme().border);
            for (f_idx, mf) in modified_files.into_iter().enumerate() {
                let raw_path = mf.path.clone();
                let resolved_path = resolve_against_cwd(&active_cwd, &raw_path);
                let resolved_open = resolved_path.clone();
                let resolved_reveal = resolved_path.clone();
                let copy_path = resolved_path.to_string_lossy().into_owned();
                let patch_open = mf.patch.clone();
                let patch_menu = mf.patch.clone();
                let app_open = app_state.clone();
                let app_menu_open = app_state.clone();

                list = list.child(
                    h_flex()
                        .px_3()
                        .py_1p5()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .hover(|s| s.bg(cx.theme().background.opacity(0.45)))
                        .child(
                            h_flex()
                                .id(("mod-file-open", msg_id * 100 + f_idx as u64))
                                .flex_1()
                                .min_w_0()
                                .gap_2()
                                .items_center()
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    let p = resolved_open.clone();
                                    let dp = patch_open.clone();
                                    app_open.update(cx, |s, cx| s.open_file_with_diff(p, dp, cx));
                                })
                                .child(Icon::new(IconName::File).xsmall())
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_xs()
                                        .truncate()
                                        .child(SharedString::from(raw_path)),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().success)
                                        .child(format!("+{}", format_commas(mf.added))),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().danger)
                                        .child(format!("-{}", format_commas(mf.removed))),
                                )
                                .child(
                                    Button::new(("mod-file-menu", msg_id * 100 + f_idx as u64))
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Ellipsis)
                                        .tooltip(tr(locale, "chat.fileActions"))
                                        .dropdown_menu(move |menu, _, _| {
                                            let app_v = app_menu_open.clone();
                                            let p_view = resolved_reveal.clone();
                                            let p_loc = resolved_reveal.clone();
                                            let p_cp = copy_path.clone();
                                            let dp_menu = patch_menu.clone();

                                            menu.item(
                                                PopupMenuItem::new(tr(locale, "chat.openDiffPreview"))
                                                    .icon(IconName::FileText)
                                                    .on_click(move |_, _, cx| {
                                                        let p = p_view.clone();
                                                        let dp = dp_menu.clone();
                                                        app_v.update(cx, |s, cx| {
                                                            s.open_file_with_diff(p, dp, cx)
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new(tr(locale, "chat.openFileLocation"))
                                                    .icon(IconName::FolderOpen)
                                                    .on_click(move |_, _, _| {
                                                        reveal_path_in_finder(&p_loc);
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new(tr(locale, "chat.copyPath"))
                                                    .icon(IconName::Copy)
                                                    .on_click(move |_, _, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(p_cp.clone()),
                                                        );
                                                    }),
                                            )
                                        }),
                                ),
                        ),
                );
            }
            mf_card = mf_card.child(list);
        }

        stack = stack.child(mf_card);
    }

    // 6. Full-width Separator + Footer:
    //    - Shown ONLY when the entire turn has settled (!is_turn_live) AND has answer text
    //    - Timestamp shows the real parsed/recorded time without hardcoded "Just now"
    if !is_turn_live && has_answer_text {
        let mut left_actions = h_flex().items_center().gap_2p5();
        let copy_md = msg.content.clone();
        left_actions = left_actions.child(
            Button::new(("copy-assistant", msg.id))
                .ghost()
                .small()
                .h(px(28.))
                .icon(IconName::Copy)
                .label(tr(locale, "chat.copy"))
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_md.clone()));
                }),
        );
        if let Some(ref u) = msg.turn_usage {
            let u_clone = u.clone();
            let tok_str = format_tokens_km(u.total_tokens);
            let model_name = msg
                .model_label
                .clone()
                .unwrap_or_else(|| SharedString::from(tr(locale, "chat.assistantLabel")));
            left_actions = left_actions.child(
                Button::new(("turn-usage-btn", msg.id))
                    .ghost()
                    .small()
                    .h(px(28.))
                    .icon(Icon::new(DatabaseIcon))
                    .label(tok_str)
                    .tooltip(tr(locale, "chat.turnUsageTitle"))
                    .dropdown_menu_with_anchor(Anchor::BottomLeft, move |menu, _, _| {
                        let mut m = menu.label(tr(locale, "chat.turnUsageTitle"));
                        let u_item = u_clone.clone();
                        let m_name = model_name.clone();
                        m = m.item(PopupMenuItem::element(move |_, cx| {
                            let mut card = v_flex()
                                .w(px(260.))
                                .p_1p5()
                                .gap_1p5()
                                .text_xs()
                                .child(usage_row(tr(locale, "chat.turnUsageModel"), m_name.clone(), cx))
                                .child(usage_row(
                                    tr(locale, "chat.turnUsageUncachedInput"),
                                    SharedString::from(format_tokens_km(u_item.input_tokens)),
                                    cx,
                                ))
                                .child(usage_row(
                                    tr(locale, "chat.turnUsageOutput"),
                                    SharedString::from(format_tokens_km(u_item.output_tokens)),
                                    cx,
                                ));
                            if u_item.cache_read_tokens > 0 {
                                card = card.child(usage_row(
                                    tr(locale, "chat.turnUsageCacheRead"),
                                    SharedString::from(format_tokens_km(u_item.cache_read_tokens)),
                                    cx,
                                ));
                            }
                            if u_item.cache_write_tokens > 0 {
                                card = card.child(usage_row(
                                    tr(locale, "panel.cacheWrite"),
                                    SharedString::from(format_tokens_km(u_item.cache_write_tokens)),
                                    cx,
                                ));
                            }
                            card = card.child(usage_row(
                                tr(locale, "chat.turnUsageTotal"),
                                SharedString::from(format_tokens_km(u_item.total_tokens)),
                                cx,
                            ));
                            if u_item.cost_usd > 0.0 {
                                card = card.child(usage_row(
                                    tr(locale, "chat.turnUsageCost"),
                                    SharedString::from(format!("${:.4}", u_item.cost_usd)),
                                    cx,
                                ));
                            }
                            card
                        }));
                        m
                    }),
            );
        }
        if let Some(ms) = msg.turn_duration_ms {
            left_actions = left_actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{:.2}s", ms as f64 / 1000.0)),
            );
        }
        if let Some(tok) = &msg.token_summary {
            left_actions = left_actions.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tok.clone()),
            );
        }

        stack = stack.child(
            h_flex()
                .w_full()
                .pt_2()
                .mt_1()
                .border_t_1()
                .border_color(cx.theme().border)
                .justify_between()
                .items_center()
                .child(left_actions)
                .when_some_time(&msg.timestamp, cx),
        );
    }

    stack.into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_tool_call_card(
    msg_id: u64,
    idx: usize,
    tc: &ToolCallItem,
    row_cache: &mut CachedRowEntities,
    locale: AppLocale,
    window: &mut Window,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> AnyElement {
    let tc_open = row_cache
        .expanded_tools
        .get(&tc.id)
        .copied()
        .unwrap_or(tc.expanded);
    let tc_id = tc.id.clone();
    let is_bash = tc.name.as_ref() == "bash" || tc.name.as_ref() == "bash (local)";
    let is_write = tc.name.as_ref() == "write";
    let is_edit = tc.name.as_ref() == "edit" || tc.name.as_ref() == "apply_patch";

    let bash_cmd = if is_bash {
        tc.arguments
            .iter()
            .find(|(k, _)| k.as_ref() == "command")
            .map(|(_, v)| v.clone())
    } else {
        None
    };

    let preview_summary = if let Some(ref cmd) = bash_cmd {
        SharedString::from(format!("$ {}", cmd.lines().next().unwrap_or("")))
    } else {
        let arg_preview = tc
            .arguments
            .first()
            .map(|(k, v)| format!("{k}: {}", v.lines().next().unwrap_or("")))
            .unwrap_or_default();
        SharedString::from(arg_preview)
    };

    let mut header_row = h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .child(
            h_flex()
                .id(("tool-hdr", msg_id * 1000 + idx as u64))
                .flex_1()
                .min_w_0()
                .px_3()
                .py_1p5()
                .gap_2()
                .items_center()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().background.opacity(0.45)))
                .on_click(cx.listener(move |panel, _, _, cx| {
                    panel.toggle_tool(msg_id, tc_id.clone(), cx);
                    cx.notify();
                }))
                .child(
                    Icon::new(if tc.is_running {
                        IconName::LoaderCircle
                    } else if is_bash {
                        IconName::SquareTerminal
                    } else if tc.is_error {
                        IconName::CircleX
                    } else {
                        IconName::CircleCheck
                    })
                    .xsmall(),
                )
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(if tc.is_error {
                            cx.theme().danger
                        } else {
                            cx.theme().foreground
                        })
                        .child(tc.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(preview_summary),
                )
                .child(Tag::secondary().small().child(if tc.is_running {
                    "running"
                } else if tc.is_error {
                    "error"
                } else {
                    "done"
                }))
                .child(
                    Icon::new(if tc_open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                ),
        );



    let mut card = v_flex()
        .w_full()
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(if tc.is_error {
            cx.theme().danger
        } else {
            cx.theme().border
        })
        .bg(cx.theme().secondary)
        .child(header_row);

    if tc_open {
        let mut body = v_flex()
            .gap_2()
            .p_3()
            .border_t_1()
            .border_color(cx.theme().border);
        // The patch is drawn full-bleed under the padded body so the line-number
        // gutter sits against the card edge instead of after 12px of padding.
        let mut full_bleed_diff: Option<gpui_kit::AnyElement> = None;

        if let Some(ref cmd) = bash_cmd {
            let cmd_copy = cmd.to_string();
            body = body.child(
                h_flex()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().background)
                    .justify_between()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .id(("bash-cmd-text", msg_id * 1000 + idx as u64))
                            .flex_1()
                            .min_w_0()
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_xs()
                            .child(SharedString::from(format!("$ {cmd}"))),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(
                                Button::new(("copy-bash-body", msg_id * 1000 + idx as u64))
                                    .outline()
                                    .xsmall()
                                    .icon(IconName::Copy)
                                    .label(tr(locale, "chat.copy"))
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(cmd_copy.clone()));
                                    }),
                            ),
                    ),
            );
        } else {
            for (k, v) in &tc.arguments {
                // Never dump raw file content/diff strings in the arguments rows for write/edit
                if (is_write || is_edit)
                    && matches!(
                        k.as_ref(),
                        "content" | "oldText" | "newText" | "old_string" | "new_string" | "edits"
                    )
                {
                    continue;
                }
                body = body.child(
                    h_flex()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .w(px(96.))
                                .font_medium()
                                .text_color(cx.theme().muted_foreground)
                                .child(k.clone()),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(v.clone())),
                );
            }
        }

        // Lazy DiffState allocation ONLY when expanded (`tc_open == true`)
        if is_edit {
            if !row_cache.diff_states.contains_key(&tc.id) {
                if let Some(patch) = &tc.diff_patch {
                    if let Ok(files) = DiffFile::parse(patch.as_ref()) {
                        // Always unified: the app no longer offers a split view.
                        let ds = cx.new(|cx| DiffState::new(files, cx));
                        cx.observe(&ds, |_, _, cx| cx.notify()).detach();
                        row_cache.diff_states.insert(tc.id.clone(), ds);
                    }
                }
            }
        }

        if let Some(diff_state) = row_cache.diff_states.get(&tc.id) {
            let mode = diff_state.read(cx).mode();
            let ds_toggle = diff_state.clone();
            body = body
                .child(
                    h_flex().justify_end().child(
                        Button::new(("diff-mode", msg_id * 1000 + idx as u64))
                            .ghost()
                            .small()
                            .icon(IconName::LayoutDashboard)
                            .label(if mode == DiffMode::Unified {
                                tr(locale, "chat.splitView")
                            } else {
                                tr(locale, "chat.unifiedView")
                            })
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                ds_toggle.update(cx, |s, cx| {
                                    let next = if s.mode() == DiffMode::Unified {
                                        DiffMode::Split
                                    } else {
                                        DiffMode::Unified
                                    };
                                    s.set_mode(next, cx);
                                });
                                panel.scroller.update(cx, |sc, cx| sc.remeasure(cx));
                                cx.notify();
                            })),
                    ),
                )
                ;
            full_bleed_diff = Some(
                div()
                    .w_full()
                    .h(px(220.))
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Diff::new(diff_state)
                            .header_visible(false)
                            .line_number(true)
                            .size_full(),
                    )
                    .into_any_element(),
            );
        } else if tc.is_error && !tc.output.is_empty() {
            let (code_opt, clean_err) = parse_error_summary(tc.output.as_ref());
            let err_line = match code_opt {
                Some(code) => format!("{} {code}: {clean_err}", tr(locale, "chat.errorPrefix")),
                None => clean_err,
            };
            body = body.child(
                div()
                    .p_2()
                    .rounded_sm()
                    .bg(cx.theme().danger.opacity(0.08))
                    .border_1()
                    .border_color(cx.theme().danger.opacity(0.35))
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(SharedString::from(err_line)),
            );
        } else {
            // For `write`, display the written `content` inside a height-capped readonly Editor;
            // for other tools, display `tc.output` inside a height-capped readonly Editor.
            let editor_text: String = if is_write {
                tc.arguments
                    .iter()
                    .find(|(k, _)| k.as_ref() == "content")
                    .map(|(_, v)| v.to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| tc.output.to_string())
            } else {
                tc.output.to_string()
            };

            if !editor_text.is_empty() {
                let lang = infer_tool_output_language(tc, &editor_text);
                match row_cache.tool_output_editors.get_mut(&tc.id) {
                    Some((prev, ed)) if *prev != editor_text => {
                        ed.update(cx, |s, cx| s.set_value(&editor_text, window, cx));
                        *prev = editor_text.clone();
                    }
                    None => {
                        let ed = cx.new(|cx| EditorState::new(window, cx).language(lang));
                        ed.update(cx, |s, cx| s.set_value(&editor_text, window, cx));
                        row_cache
                            .tool_output_editors
                            .insert(tc.id.clone(), (editor_text.clone(), ed));
                    }
                    _ => {}
                }
                if let Some((_, ed_state)) = row_cache.tool_output_editors.get(&tc.id) {
                    let line_count = editor_text.lines().count().max(1);
                    let height_px = ((line_count * 18) + 18).clamp(72, 220) as f32;
                    body = body.child(
                        div().w_full().h(px(height_px)).child(
                            Editor::new(ed_state)
                                .readonly(true)
                                .bordered(true)
                                .h(px(height_px))
                                .w_full(),
                        ),
                    );
                }
            }
        }

        card = card.child(body);
        if let Some(surface) = full_bleed_diff {
            card = card.child(surface);
        }
    }

    card.into_any_element()
}

fn count_modified_file_paths(tool_calls: &[ToolCallItem]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for tc in tool_calls {
        if tc.is_error || !matches!(tc.name.as_ref(), "edit" | "write" | "apply_patch") {
            continue;
        }
        if let Some((_, v)) = tc
            .arguments
            .iter()
            .find(|(k, _)| matches!(k.as_ref(), "path" | "file_path"))
        {
            seen.insert(v.as_ref());
        }
    }
    seen.len()
}

fn collect_modified_files(tool_calls: &[ToolCallItem]) -> Vec<ModifiedFileSummary> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, (usize, usize, Option<String>)> = HashMap::new();

    for tc in tool_calls {
        if tc.is_error || !matches!(tc.name.as_ref(), "edit" | "write" | "apply_patch") {
            continue;
        }
        let Some(path) = tc
            .arguments
            .iter()
            .find(|(k, _)| matches!(k.as_ref(), "path" | "file_path"))
            .map(|(_, v)| v.to_string())
        else {
            continue;
        };

        let (mut added, mut removed) = (0usize, 0usize);
        let mut call_patch: Option<String> = None;

        if let Some(patch) = &tc.diff_patch {
            call_patch = Some(patch.to_string());
            for line in patch.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    added += 1;
                } else if line.starts_with('-') && !line.starts_with("---") {
                    removed += 1;
                }
            }
        } else if tc.name.as_ref() == "write" {
            if let Some((_, content)) = tc.arguments.iter().find(|(k, _)| k.as_ref() == "content") {
                let lines: Vec<&str> = content.as_ref().split('\n').collect();
                let count = if lines.last() == Some(&"") {
                    lines.len().saturating_sub(1)
                } else {
                    lines.len()
                };
                added = count;
                // Generate an added-file unified diff for previewing newly written files
                let mut p = format!("--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n", count.max(1));
                for line in lines.iter().take(count) {
                    p.push('+');
                    p.push_str(line.strip_suffix('\r').unwrap_or(line));
                    p.push('\n');
                }
                call_patch = Some(p);
            }
        }

        if let Some(entry) = map.get_mut(&path) {
            entry.0 += added;
            entry.1 += removed;
            if let Some(cp) = call_patch {
                match &mut entry.2 {
                    Some(existing) => {
                        existing.push('\n');
                        existing.push_str(&cp);
                    }
                    slot => *slot = Some(cp),
                }
            }
        } else {
            order.push(path.clone());
            map.insert(path, (added, removed, call_patch));
        }
    }

    order
        .into_iter()
        .map(|path| {
            let (added, removed, patch) = map.remove(&path).unwrap_or((0, 0, None));
            ModifiedFileSummary {
                path,
                added,
                removed,
                patch: patch.map(SharedString::from),
            }
        })
        .collect()
}

fn resolve_against_cwd(cwd: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

fn reveal_path_in_finder(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        if path.exists() {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn();
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if path.exists() {
            let _ = std::process::Command::new("explorer.exe")
                .arg(format!("/select,{}", path.display()))
                .spawn();
            return;
        }
    }
    let fallback = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let _ = open::that(fallback);
}

fn strip_artifact_blocks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel) = input[cursor..].find(":::artifact{") {
        let start = cursor + rel;
        out.push_str(&input[cursor..start]);
        if let Some(rel_end) = input[start..].find("\n:::") {
            cursor = start + rel_end + 4;
        } else {
            out.push_str(&input[start..]);
            return out;
        }
    }
    out.push_str(&input[cursor..]);
    out.trim().to_string()
}

fn infer_tool_output_language(tc: &ToolCallItem, output: &str) -> &'static str {
    let tool = tc.name.as_ref();
    if tool == "bash" || tool == "bash (local)" {
        return "bash";
    }
    if let Some((_, path)) = tc
        .arguments
        .iter()
        .find(|(k, _)| matches!(k.as_ref(), "path" | "file_path"))
    {
        if let Some(ext) = Path::new(path.as_ref()).extension().and_then(|e| e.to_str()) {
            return match ext.to_ascii_lowercase().as_str() {
                "rs" => "rust",
                "ts" | "tsx" => "typescript",
                "js" | "jsx" | "mjs" | "cjs" => "javascript",
                "json" => "json",
                "toml" => "toml",
                "md" | "markdown" => "markdown",
                "css" => "css",
                "html" => "html",
                "py" => "python",
                "sh" | "bash" | "zsh" => "bash",
                "yaml" | "yml" => "yaml",
                _ => "text",
            };
        }
    }
    let t = output.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        "json"
    } else {
        "text"
    }
}

pub fn parse_error_summary(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    // Extract HTTP status code like (429), 429, (403), (500), etc.
    let mut code_opt: Option<String> = None;
    for token in trimmed.split(|c: char| !c.is_ascii_alphanumeric()) {
        if token.len() == 3
            && token.chars().all(|c| c.is_ascii_digit())
            && (token.starts_with('4') || token.starts_with('5'))
        {
            code_opt = Some(token.to_string());
            break;
        }
    }

    // Extract clean message from JSON payload if embedded
    if let Some(brace) = trimmed.find('{') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&trimmed[brace..]) {
            if let Some(err_code) = val
                .get("error")
                .and_then(|e| e.get("code"))
                .or_else(|| val.get("code"))
                .and_then(serde_json::Value::as_str)
            {
                if code_opt.is_none() {
                    code_opt = Some(err_code.to_string());
                }
            }
            if let Some(msg) = val
                .get("error")
                .and_then(|e| e.get("message"))
                .or_else(|| val.get("message"))
                .or_else(|| val.get("error"))
                .and_then(serde_json::Value::as_str)
            {
                return (code_opt, msg.trim().to_string());
            }
        }
    }

    // Strip raw HTML tags if upstream returned an HTML error page
    if trimmed.contains("<html") || trimmed.contains("<HTML") || trimmed.contains("<title") {
        let mut plain = String::new();
        let mut in_tag = false;
        for ch in trimmed.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    plain.push(' ');
                }
                _ if !in_tag => plain.push(ch),
                _ => {}
            }
        }
        let collapsed: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
        return (code_opt, collapsed);
    }

    (code_opt, trimmed.to_string())
}

fn usage_row(
    label: &'static str,
    value: SharedString,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .justify_between()
        .gap_3()
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(div().font_medium().text_color(cx.theme().foreground).child(value))
}

trait EntryExt {
    fn when_some_time(self, timestamp: &SharedString, cx: &gpui_kit::App) -> Self;
}

impl<T: ParentElement> EntryExt for T {
    fn when_some_time(self, timestamp: &SharedString, cx: &gpui_kit::App) -> Self {
        if timestamp.is_empty() || timestamp.as_ref() == "streaming" {
            return self;
        }
        self.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(timestamp.clone()),
        )
    }
}

#[cfg(test)]
mod modified_files_tests {
    use super::collect_modified_files;
    use crate::client::session_controller::parse_session_messages;

    #[test]
    fn write_and_edit_calls_become_modified_file_rows() {
        let payload = serde_json::json!({
            "messages": [
                { "role": "user", "content": "change files" },
                {
                    "role": "assistant",
                    "content": [
                        { "type": "text", "text": "done" },
                        {
                            "type": "toolCall",
                            "id": "c1",
                            "name": "write",
                            "arguments": { "path": "src/a.rs", "content": "fn a() {}\n" }
                        },
                        {
                            "type": "toolCall",
                            "id": "c2",
                            "name": "edit",
                            "arguments": {
                                "path": "src/b.rs",
                                "oldText": "let x = 1;",
                                "newText": "let x = 2;"
                            }
                        }
                    ]
                }
            ]
        });
        let mut next_id = 1u64;
        let messages = parse_session_messages(&payload, &mut next_id);
        let assistant = messages
            .iter()
            .find(|m| m.role == crate::client::types::MessageRole::Assistant)
            .expect("assistant message parsed");
        assert_eq!(assistant.tool_calls.len(), 2, "both tool calls carried arguments");

        let files = collect_modified_files(&assistant.tool_calls);
        assert_eq!(files.len(), 2, "two files summarised: {files:?}");
        assert!(files.iter().any(|f| f.path.ends_with("a.rs") && f.added > 0));
        assert!(files.iter().any(|f| f.path.ends_with("b.rs")));
    }
}
