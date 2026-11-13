use crate::app_state::{AppState, SettingsAddKind};
use crate::client::native_index::user_home;
use crate::i18n::{AppLocale, format_commas, tr};
use crate::settings::{SETTINGS_BTN_W, config_path_field};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{IconName, Sizable as _, h_flex};
use gpui_kit::{App, Axis, Entity, ParentElement as _, SharedString, Styled as _, px};
use serde_json::json;

pub fn mcp_setting_page(app_state: Entity<AppState>, locale: AppLocale, cx: &App) -> SettingPage {
    let state = app_state.read(cx);
    let servers = state.mcp_servers.clone();
    let active_cwd = state.active_cwd.clone();
    let app_trust = app_state.clone();
    let app_trust_set = app_state.clone();
    let app_add_mcp = app_state.clone();

    let mut servers_group = SettingGroup::new()
        .title(tr(locale, "settings.mcpServers"))
        .item(SettingItem::new(
            tr(locale, "settings.addMcpServer"),
            SettingField::render(move |_, _, _| {
                let app_btn = app_add_mcp.clone();
                Button::new("add-mcp-server-btn")
                    .primary()
                    .small()
                    .h(px(28.))
                    .w(SETTINGS_BTN_W)
                    .icon(IconName::Plus)
                    .label(tr(locale, "settings.add"))
                    .on_click(move |_, _, cx| {
                        app_btn.update(cx, |s, cx| {
                            s.settings_add_kind = Some(SettingsAddKind::McpServer);
                            cx.notify();
                        });
                    })
            }),
        ));

    if servers.is_empty() {
        servers_group = servers_group.item(
            SettingItem::new(
                tr(locale, "settings.emptySidecar"),
                SettingField::render(|_, _, _| Tag::secondary().small().child(SharedString::from("0"))),
            ),
        );
    } else {
        for (idx, srv) in servers.into_iter().enumerate() {
            let name = srv.name.clone();
            let status = srv.status.clone();
            let scope = srv.scope.clone();
            let transport = srv.transport.clone();
            let tools_count = srv.tools_count;
            let app_toggle_read = app_state.clone();
            let app_toggle_write = app_state.clone();
            let app_test = app_state.clone();
            let name_read = name.clone();
            let scope_read = scope.clone();
            let name_write = name.clone();
            let scope_write = scope.clone();

            let desc = if let Some(err) = srv.error {
                format!("{scope} · {transport} · {err}")
            } else if tools_count > 0 {
                format!("{scope} · {transport} · {} tools", format_commas(tools_count))
            } else {
                format!("{scope} · {transport}")
            };

            servers_group = servers_group.item(
                SettingItem::new(
                    SharedString::from(name.clone()),
                    SettingField::render(move |_, _, cx| {
                        let app_t = app_test.clone();
                        let srv_name = name.clone();
                        let srv_scope = scope.clone();
                        let enabled_now = app_toggle_read
                            .read(cx)
                            .mcp_servers
                            .iter()
                            .find(|s| s.name == name_read && s.scope == scope_read)
                            .map(|s| s.enabled)
                            .unwrap_or(true);
                        let app_sw = app_toggle_write.clone();
                        let sw_name = name_write.clone();
                        let sw_scope = scope_write.clone();

                        h_flex()
                            .flex_wrap()
                            .justify_end()
                            .gap_2()
                            .items_center()
                            .child(Tag::secondary().small().child(SharedString::from(status.clone())))
                            .child(
                                Button::new(("toggle-mcp", idx))
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .w(SETTINGS_BTN_W)
                                    .icon(if enabled_now {
                                        IconName::Close
                                    } else {
                                        IconName::Check
                                    })
                                    .label(if enabled_now {
                                        tr(locale, "settings.disable")
                                    } else {
                                        tr(locale, "settings.enable")
                                    })
                                    .on_click(move |_, _, cx| {
                                        let next = !enabled_now;
                                        let n = sw_name.clone();
                                        let sc = sw_scope.clone();
                                        app_sw.update(cx, |s, cx| {
                                            if let Some(target) = s
                                                .mcp_servers
                                                .iter_mut()
                                                .find(|x| x.name == n && x.scope == sc)
                                            {
                                                target.enabled = next;
                                                target.status = if next {
                                                    "configured".to_string()
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
                                                        .mcp_action(json!({
                                                            "action": if next { "enable" } else { "disable" },
                                                            "scope": sc,
                                                            "name": n,
                                                            "cwd": cwd,
                                                        }))
                                                        .await
                                                    {
                                                        let _ = entity.update(cx, |s, cx| {
                                                            s.apply_mcp_response(&res);
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
                            .child(
                                Button::new(("test-mcp", idx))
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .w(SETTINGS_BTN_W)
                                    .icon(IconName::RefreshCw)
                                    .label(tr(locale, "settings.test"))
                                    .on_click(move |_, _, cx| {
                                        app_t.update(cx, |s, cx| {
                                            if s.sidecar_connected {
                                                let client = s.client.clone();
                                                let n = srv_name.clone();
                                                let sc = srv_scope.clone();
                                                let cwd = s.active_cwd.to_string_lossy().into_owned();
                                                let entity = cx.entity().clone();
                                                cx.spawn(async move |_, cx| {
                                                    let _ = client
                                                        .test_mcp_server(json!({
                                                            "name": n,
                                                            "scope": sc,
                                                            "cwd": cwd,
                                                        }))
                                                        .await;
                                                    if let Ok(overview) = client.get_mcp(Some(&cwd)).await {
                                                        let _ = entity.update(cx, |s, cx| {
                                                            s.apply_mcp_response(&overview);
                                                            cx.notify();
                                                        });
                                                    }
                                                })
                                                .detach();
                                            }
                                        });
                                    }),
                            )
                    }),
                )
                .description(desc),
            );
        }
    }

    let proj_mcp_path = active_cwd.join(".pi").join("mcp.json");
    let proj_mcp_display = proj_mcp_path.to_string_lossy().into_owned();

    SettingPage::new(tr(locale, "settings.mcp"))
        .icon(IconName::Network)
        .group(
            SettingGroup::new()
                .title(tr(locale, "settings.mcpTrust"))
                .item(SettingItem::new(
                    tr(locale, "settings.trustProjectMcpTitle"),
                    SettingField::switch(
                        move |cx: &App| app_trust.read(cx).mcp_project_trusted,
                        move |val: bool, cx: &mut App| {
                            app_trust_set.update(cx, |s, cx| {
                                s.mcp_project_trusted = val;
                                let cwd = s.active_cwd.to_string_lossy().into_owned();
                                if s.sidecar_connected {
                                    let client = s.client.clone();
                                    let entity = cx.entity().clone();
                                    cx.spawn(async move |_, cx| {
                                        let _ = client.set_project_trust(&cwd, val).await;
                                        if let Ok(overview) = client.get_mcp(Some(&cwd)).await {
                                            let _ = entity.update(cx, |s, cx| {
                                                s.apply_mcp_response(&overview);
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
                .item(
                    SettingItem::new(
                        tr(locale, "settings.mcpGlobalPath"),
                        SettingField::render({
                            let app_cfg = app_state.clone();
                            move |_, _, cx| {
                                let p = user_home().join(".pi").join("agent").join("mcp.json");
                                let disp = p.to_string_lossy().into_owned();
                                config_path_field(
                                    "cfg-mcp-global",
                                    p,
                                    disp,
                                    false,
                                    locale,
                                    app_cfg.clone(),
                                    cx,
                                )
                            }
                        }),
                    )
                    .layout(Axis::Vertical),
                )
                .item(
                    SettingItem::new(
                        tr(locale, "settings.mcpProjectPath"),
                        SettingField::render({
                            let app_cfg = app_state.clone();
                            let p = proj_mcp_path.clone();
                            let disp = proj_mcp_display.clone();
                            move |_, _, cx| {
                                config_path_field(
                                    "cfg-mcp-project",
                                    p.clone(),
                                    disp.clone(),
                                    true,
                                    locale,
                                    app_cfg.clone(),
                                    cx,
                                )
                            }
                        }),
                    )
                    .layout(Axis::Vertical),
                ),
        )
        .group(servers_group)
}
