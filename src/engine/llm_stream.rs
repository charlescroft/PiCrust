//! Native Rust LLM SSE streaming client (`pi-gpui/src/engine/llm_stream.rs`).
//!
//! Resolves API keys from `~/.pi/agent/auth.json`, `~/.pi/agent/models.json`, or process
//! environment variables and streams responses directly from Anthropic (`/v1/messages`),
//! OpenAI-compatible (`/v1/chat/completions` or `/v1/responses`), and Google Gemini
//! (`:streamGenerateContent?alt=sse`) endpoints using `reqwest` + `eventsource-stream`.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// Supported upstream wire protocols for native LLM streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeLlmApiKind {
    OpenAiCompletions,
    OpenAiResponses,
    AnthropicMessages,
    GoogleGenerativeAi,
}

impl NativeLlmApiKind {
    pub fn from_api_str(api: &str, provider_id: &str) -> Self {
        let lower = api.trim().to_ascii_lowercase();
        match lower.as_str() {
            "anthropic-messages" | "anthropic" => Self::AnthropicMessages,
            "openai-responses" => Self::OpenAiResponses,
            "google-generative-ai" | "google" | "gemini" => Self::GoogleGenerativeAi,
            "openai-completions" | "openai" => Self::OpenAiCompletions,
            _ => {
                let p = provider_id.trim().to_ascii_lowercase();
                if p.contains("anthropic") || p.contains("claude") {
                    Self::AnthropicMessages
                } else if p.contains("google") || p.contains("gemini") {
                    Self::GoogleGenerativeAi
                } else {
                    Self::OpenAiCompletions
                }
            }
        }
    }
}

/// Conversation message in the native engine request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<NativeToolCall>,
}

/// Tool call emitted by an assistant turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Tool schema exposed to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Configuration for a single native LLM streaming request.
#[derive(Debug, Clone)]
pub struct NativeLlmStreamRequest {
    pub provider_id: String,
    pub model_id: String,
    pub api_kind: NativeLlmApiKind,
    pub base_url: String,
    pub api_key: String,
    pub system_prompt: Option<String>,
    pub messages: Vec<NativeChatMessage>,
    pub tools: Vec<NativeToolDefinition>,
    pub max_tokens: Option<u64>,
    pub thinking_level: Option<String>,
}

/// Incremental event yielded by the native LLM stream.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeLlmStreamEvent {
    TextDelta(String),
    ThinkingDelta(String),
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallArgumentsDelta {
        id: String,
        delta: String,
    },
    ToolCallComplete(NativeToolCall),
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
    },
    Done {
        stop_reason: Option<String>,
    },
    Error(String),
}

fn agent_dir() -> PathBuf {
    crate::client::native_index::user_home()
        .join(".pi")
        .join("agent")
}

