use crate::app_state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px, rgba,
};

/// Fixed-height modal dialog showing official sub-models & quotas for one Antigravity account.
pub fn render_agy_link_overlay(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let (details_id, agy_state_opt) = {
        let s = app_state.read(cx);
        (s.agy_link_details_account_id.clone(), s.agy_link_state.clone())
    };
    let Some(target_id) = details_id else {
        return div().into_any_element();
    };
    let Some(state) = agy_state_opt else {
        return div().into_any_element();
    };
    let Some(acc) = state.accounts.iter().find(|a| a.id == target_id) else {
        return div().into_any_element();
    };

    let email = acc.email.clone().unwrap_or_else(|| acc.alias.clone());
    let proxy = acc
        .proxy_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("system / direct");

    let app_backdrop = app_state.clone();
    let app_close = app_state.clone();

    div()
        .id("agy-models-modal-backdrop")
        .occlude()
        .absolute()
        .inset_0()
        .bg(rgba(0x00000066))
        .flex()
        .items_center()
        .justify_center()
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            app_backdrop.update(cx, |s, cx| {
                s.agy_link_details_account_id = None;
                cx.notify();
            });
        })
        .child(
            v_flex()
                .id("agy-models-modal-card")
                .occlude()
                .w(px(560.))
                .h(px(460.))
                .p_4()
                .gap_3()
                .rounded_xl()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .shadow_xl()
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .min_w_0()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .truncate()
                                        .text_color(cx.theme().foreground)
                                        .child(SharedString::from(email)),
                                )
                                .child(Tag::secondary().small().child(SharedString::from(acc.tier.clone()))),
                        )
                        .child(
                            Button::new("agy-models-modal-close")
                                .ghost()
                                .small()
                                .h(px(32.))
                                .w(px(32.))
                                .icon(IconName::Close)
                                .on_click(move |_, _, cx| {
                                    app_close.update(cx, |s, cx| {
                                        s.agy_link_details_account_id = None;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(div().child(SharedString::from(format!("Priority: {}", acc.priority))))
                        .child(div().truncate().child(SharedString::from(format!("Proxy: {proxy}"))))
                        .child(div().child(SharedString::from(format!("{} models", acc.models.len())))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_h_0()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .overflow_hidden()
                        .child(
                            h_flex()
                                .px_3()
                                .py_1p5()
                                .bg(cx.theme().secondary)
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child(div().flex_1().child("Model"))
                                .child(div().w(px(120.)).child("Reset"))
                                .child(div().w(px(64.)).text_right().child("Quota")),
                        )
                        .child(
                            v_flex()
                                .id("agy-models-modal-scroll")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .children(acc.models.iter().map(|m| {
                                    let pct = m.percentage.clamp(0, 100);
                                    let color = if pct > 50 {
                                        cx.theme().success
                                    } else if pct > 20 {
                                        cx.theme().warning
                                    } else {
                                        cx.theme().danger
                                    };
                                    h_flex()
                                        .items_center()
                                        .px_3()
                                        .py_1p5()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .text_xs()
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .font_family("Menlo")
                                                .text_color(cx.theme().foreground)
                                                .child(SharedString::from(m.name.clone())),
                                        )
                                        .child(
                                            div()
                                                .w(px(120.))
                                                .truncate()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(
                                                    m.reset_time.clone().unwrap_or_else(|| "—".to_string()),
                                                )),
                                        )
                                        .child(
                                            div()
                                                .w(px(64.))
                                                .text_right()
                                                .font_bold()
                                                .text_color(color)
                                                .child(SharedString::from(format!("{pct}%"))),
                                        )
                                })),
                        ),
                ),
        )
        .into_any_element()
}
