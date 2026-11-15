use crate::app_state::AppState;
use crate::client::types::{AgyLinkAccount, AgyLinkState};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Context, DefiniteLength, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    px, rgb, rgba,
};
use gpui_kit::prelude::FluentBuilder as _;

fn quota_bar_color(pct: i32, cx: &gpui_kit::App) -> gpui_kit::Hsla {
    if pct > 50 {
        cx.theme().success
    } else if pct > 20 {
        cx.theme().warning
    } else {
        cx.theme().danger
    }
}

/// Compact quota capsule adapted from `AGM-Rust`'s `compact_quota_capsule`.
fn compact_quota_capsule(
    label: &str,
    percentage: i32,
    reset_desc: Option<&str>,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let p = percentage.clamp(0, 100);
    let bar_color = quota_bar_color(p, cx);

    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(26.))
        .px_2()
        .rounded_md()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(
            h_flex()
                .size_full()
                .items_center()
                .justify_between()
                .pb_0p5()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(SharedString::from(label.to_string())),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1p5()
                        .children(reset_desc.map(|r| {
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(r.to_string()))
                        }))
                        .child(
                            div()
                                .text_xs()
                                .font_bold()
                                .text_color(bar_color)
                                .child(SharedString::from(format!("{p}%"))),
                        ),
                ),
        )
        .child(
            div()
                .absolute()
                .bottom(px(2.))
                .left(px(4.))
                .right(px(4.))
                .h(px(2.5))
                .rounded_full()
                .bg(rgba(0x88888833))
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .w(DefiniteLength::Fraction(p as f32 / 100.0))
                        .bg(bar_color),
                ),
        )
}

/// Account Details sub-view adapted from `AGM-Rust`'s `open_details_dialog`.
fn render_account_details(
    acc: &AgyLinkAccount,
    app_state: &Entity<AppState>,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let app_back = app_state.clone();
    let email = acc.email.clone().unwrap_or_else(|| acc.alias.clone());
    let proxy = acc
        .proxy_url
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("system / direct");

    v_flex()
        .gap_3()
        .w_full()
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("agy-details-back")
                                .ghost()
                                .small()
                                .icon(IconName::ArrowLeft)
                                .label("Back")
                                .on_click(move |_, _, cx| {
                                    app_back.update(cx, |s, cx| {
                                        s.agy_link_details_account_id = None;
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(SharedString::from(format!("Account Details: {email}"))),
                        ),
                ),
        )
        .child(
            h_flex()
                .gap_4()
                .p_2p5()
                .rounded_lg()
                .bg(cx.theme().secondary)
                .border_1()
                .border_color(cx.theme().border)
                .text_xs()
                .child(div().child(SharedString::from(format!("Tier: {}", acc.tier))))
                .child(div().child(SharedString::from(format!("Priority: {}", acc.priority))))
                .child(div().truncate().child(SharedString::from(format!("Proxy: {proxy}")))),
        )
        .child(
            div()
                .text_xs()
                .font_bold()
                .text_color(cx.theme().foreground)
                .child(SharedString::from(format!(
                    "Official Sub-models & Quotas ({} total)",
                    acc.models.len()
                ))),
        )
        .child(
            v_flex()
                .id("agy-details-models-scroll")
                .gap_1()
                .max_h(px(280.))
                .overflow_y_scroll()
                .children(acc.models.iter().map(|m| {
                    let color = quota_bar_color(m.percentage, cx);
                    h_flex()
                        .items_center()
                        .justify_between()
                        .py_1p5()
                        .px_2p5()
                        .rounded_md()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_xs()
                        .child(
                            div()
                                .font_family("Menlo")
                                .text_color(cx.theme().foreground)
                                .child(SharedString::from(m.name.clone())),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .children(m.reset_time.as_ref().map(|rt| {
                                    div()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(SharedString::from(rt.clone()))
                                }))
                                .child(
                                    div()
                                        .font_bold()
                                        .text_color(color)
                                        .child(SharedString::from(format!("{}%", m.percentage))),
                                ),
                        )
                })),
        )
}

fn render_account_card(
    acc: &AgyLinkAccount,
    idx: usize,
    app_state: &Entity<AppState>,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let email = acc.email.clone().unwrap_or_else(|| acc.alias.clone());
    let acc_id_switch = acc.id.clone();
    let acc_id_details = acc.id.clone();
    let acc_id_remove = acc.id.clone();
    let app_switch = app_state.clone();
    let app_details = app_state.clone();
    let app_remove = app_state.clone();

    v_flex()
        .w_full()
        .p_3()
        .gap_2()
        .rounded_lg()
        .bg(if acc.active {
            cx.theme().secondary
        } else {
            cx.theme().background
        })
        .border_1()
        .border_color(if acc.active {
            cx.theme().accent
        } else {
            cx.theme().border
        })
        .child(
            h_flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .truncate()
                                .text_color(cx.theme().foreground)
                                .child(SharedString::from(email)),
                        )
                        .child(Tag::secondary().small().child(SharedString::from(acc.tier.clone())))
                        .when(acc.active, |el| {
                            el.child(Tag::primary().small().child("Active"))
                        })
                        .when(acc.auth_required, |el| {
                            el.child(Tag::danger().small().child("Reauth"))
                        })
                        .when(acc.cooldown_until.is_some(), |el| {
                            el.child(Tag::warning().small().child("Cooldown"))
                        }),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1()
                        .when(!acc.active, |el| {
                            el.child(
                                Button::new(("agy-switch", idx))
                                    .outline()
                                    .xsmall()
                                    .label("Use")
                                    .on_click(move |_, _, cx| {
                                        let id = acc_id_switch.clone();
                                        app_switch.update(cx, |s, cx| {
                                            s.send_agy_command(format!("/agy switch {id}"), cx);
                                        });
                                    }),
                            )
                        })
                        .child(
                            Button::new(("agy-details", idx))
                                .ghost()
                                .xsmall()
                                .label("Details")
                                .on_click(move |_, _, cx| {
                                    let id = acc_id_details.clone();
                                    app_details.update(cx, |s, cx| {
                                        s.agy_link_details_account_id = Some(id);
                                        cx.notify();
                                    });
                                }),
                        )
                        .when(!acc.system_home, |el| {
                            el.child(
                                Button::new(("agy-remove", idx))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Delete)
                                    .on_click(move |_, _, cx| {
                                        let id = acc_id_remove.clone();
                                        app_remove.update(cx, |s, cx| {
                                            s.send_agy_command(format!("/agy remove {id}"), cx);
                                        });
                                    }),
                            )
                        }),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .children(acc.five_hour_pct.map(|p| {
                    compact_quota_capsule("Gemini 5H", p, acc.five_hour_reset.as_deref(), cx)
                }))
                .children(acc.weekly_pct.map(|p| {
                    compact_quota_capsule("Gemini 7D", p, acc.weekly_reset.as_deref(), cx)
                }))
                .children(acc.argon_pct.map(|p| {
                    compact_quota_capsule("Argon 7D", p, acc.argon_reset.as_deref(), cx)
                })),
        )
}

