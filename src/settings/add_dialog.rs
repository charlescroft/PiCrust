use crate::app_state::{AppState, SettingsAddKind};
use crate::client::types::ConfiguredModelInfo;
use crate::i18n::{AppLocale, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, IndexPath, Selectable as _, Sizable as _,
    StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
struct ApiEndpointOption {
    id: &'static str,
    label: &'static str,
}

#[derive(Clone, Debug)]
struct ApiOptionItem {
    id: SharedString,
    label: SharedString,
}

impl SelectItem for ApiOptionItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

const API_ENDPOINT_OPTIONS: &[ApiEndpointOption] = &[
    ApiEndpointOption {
        id: "openai-completions",
        label: "OpenAI Completions (/v1/chat/completions)",
    },
    ApiEndpointOption {
        id: "openai-responses",
        label: "OpenAI Responses (/v1/responses)",
    },
    ApiEndpointOption {
        id: "anthropic-messages",
        label: "Anthropic Messages (/v1/messages)",
    },
    ApiEndpointOption {
        id: "google-generative-ai",
        label: "Google Generative AI (/v1beta/models/...:generateContent)",
    },
];

pub struct SettingsAddDialogView {
    app_state: Entity<AppState>,
    last_kind: Option<SettingsAddKind>,
    last_locale: AppLocale,
    busy: bool,
    error: Option<String>,
    scope: String, // "global" | "project"
    api_type: String,
    api_select: Entity<SelectState<SearchableVec<ApiOptionItem>>>,

    // AddModelToProvider toggles
    model_input_text: bool,
    model_input_image: bool,
    model_reasoning: bool,

    // Shared / per-form inputs
    input_a: Entity<InputState>,
    input_b: Entity<InputState>,
    input_c: Entity<InputState>,
    input_d: Entity<InputState>,
    textarea_a: Entity<TextareaState>,
    provider_search_input: Entity<InputState>,

    // Skills search results
    skill_results: Vec<(String, String)>,
}

impl Render for SettingsAddDialogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_dialog(window, cx)
    }
}

