use crate::app_state::{AppState, SettingsAddKind};
use crate::i18n::{AppLocale, format_commas, tr};
use crate::settings::{SETTINGS_BTN_W, SETTINGS_DROPDOWN_W};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex};
use gpui_kit::{App, Entity, IntoElement as _, ParentElement as _, SharedString, Styled as _, div, px};
use gpui_kit::prelude::FluentBuilder as _;
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
                                .when(
                                    app_del
                                        .read(cx)
                                        .settings_deleting_keys
                                        .contains(&format!("plugin:{}", src_for_del)),
                                    |el| {
                                        el.child(
                                            h_flex()
                                                .items_center()
                                                .gap_1p5()
                                                .px_2()
                                                .child(
                                                    Spinner::new()
                                                        .xsmall()
                                                        .icon(Icon::new(crate::icons::LoaderCircleIcon)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child("删除中…"),
                                                ),
                                        )
                                    },
                                )
                                .child(
                                    Switch::new(SharedString::from(format!("plugin-sw-{}", idx)))
                                        .checked(enabled_now)
                                        .disabled(
                                            app_del
                                                .read(cx)
                                                .settings_deleting_keys
                                                .contains(&format!("plugin:{}", src_for_del)),
                                        )
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
                                    let is_row_deleting = app_del_read
                                        .read(cx)
                                        .settings_deleting_keys
                                        .contains(&format!("plugin:{}", src_for_del));
                                    let is_busy = app_del_read.read(cx).plugins_updating;
                                    let name_for_del = name.clone();
                                    Button::new(("del-plugin", idx))
                                        .ghost()
                                        .small()
                                        .h(px(26.))
                                        .w(px(26.))
                                        .icon(Icon::new(crate::icons::TrashIcon))
                                        .text_color(cx.theme().danger)
                                        .disabled(is_row_deleting || is_busy)
                                        .tooltip(tr(locale, "settings.deleteGroup"))
                                        .on_click(move |_, _, cx| {
                                            let src = src_for_del.clone();
                                            let sc = sc_for_del.clone();
                                            let pname = name_for_del.clone();
                                            let del_key = format!("plugin:{src}");
                                            app_del.update(cx, |s, cx| {
                                                if s.sidecar_connected && !s.plugins_updating {
                                                    s.plugins_updating = true;
                                                    s.settings_deleting_keys.insert(del_key.clone());
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
                                                            s.settings_deleting_keys.remove(&del_key);
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
                                })
                        })
                })
                .keywords([desc_parts.join(" · ")]),
            );
        }
    }

    SettingPage::new(tr(locale, "settings.plugins"))
        .icon(IconName::HardDrive)
        .group(group)
}

