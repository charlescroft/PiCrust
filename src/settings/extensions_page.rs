use crate::app_state::{AppState, SettingsAddKind};
use crate::i18n::{AppLocale, format_commas, tr};
use crate::settings::SETTINGS_BTN_W;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex};
use gpui_kit::{App, Entity, IntoElement as _, ParentElement as _, SharedString, Styled as _, div, px};
use serde_json::json;

pub fn subagents_setting_page(app_state: Entity<AppState>, locale: AppLocale, cx: &App) -> SettingPage {
    let state = app_state.read(cx);
    let profiles = state.subagents.clone();
    let max_conc = state.subagent_max_concurrent;
    let app_builtin_read = app_state.clone();
    let app_builtin_write = app_state.clone();
    let app_add_sub = app_state.clone();

    let settings_group = SettingGroup::new()
        .title(tr(locale, "settings.subagents"))
        .item(SettingItem::new(
            tr(locale, "settings.builtInSubagents"),
            SettingField::switch(
                move |cx: &App| app_builtin_read.read(cx).subagent_built_in_enabled,
                move |val: bool, cx: &mut App| {
                    app_builtin_write.update(cx, |s, cx| {
                        s.subagent_built_in_enabled = val;
                        let _ = crate::client::local_store::save_subagent_settings_local(
                            val,
                            s.subagent_max_concurrent,
                            None,
                        );
                        if s.sidecar_connected {
                            let client = s.client.clone();
                            cx.spawn(async move |_, _| {
                                let _ = client
                                    .set_subagent_settings(json!({ "enabled": val }))
                                    .await;
                            })
                            .detach();
                        }
                        cx.notify();
                    });
                },
            ),
        ))
        .item(
            SettingItem::new(
                tr(locale, "settings.maxConcurrentSubagents"),
                SettingField::render(move |_, _, _| {
                    Tag::secondary()
                        .small()
                        .child(SharedString::from(format_commas(max_conc)))
                }),
            ),
        )
        .item(
            SettingItem::new(
                tr(locale, "settings.addSubagent"),
                SettingField::render(move |_, _, _| {
                    let app_btn = app_add_sub.clone();
                    Button::new("add-subagent-btn")
                        .primary()
                        .small()
                        .h(px(28.))
                        .w(SETTINGS_BTN_W)
                        .icon(IconName::Plus)
                        .label(tr(locale, "settings.add"))
                        .on_click(move |_, _, cx| {
                            app_btn.update(cx, |s, cx| {
                                s.settings_add_kind = Some(SettingsAddKind::SubagentProfile);
                                cx.notify();
                            });
                        })
                }),
            ),
        );

    let mut group = SettingGroup::new().title(tr(locale, "settings.subagentProfiles"));

    if profiles.is_empty() {
        group = group.item(SettingItem::new(
            tr(locale, "settings.emptySidecar"),
            SettingField::render(|_, _, _| Tag::secondary().small().child(SharedString::from("0"))),
        ));
    } else {
        for p in profiles {
            let name = p.name.clone();
            let scope = p.scope.clone();
            let title = p.display_name.clone().unwrap_or_else(|| p.name.clone());
            let model_str = p.model.clone().unwrap_or_default();
            let scope_for_desc = scope.clone();
            let desc_for_search = format!("{scope} · {model_str} · {}", p.description);
            let app_read = app_state.clone();
            let app_write = app_state.clone();
            let name_read = name.clone();
            let scope_read = scope.clone();

            group = group.item(
                SettingItem::new(
                    SharedString::from(title),
                    SettingField::switch(
                        move |cx: &App| {
                            app_read
                                .read(cx)
                                .subagents
                                .iter()
                                .find(|x| x.name == name_read && x.scope == scope_read)
                                .map(|x| x.enabled)
                                .unwrap_or(true)
                        },
                        move |val: bool, cx: &mut App| {
                            let n = name.clone();
                            let sc = scope.clone();
                            app_write.update(cx, |s, cx| {
                                if let Some(target) = s
                                    .subagents
                                    .iter_mut()
                                    .find(|x| x.name == n && x.scope == sc)
                                {
                                    target.enabled = val;
                                }
                                let cwd = s.active_cwd.to_string_lossy().into_owned();
                                let _ = crate::client::local_store::save_subagent_profile_local(
                                    Some(&cwd),
                                    &json!({
                                        "cwd": cwd,
                                        "scope": sc,
                                        "name": n,
                                        "enabled": val,
                                    }),
                                );
                                if s.sidecar_connected {
                                    let client = s.client.clone();
                                    cx.spawn(async move |_, _| {
                                        let _ = client
                                            .toggle_subagent_profile(json!({
                                                "cwd": cwd,
                                                "scope": sc,
                                                "name": n,
                                                "enabled": val,
                                            }))
                                            .await;
                                    })
                                    .detach();
                                }
                                cx.notify();
                            });
                        },
                    ),
                )
                .description(if model_str.is_empty() {
                    scope_for_desc
                } else {
                    model_str.clone()
                })
                .keywords([desc_for_search]),
            );
        }
    }

    SettingPage::new(tr(locale, "settings.subagents"))
        .icon(IconName::Bot)
        .group(settings_group)
        .group(group)
}