/// Resolves the API key and endpoint for `(provider_id, model_id)` from local config files
/// (`~/.pi/agent/models.json`, `~/.pi/agent/auth.json`) and environment variables.
pub fn resolve_provider_endpoint_and_key(
    provider_id: &str,
) -> Result<(NativeLlmApiKind, String, String)> {
    let models_cfg = crate::client::local_store::read_models_config_local();
    let preset = crate::provider_catalog::find(provider_id);
    let mut base_url = String::new();
    let mut api_str = String::new();
    let mut raw_key = String::new();

    if let Some(prov) = models_cfg
        .get("providers")
        .and_then(Value::as_object)
        .and_then(|m| m.get(provider_id))
    {
        base_url = prov
            .get("baseUrl")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        api_str = prov
            .get("api")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        raw_key = prov
            .get("apiKey")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
    }

    // Built-in providers are not required to have a models.json entry: fall
    // back to the desktop catalog's wire protocol and endpoint.
    if api_str.is_empty()
        && let Some(p) = preset
    {
        api_str = p.api.to_string();
    }
    let api_kind = NativeLlmApiKind::from_api_str(&api_str, provider_id);
    if base_url.is_empty() {
        base_url = preset
            .map(|p| p.base_url.to_string())
            .unwrap_or_else(|| match api_kind {
                NativeLlmApiKind::AnthropicMessages => "https://api.anthropic.com/v1".to_string(),
                NativeLlmApiKind::GoogleGenerativeAi => {
                    "https://generativelanguage.googleapis.com/v1beta".to_string()
                }
                NativeLlmApiKind::OpenAiCompletions | NativeLlmApiKind::OpenAiResponses => {
                    "https://api.openai.com/v1".to_string()
                }
            });
    }

    let mut resolved_key = models_cfg
        .get("providers")
        .and_then(|p| p.get(provider_id))
        .and_then(|prov| crate::client::resolve_provider_key(prov, provider_id))
        .unwrap_or_else(|| raw_key.clone());

    // Check `~/.pi/agent/auth.json` if still empty
    if resolved_key.is_empty() {
        let auth_path = agent_dir().join("auth.json");
        if let Ok(raw) = std::fs::read_to_string(&auth_path)
            && let Ok(val) = serde_json::from_str::<Value>(&raw)
            && let Some(entry) = val.get(provider_id)
        {
            if let Some(k) = entry
                .get("key")
                .or_else(|| entry.get("access"))
                .or_else(|| entry.get("accessToken"))
                .and_then(Value::as_str)
            {
                resolved_key = k.trim().to_string();
            } else if let Some(k) = entry.as_str() {
                resolved_key = k.trim().to_string();
            }
        }
    }

    // Provider-specific environment variables first (OPENROUTER_API_KEY,
    // DEEPSEEK_API_KEY, ...), then the protocol's generic fallback.
    if resolved_key.is_empty() {
        if let Some(p) = preset {
            for var in p.env_vars {
                if let Ok(v) = std::env::var(var)
                    && !v.trim().is_empty()
                {
                    resolved_key = v.trim().to_string();
                    break;
                }
            }
        }
        if resolved_key.is_empty() {
            let env_candidates: &[&str] = match api_kind {
                NativeLlmApiKind::AnthropicMessages => &["ANTHROPIC_API_KEY"],
                NativeLlmApiKind::GoogleGenerativeAi => &["GEMINI_API_KEY", "GOOGLE_API_KEY"],
                NativeLlmApiKind::OpenAiCompletions | NativeLlmApiKind::OpenAiResponses => {
                    &["OPENAI_API_KEY"]
                }
            };
            for var in env_candidates {
                if let Ok(v) = std::env::var(var)
                    && !v.trim().is_empty()
                {
                    resolved_key = v.trim().to_string();
                    break;
                }
            }
        }
    }

    if resolved_key.is_empty() {
        return Err(anyhow!(
            "No API key configured for provider '{provider_id}'"
        ));
    }

    Ok((api_kind, base_url, resolved_key))
}