pub fn antigravity_setting_page(app_state: Entity<AppState>, _locale: AppLocale, cx: &App) -> SettingPage {
    let (agy_state, agy_loading, details_id) = {
        let s = app_state.read(cx);
        (
            s.agy_link_state.clone(),
            s.agy_link_loading,
            s.agy_link_details_account_id.clone(),
        )
    };

    let app_add = app_state.clone();
    let app_refresh = app_state.clone();
    let app_clear_cd = app_state.clone();

    // 1. Account Pool & Quotas
    let mut pool_group = SettingGroup::new()
        .title("Account Pool & Quotas")
        .item(
            SettingItem::new(
                "Account Actions",
                SettingField::render(move |_, _, _| {
                    let app_add = app_add.clone();
                    let app_refresh = app_refresh.clone();
                    let app_clear_cd = app_clear_cd.clone();
                    h_flex()
                        .gap_2()
                        .items_center()
                        .when(agy_loading, |el| {
                            el.child(
                                Spinner::new()
                                    .xsmall()
                                    .icon(Icon::new(crate::icons::LoaderCircleIcon)),
                            )
                        })
                        .child(
                            Button::new("settings-agy-add-oauth")
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
                        .child(
                            Button::new("settings-agy-clear-cd")
                                .outline()
                                .small()
                                .label("Clear Cooldowns")
                                .on_click(move |_, _, cx| {
                                    app_clear_cd.update(cx, |s, cx| {
                                        s.send_agy_command("/agy clear-cooldown".to_string(), cx);
                                    });
                                }),
                        )
                }),
            )
            .description("Manage Google Antigravity OAuth accounts, live 5H/7D/Argon quotas, and cooldown states."),
        );

    let mut details_group: Option<SettingGroup> = None;

    if let Some(ref st) = agy_state {
        if let Some(ref reason) = st.dormant_reason {
            pool_group = pool_group.item(
                SettingItem::new(
                    "Extension Dormant",
                    SettingField::render(move |_, _, _| Tag::danger().small().child("Dormant")),
                )
                .description(reason.clone()),
            );
        }

        if st.accounts.is_empty() {
            pool_group = pool_group.item(
                SettingItem::new(
                    "No Accounts",
                    SettingField::render(|_, _, _| Tag::secondary().small().child("Empty")),
                )
                .description("Click 'Add Account (OAuth)' above to sign in with a Google account."),
            );
        } else {
            for (idx, acc) in st.accounts.iter().enumerate() {
                let email = acc.email.clone().unwrap_or_else(|| acc.alias.clone());
                let q5 = acc.five_hour_pct.map(|p| format!("Gemini 5H: {p}%")).unwrap_or_else(|| "Gemini 5H: ?".to_string());
                let q7 = acc.weekly_pct.map(|p| format!("Gemini 7D: {p}%")).unwrap_or_else(|| "Gemini 7D: ?".to_string());
                let qa = acc.argon_pct.map(|p| format!(" · Argon 7D: {p}%")).unwrap_or_default();
                let status_str = if acc.auth_required {
                    "Reauth required"
                } else if acc.disabled {
                    "Disabled"
                } else if acc.cooldown_until.is_some() {
                    "Cooldown"
                } else if acc.active {
                    "Active"
                } else {
                    "Ready"
                };
                let title = format!("{email} · {}", acc.tier);
                let desc = format!("{q5} · {q7}{qa} · Status: {status_str}");

                let acc_id_use = acc.id.clone();
                let acc_id_toggle = acc.id.clone();
                let acc_id_det = acc.id.clone();
                let acc_id_rm = acc.id.clone();
                let is_active = acc.active;
                let is_disabled = acc.disabled;
                let is_system_home = acc.system_home;
                let is_details_open = details_id.as_deref() == Some(acc.id.as_str());

                let app_use = app_state.clone();
                let app_toggle = app_state.clone();
                let app_det = app_state.clone();
                let app_rm = app_state.clone();
                let is_acc_deleting = app_state
                    .read(cx)
                    .settings_deleting_keys
                    .contains(&format!("agy:{}", acc.id));

                pool_group = pool_group.item(
                    SettingItem::new(
                        title,
                        SettingField::render(move |_, _, cx| {
                            let acc_id_use = acc_id_use.clone();
                            let acc_id_toggle = acc_id_toggle.clone();
                            let acc_id_det = acc_id_det.clone();
                            let acc_id_rm = acc_id_rm.clone();
                            let app_use = app_use.clone();
                            let app_toggle = app_toggle.clone();
                            let app_det = app_det.clone();
                            let app_rm = app_rm.clone();
                            h_flex()
                                .gap_1p5()
                                .items_center()
                                .when(is_acc_deleting, |el| {
                                    el.child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .px_2()
                                            .child(
                                                Spinner::new()
                                                    .xsmall()
                                                    .icon(Icon::new(crate::icons::LoaderCircleIcon)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("删除中…"),
                                            ),
                                    )
                                })
                                .child(if is_active {
                                    Tag::primary().small().child("Active").into_any_element()
                                } else {
                                    Button::new(("settings-agy-use", idx))
                                        .outline()
                                        .small()
                                        .disabled(is_acc_deleting || is_disabled)
                                        .label("Use")
                                        .on_click(move |_, _, cx| {
                                            let id = acc_id_use.clone();
                                            app_use.update(cx, |s, cx| {
                                                s.send_agy_command(format!("/agy switch {id}"), cx);
                                            });
                                        })
                                        .into_any_element()
                                })
                                .child(
                                    Button::new(("settings-agy-enable", idx))
                                        .ghost()
                                        .small()
                                        .disabled(is_acc_deleting)
                                        .label(if is_disabled { "Enable" } else { "Disable" })
                                        .on_click(move |_, _, cx| {
                                            let id = acc_id_toggle.clone();
                                            let cmd = if is_disabled {
                                                format!("/agy enable {id}")
                                            } else {
                                                format!("/agy disable {id}")
                                            };
                                            app_toggle.update(cx, |s, cx| s.send_agy_command(cmd, cx));
                                        }),
                                )
                                .child({
                                    let btn = Button::new(("settings-agy-details", idx))
                                        .small()
                                        .disabled(is_acc_deleting)
                                        .label(if is_details_open { "Hide Models" } else { "Models" })
                                        .on_click(move |_, _, cx| {
                                            let id = acc_id_det.clone();
                                            app_det.update(cx, |s, cx| {
                                                if s.agy_link_details_account_id.as_deref() == Some(id.as_str()) {
                                                    s.agy_link_details_account_id = None;
                                                } else {
                                                    s.agy_link_details_account_id = Some(id);
                                                }
                                                cx.notify();
                                            });
                                        });
                                    if is_details_open { btn.primary() } else { btn.outline() }
                                })
                                .when(!is_system_home, |el| {
                                    el.child(
                                        Button::new(("settings-agy-remove", idx))
                                            .ghost()
                                            .small()
                                            .disabled(is_acc_deleting)
                                            .icon(IconName::Delete)
                                            .on_click(move |_, _, cx| {
                                                let id = acc_id_rm.clone();
                                                let del_key = format!("agy:{id}");
                                                app_rm.update(cx, |s, cx| {
                                                    s.settings_deleting_keys.insert(del_key);
                                                    s.send_agy_command(format!("/agy remove {id}"), cx);
                                                });
                                            }),
                                    )
                                })
                        }),
                    )
                    .description(desc),
                );

                if is_details_open {
                    let app_close_det = app_state.clone();
                    let mut dg = SettingGroup::new()
                        .title(format!("Sub-models & Quotas: {email}"))
                        .item(
                            SettingItem::new(
                                format!("Tier: {} · Priority: {}", acc.tier, acc.priority),
                                SettingField::render(move |_, _, _| {
                                    let app_close_det = app_close_det.clone();
                                    Button::new("settings-agy-close-det")
                                        .outline()
                                        .small()
                                        .label("Close")
                                        .on_click(move |_, _, cx| {
                                            app_close_det.update(cx, |s, cx| {
                                                s.agy_link_details_account_id = None;
                                                cx.notify();
                                            });
                                        })
                                }),
                            )
                            .description(format!(
                                "Proxy: {} · {} official sub-models",
                                acc.proxy_url.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("system / direct"),
                                acc.models.len()
                            )),
                        );
                    for m in &acc.models {
                        let pct = m.percentage;
                        let reset = m.reset_time.clone().unwrap_or_default();
                        dg = dg.item(
                            SettingItem::new(
                                m.name.clone(),
                                SettingField::render(move |_, _, _| {
                                    let label = format!("{pct}%");
                                    if pct > 50 {
                                        Tag::primary().small().child(label).into_any_element()
                                    } else if pct > 20 {
                                        Tag::warning().small().child(label).into_any_element()
                                    } else {
                                        Tag::danger().small().child(label).into_any_element()
                                    }
                                }),
                            )
                            .description(if reset.is_empty() {
                                "Quota remaining".to_string()
                            } else {
                                format!("Resets: {reset}")
                            }),
                        );
                    }
                    details_group = Some(dg);
                }
            }
        }
    } else {
        pool_group = pool_group.item(
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

    // 2. Auto-Switch & Pool Rotation
    let app_mode_r = app_state.clone();
    let app_mode_w = app_state.clone();
    let app_fb_r = app_state.clone();
    let app_fb_w = app_state.clone();
    let app_wu_r = app_state.clone();
    let app_wu_w = app_state.clone();

    let mode_options = vec![
        (SharedString::from("sequential"), SharedString::from("Sequential (Primary + Fallback)")),
        (SharedString::from("round-robin"), SharedString::from("Round-Robin (Load Balance)")),
    ];

    let rotation_group = SettingGroup::new()
        .title("Auto-Switch & Pool Rotation")
        .item(
            SettingItem::new(
                "Pool Rotation Mode",
                SettingField::dropdown(
                    mode_options,
                    move |cx: &App| {
                        let m = app_mode_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.mode.as_str())
                            .unwrap_or("sequential");
                        SharedString::from(m.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_mode_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy mode {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Sequential sticks to primary until 429/quota depletion; Round-Robin rotates across ready accounts."),
        )
        .item(
            SettingItem::new(
                "Auto Fallback Model",
                SettingField::switch(
                    move |cx: &App| {
                        app_fb_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.auto_fallback_model)
                            .unwrap_or(false)
                    },
                    move |val: bool, cx: &mut App| {
                        let cmd = if val { "/agy auto-fallback on" } else { "/agy auto-fallback off" };
                        app_fb_w.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                    },
                ),
            )
            .description("Automatically fall back to an available lower-tier model when the active model is exhausted."),
        )
        .item(
            SettingItem::new(
                "Auto 7-Day Quota Warmup",
                SettingField::switch(
                    move |cx: &App| {
                        app_wu_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.auto_warmup)
                            .unwrap_or(false)
                    },
                    move |val: bool, cx: &mut App| {
                        let cmd = if val { "/agy warmup on" } else { "/agy warmup off" };
                        app_wu_w.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                    },
                ),
            )
            .description("Send a minimal probe on cold start or reset window to activate the 7-day upstream timer."),
        );

    // 3. Model Display & Thinking
    let app_pw_r = app_state.clone();
    let app_pw_w = app_state.clone();
    let app_eff_r = app_state.clone();
    let app_eff_w = app_state.clone();

    let effort_options = vec![
        (SharedString::from("default"), SharedString::from("Model Default")),
        (SharedString::from("low"), SharedString::from("Low")),
        (SharedString::from("medium"), SharedString::from("Medium")),
        (SharedString::from("high"), SharedString::from("High")),
    ];

    let mut model_group = SettingGroup::new()
        .title("Model Display & Thinking")
        .item(
            SettingItem::new(
                "Pool-Wide Model Catalog",
                SettingField::switch(
                    move |cx: &App| {
                        app_pw_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.pool_wide_catalog)
                            .unwrap_or(false)
                    },
                    move |val: bool, cx: &mut App| {
                        let cmd = if val { "/agy pool-wide on" } else { "/agy pool-wide off" };
                        app_pw_w.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                    },
                ),
            )
            .description("Show the union of models supported by all enabled pool accounts in the model picker."),
        )
        .item(
            SettingItem::new(
                "Default Thinking Effort",
                SettingField::dropdown(
                    effort_options,
                    move |cx: &App| {
                        let eff = app_eff_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.default_effort.as_str())
                            .unwrap_or("");
                        SharedString::from(if eff.is_empty() { "default" } else { eff }.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_eff_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy effort {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Default reasoning effort level passed to effort-capable Antigravity models."),
        );

    if let Some(ref st) = agy_state {
        for model_id in &st.available_models {
            let mid_read = model_id.clone();
            let mid_write = model_id.clone();
            let app_hm_r = app_state.clone();
            let app_hm_w = app_state.clone();
            model_group = model_group.item(
                SettingItem::new(
                    format!("Show {model_id}"),
                    SettingField::switch(
                        move |cx: &App| {
                            !app_hm_r
                                .read(cx)
                                .agy_link_state
                                .as_ref()
                                .map(|s| s.settings.hidden_models.iter().any(|h| h == &mid_read))
                                .unwrap_or(false)
                        },
                        move |visible: bool, cx: &mut App| {
                            app_hm_w.update(cx, |s, cx| {
                                let mut list = s
                                    .agy_link_state
                                    .as_ref()
                                    .map(|st| st.settings.hidden_models.clone())
                                    .unwrap_or_default();
                                if visible {
                                    list.retain(|h| h != &mid_write);
                                } else if !list.iter().any(|h| h == &mid_write) {
                                    list.push(mid_write.clone());
                                }
                                let arg = if list.is_empty() {
                                    "clear".to_string()
                                } else {
                                    list.join(",")
                                };
                                s.send_agy_command(format!("/agy hidden-models {arg}"), cx);
                            });
                        },
                    ),
                )
                .description("Include this model in the model selector."),
            );
        }
    }

    // 4. Image Generation
    let mut img_model_options = vec![(
        SharedString::from("default"),
        SharedString::from("Default (Gemini Flash)"),
    )];
    if let Some(ref st) = agy_state {
        for m in &st.available_models {
            img_model_options.push((SharedString::from(m.clone()), SharedString::from(m.clone())));
        }
    }
    let img_effort_options = vec![
        (SharedString::from("low"), SharedString::from("Low (Fast)")),
        (SharedString::from("medium"), SharedString::from("Medium")),
        (SharedString::from("high"), SharedString::from("High")),
    ];
    let img_timeout_options = vec![
        (SharedString::from("120"), SharedString::from("120 seconds")),
        (SharedString::from("240"), SharedString::from("240 seconds")),
        (SharedString::from("360"), SharedString::from("360 seconds")),
        (SharedString::from("600"), SharedString::from("600 seconds")),
    ];
    let img_retry_options = vec![
        (SharedString::from("0"), SharedString::from("0 retries")),
        (SharedString::from("1"), SharedString::from("1 retry")),
        (SharedString::from("2"), SharedString::from("2 retries")),
        (SharedString::from("3"), SharedString::from("3 retries")),
    ];

    let app_im_r = app_state.clone();
    let app_im_w = app_state.clone();
    let app_ie_r = app_state.clone();
    let app_ie_w = app_state.clone();
    let app_it_r = app_state.clone();
    let app_it_w = app_state.clone();
    let app_ir_r = app_state.clone();
    let app_ir_w = app_state.clone();

    let image_group = SettingGroup::new()
        .title("Image Generation")
        .item(
            SettingItem::new(
                "Image Generation Model",
                SettingField::dropdown(
                    img_model_options,
                    move |cx: &App| {
                        let m = app_im_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.image_model.as_str())
                            .unwrap_or("");
                        SharedString::from(if m.is_empty() { "default" } else { m }.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_im_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy image-model {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Model that drives generate_image calls."),
        )
        .item(
            SettingItem::new(
                "Image Thinking Effort",
                SettingField::dropdown(
                    img_effort_options,
                    move |cx: &App| {
                        let e = app_ie_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.image_effort.as_str())
                            .unwrap_or("low");
                        SharedString::from(e.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_ie_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy image-effort {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Reasoning effort used for the image generation prompt hop."),
        )
        .item(
            SettingItem::new(
                "Image Generation Timeout",
                SettingField::dropdown(
                    img_timeout_options,
                    move |cx: &App| {
                        let sec = app_it_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| (s.settings.image_timeout_ms / 1000).max(10))
                            .unwrap_or(240);
                        SharedString::from(sec.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_it_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy image-timeout {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Wall-clock timeout cap for a single image generation run."),
        )
        .item(
            SettingItem::new(
                "Image Generation Retries",
                SettingField::dropdown(
                    img_retry_options,
                    move |cx: &App| {
                        let r = app_ir_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.image_retries)
                            .unwrap_or(1);
                        SharedString::from(r.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_ir_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy image-retries {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Extra retry attempts after a retryable image generation failure."),
        );

    // 5. Proxy & Execution
    let perm_options = vec![
        (SharedString::from("skip"), SharedString::from("skip (Full Auto)")),
        (SharedString::from("accept-edits"), SharedString::from("accept-edits")),
        (SharedString::from("plan"), SharedString::from("plan (Read-Only)")),
    ];
    let mc_options = vec![
        (SharedString::from("0"), SharedString::from("Unlimited")),
        (SharedString::from("2"), SharedString::from("2 streams")),
        (SharedString::from("4"), SharedString::from("4 streams")),
        (SharedString::from("8"), SharedString::from("8 streams")),
    ];
    let proxy_options = vec![
        (SharedString::from("clear"), SharedString::from("System / Direct")),
        (SharedString::from("http://127.0.0.1:7890"), SharedString::from("http://127.0.0.1:7890")),
        (SharedString::from("socks5://127.0.0.1:7890"), SharedString::from("socks5://127.0.0.1:7890")),
        (SharedString::from("http://127.0.0.1:7897"), SharedString::from("http://127.0.0.1:7897")),
    ];

    let app_px_r = app_state.clone();
    let app_px_w = app_state.clone();
    let app_pm_r = app_state.clone();
    let app_pm_w = app_state.clone();
    let app_par_r = app_state.clone();
    let app_par_w = app_state.clone();
    let app_mc_r = app_state.clone();
    let app_mc_w = app_state.clone();
    let app_sup_r = app_state.clone();
    let app_sup_w = app_state.clone();

    let exec_group = SettingGroup::new()
        .title("Proxy & Execution")
        .item(
            SettingItem::new(
                "Global Upstream Proxy",
                SettingField::dropdown(
                    proxy_options,
                    move |cx: &App| {
                        let p = app_px_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.proxy_url.as_str())
                            .unwrap_or("");
                        SharedString::from(if p.is_empty() { "clear" } else { p }.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_px_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy proxy-global {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Global HTTP/SOCKS5 proxy for agy spawns and Google quota API calls (or use /agy proxy-global <url>)."),
        )
        .item(
            SettingItem::new(
                "Permission Mode",
                SettingField::dropdown(
                    perm_options,
                    move |cx: &App| {
                        let p = app_pm_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.permission_mode.as_str())
                            .unwrap_or("skip");
                        SharedString::from(p.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_pm_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy permission {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Tool permission mode passed to agy CLI turns."),
        )
        .item(
            SettingItem::new(
                "Same-Account Parallel Sessions",
                SettingField::switch(
                    move |cx: &App| {
                        app_par_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.parallel_same_account)
                            .unwrap_or(false)
                    },
                    move |val: bool, cx: &mut App| {
                        let cmd = if val { "/agy parallel on" } else { "/agy parallel off" };
                        app_par_w.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                    },
                ),
            )
            .description("Allow multiple sessions sharing the same account to run agy in parallel."),
        )
        .item(
            SettingItem::new(
                "Max Concurrent Streams",
                SettingField::dropdown(
                    mc_options,
                    move |cx: &App| {
                        let mc = app_mc_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.max_concurrent)
                            .unwrap_or(0);
                        SharedString::from(mc.to_string())
                    },
                    move |val: SharedString, cx: &mut App| {
                        app_mc_w.update(cx, |s, cx| {
                            s.send_agy_command(format!("/agy max-concurrent {val}"), cx);
                        });
                    },
                )
                .w(SETTINGS_DROPDOWN_W),
            )
            .description("Cap concurrent agy request streams across sessions."),
        )
        .item(
            SettingItem::new(
                "Suppress agy CLI Auto-Update",
                SettingField::switch(
                    move |cx: &App| {
                        app_sup_r
                            .read(cx)
                            .agy_link_state
                            .as_ref()
                            .map(|s| s.settings.suppress_agy_auto_update)
                            .unwrap_or(false)
                    },
                    move |val: bool, cx: &mut App| {
                        let cmd = if val { "/agy suppress-update on" } else { "/agy suppress-update off" };
                        app_sup_w.update(cx, |s, cx| s.send_agy_command(cmd.to_string(), cx));
                    },
                ),
            )
            .description("Prevent agy CLI background auto-updater from spawning extra child processes."),
        );

    let mut page = SettingPage::new("Antigravity")
        .icon(IconName::Bot)
        .group(pool_group);
    if let Some(dg) = details_group {
        page = page.group(dg);
    }
    page.group(rotation_group)
        .group(model_group)
        .group(image_group)
        .group(exec_group)
}
