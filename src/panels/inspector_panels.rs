use crate::app_state::AppState;
use crate::i18n::{format_commas, format_tokens_km, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    ClipboardItem, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _,
    Window, div,
};

pub fn render_inspector_panel(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::panels::WorkbenchPanel>,
) -> impl IntoElement {
    let state = app_state.read(cx);
    let locale = state.locale;
    let active_sess = state
        .active_session_id
        .as_ref()
        .and_then(|id| state.sessions.iter().find(|s| &s.id == id));

    let sid = state
        .active_session_id
        .clone()
        .unwrap_or_else(|| tr(locale, "panel.unsaved").to_string());
    let sess_name = active_sess
        .map(|s| s.display_title())
        .unwrap_or_else(|| SharedString::from(tr(locale, "chat.newSession")));
    let msg_count = state.messages.len();
    let model = state
        .selected_model
        .as_ref()
        .map(|m| m.display_label())
        .unwrap_or_else(|| SharedString::from(tr(locale, "panel.unselected")));
    let tools = state.tool_preset.tool_names().join(", ");
    let stats = &state.stats;
    let total_tokens = stats.input_tokens
        + stats.output_tokens
        + stats.cache_read_tokens
        + stats.cache_write_tokens;
    let ctx_summary = match (stats.context_tokens, stats.context_window) {
        (Some(tok), Some(win)) => format!(
            "{} / {} ({:.1}%)",
            format_tokens_km(tok),
            format_tokens_km(win),
            stats.context_used_pct
        ),
        (Some(tok), None) => format!("{} / …", format_tokens_km(tok)),
        (None, Some(win)) => format!("— / {}", format_tokens_km(win)),
        _ if stats.has_context_usage => format!("{:.1}%", stats.context_used_pct),
        _ => "—".to_string(),
    };

    let has_session_id = state.active_session_id.is_some();
    let sid_for_copy = sid.clone();
    let copy_tooltip = tr(locale, "chat.copy");

    let mut session_card = v_flex()
        .p_3()
        .gap_1p5()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(div().text_xs().font_semibold().child(tr(locale, "panel.sessionInfo")));

    let session_rows: Vec<(&'static str, SharedString, bool)> = vec![
        (tr(locale, "panel.name"), sess_name, false),
        (tr(locale, "panel.id"), SharedString::from(sid), has_session_id),
        (
            tr(locale, "panel.directory"),
            SharedString::from(state.active_cwd.to_string_lossy().into_owned()),
            false,
        ),
        (tr(locale, "panel.messages"), SharedString::from(format_commas(msg_count)), false),
        (tr(locale, "panel.model"), model, false),
        (
            tr(locale, "panel.thinking"),
            SharedString::from(state.thinking_level.label()),
            false,
        ),
    ];

    for (k, v, copyable) in session_rows {
        let mut row = h_flex()
            .justify_between()
            .items_center()
            .gap_2()
            .text_xs()
            .child(div().text_color(cx.theme().muted_foreground).child(k))
            .child(div().flex_1().min_w_0().truncate().text_right().child(v));
        if copyable {
            let sid_c = sid_for_copy.clone();
            row = row.child(
                Button::new("inspector-copy-session-id")
                    .ghost()
                    .xsmall()
                    .compact()
                    .icon(IconName::Copy)
                    .tooltip(copy_tooltip)
                    .on_click(move |_, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(sid_c.clone()));
                        window.push_notification(Notification::success(tr(locale, "common.copied")), cx);
                    }),
            );
        }
        session_card = session_card.child(row);
    }

    let known_window_tokens = stats
        .context_window
        .or_else(|| state.selected_model.as_ref().and_then(|m| m.context_window))
        .filter(|&w| w > 0);

    let est_messages_tokens: u64 = state
        .messages
        .iter()
        .map(|m| {
            let mut chars = m.content.len() + m.thinking.len();
            for tc in &m.tool_calls {
                chars += tc.name.len() + tc.output.len();
                for (k, v) in &tc.arguments {
                    chars += k.len() + v.len();
                }
            }
            ((chars as f64) / 3.5).ceil() as u64
        })
        .sum();
    let est_tools_tokens: u64 = (state.tool_preset.tool_names().len() as u64) * 350;

    let (sys_tokens, tool_tokens, msg_tokens, total_ctx_tokens) = match stats.context_tokens {
        Some(reported) if reported > 0 => {
            let t_tok = est_tools_tokens.min(reported);
            let m_tok = est_messages_tokens.min(reported.saturating_sub(t_tok));
            let s_tok = reported.saturating_sub(t_tok + m_tok);
            (s_tok, t_tok, m_tok, reported)
        }
        _ => {
            let s_tok = if state.selected_model.is_some() { 2_000 } else { 0 };
            let total = s_tok + est_tools_tokens + est_messages_tokens;
            (s_tok, est_tools_tokens, est_messages_tokens, total)
        }
    };
    let free_tokens = known_window_tokens
        .map(|w| w.saturating_sub(total_ctx_tokens))
        .unwrap_or(0);
    let pct_of_win = |v: u64| -> f32 {
        match known_window_tokens {
            Some(w) if w > 0 => ((v as f64 / w as f64) * 100.0).clamp(0.0, 100.0) as f32,
            _ => 0.0,
        }
    };
    let sys_pct = pct_of_win(sys_tokens);
    let tool_pct = pct_of_win(tool_tokens);
    let msg_pct = pct_of_win(msg_tokens);
    let total_pct = pct_of_win(total_ctx_tokens);
    let free_pct = if known_window_tokens.is_some() {
        (100.0 - (sys_pct + tool_pct + msg_pct)).max(0.0)
    } else {
        0.0
    };
    let ctx_breakdown_header = match known_window_tokens {
        Some(w) => format!(
            "{} / {} ({:.1}%)",
            format_tokens_km(total_ctx_tokens),
            format_tokens_km(w),
            total_pct
        ),
        None => format!("{} / …", format_tokens_km(total_ctx_tokens)),
    };

    let context_card = v_flex()
        .p_3()
        .gap_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_xs().font_semibold().child(tr(locale, "panel.contextBreakdown")))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(ctx_breakdown_header),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .h(gpui_kit::px(8.))
                .rounded_full()
                .overflow_hidden()
                .bg(cx.theme().background)
                .children((sys_pct > 0.0).then(|| {
                    div()
                        .h_full()
                        .w(gpui_kit::relative((sys_pct / 100.0).clamp(0.01, 1.0)))
                        .bg(cx.theme().info)
                }))
                .children((tool_pct > 0.0).then(|| {
                    div()
                        .h_full()
                        .w(gpui_kit::relative((tool_pct / 100.0).clamp(0.01, 1.0)))
                        .bg(cx.theme().warning)
                }))
                .children((msg_pct > 0.0).then(|| {
                    div()
                        .h_full()
                        .w(gpui_kit::relative((msg_pct / 100.0).clamp(0.01, 1.0)))
                        .bg(cx.theme().primary)
                })),
        )
        .child(
            v_flex()
                .gap_1()
                .child(context_legend_row(
                    cx.theme().info,
                    tr(locale, "panel.ctxSystemPrompt"),
                    sys_tokens,
                    sys_pct,
                    cx,
                ))
                .child(context_legend_row(
                    cx.theme().warning,
                    tr(locale, "panel.ctxTools"),
                    tool_tokens,
                    tool_pct,
                    cx,
                ))
                .child(context_legend_row(
                    cx.theme().primary,
                    tr(locale, "panel.ctxMessages"),
                    msg_tokens,
                    msg_pct,
                    cx,
                ))
                .child(context_legend_row(
                    cx.theme().muted_foreground.opacity(0.35),
                    tr(locale, "panel.ctxFree"),
                    free_tokens,
                    free_pct,
                    cx,
                )),
        );

    v_flex()
        .size_full()
        .p_3()
        .gap_3()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .child(tr(locale, "panel.session")),
        )
        .child(session_card)
        .child(context_card)
        .child(inspector_card(
            tr(locale, "panel.tokensCost"),
            vec![
                (tr(locale, "panel.input"), SharedString::from(format_tokens_km(stats.input_tokens))),
                (tr(locale, "panel.output"), SharedString::from(format_tokens_km(stats.output_tokens))),
                (
                    tr(locale, "panel.cacheRead"),
                    SharedString::from(format_tokens_km(stats.cache_read_tokens)),
                ),
                (
                    tr(locale, "panel.cacheWrite"),
                    SharedString::from(format_tokens_km(stats.cache_write_tokens)),
                ),
                (tr(locale, "panel.total"), SharedString::from(format_tokens_km(total_tokens))),
                (tr(locale, "panel.cost"), SharedString::from(format!("${:.4}", stats.cost_usd))),
                (tr(locale, "panel.context"), SharedString::from(ctx_summary)),
            ],
            cx,
        ))
        .child(inspector_card(
            tr(locale, "panel.tools"),
            vec![
                (tr(locale, "panel.preset"), SharedString::from(state.tool_preset.label())),
                (
                    tr(locale, "panel.active"),
                    SharedString::from(if tools.is_empty() {
                        tr(locale, "panel.noneChatOnly").to_string()
                    } else {
                        tools
                    }),
                ),
            ],
            cx,
        ))
}

fn context_legend_row(
    dot_color: gpui_kit::Hsla,
    label: &'static str,
    tokens: u64,
    pct: f32,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .text_xs()
        .child(
            h_flex()
                .items_center()
                .gap_1p5()
                .child(
                    div()
                        .size(gpui_kit::px(8.))
                        .rounded_full()
                        .bg(dot_color),
                )
                .child(div().text_color(cx.theme().muted_foreground).child(label)),
        )
        .child(
            div().child(format!("{} ({:.1}%)", format_tokens_km(tokens), pct)),
        )
}

fn inspector_card(
    title: &'static str,
    rows: Vec<(&'static str, SharedString)>,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let mut box_el = v_flex()
        .p_3()
        .gap_1p5()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(div().text_xs().font_semibold().child(title));

    for (k, v) in rows {
        box_el = box_el.child(
            h_flex()
                .justify_between()
                .gap_3()
                .text_xs()
                .child(div().text_color(cx.theme().muted_foreground).child(k))
                .child(div().flex_1().min_w_0().truncate().text_right().child(v)),
        );
    }
    box_el
}
