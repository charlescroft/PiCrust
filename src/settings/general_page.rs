use crate::app_state::AppState;
use crate::client::native_index::user_home;
use crate::client::types::SendShortcut;
use crate::i18n::{AppLocale, tr};
use crate::settings::{SETTINGS_BTN_W, SETTINGS_DROPDOWN_W, config_path_field};
use crate::themes::{dark_theme_names, light_theme_names};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    App, Axis, Entity, ParentElement as _, SharedString, Styled as _, div, px,
};
use serde_json::json;

fn option_list<T, F>(items: &[T], label: F) -> Vec<(SharedString, SharedString)>
where
    F: Fn(&T) -> SharedString,
{
    items
        .iter()
        .map(|item| {
            let text = label(item);
            (text.clone(), text)
        })
        .collect()
}

pub fn general_setting_page(app_state: Entity<AppState>, locale: AppLocale, _cx: &App) -> SettingPage {
    // Appearance
    let app_lang_read = app_state.clone();
    let app_lang_write = app_state.clone();
    let app_theme_read = app_state.clone();
    let app_theme_write = app_state.clone();
    let app_light_read = app_state.clone();
    let app_light_write = app_state.clone();
    let app_dark_read = app_state.clone();
    let app_dark_write = app_state.clone();
    let app_chat_font = app_state.clone();
    let app_ui_font = app_state.clone();
    let app_preview_font = app_state.clone();
    let app_chat_width = app_state.clone();
    // Chat & display
    let app_sc_read = app_state.clone();
    let app_sc_write = app_state.clone();
    let app_collapse_read = app_state.clone();
    let app_collapse_write = app_state.clone();
    let app_think_def_read = app_state.clone();
    let app_think_def_write = app_state.clone();
    let app_art_read = app_state.clone();
    let app_art_write = app_state.clone();
    let app_title_read = app_state.clone();
    let app_title_write = app_state.clone();
    // Notifications
    let app_notify_read = app_state.clone();
    let app_notify_write = app_state.clone();
    let app_sound_read = app_state.clone();
    let app_sound_write = app_state.clone();
    // Pi Core
    let app_core_read = app_state.clone();
    let app_core_write = app_state.clone();
    let app_core_url_read = app_state.clone();
    let app_core_url_write = app_state.clone();
    let app_core_view = app_state.clone();
    let app_core_manual = app_state.clone();
    let app_core_status = app_state.clone();
    let app_core_status_btn = app_state.clone();
    // Sidecar / network
    let app_sidecar = app_state.clone();
    let app_sidecar_btn = app_state.clone();
    let app_proxy_read = app_state.clone();
    let app_proxy_write = app_state.clone();
    let app_proxy_url_read = app_state.clone();
    let app_proxy_url_write = app_state.clone();

    let lang_options: Vec<(SharedString, SharedString)> = AppLocale::all()
        .iter()
        .map(|&loc| (SharedString::from(loc.code()), SharedString::from(loc.label())))
        .collect();



    let send_options: Vec<(SharedString, SharedString)> = SendShortcut::all()
        .iter()
        .map(|&sc| {
            let label = match sc {
                SendShortcut::Enter => tr(locale, "settings.sendOptionEnter"),
                SendShortcut::CmdEnter => {
                    #[cfg(target_os = "macos")]
                    {
                        tr(locale, "settings.sendOptionCmdEnter")
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        tr(locale, "settings.sendOptionCtrlEnter")
                    }
                }
                SendShortcut::ShiftEnter => tr(locale, "settings.sendOptionShiftEnter"),
            };
            (SharedString::from(sc.as_str()), SharedString::from(label))
        })
        .collect();

    let collapse_options: Vec<(SharedString, SharedString)> = vec![
        (
            SharedString::from("0"),
            SharedString::from(tr(locale, "settings.userCollapseOff")),
        ),
        (SharedString::from("150"), SharedString::from("150")),
        (SharedString::from("300"), SharedString::from("300")),
        (SharedString::from("500"), SharedString::from("500")),
        (SharedString::from("1000"), SharedString::from("1000")),
    ];

    let core_options: Vec<(SharedString, SharedString)> = vec![
        (
            SharedString::from("auto"),
            SharedString::from(tr(locale, "settings.piCoreAuto")),
        ),
        (
            SharedString::from("manual"),
            SharedString::from(tr(locale, "settings.piCoreManual")),
        ),
    ];

    let light_options = option_list(&light_theme_names(), |n| (*n).clone());
    let dark_options = option_list(&dark_theme_names(), |n| (*n).clone());

    let appearance_group = SettingGroup::new()
        .title(tr(locale, "settings.appearance"))
        .item(SettingItem::new(
            tr(locale, "settings.language"),
            SettingField::dropdown(
                lang_options,
                move |cx: &App| SharedString::from(app_lang_read.read(cx).locale.code()),
                move |val: SharedString, cx: &mut App| {
                    let next = AppLocale::from_code(val.as_ref());
                    app_lang_write.update(cx, |s, cx| s.set_locale(next, cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.darkMode"),
            SettingField::render(move |_, _, cx| {
                let current_mode = app_theme_read.read(cx).theme_mode.clone();
                let app_t1 = app_theme_write.clone();
                let app_t2 = app_theme_write.clone();
                let app_t3 = app_theme_write.clone();
                div()
                    .w(SETTINGS_DROPDOWN_W)
                    .child(
                        TabBar::new("theme-mode-tabs")
                            .segmented()
                            .w_full()
                            .h(px(32.))
                            .child(
                                Tab::new()
                                    .flex_1()
                                    .label(tr(locale, "settings.themeDark"))
                                    .selected(current_mode == "dark")
                                    .on_click(move |_, _, cx| {
                                        app_t1.update(cx, |s, cx| s.set_theme_mode("dark".to_string(), cx));
                                    }),
                            )
                            .child(
                                Tab::new()
                                    .flex_1()
                                    .label(tr(locale, "settings.themeLight"))
                                    .selected(current_mode == "light")
                                    .on_click(move |_, _, cx| {
                                        app_t2.update(cx, |s, cx| s.set_theme_mode("light".to_string(), cx));
                                    }),
                            )
                            .child(
                                Tab::new()
                                    .flex_1()
                                    .label(tr(locale, "settings.themeSystem"))
                                    .selected(current_mode == "system")
                                    .on_click(move |_, _, cx| {
                                        app_t3.update(cx, |s, cx| s.set_theme_mode("system".to_string(), cx));
                                    }),
                            ),
                    )
            }),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.lightTheme"),
            SettingField::scrollable_dropdown(
                light_options,
                move |cx: &App| app_light_read.read(cx).light_theme_name.clone(),
                move |val: SharedString, cx: &mut App| {
                    app_light_write.update(cx, |s, cx| s.set_light_theme(val, cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.darkTheme"),
            SettingField::scrollable_dropdown(
                dark_options,
                move |cx: &App| app_dark_read.read(cx).dark_theme_name.clone(),
                move |val: SharedString, cx: &mut App| {
                    app_dark_write.update(cx, |s, cx| s.set_dark_theme(val, cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.uiFontSize"),
            SettingField::render(move |_, _, cx| {
                let s = app_ui_font.read(cx);
                let val = s.ui_font_size;
                let slider = s.ui_font_slider.clone();
                let app_reset = app_ui_font.clone();
                h_flex()
                    .w(SETTINGS_DROPDOWN_W)
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .w(px(48.))
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground)
                            .child(format!("{val:.0}px")),
                    )
                    .child(div().flex_1().child(Slider::new(&slider)))
                    .child(
                        Button::new("reset-ui-font")
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w(px(26.))
                            .icon(IconName::RotateCw)
                            .disabled(val == 13.0)
                            .tooltip(tr(locale, "settings.reset"))
                            .on_click(move |_, window, cx| {
                                app_reset.update(cx, |s, cx| {
                                    s.set_ui_font_size(13.0, cx);
                                    s.ui_font_slider.update(cx, |st, cx| st.set_value(13.0, window, cx));
                                });
                            }),
                    )
            }),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.chatFontSize"),
            SettingField::render(move |_, _, cx| {
                let s = app_chat_font.read(cx);
                let val = s.chat_font_size;
                let slider = s.chat_font_slider.clone();
                let app_reset = app_chat_font.clone();
                h_flex()
                    .w(SETTINGS_DROPDOWN_W)
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .w(px(48.))
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground)
                            .child(format!("{val:.0}px")),
                    )
                    .child(div().flex_1().child(Slider::new(&slider)))
                    .child(
                        Button::new("reset-chat-font")
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w(px(26.))
                            .icon(IconName::RotateCw)
                            .disabled(val == 13.0)
                            .tooltip(tr(locale, "settings.reset"))
                            .on_click(move |_, window, cx| {
                                app_reset.update(cx, |s, cx| {
                                    s.set_chat_font_size(13.0, cx);
                                    s.chat_font_slider.update(cx, |st, cx| st.set_value(13.0, window, cx));
                                });
                            }),
                    )
            }),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.previewFontSize"),
            SettingField::render(move |_, _, cx| {
                let s = app_preview_font.read(cx);
                let val = s.preview_font_size;
                let slider = s.preview_font_slider.clone();
                let app_reset = app_preview_font.clone();
                h_flex()
                    .w(SETTINGS_DROPDOWN_W)
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .w(px(56.))
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground)
                            .child(format!("{val:.1}px")),
                    )
                    .child(div().flex_1().child(Slider::new(&slider)))
                    .child(
                        Button::new("reset-preview-font")
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w(px(26.))
                            .icon(IconName::RotateCw)
                            .disabled(val == 13.0)
                            .tooltip(tr(locale, "settings.reset"))
                            .on_click(move |_, window, cx| {
                                app_reset.update(cx, |s, cx| {
                                    s.set_preview_font_size(13.0, cx);
                                    s.preview_font_slider.update(cx, |st, cx| st.set_value(13.0, window, cx));
                                });
                            }),
                    )
            }),
        ));

    let chat_group = SettingGroup::new()
        .title(tr(locale, "settings.chatPrefs"))
        .item(SettingItem::new(
            tr(locale, "settings.chatContentWidth"),
            SettingField::render(move |_, _, cx| {
                let s = app_chat_width.read(cx);
                let val = s.chat_content_width;
                let slider = s.chat_width_slider.clone();
                let app_reset = app_chat_width.clone();
                h_flex()
                    .w(SETTINGS_DROPDOWN_W)
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .w(px(56.))
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(cx.theme().foreground)
                            .child(format!("{val:.0}px")),
                    )
                    .child(div().flex_1().child(Slider::new(&slider)))
                    .child(
                        Button::new("reset-chat-width")
                            .ghost()
                            .small()
                            .h(px(26.))
                            .w(px(26.))
                            .icon(IconName::RotateCw)
                            .disabled(val == 840.0)
                            .tooltip(tr(locale, "settings.reset"))
                            .on_click(move |_, window, cx| {
                                app_reset.update(cx, |s, cx| {
                                    s.set_chat_content_width(840.0, cx);
                                    s.chat_width_slider.update(cx, |st, cx| st.set_value(840.0, window, cx));
                                });
                            }),
                    )
            }),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.sendShortcut"),
            SettingField::dropdown(
                send_options,
                move |cx: &App| SharedString::from(app_sc_read.read(cx).send_shortcut.as_str()),
                move |val: SharedString, cx: &mut App| {
                    let next = SendShortcut::from_str(val.as_ref());
                    app_sc_write.update(cx, |s, cx| s.set_send_shortcut(next, cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.userCollapseChars"),
            SettingField::dropdown(
                collapse_options,
                move |cx: &App| {
                    SharedString::from(app_collapse_read.read(cx).user_collapse_chars.to_string())
                },
                move |val: SharedString, cx: &mut App| {
                    let parsed = val.parse::<usize>().unwrap_or(300);
                    app_collapse_write.update(cx, |s, cx| s.set_user_collapse_chars(parsed, cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.thinkingExpandedDefault"),
            SettingField::switch(
                move |cx: &App| app_think_def_read.read(cx).thinking_expanded_default,
                move |val: bool, cx: &mut App| {
                    app_think_def_write.update(cx, |s, cx| s.set_thinking_expanded_default(val, cx));
                },
            ),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.artifactsInPanel"),
            SettingField::switch(
                move |cx: &App| app_art_read.read(cx).artifacts_prompt_enabled,
                move |val: bool, cx: &mut App| {
                    app_art_write.update(cx, |s, cx| {
                        s.artifacts_prompt_enabled = val;
                        if s.sidecar_connected {
                            let client = s.client.clone();
                            cx.spawn(async move |_, _| {
                                let _ = client.set_artifacts_settings(val).await;
                            })
                            .detach();
                        }
                        cx.notify();
                    });
                },
            ),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.autoTitle"),
            SettingField::switch(
                move |cx: &App| app_title_read.read(cx).auto_title_enabled,
                move |val: bool, cx: &mut App| {
                    app_title_write.update(cx, |s, cx| {
                        s.auto_title_enabled = val;
                        if s.sidecar_connected {
                            let client = s.client.clone();
                            cx.spawn(async move |_, _| {
                                let _ = client.set_session_title_settings(val).await;
                            })
                            .detach();
                        }
                        cx.notify();
                    });
                },
            ),
        ));

    let notify_group = SettingGroup::new()
        .title(tr(locale, "settings.notifications"))
        .item(SettingItem::new(
            tr(locale, "settings.notifyEnabled"),
            SettingField::switch(
                move |cx: &App| app_notify_read.read(cx).notify_enabled,
                move |val: bool, cx: &mut App| {
                    app_notify_write.update(cx, |s, cx| s.set_notify_enabled(val, cx));
                },
            ),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.notifySound"),
            SettingField::switch(
                move |cx: &App| app_sound_read.read(cx).notify_sound,
                move |val: bool, cx: &mut App| {
                    app_sound_write.update(cx, |s, cx| s.set_notify_sound(val, cx));
                },
            ),
        ));

    let app_cfg_file = app_state.clone();
    let config_group = SettingGroup::new()
        .title(tr(locale, "settings.configFiles"))
        .item(
            SettingItem::new(
                tr(locale, "settings.appConfigPath"),
                SettingField::render(move |_, _, cx| {
                    let p = crate::app_settings::settings_path();
                    let disp = p.to_string_lossy().into_owned();
                    config_path_field(
                        "cfg-gpui-settings",
                        p,
                        disp,
                        false,
                        locale,
                        app_cfg_file.clone(),
                        cx,
                    )
                }),
            )
            .layout(Axis::Vertical),
        )
        .item(
            SettingItem::new(
                tr(locale, "settings.settingsJsonPath"),
                SettingField::render({
                    let app_cfg = app_state.clone();
                    move |_, _, cx| {
                        let p = user_home().join(".pi").join("agent").join("settings.json");
                        let disp = p.to_string_lossy().into_owned();
                        config_path_field("cfg-settings-json", p, disp, true, locale, app_cfg.clone(), cx)
                    }
                }),
            )
            .layout(Axis::Vertical),
        )
        .item(SettingItem::new(
            tr(locale, "settings.diagnostics"),
            SettingField::render(move |_, _, cx| {
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(locale, "settings.rendererInfoValue"))
            }),
        ));

    // ── Pi Core ────────────────────────────────────────────────────────────
    // Auto mode lists the documented local cores in a table (address left,
    // linked switch right); manual mode reveals the address field.
    let app_core_table = app_state.clone();
    let mut core_group = SettingGroup::new()
        .title(tr(locale, "settings.piCore"))
        .item(SettingItem::new(
            tr(locale, "settings.piCore"),
            SettingField::dropdown(
                core_options,
                move |cx: &App| SharedString::from(app_core_read.read(cx).core_mode.clone()),
                move |val: SharedString, cx: &mut App| {
                    app_core_write.update(cx, |s, cx| s.set_core_mode(val.to_string(), cx));
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ))
        .item(
            SettingItem::new(
                tr(locale, "settings.services"),
                SettingField::render(move |_, _, cx| {
                    let mode = app_core_view.read(cx).core_mode.clone();
                    let results = app_core_view.read(cx).core_probe_results.clone();
                    let active_url = app_core_view.read(cx).core_url.clone();
                    let probing = app_core_view.read(cx).core_probing;
                    let app_use = app_core_table.clone();
                    let app_probe = app_core_table.clone();

                    let mut col = v_flex().w_full().items_end().gap_1p5();

                    if mode == "auto" {
                        // Table: service / address (left) / linked switch (right).
                        let mut table = v_flex()
                            .w_full()
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border);

                        table = table.child(
                            h_flex()
                                .w_full()
                                .px_2p5()
                                .py_1()
                                .gap_2()
                                .bg(cx.theme().secondary)
                                .text_xs()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .font_medium()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(tr(locale, "settings.serviceAddress")),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .w(px(52.))
                                        .text_right()
                                        .font_medium()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(tr(locale, "settings.serviceLink")),
                                ),
                        );

                        for (idx, candidate) in results.into_iter().enumerate() {
                            let is_active = !active_url.is_empty() && active_url == candidate.url;
                            let app_link = app_use.clone();
                            let url = candidate.url.clone();
                            table = table.child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_2()
                                    .px_2p5()
                                    .py_1p5()
                                    .bg(if idx % 2 == 1 {
                                        cx.theme().secondary.opacity(0.45)
                                    } else {
                                        cx.theme().background
                                    })
                                    .child(
                                        div()
                                            .w(px(6.))
                                            .h(px(6.))
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .bg(if candidate.found {
                                                cx.theme().success
                                            } else {
                                                cx.theme().danger
                                            }),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .font_medium()
                                            .child(SharedString::from(candidate.label)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(url.clone())),
                                    )
                                    .child(div().flex_shrink_0().w(px(52.)).flex().justify_end().child(
                                        Switch::new(("core-link", idx))
                                            .checked(is_active)
                                            .on_click(move |checked: &bool, _, cx| {
                                                let next = *checked;
                                                let url = url.clone();
                                                app_link.update(cx, |s, cx| {
                                                    s.set_core_url(
                                                        if next { url } else { String::new() },
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )),
                            );
                        }

                        col = col.child(table);
                    }

                    // Reconnect lives in the 服务状态 card below this table.
                    let _ = (&app_probe, probing);
                    col
                }),
            )
            .layout(Axis::Vertical),
        );

    // Manual mode reveals the address field; auto mode hides it entirely.
    if app_core_manual.read(_cx).core_mode == "manual" {
        core_group = core_group.item(
            SettingItem::new(
                tr(locale, "settings.piCoreUrl"),
                SettingField::input(
                    move |cx: &App| SharedString::from(app_core_url_read.read(cx).core_url.clone()),
                    move |val: SharedString, cx: &mut App| {
                        app_core_url_write.update(cx, |s, cx| s.set_core_url(val.to_string(), cx));
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .keywords(["core", "omp", "pi", "endpoint", "url"]),
        );
    }

    let network_group = SettingGroup::new()
        .title(tr(locale, "settings.sidecarNetwork"))
        .item(
            SettingItem::new(
                tr(locale, "settings.serviceStatus"),
                SettingField::render(move |_, _, cx| {
                    let (connected, url) = {
                        let s = app_sidecar.read(cx);
                        (s.sidecar_connected, s.client.base_url.clone())
                    };
                    let app_btn = app_sidecar_btn.clone();
                    v_flex()
                        .w_full()
                        .gap_1p5()
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .w(px(8.))
                                                .h(px(8.))
                                                .rounded_full()
                                                .bg(if connected {
                                                    cx.theme().success
                                                } else {
                                                    cx.theme().danger
                                                }),
                                        )
                                        .child(
                                            div().text_xs().font_medium().child(tr(
                                                locale,
                                                "settings.serviceStatus",
                                            )),
                                        ),
                                )
                                .child(
                                    Button::new("settings-sidecar-reconnect")
                                        .outline()
                                        .small()
                                        .h(px(28.))
                                        .w(SETTINGS_BTN_W)
                                        .icon(IconName::RefreshCw)
                                        .label(tr(locale, "settings.reconnect"))
                                        .on_click(move |_, _, cx| {
                                            app_btn.update(cx, |s, cx| s.refresh_from_sidecar(cx));
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(url)),
                        )
                }),
            )
            .layout(Axis::Vertical),
        )
        .item(SettingItem::new(
            tr(locale, "settings.proxyEnabled"),
            SettingField::switch(
                move |cx: &App| app_proxy_read.read(cx).network_proxy.enabled,
                move |val: bool, cx: &mut App| {
                    app_proxy_write.update(cx, |s, cx| {
                        s.network_proxy.enabled = val;
                        if s.sidecar_connected {
                            let client = s.client.clone();
                            let url = s.network_proxy.url.clone();
                            let no_proxy = s.network_proxy.no_proxy.clone();
                            let entity = cx.entity().clone();
                            cx.spawn(async move |_, cx| {
                                if let Ok(res) = client
                                    .set_network_proxy(json!({
                                        "enabled": val,
                                        "url": url,
                                        "noProxy": no_proxy,
                                    }))
                                    .await
                                {
                                    let _ = entity.update(cx, |s, cx| {
                                        s.apply_network_proxy_response(&res);
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }
                        cx.notify();
                    });
                },
            ),
        ))
        .item(SettingItem::new(
            tr(locale, "settings.proxyUrl"),
            SettingField::input(
                move |cx: &App| {
                    let p = &app_proxy_url_read.read(cx).network_proxy;
                    let shown = if p.url.is_empty() {
                        p.effective_url.as_deref().unwrap_or("")
                    } else {
                        p.url.as_str()
                    };
                    SharedString::from(shown.to_string())
                },
                move |val: SharedString, cx: &mut App| {
                    app_proxy_url_write.update(cx, |s, cx| {
                        s.network_proxy.url = val.to_string();
                        if s.sidecar_connected {
                            let client = s.client.clone();
                            let enabled = s.network_proxy.enabled;
                            let no_proxy = s.network_proxy.no_proxy.clone();
                            let url = val.to_string();
                            let entity = cx.entity().clone();
                            cx.spawn(async move |_, cx| {
                                if let Ok(res) = client
                                    .set_network_proxy(json!({
                                        "enabled": enabled,
                                        "url": url,
                                        "noProxy": no_proxy,
                                    }))
                                    .await
                                {
                                    let _ = entity.update(cx, |s, cx| {
                                        s.apply_network_proxy_response(&res);
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }
                        cx.notify();
                    });
                },
            )
            .w(SETTINGS_DROPDOWN_W),
        ));

    SettingPage::new(tr(locale, "settings.general"))
        .icon(IconName::Settings)
        .default_open(true)
        .group(appearance_group)
        .group(chat_group)
        .group(notify_group)
        .group(config_group)
        .group(core_group)
        .group(network_group)
}