/// Builds the HTTP request URL and JSON body for a `NativeLlmStreamRequest`.
pub fn build_stream_endpoint_and_body(req: &NativeLlmStreamRequest) -> (String, Value) {
    let trimmed = req.base_url.trim_end_matches('/');
    match req.api_kind {
        NativeLlmApiKind::AnthropicMessages => {
            let url = if trimmed.ends_with("/v1") {
                format!("{trimmed}/messages")
            } else {
                format!("{trimmed}/v1/messages")
            };
            let mut messages = Vec::new();
            for m in &req.messages {
                if m.role == "system" {
                    continue;
                }
                if m.role == "tool" || m.role == "toolResult" {
                    messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                            "content": m.content,
                        }]
                    }));
                } else if !m.tool_calls.is_empty() {
                    let mut blocks = Vec::new();
                    if !m.content.is_empty() {
                        blocks.push(json!({ "type": "text", "text": m.content }));
                    }
                    for tc in &m.tool_calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments,
                        }));
                    }
                    messages.push(json!({ "role": "assistant", "content": blocks }));
                } else {
                    messages.push(json!({
                        "role": if m.role == "assistant" { "assistant" } else { "user" },
                        "content": m.content,
                    }));
                }
            }
            let mut body = json!({
                "model": req.model_id,
                "max_tokens": req.max_tokens.unwrap_or(8192),
                "messages": messages,
                "stream": true,
            });
            if let Some(ref sys) = req.system_prompt
                && !sys.trim().is_empty()
            {
                body["system"] = json!(sys);
            }
            if !req.tools.is_empty() {
                let tools: Vec<Value> = req
                    .tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "input_schema": t.parameters,
                        })
                    })
                    .collect();
                body["tools"] = json!(tools);
            }
            (url, body)
        }
        NativeLlmApiKind::GoogleGenerativeAi => {
            let base = if trimmed.contains("/v1beta") || trimmed.contains("/v1") {
                trimmed.to_string()
            } else {
                format!("{trimmed}/v1beta")
            };
            let model_path = if req.model_id.starts_with("models/") {
                req.model_id.clone()
            } else {
                format!("models/{}", req.model_id)
            };
            let url = format!(
                "{base}/{model_path}:streamGenerateContent?alt=sse&key={}",
                req.api_key
            );
            let mut contents = Vec::new();
            for m in &req.messages {
                if m.role == "system" {
                    continue;
                }
                let role = if m.role == "assistant" { "model" } else { "user" };
                contents.push(json!({
                    "role": role,
                    "parts": [{ "text": m.content }],
                }));
            }
            let mut body = json!({ "contents": contents });
            if let Some(ref sys) = req.system_prompt
                && !sys.trim().is_empty()
            {
                body["systemInstruction"] = json!({
                    "parts": [{ "text": sys }]
                });
            }
            (url, body)
        }
        NativeLlmApiKind::OpenAiResponses => {
            let url = if trimmed.ends_with("/v1") {
                format!("{trimmed}/responses")
            } else {
                format!("{trimmed}/v1/responses")
            };
            let mut input = Vec::new();
            if let Some(ref sys) = req.system_prompt
                && !sys.trim().is_empty()
            {
                input.push(json!({ "role": "system", "content": sys }));
            }
            for m in &req.messages {
                input.push(json!({ "role": m.role, "content": m.content }));
            }
            let body = json!({
                "model": req.model_id,
                "input": input,
                "stream": true,
            });
            (url, body)
        }
        NativeLlmApiKind::OpenAiCompletions => {
            let url = if trimmed.ends_with("/v1") {
                format!("{trimmed}/chat/completions")
            } else {
                format!("{trimmed}/v1/chat/completions")
            };
            let mut messages = Vec::new();
            if let Some(ref sys) = req.system_prompt
                && !sys.trim().is_empty()
            {
                messages.push(json!({ "role": "system", "content": sys }));
            }
            for m in &req.messages {
                if m.role == "tool" || m.role == "toolResult" {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.content,
                    }));
                } else if !m.tool_calls.is_empty() {
                    let tcs: Vec<Value> = m
                        .tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "{}".to_string()),
                                }
                            })
                        })
                        .collect();
                    messages.push(json!({
                        "role": "assistant",
                        "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                        "tool_calls": tcs,
                    }));
                } else {
                    messages.push(json!({
                        "role": m.role,
                        "content": m.content,
                    }));
                }
            }
            let mut body = json!({
                "model": req.model_id,
                "messages": messages,
                "stream": true,
            });
            if let Some(max_tok) = req.max_tokens {
                body["max_tokens"] = json!(max_tok);
            }
            if !req.tools.is_empty() {
                let tools: Vec<Value> = req
                    .tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": t.parameters,
                            }
                        })
                    })
                    .collect();
                body["tools"] = json!(tools);
            }
            (url, body)
        }
    }
}

