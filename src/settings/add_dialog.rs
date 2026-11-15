use crate::app_state::{AppState, SettingsAddKind};
use crate::client::types::ConfiguredModelInfo;
use crate::i18n::{AppLocale, tr};
use crate::provider_catalog::{self, ProviderCategory};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::component::tag::Tag;
use std::time::Duration;
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


/// One row of the "Add provider" picker. Built from the static catalog plus
/// any custom provider the sidecar/models.json reports.
#[derive(Clone)]
struct PickerRow {
    id: String,
    name: String,
    detail: String,
    configured: bool,
    api_key: bool,
    oauth: bool,
    category: ProviderCategory,
    from_catalog: bool,
}

fn provider_picker_rows(state: &AppState) -> Vec<PickerRow> {
    let configured = |id: &str| {
        state
            .auth_providers
            .iter()
            .any(|(pid, _, c)| pid == id && *c)
            || state
                .oauth_providers
                .iter()
                .any(|(pid, _, c)| pid == id && *c)
    };
    let mut rows: Vec<PickerRow> = provider_catalog::all()
        .iter()
        .map(|p| PickerRow {
            id: p.id.to_string(),
            name: p.name.to_string(),
            detail: p.base_url.to_string(),
            configured: configured(p.id),
            api_key: p.api_key || p.optional_api_key,
            oauth: p.oauth,
            category: p.category,
            from_catalog: true,
        })
        .collect();

    let push_extra = |rows: &mut Vec<PickerRow>, id: &str, name: &str, is_oauth: bool| {
        if provider_catalog::is_builtin(id) {
            return;
        }
        if let Some(existing) = rows.iter_mut().find(|r| r.id == id) {
            existing.configured = existing.configured || configured(id);
            existing.oauth = existing.oauth || is_oauth;
            return;
        }
        rows.push(PickerRow {
            id: id.to_string(),
            name: if name.is_empty() { id.to_string() } else { name.to_string() },
            detail: "Custom provider · models.json".to_string(),
            configured: configured(id),
            api_key: true,
            oauth: is_oauth,
            category: ProviderCategory::Global,
            from_catalog: false,
        });
    };
    let auth_snapshot = state.auth_providers.clone();
    for (id, name, _) in &auth_snapshot {
        push_extra(&mut rows, id, name, false);
    }
    let oauth_snapshot = state.oauth_providers.clone();
    for (id, name, _) in &oauth_snapshot {
        push_extra(&mut rows, id, name, true);
    }
    rows
}

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
    discovered_models: Vec<ConfiguredModelInfo>,
    selected_model_ids: std::collections::HashSet<String>,
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

        let input_a = cx.new(|cx| InputState::new(window, cx));
        cx.subscribe(&input_a, |_, _, ev: &InputEvent, cx| {
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
            input_a,
            input_b: cx.new(|cx| InputState::new(window, cx)),
            input_c: cx.new(|cx| InputState::new(window, cx)),
            input_d: cx.new(|cx| InputState::new(window, cx)),
            textarea_a: cx.new(|cx| TextareaState::new(window, cx).rows(5)),
            provider_search_input,
            skill_results: Vec::new(),
            discovered_models: Vec::new(),
            selected_model_ids: std::collections::HashSet::new(),
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
            SettingsAddKind::DiscoveredModelsPicker { discovered, .. } => {
                self.discovered_models = discovered.clone();
                // Large catalogs (OpenRouter and friends) start unselected so
                // one click cannot inject hundreds of models into models.json;
                // the picker still offers "Select all".
                self.selected_model_ids = if discovered.len() <= 50 {
                    discovered.iter().map(|m| m.id.clone()).collect()
                } else {
                    std::collections::HashSet::new()
                };
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
        if self.busy {
            return;
        }
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
                reasoning_declared: true,
                thinking_level_map: Default::default(),
            };
            self.app_state.update(cx, |s, cx| {
                s.update_configured_model(provider_id, model_id, model, cx);
                s.settings_add_kind = None;
                cx.notify();
            });
            return;
        }

        if let SettingsAddKind::DiscoveredModelsPicker { provider_id, .. } = kind {
            let to_add: Vec<ConfiguredModelInfo> = self
                .discovered_models
                .iter()
                .filter(|m| self.selected_model_ids.contains(&m.id))
                .cloned()
                .collect();
            if to_add.is_empty() {
                self.close(cx);
                return;
            }
            self.app_state.update(cx, |s, cx| {
                s.add_models_to_configured_provider(provider_id, to_add, cx);
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
                reasoning_declared: true,
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
        let connected = state.sidecar_connected;
        let requires_sidecar = matches!(
            kind,
            SettingsAddKind::ProviderApiKey { .. }
                | SettingsAddKind::Skill
                | SettingsAddKind::Plugin
        );
        if !connected && requires_sidecar {
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
        self.app_state.update(cx, |s, cx| {
            s.settings_busy = true;
            cx.notify();
        });
        cx.notify();

        cx.spawn(async move |this, cx| {
            let mut expand_provider_id: Option<String> = None;
            let mut discover_after_save: Option<String> = None;
            let mut reload_local_after_save = false;
            let res: anyhow::Result<()> = match kind {
                SettingsAddKind::AddProviderPicker
                | SettingsAddKind::AddModelToProvider { .. }
                | SettingsAddKind::DiscoveredModelsPicker { .. }
                | SettingsAddKind::PluginInfo(_)
                | SettingsAddKind::SkillInfo(_)
                | SettingsAddKind::EditProvider { .. }
                | SettingsAddKind::EditModel { .. } => Ok(()),
                SettingsAddKind::ProviderModel => {
                    if a.is_empty() || b.is_empty() {
                        Err(anyhow::anyhow!("Provider ID and Base URL are required"))
                    } else {
                        let mut cfg = crate::client::local_store::read_models_config_local();
                        if cfg.get("providers").and_then(Value::as_object).is_none() {
                            cfg["providers"] = json!({});
                        }
                        let pid = a.clone();
                        expand_provider_id = Some(pid.clone());
                        let prov_entry = cfg["providers"]
                            .as_object_mut()
                            .unwrap()
                            .entry(pid.clone())
                            .or_insert_with(|| json!({ "models": [] }));
                        prov_entry["baseUrl"] = json!(b);
                        prov_entry["api"] = json!(api_type);
                        if !c.is_empty() {
                            prov_entry["apiKey"] = json!(c);
                        }
                        if prov_entry.get("models").and_then(Value::as_array).is_none() {
                            prov_entry["models"] = json!([]);
                        }
                        if let Err(e) = crate::client::local_store::save_models_config_local(&cfg) {
                            Err(anyhow::anyhow!(e))
                        } else {
                            if connected {
                                let _ = client.put_models_config(cfg).await;
                            }
                            reload_local_after_save = true;
                            // Download the model list right away. Custom
                            // providers have no catalog in pi, so without this
                            // step the user would have to type model ids by
                            // hand (the old behaviour they reported).
                            let local_host = b.contains("localhost") || b.contains("127.0.0.1");
                            if !c.is_empty() || local_host {
                                discover_after_save = Some(pid.clone());
                            }
                            Ok(())
                        }
                    }
                }
                SettingsAddKind::ProviderApiKey { provider_id, .. } => {
                    let optional = crate::provider_catalog::find(&provider_id)
                        .map(|p| p.optional_api_key)
                        .unwrap_or(false);
                    let pi_builtin = crate::provider_catalog::is_pi_builtin(&provider_id);
                    if a.is_empty() && !optional {
                        Err(anyhow::anyhow!("API key required"))
                    } else if !pi_builtin || optional {
                        // Providers that pi does not ship (SiliconFlow, Ark,
                        // DashScope, Ollama, LM Studio, vLLM, ...) must live in
                        // models.json so both engines can resolve their base
                        // URL, protocol, credential, and models. Local servers
                        // simply omit the key.
                        let mut cfg = crate::client::local_store::read_models_config_local();
                        if cfg.get("providers").and_then(Value::as_object).is_none() {
                            cfg["providers"] = json!({});
                        }
                        if let Some(preset) = crate::provider_catalog::find(&provider_id) {
                            let entry = cfg["providers"]
                                .as_object_mut()
                                .unwrap()
                                .entry(provider_id.clone())
                                .or_insert_with(|| json!({}));
                            entry["baseUrl"] = json!(preset.base_url);
                            entry["api"] = json!(preset.api);
                            if !a.is_empty() {
                                entry["apiKey"] = json!(a);
                            }
                            if entry.get("models").and_then(Value::as_array).is_none() {
                                entry["models"] = json!([]);
                            }
                        }
                        match crate::client::local_store::save_models_config_local(&cfg) {
                            Ok(()) => {
                                if connected {
                                    let _ = client.put_models_config(cfg).await;
                                }
                                reload_local_after_save = true;
                                discover_after_save = Some(provider_id.clone());
                                Ok(())
                            }
                            Err(err) => Err(anyhow::anyhow!(err)),
                        }
                    } else {
                        // pi already knows this provider; store the credential
                        // in auth.json. Persist first: the native engine and a
                        // restarted sidecar both read auth.json, and the desktop
                        // must keep working when the sidecar is offline.
                        match crate::client::local_store::save_api_key_local(&provider_id, &a) {
                            Ok(()) => {
                                reload_local_after_save = true;
                                if connected {
                                    // Best effort: the sidecar picks the key up
                                    // from auth.json even if this call fails.
                                    let _ = client.set_api_key(&provider_id, &a).await;
                                }
                                Ok(())
                            }
                            Err(err) => Err(anyhow::anyhow!(err)),
                        }
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
                        if connected {
                            client.mcp_action(body).await.map(|_| ())
                        } else {
                            crate::client::local_store::mutate_mcp_config_local(Some(&cwd), &body)
                                .map_err(|e| anyhow::anyhow!(e))
                        }
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
                        let payload = json!({
                            "cwd": cwd,
                            "scope": scope,
                            "profile": profile,
                        });
                        if let Err(e) = crate::client::local_store::save_subagent_profile_local(Some(&cwd), &payload) {
                            Err(anyhow::anyhow!(e))
                        } else {
                            if connected {
                                let _ = client.save_subagent_profile(payload).await;
                            }
                            Ok(())
                        }
                    }
                }
                SettingsAddKind::Skill => {
                    let pkg = normalize_skill_source(&a);
                    if pkg.is_empty() {
                        Err(anyhow::anyhow!("Skill package name required"))
                    } else {
                        client
                            .install_skill(&pkg, &scope, Some(&cwd))
                            .await
                            .map(|_| ())
                    }
                }
                SettingsAddKind::Plugin => {
                    let src = normalize_plugin_source(&a);
                    if src.is_empty() {
                        Err(anyhow::anyhow!("Plugin source required"))
                    } else {
                        client
                            .plugin_action(json!({
                                "cwd": cwd,
                                "action": "install",
                                "source": src,
                                "scope": scope,
                            }))
                            .await
                            .map(|_| ())
                    }
                }
            };

            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                app_state.update(cx, |s, cx| {
                    s.settings_busy = false;
                    if res.is_ok() {
                        if let Some(pid) = expand_provider_id.take() {
                            s.expanded_configured_providers.insert(pid);
                        }
                        if reload_local_after_save {
                            s.load_local_store_snapshot();
                        }
                        s.settings_add_kind = None;
                        s.refresh_from_sidecar(cx);
                        s.refresh_settings_data(cx);
                        if let Some(pid) = discover_after_save.take() {
                            s.discover_models_for_provider(pid, cx);
                        }
                    } else {
                        cx.notify();
                    }
                });
                if let Err(err) = res {
                    view.error = Some(err.to_string());
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
        if self.busy {
            return;
        }
        let state = self.app_state.read(cx);
        let client = state.client.clone();
        let cwd = state.active_cwd.to_string_lossy().into_owned();
        let scope = self.scope.clone();
        let app_state = self.app_state.clone();
        self.busy = true;
        self.error = None;
        self.app_state.update(cx, |s, cx| {
            s.settings_busy = true;
            cx.notify();
        });
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = client.install_skill(&pkg, &scope, Some(&cwd)).await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                app_state.update(cx, |s, cx| {
                    s.settings_busy = false;
                    if res.is_ok() {
                        s.settings_add_kind = None;
                        s.refresh_settings_data(cx);
                    } else {
                        cx.notify();
                    }
                });
                if let Err(e) = res {
                    view.error = Some(e.to_string());
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn render_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.app_state.read(cx).settings_add_kind.is_some();
        let presence = Presence::new("settings-add-dialog-presence", is_open)
            .transition(
                Transition::new(Duration::from_millis(160))
                    .easing(cx.theme().motion_tokens().easing_move.clone()),
            )
            .sample(window, cx);
        if !presence.should_render() {
            self.last_kind = None;
            return div().into_any_element();
        }
        let progress = presence.progress;

        let Some(kind) = self.app_state.read(cx).settings_add_kind.clone().or_else(|| self.last_kind.clone()) else {
            self.last_kind = None;
            return div().into_any_element();
        };
        let (locale, sidecar_url, sidecar_connected) = {
            let s = self.app_state.read(cx);
            (s.locale, s.client.base_url.clone(), s.sidecar_connected)
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
            SettingsAddKind::DiscoveredModelsPicker { provider_id, .. } => {
                format!("{} — {provider_id}", tr(locale, "settings.selectDiscoveredModels"))
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
            SettingsAddKind::PluginInfo(p) => format!("{} — {}", tr(locale, "settings.pluginInfo"), p.name),
            SettingsAddKind::SkillInfo(s) => format!("{} — {}", tr(locale, "settings.skillInfo"), s.name),
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
                        TabBar::new("add-dlg-scope-tabs")
                            .segmented()
                            .h(px(32.))
                            .child(
                                Tab::new()
                                    .label(tr(locale, "settings.scopeGlobal"))
                                    .selected(self.scope == "global")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scope = "global".to_string();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Tab::new()
                                    .label(tr(locale, "settings.scopeProject"))
                                    .selected(self.scope == "project")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scope = "project".to_string();
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }

        let is_provider_picker = matches!(kind, SettingsAddKind::AddProviderPicker);
        let is_model_picker = matches!(kind, SettingsAddKind::DiscoveredModelsPicker { .. });
        let is_info_card = matches!(kind, SettingsAddKind::PluginInfo(_) | SettingsAddKind::SkillInfo(_));

        match &kind {
            SettingsAddKind::AddProviderPicker => {
                let q = self
                    .provider_search_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_lowercase();
                let show_custom = q.is_empty()
                    || "custom".contains(&q)
                    || "openai".contains(&q)
                    || "anthropic".contains(&q);
                let rows: Vec<PickerRow> = provider_picker_rows(self.app_state.read(cx))
                    .into_iter()
                    .filter(|r| {
                        q.is_empty()
                            || r.id.to_lowercase().contains(&q)
                            || r.name.to_lowercase().contains(&q)
                            || r.detail.to_lowercase().contains(&q)
                    })
                    .collect();

                let mut picker_scroll = v_flex()
                    .id("add-provider-picker-scroll")
                    .max_h(px(460.))
                    .overflow_y_scroll()
                    .gap_2();

                if show_custom {
                    let app_custom = self.app_state.clone();
                    picker_scroll = picker_scroll.child(
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
                                    s.settings_add_kind = Some(SettingsAddKind::ProviderModel);
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
                                            .child(tr(locale, "settings.customProviderTitle")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(tr(locale, "settings.customEndpoint")),
                                    ),
                            )
                            .child(IconName::Plus),
                    );
                }

                let any_results = show_custom || !rows.is_empty();
                let mut current_category: Option<ProviderCategory> = None;
                for (idx, row) in rows.into_iter().enumerate() {
                    if current_category != Some(row.category) {
                        current_category = Some(row.category);
                        picker_scroll = picker_scroll.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child(match locale {
                                    AppLocale::ZhCn => row.category.label_zh(),
                                    AppLocale::En => row.category.label(),
                                }),
                        );
                    }

                    let pid = row.id.clone();
                    let pname = row.name.clone();
                    let pid_click = pid.clone();
                    let pname_click = pname.clone();
                    let app_key = self.app_state.clone();
                    let app_key_click = self.app_state.clone();
                    let oauth_url = format!(
                        "{}/api/auth/login/{}",
                        sidecar_url.trim_end_matches('/'),
                        pid
                    );
                    let can_key = row.api_key;
                    let can_oauth = row.oauth;
                    let configured = row.configured;
                    let from_catalog = row.from_catalog;

                    let display_name = pname.clone();
                    let display_id = pid.clone();
                    let key_label = if crate::provider_catalog::find(&pid)
                        .map(|p| p.optional_api_key)
                        .unwrap_or(false)
                    {
                        tr(locale, "settings.setupProvider")
                    } else if configured {
                        tr(locale, "settings.updateProviderKey")
                    } else {
                        tr(locale, "settings.addProviderKey")
                    };

                    let mut actions = h_flex().items_center().gap_1p5().flex_shrink_0();
                    if configured {
                        actions = actions
                            .child(Tag::secondary().small().child(tr(locale, "settings.configuredTag")));
                    }
                    if can_oauth {
                        let oauth_url = oauth_url.clone();
                        actions = actions.child(
                            Button::new(("picker-oauth", idx))
                                .outline()
                                .xsmall()
                                .h(px(26.))
                                .label("OAuth")
                                .disabled(!sidecar_connected)
                                .tooltip(if sidecar_connected {
                                    tr(locale, "settings.oauthLoginHint")
                                } else {
                                    tr(locale, "settings.oauthNeedsSidecar")
                                })
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let _ = open::that(&oauth_url);
                                }),
                        );
                    }
                    if can_key {
                        actions = actions.child(
                            Button::new(("picker-key", idx))
                                .primary()
                                .xsmall()
                                .h(px(26.))
                                .label(key_label)
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    app_key.update(cx, |s, cx| {
                                        s.settings_add_kind = Some(SettingsAddKind::ProviderApiKey {
                                            provider_id: pid.clone(),
                                            provider_name: pname.clone(),
                                        });
                                        cx.notify();
                                    });
                                }),
                        );
                    }

                    let mut card = h_flex()
                        .id(("picker-provider", idx))
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .hover(|s| s.border_color(cx.theme().primary))
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_semibold()
                                                .child(SharedString::from(display_name.clone())),
                                        )
                                        .children((!from_catalog).then(|| {
                                            Tag::secondary()
                                                .small()
                                                .child(tr(locale, "settings.customTag"))
                                        })),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(SharedString::from(format!("{display_id} · {}", row.detail))),
                                ),
                        )
                        .child(actions);

                    if can_key {
                        card = card.on_click(move |_, _, cx| {
                            app_key_click.update(cx, |s, cx| {
                                s.settings_add_kind = Some(SettingsAddKind::ProviderApiKey {
                                    provider_id: pid_click.clone(),
                                    provider_name: pname_click.clone(),
                                });
                                cx.notify();
                            });
                        });
                    } else if can_oauth {
                        let oauth_url = oauth_url.clone();
                        card = card.on_click(move |_, _, _| {
                            if sidecar_connected {
                                let _ = open::that(&oauth_url);
                            }
                        });
                    }
                    picker_scroll = picker_scroll.child(card);
                }

                if !any_results {
                    picker_scroll = picker_scroll.child(
                        div()
                            .p_4()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "settings.noProviderMatches")),
                    );
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
                    ))
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(18.))
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "settings.fetchModelsOnSave")),
                    );
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
            SettingsAddKind::ProviderApiKey { provider_id, .. } => {
                let base_hint = crate::provider_catalog::base_url_for(provider_id)
                    .map(|url| format!("{provider_id} · {url}"))
                    .unwrap_or_else(|| provider_id.clone());
                let optional = crate::provider_catalog::find(provider_id)
                    .map(|p| p.optional_api_key)
                    .unwrap_or(false);
                body = body
                    .child(field_box(
                        tr(locale, "settings.apiKeyOptional"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(optional.then(|| {
                                div()
                                    .text_xs()
                                    .line_height(px(18.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.localProviderNoKeyHint"))
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(base_hint)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(px(18.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.apiKeyStoredHint")),
                            ),
                    );
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
                let raw = self.input_a.read(cx).value().to_string();
                let trimmed = raw.trim();
                let normalized = normalize_plugin_source(trimmed);
                let show_hint = !trimmed.is_empty() && normalized != trimmed && !normalized.is_empty();
                body = body
                    .child(field_box(
                        tr(locale, "settings.pluginSource"),
                        Input::new(&self.input_a).small(),
                        cx,
                    ))
                    .children(show_hint.then(|| {
                        h_flex()
                            .gap_1p5()
                            .items_center()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(cx.theme().secondary)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr(locale, "settings.pluginSourceNormalized")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(SharedString::from(normalized)),
                            )
                    }));
            }
            SettingsAddKind::PluginInfo(p) => {
                let desc = p.description.clone().unwrap_or_else(|| "No description provided.".to_string());
                body = body
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Tag::secondary().small().child(SharedString::from(format!("v{}", p.version))))
                            .child(Tag::secondary().small().child(SharedString::from(p.scope.clone())))
                            .child(Tag::secondary().small().child(SharedString::from(p.status.clone()))),
                    )
                    .child(field_box(
                        tr(locale, "settings.pluginSource"),
                        div().text_xs().font_family(cx.theme().mono_font_family.clone()).text_color(cx.theme().muted_foreground).child(SharedString::from(p.source.clone())),
                        cx,
                    ))
                    .child(field_box(
                        "Description",
                        div().p_2p5().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().secondary).text_xs().line_height(px(18.)).child(SharedString::from(desc)),
                        cx,
                    ))
                    .children((!p.resource_summary.is_empty()).then(|| {
                        field_box(
                            "Resources",
                            div().text_xs().text_color(cx.theme().muted_foreground).child(SharedString::from(p.resource_summary.clone())),
                            cx,
                        )
                    }));
            }
            SettingsAddKind::SkillInfo(s) => {
                body = body
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(Tag::secondary().small().child(SharedString::from(s.source.clone())))
                            .child(Tag::secondary().small().child(SharedString::from(if s.enabled { "Enabled" } else { "Disabled" }))),
                    )
                    .child(field_box(
                        "File Path",
                        div().text_xs().font_family(cx.theme().mono_font_family.clone()).text_color(cx.theme().muted_foreground).child(SharedString::from(s.file_path.clone())),
                        cx,
                    ))
                    .child(field_box(
                        "Description",
                        div().p_2p5().rounded_md().border_1().border_color(cx.theme().border).bg(cx.theme().secondary).text_xs().line_height(px(18.)).child(SharedString::from(s.description.clone())),
                        cx,
                    ));
            }
            SettingsAddKind::DiscoveredModelsPicker { .. } => {
                let q = self.provider_search_input.read(cx).value().trim().to_lowercase();
                let filtered: Vec<_> = self
                    .discovered_models
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| {
                        q.is_empty()
                            || m.id.to_lowercase().contains(&q)
                            || m.name.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    })
                    .collect();

                let all_checked = !self.discovered_models.is_empty()
                    && self.selected_model_ids.len() == self.discovered_models.len();

                body = body
                    .child(
                        h_flex()
                            .w_full()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().child(Input::new(&self.provider_search_input).small()))
                            .child(
                                Button::new("toggle-all-disc-models")
                                    .ghost()
                                    .xsmall()
                                    .h(px(26.))
                                    .label(if all_checked {
                                        tr(locale, "settings.deselectAll")
                                    } else {
                                        tr(locale, "settings.selectAll")
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.selected_model_ids.len() == this.discovered_models.len() {
                                            this.selected_model_ids.clear();
                                        } else {
                                            this.selected_model_ids = this.discovered_models.iter().map(|m| m.id.clone()).collect();
                                        }
                                        cx.notify();
                                    })),
                            ),
                    );

                let mut list = v_flex()
                    .id("disc-models-scroll")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .gap_1();

                for (idx, m) in filtered {
                    let mid = m.id.clone();
                    let is_checked = self.selected_model_ids.contains(&mid);
                    let title = m.name.clone().unwrap_or_else(|| m.id.clone());
                    let mid_toggle = mid.clone();
                    list = list.child(
                        h_flex()
                            .id(("disc-m-row", idx))
                            .w_full()
                            .px_2p5()
                            .py_1p5()
                            .gap_2()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(if is_checked { cx.theme().primary.opacity(0.35) } else { cx.theme().border })
                            .bg(if is_checked { cx.theme().primary.opacity(0.06) } else { cx.theme().secondary })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.selected_model_ids.contains(&mid_toggle) {
                                    this.selected_model_ids.remove(&mid_toggle);
                                } else {
                                    this.selected_model_ids.insert(mid_toggle.clone());
                                }
                                cx.notify();
                            }))
                            .child(
                                Checkbox::new(("cb-disc-m", idx))
                                    .checked(is_checked)
                                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                        if *checked {
                                            this.selected_model_ids.insert(mid.clone());
                                        } else {
                                            this.selected_model_ids.remove(&mid);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_0p5()
                                    .child(div().text_xs().font_semibold().truncate().child(SharedString::from(title)))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).truncate().child(SharedString::from(m.id.clone()))),
                            )
                            .children(m.context_window.map(|cw| {
                                Tag::secondary().small().child(SharedString::from(format!("{} ctx", crate::i18n::format_tokens_km(cw))))
                            }))
                            .children(m.reasoning.then(|| {
                                Tag::secondary().small().child("reasoning")
                            })),
                    );
                }

                body = body.child(list);
            }
        }

        let submit_label = match kind {
            SettingsAddKind::Skill | SettingsAddKind::Plugin => tr(locale, "settings.install"),
            SettingsAddKind::DiscoveredModelsPicker { .. } => tr(locale, "settings.addSelected"),
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
            .bg(cx.theme().background.opacity(0.65 * progress))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                if !this.busy {
                    this.close(cx);
                }
            }))
            .child(
                v_flex()
                    .id("settings-add-card")
                    .occlude()
                    .w(if is_provider_picker { px(680.) } else if is_model_picker { px(380.) } else if is_info_card { px(460.) } else { px(560.) })
                    .max_w_full()
                    .mt(px(14.0 * (1.0 - progress)))
                    .opacity(progress)
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
                                    .disabled(self.busy)
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
                    .children((!is_provider_picker && !is_info_card).then(|| {
                            h_flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("settings-add-cancel")
                                        .ghost()
                                        .small()
                                        .h(px(28.))
                                        .icon(IconName::Close)
                                        .disabled(self.busy)
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

pub(crate) fn normalize_plugin_source(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    loop {
        let trimmed = s.trim();
        let unwrapped = if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
            || (trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() >= 2)
        {
            trimmed[1..trimmed.len() - 1].trim()
        } else {
            trimmed
        };

        let mut next = unwrapped;
        for prompt in ["$ ", "> "] {
            if let Some(rest) = next.strip_prefix(prompt) {
                next = rest.trim_start();
            }
        }

        let lower = next.to_ascii_lowercase();
        if let Some(rest) = ["npx ", "pnpx ", "bunx "]
            .iter()
            .find_map(|p| lower.strip_prefix(p).map(|_| &next[p.len()..]))
        {
            next = rest.trim_start();
        } else if lower.starts_with("pi install ") || lower.starts_with("pi install\t") {
            next = next["pi install ".len()..].trim_start();
        } else if lower.starts_with("pi i ") || lower.starts_with("pi i\t") {
            next = next["pi i ".len()..].trim_start();
        } else {
            let mut stripped_flag = None;
            for flag in ["--local", "--global", "--yes", "-l", "-g", "-y"] {
                if let Some(rest) = next.strip_prefix(flag) {
                    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                        stripped_flag = Some(rest.trim_start());
                        break;
                    }
                }
            }
            if let Some(rest) = stripped_flag {
                next = rest;
            }
        }

        if next == s {
            break;
        }
        s = next.to_string();
    }
    s.trim().to_string()
}

pub(crate) fn normalize_skill_source(raw: &str) -> String {
    let mut s = normalize_plugin_source(raw);
    loop {
        let trimmed = s.trim();
        let lower = trimmed.to_ascii_lowercase();
        let next = if lower.starts_with("skills add ") || lower.starts_with("skills add\t") {
            trimmed["skills add ".len()..].trim_start()
        } else if lower.starts_with("skills install ") || lower.starts_with("skills install\t") {
            trimmed["skills install ".len()..].trim_start()
        } else {
            trimmed
        };
        let next = normalize_plugin_source(next);
        if next == s {
            break;
        }
        s = next;
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{normalize_plugin_source, normalize_skill_source};

    #[test]
    fn normalizes_pi_install_prefixes_and_flags() {
        assert_eq!(
            normalize_plugin_source("pi install npm:billion-context"),
            "npm:billion-context"
        );
        assert_eq!(
            normalize_plugin_source("  $ pi install -l \"npm:@scope/pkg\"  "),
            "npm:@scope/pkg"
        );
        assert_eq!(
            normalize_plugin_source("PI I --global git:https://github.com/org/repo"),
            "git:https://github.com/org/repo"
        );
        assert_eq!(
            normalize_plugin_source("npm:billion-context"),
            "npm:billion-context"
        );
    }

    #[test]
    fn normalizes_npx_skills_add_prefixes() {
        assert_eq!(
            normalize_skill_source("npx skills add owner/repo@skill"),
            "owner/repo@skill"
        );
        assert_eq!(
            normalize_skill_source("$ npx skills add -y owner/repo@skill"),
            "owner/repo@skill"
        );
    }
}
