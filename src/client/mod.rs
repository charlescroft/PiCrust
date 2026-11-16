pub mod local_store;
pub mod native_index;
pub mod session_controller;
pub mod sidecar;
pub mod sse;
pub mod types;

use anyhow::Result;
use reqwest::{Client, Method, RequestBuilder};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;
use types::*;

pub(crate) fn tokio_runtime() -> &'static tokio::runtime::Handle {
    static RT: OnceLock<tokio::runtime::Handle> = OnceLock::new();
    RT.get_or_init(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build single-thread tokio http runtime");
        let handle = rt.handle().clone();
        std::thread::Builder::new()
            .name("picrust-http".to_string())
            .stack_size(512 * 1024)
            .spawn(move || {
                rt.block_on(std::future::pending::<()>());
            })
            .expect("spawn single-thread tokio http driver");
        handle
    })
}

async fn run_on_tokio<F, T>(fut: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = smol::channel::bounded(1);
    tokio_runtime().spawn(async move {
        let res = fut.await;
        let _ = tx.send(res).await;
    });
    rx.recv()
        .await
        .map_err(|_| anyhow::anyhow!("HTTP worker cancelled"))?
}

/// Creates a `std::process::Command` with `CREATE_NO_WINDOW` set on Windows so background
/// tools (`git`, `node`, etc.) never flash a black console window over the GUI.
pub fn hidden_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Clone)]
pub struct PiDaemonClient {
    http: Client,
    pub base_url: String,
    pub token: String,
    pub instance_id: String,
}

