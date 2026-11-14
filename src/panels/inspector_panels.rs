use crate::app_state::AppState;
use crate::i18n::{format_commas, format_tokens_km, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
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
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(sid_c.clone()));
                    }),
            );
        }
        session_card = session_card.child(row);
    }

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
