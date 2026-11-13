use crate::app_state::{AppState, SettingsAddKind};
use crate::client::native_index::user_home;
use crate::i18n::{AppLocale, format_commas, format_tokens_km, tr};
use crate::icons::{PencilIcon, TrashIcon};
use crate::settings::{SETTINGS_BTN_W, config_path_field};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::setting::{SettingField, SettingGroup, SettingItem, SettingPage};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex,
    v_flex,
};
use gpui_kit::{
    App, Axis, Entity, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

/// One provider row in the merged "Configured Models" list: its `models.json`
/// entry (when it has one) plus the enabled-model catalog entry.
#[derive(Clone, Default)]
struct MergedProvider {
    id: String,
    display_name: Option<String>,
    api: Option<String>,
    base_url: Option<String>,
    has_config: bool,
    has_api_key: bool,
    enabled_count: usize,
    from_catalog: bool,
    models: Vec<MergedModel>,
}

#[derive(Clone, Default)]
struct MergedModel {
    id: String,
    name: Option<String>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    input: Vec<String>,
    reasoning: bool,
    enabled: bool,
    from_config: bool,
    from_catalog: bool,
    model_ref: String,
}

fn merged_providers(state: &AppState) -> Vec<MergedProvider> {
    let mut out: Vec<MergedProvider> = Vec::new();

    for prov in &state.configured_providers {
        out.push(MergedProvider {
            id: prov.id.clone(),
            display_name: prov.display_name.clone(),
            api: prov.api.clone(),
            base_url: prov.base_url.clone(),
            has_config: true,
            has_api_key: prov.has_api_key,
            enabled_count: 0,
            from_catalog: false,
            models: prov
                .models
                .iter()
                .map(|m| MergedModel {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    context_window: m.context_window,
                    max_tokens: m.max_tokens,
                    input: m.input.clone(),
                    reasoning: m.reasoning,
                    enabled: true,
                    from_config: true,
                    from_catalog: false,
                    model_ref: format!("{}/{}", prov.id, m.id),
                })
                .collect(),
        });
    }

    for grp in &state.enabled_models_overview.providers {
        let existing = out.iter_mut().find(|p| p.id == grp.id);
        let target = match existing {
            Some(p) => p,
            None => {
                out.push(MergedProvider {
                    id: grp.id.clone(),
                    display_name: Some(grp.name.clone()),
                    api: None,
                    base_url: None,
                    has_config: false,
                    has_api_key: false,
                    enabled_count: 0,
                    from_catalog: true,
                    models: Vec::new(),
                });
                out.last_mut().unwrap()
            }
        };
        target.from_catalog = true;
        target.enabled_count = grp.enabled_count;
        if target.display_name.is_none() {
            target.display_name = Some(grp.name.clone());
        }
        for m in &grp.models {
            match target.models.iter_mut().find(|x| x.id == m.id) {
                Some(slot) => {
                    slot.enabled = m.enabled;
                    slot.from_catalog = true;
                    slot.model_ref = m.model_ref.clone();
                    if slot.context_window.is_none() {
                        slot.context_window = m.context_window;
                    }
                    if slot.name.is_none() {
                        slot.name = Some(m.name.clone());
                    }
                }
                None => target.models.push(MergedModel {
                    id: m.id.clone(),
                    name: Some(m.name.clone()),
                    context_window: m.context_window,
                    max_tokens: None,
                    input: Vec::new(),
                    reasoning: false,
                    enabled: m.enabled,
                    from_config: false,
                    from_catalog: true,
                    model_ref: m.model_ref.clone(),
                }),
            }
        }
    }

    out.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    out
}

pub fn models_setting_page(app_state: Entity<AppState>, locale: AppLocale, cx: &App) -> SettingPage {
    let state = app_state.read(cx);
    let providers = merged_providers(state);
    let expanded = state.expanded_configured_providers.clone();
    let discovering_pid = state.discovering_provider_id.clone();
    let discover_error = state.discover_error.clone();
    let app_refresh = app_state.clone();
    let app_add_model = app_state.clone();

    let mut page = SettingPage::new(tr(locale, "settings.models"))
        .icon(IconName::Cpu)
        .group(
            SettingGroup::new()
                .title(tr(locale, "settings.providerCatalog"))
                .item(SettingItem::new(
                    tr(locale, "settings.addProviderPickerTitle"),
                    SettingField::render(move |_, _, _| {
                        let app_btn = app_add_model.clone();
                        Button::new("add-provider-model-btn")
                            .primary()
                            .small()
                            .h(px(28.))
                            .w(SETTINGS_BTN_W)
                            .icon(IconName::Plus)
                            .label(tr(locale, "settings.add"))
                            .on_click(move |_, _, cx| {
                                app_btn.update(cx, |s, cx| {
                                    s.settings_add_kind = Some(SettingsAddKind::AddProviderPicker);
                                    cx.notify();
                                });
                            })
                    }),
                ))
                .item(SettingItem::new(
                    tr(locale, "settings.refreshCatalog"),
                    SettingField::render(move |_, _, _| {
                        let app_btn = app_refresh.clone();
                        Button::new("refresh-catalog-btn")
                            .outline()
                            .small()
                            .h(px(28.))
                            .w(SETTINGS_BTN_W)
                            .icon(IconName::RefreshCw)
                            .label(tr(locale, "page.refresh"))
                            .on_click(move |_, _, cx| {
                                app_btn.update(cx, |s, cx| {
                                    if s.sidecar_connected {
                                        let client = s.client.clone();
                                        let entity = cx.entity().clone();
                                        cx.spawn(async move |_, cx| {
                                            let _ = client.refresh_model_catalog().await;
                                            let _ = entity.update(cx, |s, cx| {
                                                s.refresh_from_sidecar(cx);
                                            });
                                        })
                                        .detach();
                                    }
                                });
                            })
                    }),
                ))
                .item(
                    SettingItem::new(
                        tr(locale, "settings.modelsConfigPath"),
                        SettingField::render({
                            let app_cfg = app_state.clone();
                            move |_, _, cx| {
                                let p = user_home().join(".pi").join("agent").join("models.json");
                                let disp = p.to_string_lossy().into_owned();
                                config_path_field(
                                    "cfg-models-json",
                                    p,
                                    disp,
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
        );

    // One merged list: `models.json` entries and the enabled-model catalog.
    let app_cards = app_state.clone();
    let providers_for_list = providers.clone();
    let mut configured_group = SettingGroup::new().title(tr(locale, "settings.configuredModels"));
    if providers_for_list.is_empty() {
        configured_group = configured_group.item(SettingItem::new(
            tr(locale, "settings.noModelsConfigured"),
            SettingField::render(|_, _, _| Tag::secondary().small().child(SharedString::from("0"))),
        ));
    } else {
        // A single title comes from the group; the card list itself has no second
        // heading, so render it as a plain element item.
        configured_group = configured_group.item(
            SettingItem::render(move |_, _, cx| {
                    let mut col = v_flex().w_full().gap_2();
                    for (pidx, prov) in providers_for_list.iter().enumerate() {
                        let pid = prov.id.clone();
                        let title = prov.display_name.clone().filter(|n| !n.is_empty());
                        let is_open = expanded.contains(&pid);
                        let is_discovering = discovering_pid.as_deref() == Some(pid.as_str());
                        let discovery_error = discover_error
                            .as_ref()
                            .filter(|(id, _)| id == &pid)
                            .map(|(_, msg)| msg.clone());
                        let enabled_on = prov.enabled_count > 0
                            || prov.models.iter().any(|m| m.enabled && m.from_config);

                        let app_toggle = app_cards.clone();
                        let app_del_prov = app_cards.clone();
                        let app_discover = app_cards.clone();
                        let app_edit_prov = app_cards.clone();
                        let pid_toggle = pid.clone();
                        let pid_del = pid.clone();
                        let pid_disc = pid.clone();
                        let pid_add = pid.clone();
                        let pid_edit = pid.clone();
                        let pid_enable = pid.clone();
                        let toggle_on = enabled_on;
                        let has_config = prov.has_config;
                        let from_catalog = prov.from_catalog;

                        let meta_summary = match (&prov.api, &prov.base_url) {
                            (Some(api), Some(url)) => format!("{api} · {url}"),
                            (Some(api), None) => api.clone(),
                            (None, Some(url)) => url.clone(),
                            (None, None) => String::new(),
                        };

                        let header = h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .child(
                                h_flex()
                                    .id(("cfg-prov-hdr", pidx))
                                    .flex_1()
                                    .min_w_0()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .on_click(move |_, _, cx| {
                                        app_toggle.update(cx, |s, cx| {
                                            if s.expanded_configured_providers.contains(&pid_toggle) {
                                                s.expanded_configured_providers.remove(&pid_toggle);
                                            } else {
                                                s.expanded_configured_providers
                                                    .insert(pid_toggle.clone());
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        Icon::new(if is_open {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        })
                                        .xsmall(),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_sm()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .child(SharedString::from(pid.clone())),
                                    )
                                    .children(title.map(|name| {
                                        div()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(name))
                                    }))
                                    .child(Tag::secondary().small().child(SharedString::from(
                                        format_commas(prov.models.len()),
                                    )))
                                    .children(prov.has_api_key.then(|| {
                                        Tag::secondary().small().child("API Key")
                                    }))
                                    .children((!meta_summary.is_empty()).then(|| {
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_xs()
                                            .truncate()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(meta_summary))
                                    })),
                            )
                            // Enable switch + pencil + red trash.
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        Switch::new(("cfg-prov-enable", pidx))
                                            .checked(toggle_on)
                                            .on_click({
                                                let app_hdr_toggle = app_cards.clone();
                                                move |checked: &bool, _, cx| {
                                                    let next = *checked;
                                                    let prov = pid_enable.clone();
                                                    app_hdr_toggle.update(cx, |s, cx| {
                                                        s.set_provider_enabled(prov.clone(), next, cx);
                                                    });
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(("cfg-prov-edit", pidx))
                                            .ghost()
                                            .small()
                                            .h(px(26.))
                                            .w(px(26.))
                                            .icon(Icon::new(PencilIcon))
                                            .disabled(!has_config)
                                            .tooltip(tr(locale, "settings.editingProvider"))
                                            .on_click(move |_, _, cx| {
                                                app_edit_prov.update(cx, |s, cx| {
                                                    s.settings_add_kind =
                                                        Some(SettingsAddKind::EditProvider {
                                                            provider_id: pid_edit.clone(),
                                                        });
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new(("cfg-prov-del", pidx))
                                            .ghost()
                                            .small()
                                            .h(px(26.))
                                            .w(px(26.))
                                            .icon(Icon::new(TrashIcon))
                                            .text_color(cx.theme().danger)
                                            .disabled(!has_config)
                                            .tooltip(tr(locale, "settings.deleteGroup"))
                                            .on_click(move |_, _, cx| {
                                                app_del_prov.update(cx, |s, cx| {
                                                    s.delete_configured_provider(
                                                        pid_del.clone(),
                                                        cx,
                                                    );
                                                });
                                            }),
                                    ),
                            );

                        let mut card = v_flex()
                            .w_full()
                            .overflow_hidden()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .child(header);

                        if is_open {
                            let mut body = v_flex()
                                .w_full()
                                .gap_1p5()
                                .px_3()
                                .py_2()
                                .border_t_1()
                                .border_color(cx.theme().border);

                            if prov.models.is_empty() {
                                body = body.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(tr(locale, "settings.noModelsConfigured")),
                                );
                            } else {
                                for (midx, m) in prov.models.iter().enumerate() {
                                    let app_m_enable = app_cards.clone();
                                    let app_m_edit = app_cards.clone();
                                    let app_m_del = app_cards.clone();
                                    let pid_m = pid.clone();
                                    let mid = m.id.clone();
                                    let mref = m.model_ref.clone();
                                    let m_enabled = m.enabled;
                                    let m_from_config = m.from_config;
                                    let m_from_catalog = m.from_catalog;
                                    let pid_me = pid.clone();
                                    let mid_me = m.id.clone();
                                    let pid_md = pid.clone();
                                    let mid_md = m.id.clone();

                                    let title = m
                                        .name
                                        .as_deref()
                                        .filter(|n| !n.is_empty())
                                        .unwrap_or(&m.id)
                                        .to_string();
                                    let mut meta_parts = vec![m.id.clone()];
                                    if let Some(cw) = m.context_window {
                                        meta_parts.push(format!("{} ctx", format_tokens_km(cw)));
                                    }
                                    if let Some(mt) = m.max_tokens {
                                        meta_parts.push(format!("{} max", format_tokens_km(mt)));
                                    }
                                    if !m.input.is_empty() {
                                        meta_parts.push(m.input.join("+"));
                                    }
                                    if m.reasoning {
                                        meta_parts.push("reasoning".to_string());
                                    }

                                    body = body.child(
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .px_2p5()
                                            .py_1p5()
                                            .rounded_sm()
                                            .bg(cx.theme().secondary)
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_semibold()
                                                            .truncate()
                                                            .child(SharedString::from(title)),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .truncate()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(SharedString::from(
                                                                meta_parts.join(" · "),
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_shrink_0()
                                                    .items_center()
                                                    .gap_1p5()
                                                    .child(
                                                        Switch::new((
                                                            "cfg-model-enable",
                                                            pidx * 1000 + midx,
                                                        ))
                                                            .checked(m_enabled)
                                                            .on_click(move |checked: &bool, _, cx| {
                                                                let next = *checked;
                                                                let prov = pid_m.clone();
                                                                let id = mid.clone();
                                                                let reference = mref.clone();
                                                                let r = if m_from_catalog {
                                                                    reference.clone()
                                                                } else {
                                                                    format!("{}/{}", prov, id)
                                                                };
                                                                app_m_enable.update(cx, |s, cx| {
                                                                    s.set_model_enabled(r.clone(), next, cx);
                                                                });
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new((
                                                            "cfg-model-edit",
                                                            pidx * 1000 + midx,
                                                        ))
                                                            .ghost()
                                                            .small()
                                                            .h(px(26.))
                                                            .w(px(26.))
                                                            .icon(Icon::new(PencilIcon))
                                                            .disabled(!m_from_config)
                                                            .tooltip(tr(locale, "settings.editingModel"))
                                                            .on_click(move |_, _, cx| {
                                                                app_m_edit.update(cx, |s, cx| {
                                                                    s.settings_add_kind = Some(
                                                                        SettingsAddKind::EditModel {
                                                                            provider_id: pid_me
                                                                                .clone(),
                                                                            model_id: mid_me
                                                                                .clone(),
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                });
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new((
                                                            "cfg-model-del",
                                                            pidx * 1000 + midx,
                                                        ))
                                                            .ghost()
                                                            .small()
                                                            .h(px(26.))
                                                            .w(px(26.))
                                                            .icon(Icon::new(TrashIcon))
                                                            .text_color(cx.theme().danger)
                                                            .disabled(!m_from_config)
                                                            .tooltip(tr(locale, "settings.deleteGroup"))
                                                            .on_click(move |_, _, cx| {
                                                                app_m_del.update(cx, |s, cx| {
                                                                    s.delete_model_from_configured_provider(
                                                                        pid_md.clone(),
                                                                        mid_md.clone(),
                                                                        cx,
                                                                    );
                                                                });
                                                            }),
                                                    ),
                                            ),
                                    );
                                }
                            }

                            // Provider footer actions: discover (catalog providers) and add model.
                            let app_footer = app_cards.clone();
                            body = body.child(
                                h_flex()
                                    .w_full()
                                    .flex_wrap()
                                    .justify_end()
                                    .gap_2()
                                    .pt_1()
                                    .children(from_catalog.then(|| {
                                        let app_d = app_discover.clone();
                                        let target = pid_disc.clone();
                                        Button::new(("cfg-prov-discover", pidx))
                                            .outline()
                                            .small()
                                            .h(px(28.))
                                            .w(SETTINGS_BTN_W)
                                            .disabled(is_discovering)
                                            .icon(IconName::Search)
                                            .label(tr(locale, "settings.discoverModels"))
                                            .on_click(move |_, _, cx| {
                                                let target = target.clone();
                                                app_d.update(cx, |s, cx| {
                                                    s.discover_models_for_provider(target, cx);
                                                });
                                            })
                                    }))
                                    .child({
                                        let target = pid_add.clone();
                                        Button::new(("cfg-prov-add-model", pidx))
                                            .primary()
                                            .small()
                                            .h(px(28.))
                                            .w(SETTINGS_BTN_W)
                                            .icon(IconName::Plus)
                                            .label(tr(locale, "settings.addModel"))
                                            .on_click(move |_, _, cx| {
                                                app_footer.update(cx, |s, cx| {
                                                    s.settings_add_kind =
                                                        Some(SettingsAddKind::AddModelToProvider {
                                                            provider_id: target.clone(),
                                                        });
                                                    cx.notify();
                                                });
                                            })
                                    }),
                            );

                            if let Some(err) = discovery_error {
                                body = body.child(
                                    div()
                                        .w_full()
                                        .text_xs()
                                        .text_color(cx.theme().danger)
                                        .child(SharedString::from(err)),
                                );
                            }

                            card = card.child(body);
                        }

                        col = col.child(card);
                    }
                col
            }),
        );
    }
    page = page.group(configured_group);

    page
}