#[allow(dead_code)]
impl PiDaemonClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>, instance_id: impl Into<String>) -> Self {
        let http = Client::builder()
            .no_proxy()
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .pool_idle_timeout(None)
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            base_url: base_url.into(),
            token: token.into(),
            instance_id: instance_id.into(),
        }
    }

    pub fn raw_http(&self) -> Client {
        self.http.clone()
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        self.http
            .request(method, url)
            .header("x-pi-desktop-token", &self.token)
            .header("x-pi-desktop-instance", &self.instance_id)
    }

    async fn get_json<T: DeserializeOwned + Send + 'static>(&self, path: &str) -> Result<T> {
        let req1 = self.request(Method::GET, path);
        let req2 = self.request(Method::GET, path);
        run_on_tokio(async move {
            match req1.send().await {
                Ok(resp) => Ok(resp.error_for_status()?.json::<T>().await?),
                Err(e) if e.is_connect() => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    let resp = req2.send().await?.error_for_status()?;
                    Ok(resp.json::<T>().await?)
                }
                Err(e) => Err(e.into()),
            }
        })
        .await
    }

    async fn post_json<B: Serialize, T: DeserializeOwned + Send + 'static>(&self, path: &str, body: &B) -> Result<T> {
        let req1 = self.request(Method::POST, path).json(body);
        let req2 = self.request(Method::POST, path).json(body);
        run_on_tokio(async move {
            match req1.send().await {
                Ok(resp) => Ok(resp.error_for_status()?.json::<T>().await?),
                Err(e) if e.is_connect() => {
                    tokio::time::sleep(Duration::from_millis(350)).await;
                    let resp = req2.send().await?.error_for_status()?;
                    Ok(resp.json::<T>().await?)
                }
                Err(e) => Err(e.into()),
            }
        })
        .await
    }

    async fn put_json<B: Serialize, T: DeserializeOwned + Send + 'static>(&self, path: &str, body: &B) -> Result<T> {
        let req1 = self.request(Method::PUT, path).json(body);
        let req2 = self.request(Method::PUT, path).json(body);
        run_on_tokio(async move {
            match req1.send().await {
                Ok(resp) => Ok(resp.error_for_status()?.json::<T>().await?),
                Err(e) if e.is_connect() => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    let resp = req2.send().await?.error_for_status()?;
                    Ok(resp.json::<T>().await?)
                }
                Err(e) => Err(e.into()),
            }
        })
        .await
    }

    pub async fn health(&self) -> Result<Value> {
        let req = self.request(Method::GET, "/health").timeout(Duration::from_millis(2000));
        let fallback = self.request(Method::GET, "/api/home").timeout(Duration::from_millis(2000));
        run_on_tokio(async move {
            match req.send().await.and_then(|r| r.error_for_status()) {
                Ok(resp) => Ok(resp.json::<Value>().await?),
                Err(_) => {
                    let resp = fallback.send().await?.error_for_status()?;
                    Ok(resp.json::<Value>().await?)
                }
            }
        })
        .await
    }

    pub async fn get_json_path(&self, path: &str) -> Result<Value> {
        self.get_json(path).await
    }

    // --- Sessions ---
    pub async fn list_sessions(&self, summary: bool) -> Result<Value> {
        let path = if summary { "/api/sessions?summary=1" } else { "/api/sessions" };
        self.get_json(path).await
    }

    pub async fn get_session(&self, id: &str) -> Result<Value> {
        self.get_json(&format!("/api/sessions/{id}?tail=1000&wholeTurns=1")).await
    }

    pub async fn get_session_context(&self, id: &str, leaf_id: Option<&str>) -> Result<Value> {
        let path = match leaf_id {
            Some(leaf) => format!("/api/sessions/{id}/context?leafId={leaf}"),
            None => format!("/api/sessions/{id}/context"),
        };
        self.get_json(&path).await
    }

    pub async fn rename_session(&self, id: &str, name: &str) -> Result<Value> {
        let req = self
            .request(Method::PATCH, &format!("/api/sessions/{id}"))
            .json(&json!({ "name": name }));
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    pub async fn delete_session(&self, id: &str) -> Result<Value> {
        let req = self.request(Method::DELETE, &format!("/api/sessions/{id}"));
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    pub async fn fork_session(&self, id: &str, entry_id: &str) -> Result<Value> {
        self.post_json(&format!("/api/sessions/{id}/fork"), &json!({ "entryId": entry_id })).await
    }

    pub async fn get_chat_workspace(&self) -> Result<std::path::PathBuf> {
        let val: Value = self.get_json("/api/chat-workspace").await?;
        let cwd = val
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("Missing cwd in /api/chat-workspace response"))?;
        Ok(std::path::PathBuf::from(cwd))
    }

    // --- Agent commands ---
    pub async fn start_new_session(
        &self,
        cwd: &str,
        message: &str,
        tool_names: &[&str],
        provider: Option<&str>,
        model_id: Option<&str>,
        thinking_level: Option<&str>,
    ) -> Result<Value> {
        let mut body = json!({
            "cwd": cwd,
            "type": "prompt",
            "message": message,
            "toolNames": tool_names,
            "provider": provider,
            "modelId": model_id,
        });
        if let Some(tl) = thinking_level {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("thinkingLevel".to_string(), json!(tl));
            }
        }
        self.post_json("/api/agent/new", &body).await
    }

    pub async fn get_agent_state(&self, session_id: &str) -> Result<Value> {
        self.get_json(&format!("/api/agent/{session_id}?systemPrompt=0"))
            .await
    }

    pub async fn send_agent_command(&self, session_id: &str, cmd: Value) -> Result<Value> {
        self.post_json(&format!("/api/agent/{session_id}"), &cmd).await
    }

    pub async fn abort_agent(&self, session_id: &str) -> Result<Value> {
        self.send_agent_command(session_id, json!({ "type": "abort" })).await
    }

    pub async fn navigate_tree(&self, session_id: &str, target_id: &str) -> Result<Value> {
        self.send_agent_command(
            session_id,
            json!({ "type": "navigate_tree", "targetId": target_id }),
        )
        .await
    }

    pub async fn set_session_model(&self, session_id: &str, provider: &str, model_id: &str) -> Result<Value> {
        self.send_agent_command(
            session_id,
            json!({ "type": "set_model", "provider": provider, "modelId": model_id }),
        )
        .await
    }

    pub async fn set_thinking_level(&self, session_id: &str, level: ThinkingLevel) -> Result<Value> {
        self.send_agent_command(
            session_id,
            json!({ "type": "set_thinking_level", "level": level.as_str() }),
        )
        .await
    }

    pub async fn set_session_tools(&self, session_id: &str, tool_names: &[&str]) -> Result<Value> {
        self.send_agent_command(
            session_id,
            json!({ "type": "set_tools", "toolNames": tool_names }),
        )
        .await
    }

    pub async fn respond_extension_ui(&self, session_id: &str, request_id: &str, response: Value) -> Result<Value> {
        let mut body = serde_json::Map::new();
        body.insert("type".to_string(), json!("extension_ui_response"));
        body.insert("id".to_string(), json!(request_id));
        if let Value::Object(map) = response {
            for (k, v) in map {
                body.insert(k, v);
            }
        }
        self.send_agent_command(session_id, Value::Object(body)).await
    }

    pub fn agent_events_url(&self, session_id: &str) -> String {
        format!(
            "{}/api/agent/{}/events?holder={}",
            self.base_url.trim_end_matches('/'),
            session_id,
            self.instance_id
        )
    }

    pub fn running_events_url(&self) -> String {
        format!("{}/api/agent/running/events", self.base_url.trim_end_matches('/'))
    }

    // --- Models & Auth ---
    pub async fn get_models(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/models?cwd={}", url_encode(c)),
            None => "/api/models".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn get_enabled_models(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/models/enabled?cwd={}", url_encode(c)),
            None => "/api/models/enabled".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn set_enabled_model(&self, action: Value) -> Result<Value> {
        // Minimal edit delegated to backend per AGENTS.md hard rule
        self.put_json("/api/models/enabled", &action).await
    }

    pub async fn refresh_model_catalog(&self) -> Result<Value> {
        self.post_json("/api/models/refresh", &json!({})).await
    }

    pub async fn get_auth_all_providers(&self) -> Result<Value> {
        self.get_json("/api/auth/all-providers").await
    }

    pub async fn get_oauth_providers(&self) -> Result<Value> {
        self.get_json("/api/auth/providers").await
    }

    pub async fn set_api_key(&self, provider: &str, api_key: &str) -> Result<Value> {
        self.post_json(&format!("/api/auth/api-key/{provider}"), &json!({ "apiKey": api_key })).await
    }

    pub async fn get_models_config(&self) -> Result<Value> {
        self.get_json("/api/models-config").await
    }

    pub async fn put_models_config(&self, config: Value) -> Result<Value> {
        self.put_json("/api/models-config", &config).await
    }

    /// Lists a provider's models by asking the provider's own Base URL, mirroring
    /// pi's `buildModelsListUrl` (the model list lives on the API root, so chat
    /// endpoints are stripped first). No pi service is involved.
    pub async fn discover_models_from_provider(
        &self,
        provider_name: &str,
        provider: &Value,
    ) -> Result<Value> {
        let base = provider
            .get("baseUrl")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Provider has no Base URL"))?
            .trim_end_matches('/')
            .to_string();
        let api = provider
            .get("api")
            .and_then(Value::as_str)
            .unwrap_or("openai-completions")
            .to_string();
        // Credentials come from, in order: a models.json `$ENV` reference (resolved
        // here), a literal `apiKey`, then pi's own credential store — GET redacts
        // literal keys, and without a token every authenticated provider answers 401.
        let api_key = resolve_provider_key(provider, provider_name);

        let mut url = build_models_url(&base, &api)?;
        match api.as_str() {
            "anthropic-messages" => {
                if !url.contains("limit=") {
                    url.push_str(if url.contains('?') { "&limit=100" } else { "?limit=100" });
                }
            }
            "google-generative-ai" => {
                if !url.contains("pageSize=") {
                    url.push_str(if url.contains('?') { "&pageSize=100" } else { "?pageSize=100" });
                }
            }
            _ => {}
        }

        let http = self.http.clone();
        let client = self.clone();
        let req = http.get(&url).header("Accept", "application/json");
        let req = match api.as_str() {
            "anthropic-messages" => {
                let mut r = req.header("anthropic-version", "2023-06-01");
                if let Some(key) = api_key.as_deref() {
                    r = r.header("x-api-key", key);
                }
                r
            }
            "google-generative-ai" => match api_key.as_deref() {
                Some(key) => req.header("x-goog-api-key", key),
                None => req,
            },
            _ => match api_key.as_deref() {
                Some(key) => req.header("Authorization", format!("Bearer {key}")),
                None => req,
            },
        };
        let _ = client;

        run_on_tokio(async move {
            let resp = req.send().await?;
            let status = resp.status();
            let text = resp.text().await?;
            if !status.is_success() {
                anyhow::bail!(
                    "{} -> HTTP {}: {}",
                    url,
                    status.as_u16(),
                    text.chars().take(300).collect::<String>()
                );
            }
            let payload: Value = serde_json::from_str(&text)
                .map_err(|_| anyhow::anyhow!("Upstream model list was not valid JSON"))?;
            let models = parse_models_list(&payload);
            if models.is_empty() {
                anyhow::bail!("No models found in the upstream response");
            }
            Ok(json!({ "models": models, "endpoint": url }))
        })
        .await
    }

    pub async fn discover_provider_models(&self, provider_name: &str, provider: Value) -> Result<Value> {
        self.post_json(
            "/api/models-config/discover",
            &json!({
                "providerName": provider_name,
                "provider": provider,
            }),
        )
        .await
    }

    pub async fn get_artifacts_settings(&self) -> Result<Value> {
        self.get_json("/api/artifacts/settings").await
    }

    pub async fn set_artifacts_settings(&self, enabled: bool) -> Result<Value> {
        self.put_json("/api/artifacts/settings", &json!({ "enabled": enabled })).await
    }

    pub async fn get_session_title_settings(&self) -> Result<Value> {
        self.get_json("/api/session-title/settings").await
    }

    pub async fn set_session_title_settings(&self, enabled: bool) -> Result<Value> {
        self.put_json("/api/session-title/settings", &json!({ "enabled": enabled })).await
    }

    // --- Network Proxy ---
    pub async fn get_network_proxy(&self) -> Result<Value> {
        self.get_json("/api/network-proxy").await
    }

    pub async fn set_network_proxy(&self, payload: Value) -> Result<Value> {
        self.put_json("/api/network-proxy", &payload).await
    }

    // --- Worktrees & Git ---
    pub async fn validate_cwd(&self, cwd: &str) -> Result<Value> {
        self.post_json("/api/cwd/validate", &json!({ "cwd": cwd })).await
    }

    pub async fn list_worktrees(&self, cwd: &str) -> Result<Value> {
        self.get_json(&format!("/api/worktrees?cwd={}", url_encode(cwd))).await
    }

    pub async fn git_status(&self, cwd: &str) -> Result<Value> {
        self.get_json(&format!("/api/git/status?cwd={}", url_encode(cwd))).await
    }

    pub async fn git_diff(&self, cwd: &str, path: Option<&str>) -> Result<Value> {
        let mut url = format!("/api/git/diff?cwd={}", url_encode(cwd));
        if let Some(p) = path {
            url.push_str(&format!("&path={}", url_encode(p)));
        }
        self.get_json(&url).await
    }

    // --- MCP / Skills / Plugins / Subagents ---
    pub async fn get_mcp(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/mcp?cwd={}", url_encode(c)),
            None => "/api/mcp".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn mcp_action(&self, payload: Value) -> Result<Value> {
        self.post_json("/api/mcp", &payload).await
    }

    pub async fn test_mcp_server(&self, payload: Value) -> Result<Value> {
        self.post_json("/api/mcp/test", &payload).await
    }

    pub async fn set_project_trust(&self, cwd: &str, trusted: bool) -> Result<Value> {
        self.put_json("/api/project-trust", &json!({ "cwd": cwd, "trusted": trusted })).await
    }

    pub async fn get_skills(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/skills?cwd={}", url_encode(c)),
            None => "/api/skills".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn toggle_skill(&self, file_path: &str, disable_model_invocation: bool) -> Result<Value> {
        let req = self
            .request(Method::PATCH, "/api/skills")
            .json(&json!({
                "filePath": file_path,
                "disableModelInvocation": disable_model_invocation,
            }));
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    pub async fn search_skills(&self, query: &str) -> Result<Value> {
        self.post_json("/api/skills/search", &json!({ "query": query, "limit": 20 })).await
    }

    pub async fn install_skill(&self, package: &str, scope: &str, cwd: Option<&str>) -> Result<Value> {
        self.post_json(
            "/api/skills/install",
            &json!({
                "package": package,
                "scope": scope,
                "cwd": cwd,
            }),
        )
        .await
    }

    pub async fn save_subagent_profile(&self, payload: Value) -> Result<Value> {
        self.put_json("/api/subagents/profiles", &payload).await
    }

    pub async fn get_plugins(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/plugins?cwd={}", url_encode(c)),
            None => "/api/plugins".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn plugin_action(&self, payload: Value) -> Result<Value> {
        self.post_json("/api/plugins", &payload).await
    }

    pub async fn get_subagent_settings(&self) -> Result<Value> {
        self.get_json("/api/subagents/settings").await
    }

    pub async fn set_subagent_settings(&self, payload: Value) -> Result<Value> {
        self.put_json("/api/subagents/settings", &payload).await
    }

    pub async fn get_subagents(&self, cwd: Option<&str>) -> Result<Value> {
        let path = match cwd {
            Some(c) => format!("/api/subagents/profiles?cwd={}", url_encode(c)),
            None => "/api/subagents/profiles".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn toggle_subagent_profile(&self, payload: Value) -> Result<Value> {
        let req = self
            .request(Method::PATCH, "/api/subagents/profiles")
            .json(&payload);
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    // --- Tasks / Scheduled / Artifacts ---
    pub async fn get_agent_tasks(&self) -> Result<Value> {
        self.get_json("/api/agent/tasks").await
    }

    pub async fn get_scheduled_tasks(&self) -> Result<Value> {
        self.get_json("/api/scheduled-tasks").await
    }

    pub async fn create_scheduled_task_payload(&self, payload: Value) -> Result<Value> {
        self.post_json("/api/scheduled-tasks", &payload).await
    }

    pub async fn update_scheduled_task(&self, id: &str, payload: Value) -> Result<Value> {
        let req = self
            .request(Method::PATCH, &format!("/api/scheduled-tasks/{id}"))
            .json(&payload);
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    pub async fn delete_scheduled_task(&self, id: &str) -> Result<Value> {
        let req = self.request(Method::DELETE, &format!("/api/scheduled-tasks/{id}"));
        run_on_tokio(async move {
            let resp = req.send().await?.error_for_status()?;
            Ok(resp.json().await?)
        })
        .await
    }

    pub async fn run_scheduled_task(&self, id: &str) -> Result<Value> {
        self.post_json(&format!("/api/scheduled-tasks/{id}/run"), &json!({})).await
    }

    pub async fn get_scheduled_runs(&self, id: &str, limit: usize, before: Option<&str>) -> Result<Value> {
        let mut path = format!("/api/scheduled-tasks/{id}/runs?limit={limit}");
        if let Some(b) = before {
            path.push_str(&format!("&before={}", url_encode(b)));
        }
        self.get_json(&path).await
    }

    pub async fn mark_scheduled_run_seen(&self, id: &str, run_id: &str) -> Result<Value> {
        self.post_json(
            &format!("/api/scheduled-tasks/{id}/runs/{run_id}/seen"),
            &json!({}),
        )
        .await
    }

    pub async fn preview_scheduled_cron(&self, expr: &str, timezone: &str) -> Result<Value> {
        self.post_json(
            "/api/scheduled-tasks/preview",
            &json!({ "expr": expr, "timezone": timezone }),
        )
        .await
    }

    pub async fn takeover_scheduler(&self) -> Result<Value> {
        self.post_json("/api/scheduled-tasks/scheduler/takeover", &json!({}))
            .await
    }

    pub fn scheduled_events_url(&self) -> String {
        format!(
            "{}/api/scheduled-tasks/events",
            self.base_url.trim_end_matches('/')
        )
    }

    pub async fn get_artifacts(&self, query: Option<&str>) -> Result<Value> {
        let path = match query {
            Some(q) if !q.is_empty() => format!("/api/artifacts?q={}", url_encode(q)),
            _ => "/api/artifacts".to_string(),
        };
        self.get_json(&path).await
    }

    pub async fn get_artifact_item(&self, session_id: &str, identifier: &str) -> Result<Value> {
        let path = format!(
            "/api/artifacts/item?session={}&identifier={}",
            url_encode(session_id),
            url_encode(identifier)
        );
        self.get_json(&path).await
    }
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", byte));
            }
        }
    }
    out
}

/// Strips a pasted chat endpoint and appends the API root's `/models` path.
fn build_models_url(base: &str, api: &str) -> Result<String> {
    let mut url = base.trim().to_string();
    for suffix in [
        "/chat/completions",
        "/completions",
        "/messages",
        "/responses",
        "/embeddings",
        "/rerankings",
        "/rerank",
    ] {
        if let Some(stripped) = url.strip_suffix(suffix) {
            url = stripped.to_string();
            break;
        }
    }
    let url = url.trim_end_matches('/').to_string();
    if url.to_lowercase().ends_with("/models") {
        return Ok(url);
    }
    let lower_tail = url.to_lowercase();
    let has_version = lower_tail
        .rsplit('/')
        .next()
        .map(|seg| {
            let rest = seg.strip_prefix('v').unwrap_or("");
            let rest = rest.strip_suffix("beta").unwrap_or(rest);
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
        })
        .unwrap_or(false);
    let version_path = if has_version {
        String::new()
    } else {
        match api {
            "anthropic-messages" => "/v1".to_string(),
            "google-generative-ai" => "/v1beta".to_string(),
            _ => String::new(),
        }
    };
    Ok(format!("{url}{version_path}/models"))
}

/// Reads `{data:[...]}`, `{models:[...]}`, or a map of models into the
/// `models.json` entry shape the settings page understands.
pub fn parse_models_list(payload: &Value) -> Vec<Value> {
    let items: Vec<&Value> = ["data", "models", "items", "results"]
        .iter()
        .find_map(|key| payload.get(*key))
        .map(|arr| match arr {
            Value::Array(a) => a.iter().collect(),
            Value::Object(o) => o.values().collect(),
            _ => Vec::new(),
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let (raw_id, name) = match item {
            Value::String(s) => (s.trim().to_string(), None),
            Value::Object(o) => {
                let id = o
                    .get("id")
                    .or_else(|| o.get("name"))
                    .or_else(|| o.get("model"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .trim_start_matches("models/")
                    .to_string();
                let name = o
                    .get("display_name")
                    .or_else(|| o.get("displayName"))
                    .or_else(|| o.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|n| !n.is_empty() && n != &id);
                (id, name)
            }
            _ => continue,
        };
        if raw_id.is_empty() || !seen.insert(raw_id.clone()) {
            continue;
        }
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), json!(raw_id));
        if let Some(name) = name {
            obj.insert("name".to_string(), json!(name));
        }

        // Capabilities: reasoning/thinking, context window, max tokens, images
        if let Value::Object(o) = item {
            let is_reasoning = o.get("supports_thinking").and_then(Value::as_bool)
                .or_else(|| o.get("supports_reasoning").and_then(Value::as_bool))
                .or_else(|| o.get("reasoning").and_then(Value::as_bool))
                .or_else(|| o.get("thinking").and_then(Value::as_bool))
                .unwrap_or(false);
            if is_reasoning {
                obj.insert("reasoning".to_string(), json!(true));
            }

            let ctx = ["context_length", "max_context_length", "context_window", "contextWindow", "max_model_len"]
                .iter()
                .find_map(|k| o.get(*k))
                .and_then(Value::as_u64);
            if let Some(cw) = ctx {
                obj.insert("contextWindow".to_string(), json!(cw));
            }

            let max_out = ["max_output_tokens", "max_completion_tokens", "maxTokens", "outputTokenLimit"]
                .iter()
                .find_map(|k| o.get(*k))
                .and_then(Value::as_u64);
            if let Some(mt) = max_out {
                obj.insert("maxTokens".to_string(), json!(mt));
            }

            let has_image = o.get("supports_images").and_then(Value::as_bool).unwrap_or(false);
            if has_image {
                obj.insert("input".to_string(), json!(["text", "image"]));
            }
        }

        out.push(Value::Object(obj));
    }
    out.sort_by(|a, b| {
        let ka = a.get("id").and_then(Value::as_str).unwrap_or("").to_lowercase();
        let kb = b.get("id").and_then(Value::as_str).unwrap_or("").to_lowercase();
        ka.cmp(&kb)
    });
    out
}

#[cfg(test)]
mod discovery_tests {
    use super::*;

    #[test]
    fn builds_models_url_per_api() {
        assert_eq!(
            build_models_url("https://api.openai.com/v1", "openai-completions").unwrap(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            build_models_url("https://api.anthropic.com", "anthropic-messages").unwrap(),
            "https://api.anthropic.com/v1/models"
        );
        assert_eq!(
            build_models_url("https://api.anthropic.com/v1", "anthropic-messages").unwrap(),
            "https://api.anthropic.com/v1/models"
        );
        assert_eq!(
            build_models_url("https://generativelanguage.googleapis.com", "google-generative-ai")
                .unwrap(),
            "https://generativelanguage.googleapis.com/v1beta/models"
        );
        // A pasted chat endpoint is stripped before /models is appended.
        assert_eq!(
            build_models_url("https://host/v1/chat/completions", "openai-completions").unwrap(),
            "https://host/v1/models"
        );
        assert_eq!(
            build_models_url("http://127.0.0.1:8045", "openai-responses").unwrap(),
            "http://127.0.0.1:8045/models"
        );
    }

    #[test]
    fn every_catalog_provider_builds_a_models_url() {
        for preset in crate::provider_catalog::all() {
            let url = build_models_url(preset.base_url, preset.api)
                .unwrap_or_else(|e| panic!("{}: {e}", preset.id));
            assert!(
                url.ends_with("/models"),
                "{} produced unexpected model URL {url}",
                preset.id
            );
        }
    }

    #[test]
    fn resolves_env_and_literal_keys_for_probing() {
        // SAFETY: single-threaded test setup, no other thread reads the environment.
        unsafe { std::env::set_var("PI_GPUI_TEST_KEY", "sk-test-123") };
        let env_ref = serde_json::json!({ "apiKey": "$PI_GPUI_TEST_KEY" });
        assert_eq!(
            resolve_provider_key(&env_ref, "unused"),
            Some("sk-test-123".to_string())
        );
        let literal = serde_json::json!({ "apiKey": "sk-literal" });
        assert_eq!(
            resolve_provider_key(&literal, "unused"),
            Some("sk-literal".to_string())
        );
        // A shell reference is never executed by the client.
        let shell = serde_json::json!({ "apiKey": "!echo secret" });
        assert_eq!(resolve_provider_key(&shell, ""), None);
    }

    #[test]
    fn parses_capabilities_from_model_list() {
        let payload = serde_json::json!({
            "data": [{
                "id": "claude-opus-4-6",
                "display_name": "Claude Opus 4.6",
                "supports_thinking": true,
                "context_length": 250000,
                "max_output_tokens": 64000,
                "supports_images": true
            }]
        });
        let parsed = parse_models_list(&payload);
        assert_eq!(parsed.len(), 1);
        let m = &parsed[0];
        assert_eq!(m["id"], "claude-opus-4-6");
        assert_eq!(m["name"], "Claude Opus 4.6");
        assert_eq!(m["reasoning"], true);
        assert_eq!(m["contextWindow"], 250000);
        assert_eq!(m["maxTokens"], 64000);
        assert_eq!(m["input"], serde_json::json!(["text", "image"]));
    }

    #[test]
    fn parses_openai_anthropic_and_google_lists() {
        let openai = serde_json::json!({ "data": [{ "id": "m-2" }, { "id": "m-1" }] });
        let parsed = parse_models_list(&openai);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"], "m-1");

        let anthropic = serde_json::json!({
            "data": [{ "id": "claude-x", "display_name": "Claude X" }]
        });
        let parsed = parse_models_list(&anthropic);
        assert_eq!(parsed[0]["display_name".to_string()], serde_json::Value::Null);
        assert_eq!(parsed[0]["name"], "Claude X");

        let google = serde_json::json!({
            "models": [{ "name": "models/gemini-2", "displayName": "Gemini 2" }]
        });
        let parsed = parse_models_list(&google);
        assert_eq!(parsed[0]["id"], "gemini-2");
        assert_eq!(parsed[0]["name"], "Gemini 2");
    }
}

/// Resolves the token used to probe a provider's model list.
///
/// Order mirrors pi: a models.json `$ENV` reference (read from the environment),
/// then a literal `apiKey` when the file was written locally, then pi's own
/// `~/.pi/agent/auth.json` entry for that provider (api_key `key`, or an OAuth
/// `access` token). `!command` references are deliberately not executed here.
pub fn resolve_provider_key(provider: &Value, provider_name: &str) -> Option<String> {
    let declared = provider.get("apiKey").and_then(Value::as_str);
    if let Some(raw) = declared {
        let raw = raw.trim();
        if let Some(var) = raw.strip_prefix('$') {
            let name = var.trim_start_matches('{').trim_end_matches('}');
            if let Ok(value) = std::env::var(name) {
                if !value.trim().is_empty() {
                    return Some(value.trim().to_string());
                }
            }
        } else if !raw.is_empty() && !raw.starts_with('!') {
            return Some(raw.to_string());
        }
    }
    if provider_name.is_empty() {
        return None;
    }
    stored_credential_key(provider_name)
}

/// Reads a provider credential from pi's credential store.
fn stored_credential_key(provider_name: &str) -> Option<String> {
    let home = crate::client::native_index::user_home();
    // 1. Check ~/.pi/agent/models.json directly for unredacted provider apiKey!
    let models_path = home.join(".pi").join("agent").join("models.json");
    if let Ok(raw) = std::fs::read_to_string(&models_path) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
            if let Some(key) = parsed
                .get("providers")
                .and_then(|p| p.get(provider_name))
                .and_then(|p| p.get("apiKey"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|k| !k.is_empty() && !k.starts_with('$') && !k.starts_with('!'))
            {
                return Some(key.to_string());
            }
        }
    }

    // 2. Check ~/.pi/agent/auth.json for OAuth or CLI-stored API keys!
    let auth_path = home.join(".pi").join("agent").join("auth.json");
    if let Ok(raw) = std::fs::read_to_string(&auth_path) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
            if let Some(entry) = parsed.get(provider_name) {
                if let Some(key) = entry
                    .get("key")
                    .or_else(|| entry.get("apiKey"))
                    .or_else(|| entry.get("access"))
                    .or_else(|| entry.get("accessToken"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                {
                    return Some(key.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod live_antigravity_discovery_test {
    use super::*;

    #[test]
    fn probes_antigravity_manager_with_stored_key() {
        let home = crate::client::native_index::user_home();
        let models_path = home.join(".pi").join("agent").join("models.json");
        let Ok(raw) = std::fs::read_to_string(&models_path) else { return; };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else { return; };
        let Some(provider) = parsed.get("providers").and_then(|p| p.get("antigravity")) else { return; };

        let key = resolve_provider_key(provider, "antigravity");
        assert!(key.is_some(), "antigravity apiKey should be found in models.json: {key:?}");
        assert_eq!(key.as_deref(), Some("sk-dcd0bdc0b8f24465a7c9e991299c4cdc"));

        // Only run the live network probe if 8045 is currently listening
        let addr: std::net::SocketAddr = "127.0.0.1:8045".parse().unwrap();
        if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_err() {
            return;
        }

        let client = PiDaemonClient::new("http://127.0.0.1:38471", "test", "test");
        let (tx, rx) = smol::channel::bounded(1);
        let prov = provider.clone();
        tokio_runtime().spawn(async move {
            let res = client.discover_models_from_provider("antigravity", &prov).await;
            let _ = tx.send(res).await;
        });
        let result = smol::block_on(rx.recv()).unwrap();
        assert!(result.is_ok(), "discovery should succeed: {:?}", result.err());
        let payload = result.unwrap();
        let models = payload["models"].as_array().expect("models array");
        assert!(!models.is_empty(), "should discover models from 8045");
        let first = &models[0];
        eprintln!("[test] discovered {} models from 8045! first={:?}", models.len(), first);
        assert!(models.iter().any(|m| m["reasoning"] == true), "should discover reasoning capabilities");
    }
}