impl SettingsAddDialogView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let initial_locale = app_state.read(cx).locale;
        let api_items: Vec<ApiOptionItem> = API_ENDPOINT_OPTIONS
            .iter()
            .map(|o| ApiOptionItem {
                id: SharedString::from(o.id),
                label: SharedString::from(o.label),
            })
            .collect();
        let api_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(api_items),
                Some(IndexPath::default().row(0)),
                window,
                cx,
            )
        });
        cx.subscribe(
            &api_select,
            |this, _, ev: &SelectEvent<SearchableVec<ApiOptionItem>>, cx| {
                if let SelectEvent::Confirm(Some(val)) = ev {
                    this.api_type = val.to_string();
                    cx.notify();
                }
            },
        )
        .detach();

        let provider_search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr(initial_locale, "settings.searchProviders"))
        });
        cx.subscribe(&provider_search_input, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        Self {
            app_state,
            last_kind: None,
            last_locale: initial_locale,
            busy: false,
            error: None,
            scope: "global".to_string(),
            api_type: "openai-completions".to_string(),
            api_select,
            model_input_text: true,
            model_input_image: false,
            model_reasoning: false,
            input_a: cx.new(|cx| InputState::new(window, cx)),
            input_b: cx.new(|cx| InputState::new(window, cx)),
            input_c: cx.new(|cx| InputState::new(window, cx)),
            input_d: cx.new(|cx| InputState::new(window, cx)),
            textarea_a: cx.new(|cx| TextareaState::new(window, cx).rows(5)),
            provider_search_input,
            skill_results: Vec::new(),
        }
    }

    fn sync_for_kind(&mut self, kind: &SettingsAddKind, window: &mut Window, cx: &mut Context<Self>) {
        if self.last_kind.as_ref() == Some(kind) {
            return;
        }
        self.last_kind = Some(kind.clone());
        self.busy = false;
        self.error = None;
        self.scope = "global".to_string();
        self.skill_results.clear();
        self.model_input_text = true;
        self.model_input_image = false;
        self.model_reasoning = false;

        self.input_a.update(cx, |s, cx| s.set_value("", window, cx));
        self.input_b.update(cx, |s, cx| s.set_value("", window, cx));
        self.input_c.update(cx, |s, cx| s.set_value("", window, cx));
        self.input_d.update(cx, |s, cx| s.set_value("", window, cx));
        self.textarea_a.update(cx, |s, cx| s.set_value("", window, cx));
        self.provider_search_input
            .update(cx, |s, cx| s.set_value("", window, cx));

        match kind {
            SettingsAddKind::ProviderModel => {
                self.api_type = "openai-completions".to_string();
                self.api_select.update(cx, |sel, cx| {
                    sel.set_selected_index(Some(IndexPath::default().row(0)), window, cx);
                });
            }
            SettingsAddKind::AddModelToProvider { .. } => {
                self.input_c.update(cx, |s, cx| s.set_value("128000", window, cx));
                self.input_d.update(cx, |s, cx| s.set_value("8192", window, cx));
            }
            SettingsAddKind::EditProvider { provider_id } => {
                let prefill = {
                    let state = self.app_state.read(cx);
                    state
                        .configured_providers
                        .iter()
                        .find(|p| &p.id == provider_id)
                        .map(|prov| {
                            let raw_key = state
                                .configured_providers_raw
                                .get("providers")
                                .and_then(|p| p.get(provider_id))
                                .and_then(|p| p.get("apiKey"))
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            (
                                prov.display_name.clone().unwrap_or_default(),
                                prov.base_url.clone().unwrap_or_default(),
                                prov.api
                                    .clone()
                                    .unwrap_or_else(|| "openai-completions".into()),
                                raw_key,
                            )
                        })
                };
                if let Some((name, base, api, raw_key)) = prefill {
                    self.api_type = api.clone();
                    let row_ix = API_ENDPOINT_OPTIONS
                        .iter()
                        .position(|o| o.id == api)
                        .unwrap_or(0);
                    self.input_a.update(cx, |s, cx| s.set_value(name, window, cx));
                    self.input_b.update(cx, |s, cx| s.set_value(base, window, cx));
                    self.input_c.update(cx, |s, cx| s.set_value(raw_key, window, cx));
                    self.api_select.update(cx, |sel, cx| {
                        sel.set_selected_index(Some(IndexPath::default().row(row_ix)), window, cx);
                    });
                }
            }
            SettingsAddKind::EditModel {
                provider_id,
                model_id,
            } => {
                let prefill = {
                    let state = self.app_state.read(cx);
                    state
                        .configured_providers
                        .iter()
                        .find(|p| &p.id == provider_id)
                        .and_then(|prov| prov.models.iter().find(|m| &m.id == model_id))
                        .cloned()
                };
                if let Some(m) = prefill {
                    self.model_input_text = m.input.iter().any(|i| i == "text");
                    self.model_input_image = m.input.iter().any(|i| i == "image");
                    self.model_reasoning = m.reasoning;
                    self.input_a.update(cx, |s, cx| s.set_value(m.id.clone(), window, cx));
                    self.input_b.update(cx, |s, cx| {
                        s.set_value(m.name.clone().unwrap_or_default(), window, cx)
                    });
                    self.input_c.update(cx, |s, cx| {
                        s.set_value(
                            m.context_window.map(|v| v.to_string()).unwrap_or_default(),
                            window,
                            cx,
                        )
                    });
                    self.input_d.update(cx, |s, cx| {
                        s.set_value(
                            m.max_tokens.map(|v| v.to_string()).unwrap_or_default(),
                            window,
                            cx,
                        )
                    });
                }
            }
            SettingsAddKind::McpServer => {
                self.textarea_a.update(cx, |s, cx| {
                    s.set_value("{\n  \"mcpServers\": {\n    \n  }\n}", window, cx)
                });
            }
            _ => {}
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.app_state.update(cx, |s, cx| {
            s.settings_add_kind = None;
            cx.notify();
        });
    }

    fn submit(&mut self, kind: SettingsAddKind, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let a = self.input_a.read(cx).value().trim().to_string();
        let b = self.input_b.read(cx).value().trim().to_string();
        let c = self.input_c.read(cx).value().trim().to_string();
        let d = self.input_d.read(cx).value().trim().to_string();
        let ta = self.textarea_a.read(cx).value().to_string();

        if let SettingsAddKind::EditProvider { provider_id } = kind {
            let name = a.clone();
            let base = b.clone();
            let key = c.clone();
            let api = self.api_type.clone();
            self.app_state.update(cx, |s, cx| {
                s.update_configured_provider(provider_id, name, base, api, key, cx);
                s.settings_add_kind = None;
                cx.notify();
            });
            return;
        }

        if let SettingsAddKind::EditModel {
            provider_id,
            model_id,
        } = kind
        {
            if a.is_empty() {
                self.error = Some("Model ID is required".to_string());
                cx.notify();
                return;
            }
            let cw = c.parse::<u64>().ok().filter(|v| *v > 0);
            let mt = d.parse::<u64>().ok().filter(|v| *v > 0);
            let mut input_types = Vec::new();
            if self.model_input_text {
                input_types.push("text".to_string());
            }
            if self.model_input_image {
                input_types.push("image".to_string());
            }
            if input_types.is_empty() {
                input_types.push("text".to_string());
            }
            let model = ConfiguredModelInfo {
                id: a.clone(),
                name: if b.is_empty() { None } else { Some(b) },
                context_window: cw,
                max_tokens: mt,
                input: input_types,
                reasoning: self.model_reasoning,
                thinking_level_map: Default::default(),
            };
            self.app_state.update(cx, |s, cx| {
                s.update_configured_model(provider_id, model_id, model, cx);
                s.settings_add_kind = None;
                cx.notify();
            });
            return;
        }

        if let SettingsAddKind::AddModelToProvider { provider_id } = kind {
            if a.is_empty() {
                self.error = Some("Model ID is required".to_string());
                cx.notify();
                return;
            }
            let cw = c.parse::<u64>().ok().filter(|v| *v > 0);
            let mt = d.parse::<u64>().ok().filter(|v| *v > 0);
            let mut input_types = Vec::new();
            if self.model_input_text {
                input_types.push("text".to_string());
            }
            if self.model_input_image {
                input_types.push("image".to_string());
            }
            if input_types.is_empty() {
                input_types.push("text".to_string());
            }
            let model = ConfiguredModelInfo {
                id: a.clone(),
                name: if b.is_empty() { None } else { Some(b) },
                context_window: cw,
                max_tokens: mt,
                input: input_types,
                reasoning: self.model_reasoning,
                thinking_level_map: Default::default(),
            };
            self.app_state.update(cx, |s, cx| {
                s.add_model_to_configured_provider(provider_id, model, cx);
                s.settings_add_kind = None;
                cx.notify();
            });
            return;
        }

        let state = self.app_state.read(cx);
        if !state.sidecar_connected {
            self.error = Some("Sidecar disconnected".to_string());
            cx.notify();
            return;
        }
        let client = state.client.clone();
        let cwd = state.active_cwd.to_string_lossy().into_owned();
        let scope = self.scope.clone();
        let api_type = self.api_type.clone();
        let app_state = self.app_state.clone();

        self.busy = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let mut expand_provider_id: Option<String> = None;
            let res: anyhow::Result<()> = match kind {
                SettingsAddKind::AddProviderPicker
                | SettingsAddKind::AddModelToProvider { .. }
                | SettingsAddKind::EditProvider { .. }
                | SettingsAddKind::EditModel { .. } => Ok(()),
                SettingsAddKind::ProviderModel => {
                    if a.is_empty() || b.is_empty() {
                        Err(anyhow::anyhow!("Provider ID and Base URL are required"))
                    } else {
                        let mut cfg = client
                            .get_models_config()
                            .await
                            .unwrap_or_else(|_| json!({ "providers": {} }));
                        if cfg.get("providers").and_then(Value::as_object).is_none() {
                            cfg["providers"] = json!({});
                        }
                        let pid = a.clone();
                        expand_provider_id = Some(pid.clone());
                        let prov_entry = cfg["providers"]
                            .as_object_mut()
                            .unwrap()
                            .entry(pid)
                            .or_insert_with(|| json!({ "models": [] }));
                        prov_entry["baseUrl"] = json!(b);
                        prov_entry["api"] = json!(api_type);
                        if !c.is_empty() {
                            prov_entry["apiKey"] = json!(c);
                        }
                        if prov_entry.get("models").and_then(Value::as_array).is_none() {
                            prov_entry["models"] = json!([]);
                        }
                        client.put_models_config(cfg).await.map(|_| ())
                    }
                }
                SettingsAddKind::ProviderApiKey { provider_id, .. } => {
                    if a.is_empty() {
                        Err(anyhow::anyhow!("API key required"))
                    } else {
                        client.set_api_key(&provider_id, &a).await.map(|_| ())
                    }
                }
                SettingsAddKind::McpServer => {
                    if ta.trim().is_empty() {
                        Err(anyhow::anyhow!("Server config, command, or URL required"))
                    } else {
                        let mut body = json!({
                            "action": "add",
                            "scope": scope,
                            "text": ta,
                            "cwd": cwd,
                        });
                        if !a.is_empty() {
                            body["name"] = json!(a);
                        }
                        client.mcp_action(body).await.map(|_| ())
                    }
                }
                SettingsAddKind::SubagentProfile => {
                    if a.is_empty() {
                        Err(anyhow::anyhow!("Profile name required"))
                    } else {
                        let mut profile = json!({
                            "name": a,
                            "description": b,
                            "systemPrompt": ta,
                            "enabled": true,
                        });
                        if !c.is_empty() {
                            profile["model"] = json!(c);
                        }
                        client
                            .save_subagent_profile(json!({
                                "cwd": cwd,
                                "scope": scope,
                                "profile": profile,
                            }))
                            .await
                            .map(|_| ())
                    }
                }
                SettingsAddKind::Skill => {
                    if a.is_empty() {
                        Err(anyhow::anyhow!("Skill package name required"))
                    } else {
                        client
                            .install_skill(&a, &scope, Some(&cwd))
                            .await
                            .map(|_| ())
                    }
                }
                SettingsAddKind::Plugin => {
                    if a.is_empty() {
                        Err(anyhow::anyhow!("Plugin source required"))
                    } else {
                        client
                            .plugin_action(json!({
                                "cwd": cwd,
                                "action": "install",
                                "source": a,
                                "scope": scope,
                            }))
                            .await
                            .map(|_| ())
                    }
                }
            };

            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match res {
                    Ok(()) => {
                        app_state.update(cx, |s, cx| {
                            if let Some(pid) = expand_provider_id.take() {
                                s.expanded_configured_providers.insert(pid);
                            }
                            s.settings_add_kind = None;
                            s.refresh_from_sidecar(cx);
                            s.refresh_settings_data(cx);
                        });
                    }
                    Err(err) => {
                        view.error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn search_skills(&mut self, cx: &mut Context<Self>) {
        let q = self.input_a.read(cx).value().trim().to_string();
        if q.is_empty() || self.busy {
            return;
        }
        let client = self.app_state.read(cx).client.clone();
        self.busy = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = client.search_skills(&q).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match res {
                    Ok(val) => {
                        view.skill_results = val
                            .get("results")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|item| {
                                        let pkg = item.get("package")?.as_str()?.to_string();
                                        let inst = item
                                            .get("installs")
                                            .and_then(Value::as_str)
                                            .unwrap_or("")
                                            .to_string();
                                        Some((pkg, inst))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                    Err(e) => {
                        view.error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn install_skill_pkg(&mut self, pkg: String, cx: &mut Context<Self>) {
        let state = self.app_state.read(cx);
        let client = state.client.clone();
        let cwd = state.active_cwd.to_string_lossy().into_owned();
        let scope = self.scope.clone();
        let app_state = self.app_state.clone();
        self.busy = true;
        self.error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = client.install_skill(&pkg, &scope, Some(&cwd)).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match res {
                    Ok(_) => {
                        app_state.update(cx, |s, cx| {
                            s.settings_add_kind = None;
                            s.refresh_settings_data(cx);
                        });
                    }
                    Err(e) => {
                        view.error = Some(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn render_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(kind) = self.app_state.read(cx).settings_add_kind.clone() else {
            self.last_kind = None;
            return div().into_any_element();
        };
        let (locale, auth_providers, oauth_providers, sidecar_url) = {
            let s = self.app_state.read(cx);
            (
                s.locale,
                s.auth_providers.clone(),
                s.oauth_providers.clone(),
                s.client.base_url.clone(),
            )
        };
        if self.last_locale != locale {
            self.last_locale = locale;
            self.provider_search_input.update(cx, |s, cx| {
                s.set_placeholder(tr(locale, "settings.searchProviders"), window, cx);
            });
        }
        self.sync_for_kind(&kind, window, cx);

        let title = match &kind {
            SettingsAddKind::AddProviderPicker => {
                tr(locale, "settings.addProviderPickerTitle").to_string()
            }
            SettingsAddKind::ProviderModel => tr(locale, "settings.addProviderModel").to_string(),
            SettingsAddKind::AddModelToProvider { provider_id } => {
                format!("{} — {provider_id}", tr(locale, "settings.addModel"))
            }
            SettingsAddKind::EditProvider { provider_id } => {
                format!("{} — {provider_id}", tr(locale, "settings.editingProvider"))
            }
            SettingsAddKind::EditModel {
                provider_id,
                model_id,
            } => format!(
                "{} — {provider_id}/{model_id}",
                tr(locale, "settings.editingModel")
            ),
            SettingsAddKind::ProviderApiKey { provider_name, .. } => {
                format!("{} — {provider_name}", tr(locale, "settings.setApiKeyTitle"))
            }
            SettingsAddKind::McpServer => tr(locale, "settings.addMcpServer").to_string(),
            SettingsAddKind::SubagentProfile => tr(locale, "settings.addSubagent").to_string(),
            SettingsAddKind::Skill => tr(locale, "settings.installSkill").to_string(),
            SettingsAddKind::Plugin => tr(locale, "settings.installPlugin").to_string(),
        };

        let mut body = v_flex().gap_2p5();

        if matches!(
            kind,
            SettingsAddKind::McpServer
                | SettingsAddKind::SubagentProfile
                | SettingsAddKind::Skill
                | SettingsAddKind::Plugin
        ) {
            body = body.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "settings.scope")),
                    )
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(
                                Button::new("add-dlg-scope-global")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .selected(self.scope == "global")
                                    .label(tr(locale, "settings.scopeGlobal"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scope = "global".to_string();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("add-dlg-scope-proj")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .selected(self.scope == "project")
                                    .label(tr(locale, "settings.scopeProject"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scope = "project".to_string();
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }

        let is_picker = matches!(kind, SettingsAddKind::AddProviderPicker);

        match &kind {
            SettingsAddKind::AddProviderPicker => {
                let q = self.provider_search_input.read(cx).value().trim().to_lowercase();
                let show_custom = q.is_empty()
                    || "custom".contains(&q)
                    || "openai".contains(&q)
                    || "anthropic".contains(&q);
                let avail_oauth: Vec<_> = oauth_providers
                    .into_iter()
                    .filter(|(id, name, logged_in)| {
                        !*logged_in
                            && (q.is_empty()
                                || id.to_lowercase().contains(&q)
                                || name.to_lowercase().contains(&q))
                    })
                    .collect();
                let avail_apikey: Vec<_> = auth_providers
                    .into_iter()
                    .filter(|(id, name, configured)| {
                        !*configured
                            && (q.is_empty()
                                || id.to_lowercase().contains(&q)
                                || name.to_lowercase().contains(&q))
                    })
                    .collect();

                let mut picker_scroll = v_flex()
                    .id("add-provider-picker-scroll")
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .gap_3();

                if show_custom {
                    let app_custom = self.app_state.clone();
                    picker_scroll = picker_scroll.child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.addProviderModel")),
                            )
                            .child(
                                h_flex()
                                    .id("picker-custom-card")
                                    .w_full()
                                    .p_3()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary)
                                    .justify_between()
                                    .items_center()
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(cx.theme().primary))
                                    .on_click(move |_, _, cx| {
                                        app_custom.update(cx, |s, cx| {
                                            s.settings_add_kind =
                                                Some(SettingsAddKind::ProviderModel);
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .child("OpenAI / Anthropic / Google compatible"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "settings.customEndpoint")),
                                            ),
                                    )
                                    .child(IconName::Plus),
                            ),
                    );
                }

                if !avail_oauth.is_empty() {
                    let mut oauth_grid = v_flex().gap_1p5().child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "settings.subscriptions")),
                    );
                    for (idx, (pid, pname, _)) in avail_oauth.into_iter().enumerate() {
                        let login_url = format!(
                            "{}/api/auth/login/{pid}",
                            sidecar_url.trim_end_matches('/')
                        );
                        oauth_grid = oauth_grid.child(
                            h_flex()
                                .id(("picker-oauth", idx))
                                .w_full()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .justify_between()
                                .items_center()
                                .cursor_pointer()
                                .hover(|s| s.border_color(cx.theme().primary))
                                .on_click(move |_, _, _| {
                                    let _ = open::that(&login_url);
                                })
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .child(SharedString::from(pname)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(format!("OAuth · {pid}"))),
                                        ),
                                )
                                .child(Tag::secondary().small().child("OAuth")),
                        );
                    }
                    picker_scroll = picker_scroll.child(oauth_grid);
                }

                if !avail_apikey.is_empty() {
                    let mut key_grid = v_flex().gap_1p5().child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().muted_foreground)
                            .child("API Key"),
                    );
                    for (idx, (pid, pname, _)) in avail_apikey.into_iter().enumerate() {
                        let app_pick = self.app_state.clone();
                        let p_id = pid.clone();
                        let p_name = pname.clone();
                        key_grid = key_grid.child(
                            h_flex()
                                .id(("picker-apikey", idx))
                                .w_full()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .justify_between()
                                .items_center()
                                .cursor_pointer()
                                .hover(|s| s.border_color(cx.theme().primary))
                                .on_click(move |_, _, cx| {
                                    app_pick.update(cx, |s, cx| {
                                        s.settings_add_kind =
                                            Some(SettingsAddKind::ProviderApiKey {
                                                provider_id: p_id.clone(),
                                                provider_name: p_name.clone(),
                                            });
                                        cx.notify();
                                    });
                                })
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .child(SharedString::from(pname)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(pid)),
                                        ),
                                )
                                .child(Tag::secondary().small().child("API Key")),
                        );
                    }
                    picker_scroll = picker_scroll.child(key_grid);
                }

                body = body
                    .child(Input::new(&self.provider_search_input).small())
                    .child(picker_scroll);
            }
            SettingsAddKind::ProviderModel => {
                // Provider definition only: Name/ID, Base URL, Protocol/Endpoint dropdown, API Key (one per row)
                body = body
                    .child(field_box(
                        tr(locale, "settings.providerId"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.baseUrl"),
                        Input::new(&self.input_b).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.apiFormat"),
                        Select::new(&self.api_select).small().w_full(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.apiKeyOptional"),
                        Input::new(&self.input_c).small(),
                        cx,
                    ));
            }
            SettingsAddKind::AddModelToProvider { .. } => {
                // Row 1: Model ID + Display Name (paired on one row)
                // Row 2: Context Window + Max Output Tokens (paired on one row)
                // Row 3: Input Types (Text / Image toggles)
                // Row 4: Reasoning On/Off switch
                body = body
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2p5()
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.modelId"),
                                Input::new(&self.input_a).small(),
                                cx,
                            )))
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.modelName"),
                                Input::new(&self.input_b).small(),
                                cx,
                            ))),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2p5()
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.contextWindow"),
                                Input::new(&self.input_c).small(),
                                cx,
                            )))
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.maxOutputTokens"),
                                Input::new(&self.input_d).small(),
                                cx,
                            ))),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.inputTypes")),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        Checkbox::new("add-model-input-text")
                                            .checked(self.model_input_text)
                                            .label(tr(locale, "settings.inputText"))
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.model_input_text = *checked;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Checkbox::new("add-model-input-image")
                                            .checked(self.model_input_image)
                                            .label(tr(locale, "settings.inputImage"))
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.model_input_image = *checked;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.reasoningToggle")),
                            )
                            .child(
                                Switch::new("add-model-reasoning-switch")
                                    .checked(self.model_reasoning)
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.model_reasoning = *checked;
                                        cx.notify();
                                    })),
                            ),
                    );
            }
            SettingsAddKind::EditProvider { .. } => {
                body = body
                    .child(field_box(
                        tr(locale, "settings.displayName"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.apiFormat"),
                        Select::new(&self.api_select).small().w_full(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.baseUrl"),
                        Input::new(&self.input_b).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.apiKeyOptional"),
                        Input::new(&self.input_c).small(),
                        cx,
                    ));
            }
            SettingsAddKind::EditModel { .. } => {
                body = body
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2p5()
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.modelId"),
                                Input::new(&self.input_a).small(),
                                cx,
                            )))
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.modelName"),
                                Input::new(&self.input_b).small(),
                                cx,
                            ))),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2p5()
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.contextWindow"),
                                Input::new(&self.input_c).small(),
                                cx,
                            )))
                            .child(div().flex_1().min_w_0().child(field_box(
                                tr(locale, "settings.maxOutputTokens"),
                                Input::new(&self.input_d).small(),
                                cx,
                            ))),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.inputTypes")),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        Checkbox::new("edit-model-input-text")
                                            .checked(self.model_input_text)
                                            .label(tr(locale, "settings.inputText"))
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.model_input_text = *checked;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Checkbox::new("edit-model-input-image")
                                            .checked(self.model_input_image)
                                            .label(tr(locale, "settings.inputImage"))
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.model_input_image = *checked;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.reasoningToggle")),
                            )
                            .child(
                                Switch::new("edit-model-reasoning-switch")
                                    .checked(self.model_reasoning)
                                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                        this.model_reasoning = *checked;
                                        cx.notify();
                                    })),
                            ),
                    );
            }
            SettingsAddKind::ProviderApiKey { .. } => {
                body = body.child(field_box(
                    tr(locale, "settings.apiKeyOptional"),
                    Input::new(&self.input_a).small(),
                    cx,
                ));
            }
            SettingsAddKind::McpServer => {
                body = body
                    .child(field_box(
                        tr(locale, "settings.mcpNameOptional"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.mcpInputLabel"),
                        Textarea::new(&self.textarea_a),
                        cx,
                    ));
            }
            SettingsAddKind::SubagentProfile => {
                body = body
                    .child(field_box(
                        tr(locale, "settings.subagentName"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.subagentModel"),
                        Input::new(&self.input_c).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.subagentDesc"),
                        Input::new(&self.input_b).small(),
                        cx,
                    ))
                    .child(field_box(
                        tr(locale, "settings.subagentPrompt"),
                        Textarea::new(&self.textarea_a),
                        cx,
                    ));
            }
            SettingsAddKind::Skill => {
                body = body.child(
                    h_flex()
                        .gap_2()
                        .items_end()
                        .child(div().flex_1().child(field_box(
                            tr(locale, "settings.skillSearchQuery"),
                            Input::new(&self.input_a).small(),
                            cx,
                        )))
                        .child(
                            Button::new("skill-search-btn")
                                .outline()
                                .small()
                                .h(px(28.))
                                .icon(IconName::Search)
                                .disabled(self.busy)
                                .label(tr(locale, "settings.search"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.search_skills(cx);
                                })),
                        ),
                );
                if !self.skill_results.is_empty() {
                    let mut list = v_flex()
                        .id("skill-search-results")
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .gap_1();
                    for (idx, (pkg, inst)) in self.skill_results.iter().enumerate() {
                        let pkg_install = pkg.clone();
                        list = list.child(
                            h_flex()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().secondary)
                                .justify_between()
                                .items_center()
                                .gap_2()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_medium()
                                                .truncate()
                                                .child(SharedString::from(pkg.clone())),
                                        )
                                        .children((!inst.is_empty()).then(|| {
                                            Tag::secondary()
                                                .small()
                                                .child(SharedString::from(inst.clone()))
                                        })),
                                )
                                .child(
                                    Button::new(("skill-res-inst", idx))
                                        .primary()
                                        .xsmall()
                                        .h(px(24.))
                                        .icon(IconName::Plus)
                                        .label(tr(locale, "settings.install"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.install_skill_pkg(pkg_install.clone(), cx);
                                        })),
                                ),
                        );
                    }
                    body = body.child(list);
                }
            }
            SettingsAddKind::Plugin => {
                body = body.child(field_box(
                    tr(locale, "settings.pluginSource"),
                    Input::new(&self.input_a).small(),
                    cx,
                ));
            }
        }

        let submit_label = match kind {
            SettingsAddKind::Skill | SettingsAddKind::Plugin => tr(locale, "settings.install"),
            _ => tr(locale, "scheduled.save"),
        };
        let kind_for_submit = kind.clone();

        div()
            .id("settings-add-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .p_6()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.65))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.close(cx);
            }))
            .child(
                v_flex()
                    .id("settings-add-card")
                    .occlude()
                    .w(if is_picker { px(680.) } else { px(560.) })
                    .max_w_full()
                    .p_4()
                    .gap_3()
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(div().text_sm().font_semibold().child(SharedString::from(title)))
                            .child(
                                Button::new("settings-add-close")
                                    .ghost()
                                    .small()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                            ),
                    )
                    .child(body)
                    .children(self.error.as_ref().map(|err| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(SharedString::from(err.clone()))
                    }))
                    .children((!is_picker).then(|| {
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("settings-add-cancel")
                                    .ghost()
                                    .small()
                                    .h(px(28.))
                                    .icon(IconName::Close)
                                    .label(tr(locale, "scheduled.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                            )
                            .child(
                                Button::new("settings-add-submit")
                                    .primary()
                                    .small()
                                    .h(px(28.))
                                    .icon(IconName::Check)
                                    .disabled(self.busy)
                                    .label(submit_label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.submit(kind_for_submit.clone(), cx);
                                    })),
                            )
                    })),
            )
            .into_any_element()
    }
}

fn field_box(label: &'static str, control: impl IntoElement, cx: &gpui_kit::App) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(control)
}