/// Parses a single SSE JSON payload into zero or more `NativeLlmStreamEvent`s.
pub fn parse_sse_chunk(
    api_kind: NativeLlmApiKind,
    data: &str,
    tool_accumulators: &mut HashMap<String, (String, String)>,
) -> Vec<NativeLlmStreamEvent> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        if trimmed == "[DONE]" {
            let mut out = Vec::new();
            for (id, (name, args_raw)) in tool_accumulators.drain() {
                let args = serde_json::from_str::<Value>(&args_raw).unwrap_or_else(|_| json!({}));
                out.push(NativeLlmStreamEvent::ToolCallComplete(NativeToolCall {
                    id,
                    name,
                    arguments: args,
                }));
            }
            out.push(NativeLlmStreamEvent::Done { stop_reason: None });
            return out;
        }
        return Vec::new();
    }

    let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
        return Vec::new();
    };
    let mut events = Vec::new();

    match api_kind {
        NativeLlmApiKind::AnthropicMessages => {
            let ev_type = val.get("type").and_then(Value::as_str).unwrap_or("");
            match ev_type {
                "content_block_start" => {
                    if let Some(cb) = val.get("content_block")
                        && cb.get("type").and_then(Value::as_str) == Some("tool_use")
                    {
                        let idx = val
                            .get("index")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .to_string();
                        let id = cb
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or(&idx)
                            .to_string();
                        let name = cb
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        tool_accumulators.insert(idx, (id.clone(), String::new()));
                        tool_accumulators.insert(format!("name:{id}"), (name.clone(), String::new()));
                        events.push(NativeLlmStreamEvent::ToolCallStart { id, name });
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = val.get("delta") {
                        let dtype = delta.get("type").and_then(Value::as_str).unwrap_or("");
                        if dtype == "text_delta" {
                            if let Some(t) = delta.get("text").and_then(Value::as_str)
                                && !t.is_empty()
                            {
                                events.push(NativeLlmStreamEvent::TextDelta(t.to_string()));
                            }
                        } else if dtype == "thinking_delta" {
                            if let Some(t) = delta.get("thinking").and_then(Value::as_str)
                                && !t.is_empty()
                            {
                                events.push(NativeLlmStreamEvent::ThinkingDelta(t.to_string()));
                            }
                        } else if dtype == "input_json_delta"
                            && let Some(part) = delta.get("partial_json").and_then(Value::as_str)
                        {
                            let idx = val
                                .get("index")
                                .and_then(Value::as_u64)
                                .unwrap_or(0)
                                .to_string();
                            if let Some((id, buf)) = tool_accumulators.get_mut(&idx) {
                                buf.push_str(part);
                                events.push(NativeLlmStreamEvent::ToolCallArgumentsDelta {
                                    id: id.clone(),
                                    delta: part.to_string(),
                                });
                            }
                        }
                    }
                }
                "content_block_stop" => {
                    let idx = val
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .to_string();
                    if let Some((id, args_raw)) = tool_accumulators.remove(&idx) {
                        let name = tool_accumulators
                            .remove(&format!("name:{id}"))
                            .map(|(n, _)| n)
                            .unwrap_or_default();
                        let arguments = serde_json::from_str::<Value>(&args_raw)
                            .unwrap_or_else(|_| json!({}));
                        events.push(NativeLlmStreamEvent::ToolCallComplete(NativeToolCall {
                            id,
                            name,
                            arguments,
                        }));
                    }
                }
                "message_delta" => {
                    if let Some(usage) = val.get("usage") {
                        let input = usage
                            .get("input_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        let output = usage
                            .get("output_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        let cache_read = usage
                            .get("cache_read_input_tokens")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);
                        events.push(NativeLlmStreamEvent::Usage {
                            input_tokens: input,
                            output_tokens: output,
                            cache_read_tokens: cache_read,
                        });
                    }
                    let stop_reason = val
                        .get("delta")
                        .and_then(|d| d.get("stop_reason"))
                        .and_then(Value::as_str)
                        .map(String::from);
                    if stop_reason.is_some() {
                        events.push(NativeLlmStreamEvent::Done { stop_reason });
                    }
                }
                "message_stop" => {
                    events.push(NativeLlmStreamEvent::Done { stop_reason: None });
                }
                "error" => {
                    let msg = val
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or("Anthropic stream error")
                        .to_string();
                    events.push(NativeLlmStreamEvent::Error(msg));
                }
                _ => {}
            }
        }
        NativeLlmApiKind::GoogleGenerativeAi => {
            if let Some(candidates) = val.get("candidates").and_then(Value::as_array) {
                for cand in candidates {
                    if let Some(parts) = cand
                        .get("content")
                        .and_then(|c| c.get("parts"))
                        .and_then(Value::as_array)
                    {
                        for part in parts {
                            let is_thought = part
                                .get("thought")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            if let Some(t) = part.get("text").and_then(Value::as_str)
                                && !t.is_empty()
                            {
                                if is_thought {
                                    events.push(NativeLlmStreamEvent::ThinkingDelta(t.to_string()));
                                } else {
                                    events.push(NativeLlmStreamEvent::TextDelta(t.to_string()));
                                }
                            }
                        }
                    }
                    if let Some(reason) = cand.get("finishReason").and_then(Value::as_str) {
                        events.push(NativeLlmStreamEvent::Done {
                            stop_reason: Some(reason.to_string()),
                        });
                    }
                }
            }
        }
        NativeLlmApiKind::OpenAiResponses => {
            let ev_type = val.get("type").and_then(Value::as_str).unwrap_or("");
            if ev_type == "response.output_text.delta" {
                if let Some(d) = val.get("delta").and_then(Value::as_str)
                    && !d.is_empty()
                {
                    events.push(NativeLlmStreamEvent::TextDelta(d.to_string()));
                }
            } else if ev_type == "response.completed" {
                events.push(NativeLlmStreamEvent::Done {
                    stop_reason: Some("stop".to_string()),
                });
            }
        }
        NativeLlmApiKind::OpenAiCompletions => {
            if let Some(choices) = val.get("choices").and_then(Value::as_array) {
                for choice in choices {
                    if let Some(delta) = choice.get("delta") {
                        if let Some(reasoning) = delta
                            .get("reasoning_content")
                            .or_else(|| delta.get("reasoning"))
                            .and_then(Value::as_str)
                            && !reasoning.is_empty()
                        {
                            events.push(NativeLlmStreamEvent::ThinkingDelta(
                                reasoning.to_string(),
                            ));
                        }
                        if let Some(content) = delta.get("content").and_then(Value::as_str)
                            && !content.is_empty()
                        {
                            events.push(NativeLlmStreamEvent::TextDelta(content.to_string()));
                        }
                        if let Some(tcs) = delta.get("tool_calls").and_then(Value::as_array) {
                            for tc in tcs {
                                let idx = tc
                                    .get("index")
                                    .and_then(Value::as_u64)
                                    .unwrap_or(0)
                                    .to_string();
                                let id_opt = tc.get("id").and_then(Value::as_str);
                                let func = tc.get("function");
                                let name_opt =
                                    func.and_then(|f| f.get("name")).and_then(Value::as_str);
                                let args_opt = func
                                    .and_then(|f| f.get("arguments"))
                                    .and_then(Value::as_str);

                                let entry = tool_accumulators.entry(idx.clone()).or_insert_with(|| {
                                    let cid = id_opt.unwrap_or(&idx).to_string();
                                    let cname = name_opt.unwrap_or("").to_string();
                                    events.push(NativeLlmStreamEvent::ToolCallStart {
                                        id: cid.clone(),
                                        name: cname.clone(),
                                    });
                                    (format!("{cid}::{cname}"), String::new())
                                });
                                if let Some(chunk) = args_opt
                                    && !chunk.is_empty()
                                {
                                    entry.1.push_str(chunk);
                                    let cid = entry
                                        .0
                                        .split("::")
                                        .next()
                                        .unwrap_or(&idx)
                                        .to_string();
                                    events.push(NativeLlmStreamEvent::ToolCallArgumentsDelta {
                                        id: cid,
                                        delta: chunk.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
                        for (_, (id_and_name, args_raw)) in tool_accumulators.drain() {
                            let mut parts = id_and_name.splitn(2, "::");
                            let id = parts.next().unwrap_or("call_0").to_string();
                            let name = parts.next().unwrap_or("").to_string();
                            let arguments = serde_json::from_str::<Value>(&args_raw)
                                .unwrap_or_else(|_| json!({}));
                            events.push(NativeLlmStreamEvent::ToolCallComplete(NativeToolCall {
                                id,
                                name,
                                arguments,
                            }));
                        }
                        events.push(NativeLlmStreamEvent::Done {
                            stop_reason: Some(finish.to_string()),
                        });
                    }
                }
            }
            if let Some(usage) = val.get("usage") {
                let prompt = usage
                    .get("prompt_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let completion = usage
                    .get("completion_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if prompt > 0 || completion > 0 {
                    events.push(NativeLlmStreamEvent::Usage {
                        input_tokens: prompt,
                        output_tokens: completion,
                        cache_read_tokens: 0,
                    });
                }
            }
        }
    }

    events
}

/// Spawns an SSE stream request on the single-threaded Tokio HTTP driver and forwards
/// `NativeLlmStreamEvent` items over an `mpsc::UnboundedReceiver`.
pub fn spawn_native_llm_stream(
    http: reqwest::Client,
    req: NativeLlmStreamRequest,
) -> mpsc::UnboundedReceiver<NativeLlmStreamEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    crate::client::tokio_runtime().spawn(async move {
        if let Err(err) = run_native_llm_stream_inner(http, req, &tx).await {
            let _ = tx.send(NativeLlmStreamEvent::Error(err.to_string()));
        }
    });
    rx
}

async fn run_native_llm_stream_inner(
    http: reqwest::Client,
    req: NativeLlmStreamRequest,
    tx: &mpsc::UnboundedSender<NativeLlmStreamEvent>,
) -> Result<()> {
    let (url, body) = build_stream_endpoint_and_body(&req);
    let mut builder = http
        .post(&url)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json");

    match req.api_kind {
        NativeLlmApiKind::AnthropicMessages => {
            builder = builder
                .header("x-api-key", &req.api_key)
                .header("anthropic-version", "2023-06-01");
        }
        NativeLlmApiKind::GoogleGenerativeAi => {
            builder = builder.header("x-goog-api-key", &req.api_key);
        }
        NativeLlmApiKind::OpenAiCompletions | NativeLlmApiKind::OpenAiResponses => {
            builder = builder.header("Authorization", format!("Bearer {}", req.api_key));
        }
    }

    let resp = builder
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Failed to connect to {}", req.base_url))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("HTTP {status}: {text}"));
    }

    let mut stream = resp.bytes_stream().eventsource();
    let mut accumulators: HashMap<String, (String, String)> = HashMap::new();

    while let Some(item) = stream.next().await {
        match item {
            Ok(ev) => {
                for parsed in parse_sse_chunk(req.api_kind, &ev.data, &mut accumulators) {
                    if tx.send(parsed).is_err() {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(NativeLlmStreamEvent::Error(e.to_string()));
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_and_anthropic_sse_chunks() {
        let mut acc = HashMap::new();
        let oai_text = r#"{"choices":[{"delta":{"reasoning_content":"step 1","content":"Hello"}}]}"#;
        let evs = parse_sse_chunk(NativeLlmApiKind::OpenAiCompletions, oai_text, &mut acc);
        assert_eq!(
            evs,
            vec![
                NativeLlmStreamEvent::ThinkingDelta("step 1".to_string()),
                NativeLlmStreamEvent::TextDelta("Hello".to_string()),
            ]
        );

        let anth_start = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read"}}"#;
        let evs2 = parse_sse_chunk(NativeLlmApiKind::AnthropicMessages, anth_start, &mut acc);
        assert_eq!(
            evs2,
            vec![NativeLlmStreamEvent::ToolCallStart {
                id: "toolu_1".to_string(),
                name: "read".to_string(),
            }]
        );

        let anth_delta = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"Cargo.toml\"}"}}"#;
        let _ = parse_sse_chunk(NativeLlmApiKind::AnthropicMessages, anth_delta, &mut acc);

        let anth_stop = r#"{"type":"content_block_stop","index":1}"#;
        let evs3 = parse_sse_chunk(NativeLlmApiKind::AnthropicMessages, anth_stop, &mut acc);
        assert_eq!(
            evs3,
            vec![NativeLlmStreamEvent::ToolCallComplete(NativeToolCall {
                id: "toolu_1".to_string(),
                name: "read".to_string(),
                arguments: json!({"path": "Cargo.toml"}),
            })]
        );
    }
}