pub fn render_agy_link_overlay(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let (is_open, agy_state, details_id) = {
        let s = app_state.read(cx);
        (
            s.agy_link_modal_open,
            s.agy_link_state.clone(),
            s.agy_link_details_account_id.clone(),
        )
    };
    if !is_open {
        return div().into_any_element();
    }

    let state = agy_state.unwrap_or_else(AgyLinkState::default);
    let app_backdrop = app_state.clone();
    let app_close = app_state.clone();
    let app_add = app_state.clone();
    let app_refresh = app_state.clone();

    let details_acc = details_id
        .as_ref()
        .and_then(|id| state.accounts.iter().find(|a| &a.id == id));

    div()
        .id("agy-link-modal-backdrop")
        .absolute()
        .inset_0()
        .bg(rgba(0x00000066))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            app_backdrop.update(cx, |s, cx| s.close_agy_link_modal(cx));
        })
        .child(
            v_flex()
                .id("agy-link-modal-card")
                .w(px(640.))
                .max_h(px(560.))
                .p_4()
                .gap_3()
                .rounded_xl()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .shadow_xl()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(IconName::Bot).small())
                                .child(
                                    div()
                                        .text_base()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child("Antigravity (agy-link) Pool"),
                                )
                                .children(state.version.as_ref().map(|v| {
                                    Tag::secondary().small().child(SharedString::from(format!("v{v}")))
                                })),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    Button::new("agy-add-account-btn")
                                        .primary()
                                        .small()
                                        .icon(IconName::Plus)
                                        .label("Add Account (OAuth)")
                                        .on_click(move |_, _, cx| {
                                            app_add.update(cx, |s, cx| {
                                                s.send_agy_command("/agy add".to_string(), cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("agy-refresh-quota-btn")
                                        .outline()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .label("Refresh")
                                        .on_click(move |_, _, cx| {
                                            app_refresh.update(cx, |s, cx| {
                                                s.send_agy_command("/agy refresh".to_string(), cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("agy-close-modal-btn")
                                        .ghost()
                                        .small()
                                        .icon(IconName::Close)
                                        .on_click(move |_, _, cx| {
                                            app_close.update(cx, |s, cx| s.close_agy_link_modal(cx));
                                        }),
                                ),
                        ),
                )
                .children(state.dormant_reason.as_ref().map(|reason| {
                    div()
                        .p_2()
                        .rounded_md()
                        .bg(rgb(0x451a1a))
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(SharedString::from(format!("Dormant: {reason}")))
                }))
                .child(if let Some(acc) = details_acc {
                    render_account_details(acc, app_state, cx).into_any_element()
                } else if state.accounts.is_empty() {
                    div()
                        .py_8()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No Antigravity accounts yet. Click 'Add Account (OAuth)' to sign in.")
                        .into_any_element()
                } else {
                    v_flex()
                        .id("agy-accounts-scroll")
                        .gap_2()
                        .max_h(px(420.))
                        .overflow_y_scroll()
                        .children(
                            state
                                .accounts
                                .iter()
                                .enumerate()
                                .map(|(i, acc)| render_account_card(acc, i, app_state, cx)),
                        )
                        .into_any_element()
                }),
        )
        .into_any_element()
}