pub fn skills_setting_page(app_state: Entity<AppState>, locale: AppLocale, cx: &App) -> SettingPage {
    let skills = app_state.read(cx).skills.clone();
    let app_add_skill = app_state.clone();

    let mut group = SettingGroup::new()
        .title(tr(locale, "settings.skillsTitle"))
        .item(
            SettingItem::new(
                tr(locale, "settings.installSkill"),
                SettingField::render(move |_, _, _| {
                    let app_btn = app_add_skill.clone();
                    Button::new("add-skill-btn")
                        .primary()
                        .small()
                        .h(px(28.))
                        .w(SETTINGS_BTN_W)
                        .icon(IconName::Plus)
                        .label(tr(locale, "settings.add"))
                        .on_click(move |_, _, cx| {
                            app_btn.update(cx, |s, cx| {
                                s.settings_add_kind = Some(SettingsAddKind::Skill);
                                cx.notify();
                            });
                        })
                }),
            ),
        );

    if skills.is_empty() {
        group = group.item(SettingItem::new(
            tr(locale, "settings.emptySidecar"),
            SettingField::render(|_, _, _| Tag::secondary().small().child(SharedString::from("0"))),
        ));
    } else {
        for s in skills {
            let name = s.name.clone();
            let source = s.source.clone();
            let desc = s.description.clone();
            let app_read = app_state.clone();
            let app_write = app_state.clone();
            let name_read = name.clone();
            let file_path_view = s.file_path.clone();
            let source_for_search = s.source.clone();

            let skill_info_item = s.clone();
            let app_info = app_state.clone();
            group = group.item(
                SettingItem::render(move |_, _, cx| {
                    let enabled = app_read
                        .read(cx)
                        .skills
                        .iter()
                        .find(|x| x.name == name_read)
                        .map(|x| x.enabled)
                        .unwrap_or(true);
                    let app_w = app_write.clone();
                    let n = name.clone();
                    let fp_for_switch = file_path_view.clone();
                    let app_info_click = app_info.clone();
                    let skill_for_click = skill_info_item.clone();
                    let n_title = n.clone();

                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .text_color(cx.theme().foreground)
                                        .child(SharedString::from(n_title)),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("skill-info-{}", n)))
                                        .ghost()
                                        .xsmall()
                                        .h(px(22.))
                                        .w(px(22.))
                                        .icon(IconName::Info)
                                        .tooltip(tr(locale, "settings.details"))
                                        .on_click(move |_, _, cx| {
                                            app_info_click.update(cx, |st, cx| {
                                                st.settings_add_kind = Some(SettingsAddKind::SkillInfo(skill_for_click.clone()));
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(Tag::secondary().small().child(SharedString::from(source.clone())))
                                .child(
                                    Switch::new(SharedString::from(format!("skill-switch-{}", n)))
                                        .checked(enabled)
                                        .on_click(move |checked: &bool, _, cx| {
                                            let val = *checked;
                                            let n = n.clone();
                                            let fp = fp_for_switch.clone();
                                            app_w.update(cx, |st, cx| {
                                                if let Some(target) = st.skills.iter_mut().find(|x| x.name == n) {
                                                    target.enabled = val;
                                                    target.disable_model_invocation = !val;
                                                }
                                                if !fp.is_empty() {
                                                    let _ = crate::client::local_store::toggle_skill_file_local(
                                                        std::path::Path::new(&fp),
                                                        !val,
                                                    );
                                                }
                                                if st.sidecar_connected && !fp.is_empty() {
                                                    let client = st.client.clone();
                                                    cx.spawn(async move |_, _| {
                                                        let _ = client.toggle_skill(&fp, !val).await;
                                                    })
                                                    .detach();
                                                }
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                })
                .keywords([desc, source_for_search.clone()]),
            );
        }
    }

    SettingPage::new(tr(locale, "settings.skills"))
        .icon(IconName::BookOpen)
        .group(group)
}

pub fn plugins_setting_page(app_state: Entity<AppState>, locale: AppLocale, cx: &App) -> SettingPage {
    let plugins = app_state.read(cx).plugins.clone();
    let app_add_plugin = app_state.clone();
    let app_upd_all = app_state.clone();

    let mut group = SettingGroup::new()
        .title(tr(locale, "settings.pluginsTitle"))
        .item(
            SettingItem::new(
                tr(locale, "settings.pluginManagement"),
                SettingField::render({
                    let app_btn = app_add_plugin.clone();
                    let app_upd = app_upd_all.clone();
                    let app_read_btn = app_state.clone();
                    move |_, _, cx| {
                        let is_upd = app_read_btn.read(cx).plugins_updating;
                        let app_btn = app_btn.clone();
                        let app_upd = app_upd.clone();
                        h_flex()
                            .w_full()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("add-plugin-btn")
                                    .primary()
                                    .small()
                                    .h(px(28.))
                                    .w(SETTINGS_BTN_W)
                                    .icon(IconName::Plus)
                                    .label(tr(locale, "settings.add"))
                                    .on_click(move |_, _, cx| {
                                        app_btn.update(cx, |s, cx| {
                                            s.settings_add_kind = Some(SettingsAddKind::Plugin);
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child({
                            Button::new("update-all-plugins-btn")
                                .outline()
                                .small()
                                .h(px(28.))
                                .w(SETTINGS_BTN_W)
                                .disabled(is_upd)
                                .loading(is_upd)
                                .loading_icon(Icon::new(crate::icons::LoaderCircleIcon))
                                .icon(IconName::RefreshCw)
                                .label(if is_upd {
                                    tr(locale, "settings.updating")
                                } else {
                                    tr(locale, "settings.updateAll")
                                })
                                .on_click(move |_, _, cx| {
                                    app_upd.update(cx, |s, cx| {
                                        if s.sidecar_connected && !s.plugins_updating {
                                            s.plugins_updating = true;
                                            cx.notify();
                                            let client = s.client.clone();
                                            let cwd = s.active_cwd.to_string_lossy().into_owned();
                                            let entity = cx.entity().clone();
                                            cx.spawn(async move |_, cx| {
                                                let res = client
                                                    .plugin_action(json!({
                                                        "cwd": cwd,
                                                        "action": "update",
                                                    }))
                                                    .await;
                                                let _ = entity.update(cx, |s, cx| {
                                                    s.plugins_updating = false;
                                                    if let Ok(val) = res {
                                                        s.apply_plugins_response(&val);
                                                    }
                                                    cx.notify();
                                                });
                                            })
                                            .detach();
                                        }
                                    });
                                })
                        })
                    }
                }),
            ),
        );

    if plugins.is_empty() {
        let is_loading = app_state.read(cx).settings_loading;
        if is_loading {
            group = group.item(SettingItem::new(
                "Loading plugins…",
                SettingField::render(|_, _, _| {
                    Spinner::new().icon(Icon::new(crate::icons::LoaderCircleIcon))
                }),
            ));
        } else {
            group = group.item(SettingItem::new(
                tr(locale, "settings.emptySidecar"),
                SettingField::render(|_, _, _| Tag::secondary().small().child(SharedString::from("0"))),
            ));
        }
    } else {
        for (idx, p) in plugins.into_iter().enumerate() {
            let name = p.name.clone();
            let source = p.source.clone();
            let scope = p.scope.clone();
            let ver = p.version.clone();
            let summary = p.resource_summary.clone();
            let desc_text = p.description.clone().unwrap_or_default();
            let mut desc_parts = vec![scope.clone()];
            if !summary.is_empty() {
                desc_parts.push(summary);
            }
            if !desc_text.is_empty() {
                desc_parts.push(desc_text);
            }
            let app_read = app_state.clone();
            let app_write = app_state.clone();
            let src_read = source.clone();
            let sc_read = scope.clone();

            let plugin_info_item = p.clone();
            let app_info = app_state.clone();
            group = group.item(
                SettingItem::render(move |_, _, cx| {
                    let enabled_now = app_read
                        .read(cx)
                        .plugins
                        .iter()
                        .find(|x| x.source == src_read && x.scope == sc_read)
                        .map(|x| x.enabled)
                        .unwrap_or(true);
                    let app_w = app_write.clone();
                    let src_w = source.clone();
                    let sc_w = scope.clone();
                    let app_info_click = app_info.clone();
                    let plugin_for_click = plugin_info_item.clone();
                    let name_title = name.clone();

                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .text_color(cx.theme().foreground)
                                        .child(SharedString::from(name_title)),
                                )
                                .child(
                                    Button::new(("plugin-info-btn", idx))
                                        .ghost()
                                        .xsmall()
                                        .h(px(22.))
                                        .w(px(22.))
                                        .icon(IconName::Info)
                                        .tooltip(tr(locale, "settings.details"))
                                        .on_click(move |_, _, cx| {
                                            app_info_click.update(cx, |st, cx| {
                                                st.settings_add_kind = Some(SettingsAddKind::PluginInfo(plugin_for_click.clone()));
                                                cx.notify();
                                            });
                                        }),
                                )
                                .children(app_read.read(cx).plugins_updating.then(|| {
                                    Spinner::new()
                                        .icon(Icon::new(crate::icons::LoaderCircleIcon))
                                        .xsmall()
                                        .color(cx.theme().primary)
                                }))
                        )
                        .child({
                            let src_for_del = src_w.clone();
                            let sc_for_del = sc_w.clone();
                            let app_del = app_w.clone();
                            let source_tag = if src_w.starts_with("npm:") {
                                "npm".to_string()
                            } else {
                                src_w.clone()
                            };

                            h_flex()
                                .flex_wrap()
                                .justify_end()
                                .gap_2()
                                .items_center()
                                .child(Tag::secondary().small().child(SharedString::from(
                                    if src_w.starts_with("npm:") {
                                        "npm".to_string()
                                    } else if src_w.starts_with("git:") || src_w.starts_with("http") || src_w.ends_with(".git") {
                                        "git".to_string()
                                    } else {
                                        "local".to_string()
                                    }
                                )))
                                .child(Tag::secondary().small().child(SharedString::from(format!("v{}", ver))))
                                .child(
                                    Switch::new(SharedString::from(format!("plugin-sw-{}", idx)))
                                        .checked(enabled_now)
                                        .on_click(move |checked: &bool, _, cx| {
                                            let next = *checked;
                                            let src = src_w.clone();
                                            let sc = sc_w.clone();
                                            app_w.update(cx, |s, cx| {
                                                if let Some(target) = s
                                                    .plugins
                                                    .iter_mut()
                                                    .find(|x| x.source == src && x.scope == sc)
                                                {
                                                    target.enabled = next;
                                                    target.status = if next {
                                                        "loaded".to_string()
                                                    } else {
                                                        "disabled".to_string()
                                                    };
                                                }
                                                if s.sidecar_connected {
                                                    let client = s.client.clone();
                                                    let cwd = s.active_cwd.to_string_lossy().into_owned();
                                                    let entity = cx.entity().clone();
                                                    cx.spawn(async move |_, cx| {
                                                        if let Ok(res) = client
                                                            .plugin_action(json!({
                                                                "cwd": cwd,
                                                                "action": if next { "enable" } else { "disable" },
                                                                "source": src,
                                                                "scope": sc,
                                                            }))
                                                            .await
                                                        {
                                                            let _ = entity.update(cx, |s, cx| {
                                                                s.apply_plugins_response(&res);
                                                                cx.notify();
                                                            });
                                                        }
                                                    })
                                                    .detach();
                                                }
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child({
                                    let app_del_read = app_del.clone();
                                    let is_busy = app_del_read.read(cx).plugins_updating;
                                    let name_for_del = name.clone();
                                    if is_busy {
                                        h_flex()
                                            .h(px(24.))
                                            .px_1p5()
                                            .items_center()
                                            .gap_1()
                                            .child(Spinner::new().xsmall().icon(Icon::new(crate::icons::LoaderCircleIcon)))
                                            .into_any_element()
                                    } else {
                                        Button::new(("del-plugin", idx))
                                            .ghost()
                                            .xsmall()
                                            .h(px(24.))
                                            .w(px(24.))
                                            .icon(Icon::new(crate::icons::TrashIcon))
                                            .text_color(cx.theme().danger)
                                            .tooltip(tr(locale, "settings.deleteGroup"))
                                            .on_click(move |_, _, cx| {
                                                let src = src_for_del.clone();
                                                let sc = sc_for_del.clone();
                                                let pname = name_for_del.clone();
                                                app_del.update(cx, |s, cx| {
                                                    if s.sidecar_connected && !s.plugins_updating {
                                                        s.plugins_updating = true;
                                                        s.status_banner = Some(SharedString::from(format!("Removing {pname}…")));
                                                        cx.notify();
                                                        let client = s.client.clone();
                                                        let cwd = s.active_cwd.to_string_lossy().into_owned();
                                                        let entity = cx.entity().clone();
                                                        cx.spawn(async move |_, cx| {
                                                            let res = client
                                                                .plugin_action(json!({
                                                                    "cwd": cwd,
                                                                    "action": "remove",
                                                                    "source": src,
                                                                    "scope": sc,
                                                                }))
                                                                .await;
                                                            let _ = entity.update(cx, |s, cx| {
                                                                s.plugins_updating = false;
                                                                match res {
                                                                    Ok(val) => {
                                                                        s.apply_plugins_response(&val);
                                                                        s.status_banner = Some(SharedString::from(format!("Removed plugin: {pname}")));
                                                                    }
                                                                    Err(err) => {
                                                                        s.status_banner = Some(SharedString::from(format!("Failed to remove {pname}: {err}")));
                                                                    }
                                                                }
                                                                cx.notify();
                                                            });
                                                        })
                                                        .detach();
                                                    }
                                                });
                                            })
                                            .into_any_element()
                                    }
                                })
                        })
                })
                .keywords([desc_parts.join(" · ")]),
            );
        }
    }

    let app_agy_open = app_state.clone();
    let agy_group = SettingGroup::new()
        .title("Antigravity (pi-agy-link)")
        .item(
            SettingItem::new(
                "Account Pool & Quotas",
                SettingField::render(move |_, _, _| {
                    let app_agy = app_agy_open.clone();
                    Button::new("open-agy-pool-from-settings")
                        .primary()
                        .small()
                        .icon(IconName::Bot)
                        .label("Open Pool & Quotas")
                        .on_click(move |_, _, cx| {
                            app_agy.update(cx, |s, cx| {
                                s.settings_open = false;
                                s.open_agy_link_modal(cx);
                            });
                        })
                }),
            )
            .description("Manage Google Antigravity accounts, 5H/7D/Argon quotas, OAuth login, and per-account proxies."),
        );

    SettingPage::new(tr(locale, "settings.plugins"))
        .icon(IconName::HardDrive)
        .group(agy_group)
        .group(group)
}

pub fn antigravity_setting_page(app_state: Entity<AppState>, _locale: AppLocale, cx: &App) -> SettingPage {
    let agy_state = app_state.read(cx).agy_link_state.clone();
    let app_open = app_state.clone();
    let app_add = app_state.clone();
    let app_refresh = app_state.clone();

    let mut group = SettingGroup::new()
        .title("Antigravity (pi-agy-link)")
        .item(
            SettingItem::new(
                "Account Pool & Quotas Console",
                SettingField::render(move |_, _, _| {
                    let app_open = app_open.clone();
                    let app_add = app_add.clone();
                    let app_refresh = app_refresh.clone();
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("settings-agy-open-modal")
                                .primary()
                                .small()
                                .icon(IconName::Bot)
                                .label("Open Pool & Quotas")
                                .on_click(move |_, _, cx| {
                                    app_open.update(cx, |s, cx| {
                                        s.settings_open = false;
                                        s.open_agy_link_modal(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("settings-agy-add-oauth")
                                .outline()
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
                            Button::new("settings-agy-refresh")
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
                }),
            )
            .description("Manage Google Antigravity accounts, 5H/7D/Argon quotas, OAuth login, and per-account proxies."),
        );

    let agy_loading = app_state.read(cx).agy_link_loading;
    if let Some(st) = agy_state {
        for (idx, acc) in st.accounts.into_iter().enumerate() {
            let email = acc.email.clone().unwrap_or_else(|| acc.alias.clone());
            let q5 = acc.five_hour_pct.map(|p| format!("5H: {p}%")).unwrap_or_else(|| "5H: ?".to_string());
            let q7 = acc.weekly_pct.map(|p| format!("7D: {p}%")).unwrap_or_else(|| "7D: ?".to_string());
            let desc = format!("Tier: {} · {} · {}", acc.tier, q5, q7);
            let acc_id = acc.id.clone();
            let app_use = app_state.clone();
            group = group.item(
                SettingItem::new(
                    email,
                    SettingField::render(move |_, _, _| {
                        let app_use = app_use.clone();
                        let acc_id = acc_id.clone();
                        if acc.active {
                            Tag::primary().small().child("Active").into_any_element()
                        } else {
                            Button::new(("settings-agy-use", idx))
                                .outline()
                                .xsmall()
                                .label("Use")
                                .on_click(move |_, _, cx| {
                                    let id = acc_id.clone();
                                    app_use.update(cx, |s, cx| {
                                        s.send_agy_command(format!("/agy switch {id}"), cx);
                                    });
                                })
                                .into_any_element()
                        }
                    }),
                )
                .description(desc),
            );
        }
    } else {
        group = group.item(
            SettingItem::new(
                "Loading Antigravity Pool",
                SettingField::render(move |_, _, _| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Spinner::new().xsmall().icon(Icon::new(crate::icons::LoaderCircleIcon)))
                        .child(
                            div()
                                .text_xs()
                                .child(if agy_loading { "Refreshing…" } else { "Connecting…" }),
                        )
                }),
            )
            .description("Waiting for pi-agy-link extension to report account pool and quotas…"),
        );
    }

    SettingPage::new("Antigravity")
        .icon(IconName::Bot)
        .group(group)
}
