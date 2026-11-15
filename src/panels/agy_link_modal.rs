use crate::app_state::AppState;
use crate::client::types::{AgyLinkAccount, AgyLinkSettings, AgyLinkState};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, DefiniteLength, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, rgb, rgba,
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
                        .when(acc.disabled, |el| {
                            el.child(Tag::secondary().small().child("Disabled"))
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
                        .when(!acc.active && !acc.disabled, |el| {
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
                        .child({
                            let app_toggle = app_state.clone();
                            let acc_id_toggle = acc.id.clone();
                            let is_disabled = acc.disabled;
                            Button::new(("agy-toggle-enable", idx))
                                .ghost()
                                .xsmall()
                                .label(if is_disabled { "Enable" } else { "Disable" })
                                .on_click(move |_, _, cx| {
                                    let id = acc_id_toggle.clone();
                                    let cmd = if is_disabled {
                                        format!("/agy enable {id}")
                                    } else {
                                        format!("/agy disable {id}")
                                    };
                                    app_toggle.update(cx, |s, cx| {
                                        s.send_agy_command(cmd, cx);
                                    });
                                })
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

fn render_agy_settings_tab(
    settings: &AgyLinkSettings,
    proxy_input: &Entity<InputState>,
    app_state: &Entity<AppState>,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let is_round_robin = settings.mode == "round-robin";
    let app_seq = app_state.clone();
    let app_rr = app_state.clone();
    let app_cd = app_state.clone();
    let app_proxy_save = app_state.clone();
    let app_proxy_clear = app_state.clone();
    let proxy_input_save = proxy_input.clone();
    let proxy_input_clear = proxy_input.clone();

    let app_perm_skip = app_state.clone();
    let app_perm_edit = app_state.clone();
    let app_perm_plan = app_state.clone();
    let app_eff_def = app_state.clone();
    let app_eff_low = app_state.clone();
    let app_eff_med = app_state.clone();
    let app_eff_high = app_state.clone();
    let app_par = app_state.clone();
    let parallel_on = settings.parallel_same_account;
    let app_sup = app_state.clone();
    let suppress_on = settings.suppress_agy_auto_update;
    let app_mc0 = app_state.clone();
    let app_mc2 = app_state.clone();
    let app_mc4 = app_state.clone();
    let app_mc8 = app_state.clone();

    v_flex()
        .id("agy-settings-scroll")
        .gap_3()
        .max_h(px(420.))
        .overflow_y_scroll()
        // 1. Auto-Switch & Pool Rotation (from AGM-Rust SettingsView)
        .child(
            v_flex()
                .p_3()
                .gap_2p5()
                .rounded_lg()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            v_flex()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(cx.theme().foreground)
                                        .child("Auto-Switch & Pool Rotation"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Sequential sticks to primary and rotates on 429/quota depletion; Round-Robin balances across ready accounts."),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("agy-mode-seq")
                                        .xsmall()
                                        .map(|b| if !is_round_robin { b.primary() } else { b.outline() })
                                        .label("Sequential")
                                        .on_click(move |_, _, cx| {
                                            app_seq.update(cx, |s, cx| {
                                                s.send_agy_command("/agy mode sequential".to_string(), cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("agy-mode-rr")
                                        .xsmall()
                                        .map(|b| if is_round_robin { b.primary() } else { b.outline() })
                                        .label("Round-Robin")
                                        .on_click(move |_, _, cx| {
                                            app_rr.update(cx, |s, cx| {
                                                s.send_agy_command("/agy mode round-robin".to_string(), cx);
                                            });
                                        }),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .pt_1()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Backoff: Exponential + Jitter · Circuit Breaker: Auto-suspend 429/exhausted slots"),
                        )
                        .child(
                            Button::new("agy-clear-cooldowns")
                                .outline()
                                .xsmall()
                                .icon(IconName::RefreshCw)
                                .label("Clear Cooldowns")
                                .on_click(move |_, _, cx| {
                                    app_cd.update(cx, |s, cx| {
                                        s.send_agy_command("/agy clear-cooldown".to_string(), cx);
                                    });
                                }),
                        ),
                ),
        )
        // 2. Proxy & Network (from AGM-Rust SettingsView)
        .child(
            v_flex()
                .p_3()
                .gap_2()
                .rounded_lg()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child("Proxy & Network"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Upstream SOCKS5 / HTTP Proxy (e.g. socks5://127.0.0.1:7890 or http://127.0.0.1:7890). Leave blank to use system proxy."),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().flex_1().child(Input::new(proxy_input).small()))
                        .child(
                            Button::new("agy-proxy-save")
                                .primary()
                                .small()
                                .label("Save")
                                .on_click(move |_, _, cx| {
                                    let val = proxy_input_save.read(cx).value().trim().to_string();
                                    let cmd = if val.is_empty() {
                                        "/agy proxy-global clear".to_string()
                                    } else {
                                        format!("/agy proxy-global {val}")
                                    };
                                    app_proxy_save.update(cx, |s, cx| {
                                        s.send_agy_command(cmd, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("agy-proxy-clear")
                                .outline()
                                .small()
                                .label("Clear")
                                .on_click(move |_, window, cx| {
                                    proxy_input_clear.update(cx, |inp, cx| {
                                        inp.set_value("", window, cx);
                                    });
                                    app_proxy_clear.update(cx, |s, cx| {
                                        s.send_agy_command("/agy proxy-global clear".to_string(), cx);
                                    });
                                }),
                        ),
                ),
        )
        // 3. Execution, Thinking & Concurrency
        .child(
            v_flex()
                .p_3()
                .gap_2p5()
                .rounded_lg()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(cx.theme().foreground)
                        .child("Execution & Concurrency"),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child("Permission Mode"),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("agy-perm-skip")
                                        .xsmall()
                                        .map(|b| if settings.permission_mode == "skip" { b.primary() } else { b.outline() })
                                        .label("skip")
                                        .on_click(move |_, _, cx| {
                                            app_perm_skip.update(cx, |s, cx| s.send_agy_command("/agy permission skip".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-perm-accept")
                                        .xsmall()
                                        .map(|b| if settings.permission_mode == "accept-edits" { b.primary() } else { b.outline() })
                                        .label("accept-edits")
                                        .on_click(move |_, _, cx| {
                                            app_perm_edit.update(cx, |s, cx| s.send_agy_command("/agy permission accept-edits".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-perm-plan")
                                        .xsmall()
                                        .map(|b| if settings.permission_mode == "plan" { b.primary() } else { b.outline() })
                                        .label("plan")
                                        .on_click(move |_, _, cx| {
                                            app_perm_plan.update(cx, |s, cx| s.send_agy_command("/agy permission plan".to_string(), cx));
                                        }),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child("Default Thinking Effort"),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("agy-eff-def")
                                        .xsmall()
                                        .map(|b| if settings.default_effort.is_empty() { b.primary() } else { b.outline() })
                                        .label("default")
                                        .on_click(move |_, _, cx| {
                                            app_eff_def.update(cx, |s, cx| s.send_agy_command("/agy effort default".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-eff-low")
                                        .xsmall()
                                        .map(|b| if settings.default_effort == "low" { b.primary() } else { b.outline() })
                                        .label("low")
                                        .on_click(move |_, _, cx| {
                                            app_eff_low.update(cx, |s, cx| s.send_agy_command("/agy effort low".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-eff-med")
                                        .xsmall()
                                        .map(|b| if settings.default_effort == "medium" { b.primary() } else { b.outline() })
                                        .label("medium")
                                        .on_click(move |_, _, cx| {
                                            app_eff_med.update(cx, |s, cx| s.send_agy_command("/agy effort medium".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-eff-high")
                                        .xsmall()
                                        .map(|b| if settings.default_effort == "high" { b.primary() } else { b.outline() })
                                        .label("high")
                                        .on_click(move |_, _, cx| {
                                            app_eff_high.update(cx, |s, cx| s.send_agy_command("/agy effort high".to_string(), cx));
                                        }),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child("Same-Account Parallel Sessions"),
                        )
                        .child(
                            Button::new("agy-parallel-toggle")
                                .xsmall()
                                .map(|b| if parallel_on { b.primary() } else { b.outline() })
                                .label(if parallel_on { "Enabled" } else { "Disabled" })
                                .on_click(move |_, _, cx| {
                                    let cmd = if parallel_on { "/agy parallel off" } else { "/agy parallel on" };
                                    app_par.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                                }),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child("Max Concurrent Streams"),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("agy-mc-0")
                                        .xsmall()
                                        .map(|b| if settings.max_concurrent == 0 { b.primary() } else { b.outline() })
                                        .label("Unlimited")
                                        .on_click(move |_, _, cx| {
                                            app_mc0.update(cx, |s, cx| s.send_agy_command("/agy max-concurrent 0".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-mc-2")
                                        .xsmall()
                                        .map(|b| if settings.max_concurrent == 2 { b.primary() } else { b.outline() })
                                        .label("2")
                                        .on_click(move |_, _, cx| {
                                            app_mc2.update(cx, |s, cx| s.send_agy_command("/agy max-concurrent 2".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-mc-4")
                                        .xsmall()
                                        .map(|b| if settings.max_concurrent == 4 { b.primary() } else { b.outline() })
                                        .label("4")
                                        .on_click(move |_, _, cx| {
                                            app_mc4.update(cx, |s, cx| s.send_agy_command("/agy max-concurrent 4".to_string(), cx));
                                        }),
                                )
                                .child(
                                    Button::new("agy-mc-8")
                                        .xsmall()
                                        .map(|b| if settings.max_concurrent == 8 { b.primary() } else { b.outline() })
                                        .label("8")
                                        .on_click(move |_, _, cx| {
                                            app_mc8.update(cx, |s, cx| s.send_agy_command("/agy max-concurrent 8".to_string(), cx));
                                        }),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().foreground)
                                .child("Suppress agy CLI Auto-Update"),
                        )
                        .child(
                            Button::new("agy-suppress-update")
                                .xsmall()
                                .map(|b| if suppress_on { b.primary() } else { b.outline() })
                                .label(if suppress_on { "Enabled" } else { "Disabled" })
                                .on_click(move |_, _, cx| {
                                    let cmd = if suppress_on { "/agy suppress-update off" } else { "/agy suppress-update on" };
                                    app_sup.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                                }),
                        ),
                ),
        )
}

pub fn render_agy_link_overlay(
    app_state: &Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let (is_open, agy_state_opt, details_id, active_tab, is_loading) = {
        let s = app_state.read(cx);
        (
            s.agy_link_modal_open,
            s.agy_link_state.clone(),
            s.agy_link_details_account_id.clone(),
            s.agy_link_modal_tab,
            s.agy_link_loading,
        )
    };
    if !is_open {
        return div().into_any_element();
    }

    let initial_proxy = agy_state_opt
        .as_ref()
        .map(|s| s.settings.proxy_url.clone())
        .unwrap_or_default();
    let proxy_input = {
        let existing = app_state.read(cx).agy_link_proxy_input.clone();
        match existing {
            Some(inp) => inp,
            None => app_state.update(cx, |s, cx| {
                let inp = cx.new(|cx| {
                    let mut st = InputState::new(window, cx).placeholder("socks5://127.0.0.1:7890");
                    if !initial_proxy.is_empty() {
                        st.set_value(&initial_proxy, window, cx);
                    }
                    st
                });
                s.agy_link_proxy_input = Some(inp.clone());
                inp
            }),
        }
    };

    let not_yet_loaded = agy_state_opt.is_none();
    let state = agy_state_opt.unwrap_or_else(AgyLinkState::default);
    let app_backdrop = app_state.clone();
    let app_close = app_state.clone();
    let app_add = app_state.clone();
    let app_refresh = app_state.clone();
    let app_tab = app_state.clone();

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
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            app_backdrop.update(cx, |s, cx| s.close_agy_link_modal(cx));
        })
        .child(
            v_flex()
                .id("agy-link-modal-card")
                .occlude()
                .w(px(680.))
                .max_h(px(620.))
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
                                        .child("Antigravity (pi-agy-link)"),
                                )
                                .children(state.version.as_ref().map(|v| {
                                    Tag::secondary().small().child(SharedString::from(format!("v{v}")))
                                }))
                                .when(is_loading, |el| {
                                    el.child(
                                        Spinner::new()
                                            .xsmall()
                                            .icon(Icon::new(crate::icons::LoaderCircleIcon)),
                                    )
                                }),
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
                .child(
                    TabBar::new("agy-modal-top-tabs")
                        .segmented()
                        .small()
                        .selected_index(active_tab)
                        .on_click(move |idx, _, cx| {
                            let next = *idx;
                            app_tab.update(cx, |s, cx| {
                                s.agy_link_modal_tab = next;
                                cx.notify();
                            });
                        })
                        .child(Tab::new().label("Accounts & Quotas"))
                        .child(Tab::new().label("Settings")),
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
                .child(if active_tab == 1 {
                    render_agy_settings_tab(&state.settings, &proxy_input, app_state, cx)
                        .into_any_element()
                } else if let Some(acc) = details_acc {
                    render_account_details(acc, app_state, cx).into_any_element()
                } else if not_yet_loaded || (is_loading && state.accounts.is_empty()) {
                    h_flex()
                        .py_8()
                        .gap_2()
                        .items_center()
                        .justify_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(Spinner::new().small().icon(Icon::new(crate::icons::LoaderCircleIcon)))
                        .child("Loading Antigravity pool & quotas…")
                        .into_any_element()
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
