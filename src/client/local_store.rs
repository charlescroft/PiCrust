//! Phase 6.3.1: Local-First Direct File & Configuration Store (`local_store.rs`).
//!
//! Provides zero-RPC, atomic local filesystem read/write operations for all
//! static configurations, session metadata, scheduled tasks, subagent profiles,
//! MCP servers, project trust, and Git worktrees in `~/.pi/agent/` and `<cwd>/.pi/`.
//!
//! Every write uses a temporary sibling file (`*.tmp-<pid>-<nanos>`) flushed and
//! atomically renamed (`fs::rename`) so concurrent readers or power loss never
//! observe partial JSON/JSONL or Markdown files.

use crate::client::native_index::{pi_sessions_root, scan_local_sessions, user_home};
use crate::client::types::{
    McpServerRow, ScheduledRunView, ScheduledTask, SubagentProfileRow, WorktreeInfo,
};
use chrono::Utc;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Root directory of the Pi agent (`~/.pi/agent` or `$PI_CODING_AGENT_DIR`).
pub fn pi_agent_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    user_home().join(".pi").join("agent")
}

/// Atomic file replacement: writes `bytes` to a sibling temp file and renames over `path`.
pub fn write_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file.tmp");
    let tmp_path = path.with_file_name(format!(".{file_name}.tmp-{pid}-{nanos}"));

    {
        let mut f = File::create(&tmp_path)
            .map_err(|e| format!("create tmp {}: {e}", tmp_path.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("write tmp {}: {e}", tmp_path.display()))?;
        let _ = f.sync_all();
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Preserve existing permissions if the target exists, else default to 0600 for agent files
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(&tmp_path, meta.permissions());
        } else if path.starts_with(pi_agent_dir()) {
            let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
        }
    }

    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        format!("rename {} -> {}: {e}", tmp_path.display(), path.display())
    })
}

pub fn read_json_object_or_empty(path: &Path) -> Map<String, Value> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Map::new();
    };
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

pub fn write_json_pretty_atomic(path: &Path, val: &Value) -> Result<(), String> {
    let mut text =
        serde_json::to_string_pretty(val).map_err(|e| format!("serialize json: {e}"))?;
    text.push('\n');
    write_file_atomic(path, text.as_bytes())
}

fn random_hex_8() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id() as u128;
    let mixed = nanos
        .wrapping_mul(6364136223846793005)
        .wrapping_add(pid ^ (nanos >> 32));
    format!("{:08x}", (mixed as u32))
}

fn generate_uuid_v4() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let a = (nanos & 0xffff_ffff) as u32;
    let b = ((nanos >> 32) & 0xffff) as u16;
    let c = (((nanos >> 48) & 0x0fff) as u16) | 0x4000;
    let d = (((nanos >> 60) & 0x3fff) as u16) | 0x8000;
    let e = (nanos.wrapping_mul(1103515245).wrapping_add(12345)) & 0xffff_ffff_ffff;
    format!("{a:08x}-{b:04x}-{c:04x}-{d:04x}-{e:012x}")
}

// ============================================================================
// 1. Session Rename & Delete (`~/.pi/agent/sessions/<encoded-cwd>/*.jsonl`)
// ============================================================================

/// Locate a session `.jsonl` file by session ID across `~/.pi/agent/sessions/`.
pub fn find_session_file_by_id(session_id: &str) -> Option<PathBuf> {
    let sessions = scan_local_sessions(&pi_sessions_root());
    sessions
        .into_iter()
        .find(|s| s.id == session_id)
        .and_then(|s| s.path.map(PathBuf::from))
}

/// Appends a `{"type":"session_info","id":"<8hex>","parentId":...,"timestamp":...,"name":...}`
/// entry to the session `.jsonl` file, matching `PATCH /api/sessions/[id]`.
pub fn rename_session_local(session_id: &str, new_name: &str) -> Result<PathBuf, String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("Session name cannot be empty".to_string());
    }
    let path = find_session_file_by_id(session_id)
        .ok_or_else(|| format!("Session {session_id} not found on disk"))?;

    let last_entry_id = read_last_entry_id(&path);
    let entry = json!({
        "type": "session_info",
        "id": random_hex_8(),
        "parentId": last_entry_id,
        "timestamp": Utc::now().to_rfc3339(),
        "name": trimmed,
    });

    let mut line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("append {}: {e}", path.display()))?;
    let _ = file.sync_all();
    Ok(path)
}

fn read_last_entry_id(path: &Path) -> Value {
    let Ok(file) = File::open(path) else {
        return Value::Null;
    };
    let reader = BufReader::new(file);
    let mut last_id: Option<String> = None;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            if val.get("type").and_then(Value::as_str) != Some("session") {
                if let Some(id) = val.get("id").and_then(Value::as_str) {
                    last_id = Some(id.to_string());
                }
            }
        }
    }
    last_id.map(Value::String).unwrap_or(Value::Null)
}

/// Deletes a session `.jsonl` file, cascades deletion to subagent child sessions,
/// and re-parents forked child sessions (`header.parentSession`) to the deleted
/// session's own `parentSession`, matching `DELETE /api/sessions/[id]`.
pub fn delete_session_local(session_id: &str) -> Result<(), String> {
    let all_sessions = scan_local_sessions(&pi_sessions_root());
    let Some(target) = all_sessions.iter().find(|s| s.id == session_id) else {
        return Err(format!("Session {session_id} not found"));
    };
    let Some(ref target_path_str) = target.path else {
        return Err(format!("Session {session_id} has no file path"));
    };
    let target_path = PathBuf::from(target_path_str);
    let deleted_parent_session = target.parent_session.clone();

    // Collect cascade delete set (target + recursive subagent children)
    let mut to_delete_ids = HashSet::new();
    to_delete_ids.insert(session_id.to_string());
    let mut changed = true;
    while changed {
        changed = false;
        for s in &all_sessions {
            if to_delete_ids.contains(&s.id) {
                continue;
            }
            if let Some(ref rel) = s.relation {
                if rel.kind.as_deref() == Some("subagent") {
                    if let Some(ref pid) = rel.parent_session_id {
                        if to_delete_ids.contains(pid) {
                            to_delete_ids.insert(s.id.clone());
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    // Re-parent forked children whose header.parentSession points to the deleted file path
    for s in &all_sessions {
        if to_delete_ids.contains(&s.id) {
            continue;
        }
        if s.parent_session.as_deref() == Some(target_path_str.as_str()) {
            if let Some(ref child_path_str) = s.path {
                let child_path = PathBuf::from(child_path_str);
                let _ = reparent_session_header(&child_path, deleted_parent_session.as_deref());
            }
        }
    }

    // Delete target and subagent files
    for s in &all_sessions {
        if to_delete_ids.contains(&s.id) {
            if let Some(ref p) = s.path {
                let _ = fs::remove_file(p);
            }
        }
    }
    if target_path.exists() {
        let _ = fs::remove_file(&target_path);
    }
    Ok(())
}

fn reparent_session_header(path: &Path, new_parent: Option<&str>) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = raw.lines().map(String::from).collect();
    if let Some(first) = lines.first_mut() {
        if let Ok(mut hdr) = serde_json::from_str::<Value>(first) {
            if let Some(obj) = hdr.as_object_mut() {
                match new_parent {
                    Some(p) if !p.is_empty() => {
                        obj.insert("parentSession".to_string(), Value::String(p.to_string()));
                    }
                    _ => {
                        obj.remove("parentSession");
                    }
                }
                if let Ok(serialized) = serde_json::to_string(&hdr) {
                    *first = serialized;
                    let mut out = lines.join("\n");
                    out.push('\n');
                    return write_file_atomic(path, out.as_bytes());
                }
            }
        }
    }
    Ok(())
}

/// Fork a session locally up to `entry_id` into a new `.jsonl` file in the same directory.
pub fn fork_session_local(session_id: &str, entry_id: Option<&str>) -> Result<(String, PathBuf), String> {
    let src_path = find_session_file_by_id(session_id)
        .ok_or_else(|| format!("Session {session_id} not found"))?;
    let raw = fs::read_to_string(&src_path).map_err(|e| e.to_string())?;
    let parent_dir = src_path
        .parent()
        .ok_or_else(|| "Invalid session directory".to_string())?;

    let new_id = generate_uuid_v4();
    let now = Utc::now();
    let ts_prefix = now.format("%Y-%m-%dT%H-%M-%S-%3fZ").to_string();
    let new_path = parent_dir.join(format!("{ts_prefix}_{new_id}.jsonl"));

    let mut out_lines = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(mut val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if idx == 0 && val.get("type").and_then(Value::as_str) == Some("session") {
            if let Some(obj) = val.as_object_mut() {
                obj.insert("id".to_string(), Value::String(new_id.clone()));
                obj.insert("timestamp".to_string(), Value::String(now.to_rfc3339()));
                obj.insert(
                    "parentSession".to_string(),
                    Value::String(src_path.to_string_lossy().into_owned()),
                );
            }
            if let Ok(s) = serde_json::to_string(&val) {
                out_lines.push(s);
            }
            continue;
        }
        if let Ok(s) = serde_json::to_string(&val) {
            out_lines.push(s);
        }
        if let Some(target_eid) = entry_id {
            if val.get("id").and_then(Value::as_str) == Some(target_eid) {
                break;
            }
        }
    }

    let mut content = out_lines.join("\n");
    content.push('\n');
    write_file_atomic(&new_path, content.as_bytes())?;
    Ok((new_id, new_path))
}

// ============================================================================
// 2. Global Settings (`~/.pi/agent/settings.json`) & Pi-Web Settings
// ============================================================================

pub fn global_settings_path() -> PathBuf {
    pi_agent_dir().join("settings.json")
}

pub fn pi_web_settings_path() -> PathBuf {
    pi_agent_dir().join("pi-web").join("settings.json")
}

pub fn read_global_settings() -> Map<String, Value> {
    read_json_object_or_empty(&global_settings_path())
}

pub fn update_global_settings<F>(mutate: F) -> Result<Map<String, Value>, String>
where
    F: FnOnce(&mut Map<String, Value>),
{
    let path = global_settings_path();
    let mut map = read_json_object_or_empty(&path);
    mutate(&mut map);
    write_json_pretty_atomic(&path, &Value::Object(map.clone()))?;
    Ok(map)
}

pub fn read_pi_web_settings() -> Map<String, Value> {
    let mut map = read_json_object_or_empty(&pi_web_settings_path());
    map.entry("version".to_string()).or_insert_with(|| json!(1));
    map
}

pub fn update_pi_web_settings<F>(mutate: F) -> Result<Map<String, Value>, String>
where
    F: FnOnce(&mut Map<String, Value>),
{
    let path = pi_web_settings_path();
    let mut map = read_pi_web_settings();
    mutate(&mut map);
    map.insert("version".to_string(), json!(1));
    write_json_pretty_atomic(&path, &Value::Object(map.clone()))?;
    Ok(map)
}

pub fn set_artifacts_prompt_local(enabled: bool) -> Result<(), String> {
    update_pi_web_settings(|map| {
        map.insert("artifactsPrompt".to_string(), Value::Bool(enabled));
    })
    .map(|_| ())
}

pub fn set_auto_session_title_local(enabled: bool) -> Result<(), String> {
    update_pi_web_settings(|map| {
        map.insert("autoTitle".to_string(), Value::Bool(enabled));
    })
    .map(|_| ())
}

pub fn save_network_proxy_local(enabled: bool, url: &str, no_proxy: &str) -> Result<(), String> {
    update_pi_web_settings(|map| {
        map.insert(
            "networkProxy".to_string(),
            json!({
                "enabled": enabled,
                "url": url.trim(),
                "noProxy": no_proxy.trim(),
            }),
        );
    })
    .map(|_| ())
}

/// Update `defaultTools` in `~/.pi/agent/settings.json` for `codemode` or `powershell`,
/// preserving pi's `+tool` / `-tool` modifier rules.
pub fn update_tool_settings_local(body: &Value) -> Result<(), String> {
    update_global_settings(|settings| {
        if let Some(codemode_pref) = body.get("codemode").and_then(Value::as_str) {
            let mut entries: Vec<String> = settings
                .get("defaultTools")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            entries.retain(|e| e.trim_start_matches(['+', '-']) != "codemode");
            if codemode_pref == "always" {
                entries.push("+codemode".to_string());
            }
            if entries.is_empty() {
                settings.remove("defaultTools");
            } else {
                settings.insert("defaultTools".to_string(), json!(entries));
            }
        }
        if let Some(ps_enabled) = body.get("enabled").and_then(Value::as_bool) {
            let mut entries: Vec<String> = settings
                .get("defaultTools")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            entries.retain(|e| {
                let bare = e.trim_start_matches(['+', '-']);
                bare != "powershell" && bare != "bash"
            });
            if ps_enabled {
                entries.push("-bash".to_string());
                entries.push("+powershell".to_string());
            }
            if entries.is_empty() {
                settings.remove("defaultTools");
            } else {
                settings.insert("defaultTools".to_string(), json!(entries));
            }
        }
        if let Some(mode) = body.get("codemodeMode").and_then(Value::as_str) {
            let mut cm = settings
                .get("codemode")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if mode == "on" {
                cm.remove("mode");
            } else {
                cm.insert("mode".to_string(), Value::String(mode.to_string()));
            }
            if cm.is_empty() {
                settings.remove("codemode");
            } else {
                settings.insert("codemode".to_string(), Value::Object(cm));
            }
        }
        if let Some(budget) = body.get("codemodeInlineBudget") {
            let mut cm = settings
                .get("codemode")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if budget.as_str() == Some("default") || budget.is_null() {
                cm.remove("inlineBudget");
            } else {
                cm.insert("inlineBudget".to_string(), budget.clone());
            }
            if cm.is_empty() {
                settings.remove("codemode");
            } else {
                settings.insert("codemode".to_string(), Value::Object(cm));
            }
        }
    })
    .map(|_| ())
}

// ============================================================================
// 3. Models Config (`~/.pi/agent/models.json`) & Enabled Models (`settings.json`)
// ============================================================================

pub fn models_config_path() -> PathBuf {
    pi_agent_dir().join("models.json")
}

pub fn read_models_config_local() -> Value {
    let path = models_config_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return json!({ "providers": {} });
    };
    serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({ "providers": {} }))
}

/// Saves `~/.pi/agent/models.json`, preserving existing literal `apiKey` values when
/// the incoming payload omits `apiKey` or passes `"__REDACTED__"`.
pub fn save_models_config_local(incoming: &Value) -> Result<(), String> {
    let path = models_config_path();
    let existing = read_models_config_local();
    let mut merged = incoming.clone();

    if let (Some(existing_providers), Some(incoming_providers)) = (
        existing.get("providers").and_then(Value::as_object),
        merged
            .get_mut("providers")
            .and_then(Value::as_object_mut),
    ) {
        for (prov_id, prov_val) in incoming_providers.iter_mut() {
            if let Some(prov_obj) = prov_val.as_object_mut() {
                let incoming_key = prov_obj.get("apiKey").and_then(Value::as_str);
                let should_preserve = matches!(incoming_key, None | Some("") | Some("__REDACTED__"));
                if should_preserve {
                    if let Some(old_key) = existing_providers
                        .get(prov_id)
                        .and_then(|p| p.get("apiKey"))
                        .and_then(Value::as_str)
                    {
                        if !old_key.is_empty() {
                            prov_obj.insert("apiKey".to_string(), Value::String(old_key.to_string()));
                        }
                    }
                }
            }
        }
    }

    write_json_pretty_atomic(&path, &merged)
}

/// Persists a provider API key in `~/.pi/agent/auth.json` so the native engine
/// and the sidecar can both use it, even while the sidecar is offline. The
/// shape matches pi's credential store (`{ "provider": { "type": "api_key",
/// "key": "..." } }`) and unrelated credentials are preserved.
pub fn save_api_key_local(provider_id: &str, api_key: &str) -> Result<(), String> {
    let provider_id = provider_id.trim();
    let api_key = api_key.trim();
    if provider_id.is_empty() || api_key.is_empty() {
        return Err("provider id and API key are required".to_string());
    }
    let path = pi_agent_dir().join("auth.json");
    let mut root = read_json_object_or_empty(&path);
    root.insert(
        provider_id.to_string(),
        json!({ "type": "api_key", "key": api_key }),
    );
    write_json_pretty_atomic(&path, &Value::Object(root))
}

/// Minimal-edit update of `enabledModels` in `~/.pi/agent/settings.json`.
pub fn update_enabled_models_local(body: &Value) -> Result<Vec<String>, String> {
    let mut updated_list = Vec::new();
    update_global_settings(|settings| {
        let mut patterns: Vec<String> = settings
            .get("enabledModels")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let action = body.get("action").and_then(Value::as_str).unwrap_or("set");
        match action {
            "clear" => {
                patterns.clear();
            }
            "set" | _ => {
                let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                if let Some(models) = body.get("models").and_then(Value::as_array) {
                    for m in models.iter().filter_map(Value::as_str) {
                        if enabled {
                            if !patterns.iter().any(|p| p == m) {
                                patterns.push(m.to_string());
                            }
                        } else {
                            patterns.retain(|p| p != m);
                        }
                    }
                }
            }
        }

        updated_list = patterns.clone();
        if patterns.is_empty() {
            settings.remove("enabledModels");
        } else {
            settings.insert("enabledModels".to_string(), json!(patterns));
        }
    })?;
    Ok(updated_list)
}

// ============================================================================
// 4. Scheduled Tasks (`~/.pi/agent/scheduled-tasks.json` & `runs/*.jsonl`)
// ============================================================================

pub fn scheduled_tasks_file_path() -> PathBuf {
    pi_agent_dir().join("scheduled-tasks.json")
}

pub fn scheduled_runs_dir() -> PathBuf {
    pi_agent_dir().join("scheduled-tasks").join("runs")
}

pub fn load_scheduled_tasks_local() -> Vec<ScheduledTask> {
    let path = scheduled_tasks_file_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    val.get("tasks")
        .and_then(|t| serde_json::from_value::<Vec<ScheduledTask>>(t.clone()).ok())
        .unwrap_or_default()
}

fn read_scheduled_tasks_raw_array() -> Vec<Value> {
    let path = scheduled_tasks_file_path();
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|v| v.get("tasks").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn write_scheduled_tasks_raw_array(tasks: &[Value]) -> Result<(), String> {
    let doc = json!({
        "version": 1,
        "tasks": tasks,
    });
    write_json_pretty_atomic(&scheduled_tasks_file_path(), &doc)
}

pub fn create_scheduled_task_local(body: &Value) -> Result<Value, String> {
    let mut tasks = read_scheduled_tasks_raw_array();
    let now = Utc::now().to_rfc3339();
    let mut item = body.as_object().cloned().unwrap_or_default();
    let id = item
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(generate_uuid_v4);
    item.insert("id".to_string(), Value::String(id));
    item.entry("enabled".to_string()).or_insert(Value::Bool(true));
    item.entry("createdAt".to_string())
        .or_insert_with(|| Value::String(now.clone()));
    item.insert("updatedAt".to_string(), Value::String(now));

    let created = Value::Object(item);
    tasks.push(created.clone());
    write_scheduled_tasks_raw_array(&tasks)?;
    Ok(created)
}

pub fn update_scheduled_task_local(id: &str, patch: &Value) -> Result<Value, String> {
    let mut tasks = read_scheduled_tasks_raw_array();
    let Some(target) = tasks
        .iter_mut()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(id))
    else {
        return Err(format!("Scheduled task {id} not found"));
    };
    if let (Some(obj), Some(patch_obj)) = (target.as_object_mut(), patch.as_object()) {
        for (k, v) in patch_obj {
            if k != "id" {
                if v.is_null() {
                    obj.remove(k);
                } else {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        obj.insert("updatedAt".to_string(), Value::String(Utc::now().to_rfc3339()));
    }
    let updated = target.clone();
    write_scheduled_tasks_raw_array(&tasks)?;
    Ok(updated)
}

pub fn delete_scheduled_task_local(id: &str) -> Result<(), String> {
    let mut tasks = read_scheduled_tasks_raw_array();
    let before = tasks.len();
    tasks.retain(|t| t.get("id").and_then(Value::as_str) != Some(id));
    if tasks.len() == before {
        return Err(format!("Scheduled task {id} not found"));
    }
    write_scheduled_tasks_raw_array(&tasks)?;
    let runs_file = scheduled_runs_dir().join(format!("{id}.jsonl"));
    let archive_file = scheduled_runs_dir().join(format!("{id}.archive.jsonl"));
    let _ = fs::remove_file(runs_file);
    let _ = fs::remove_file(archive_file);
    Ok(())
}

pub fn load_scheduled_task_runs_local(task_id: &str) -> Vec<ScheduledRunView> {
    let runs_file = scheduled_runs_dir().join(format!("{task_id}.jsonl"));
    let Ok(raw) = fs::read_to_string(&runs_file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(run) = serde_json::from_str::<ScheduledRunView>(trimmed) {
            out.push(run);
        }
    }
    out.reverse();
    out
}

pub fn append_scheduled_task_run_local(
    task_id: &str,
    run_record: &Value,
) -> Result<(), String> {
    let dir = scheduled_runs_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let runs_file = dir.join(format!("{task_id}.jsonl"));
    let mut line = serde_json::to_string(run_record).map_err(|e| e.to_string())?;
    line.push('\n');
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&runs_file)
        .map_err(|e| e.to_string())?;
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let _ = f.sync_all();
    Ok(())
}

// ============================================================================
// 5. Subagent Settings & Profiles (`~/.pi/agent/agents/`)
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct LocalSubagentSettings {
    pub enabled: bool,
    pub max_concurrent: usize,
    pub disabled_built_ins: Vec<String>,
}

pub fn subagent_settings_path() -> PathBuf {
    pi_agent_dir().join("agents").join("settings.json")
}

pub fn read_subagent_settings_local() -> LocalSubagentSettings {
    let map = read_json_object_or_empty(&subagent_settings_path());
    let enabled = map
        .get("builtInEnabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let max_concurrent = map
        .get("maxConcurrent")
        .and_then(Value::as_u64)
        .map(|n| n.clamp(1, 8) as usize)
        .unwrap_or(4);
    let disabled_built_ins: Vec<String> = map
        .get("disabledBuiltIns")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    LocalSubagentSettings {
        enabled,
        max_concurrent,
        disabled_built_ins,
    }
}

pub fn save_subagent_settings_local(
    enabled: bool,
    max_concurrent: usize,
    disabled_built_ins: Option<Vec<String>>,
) -> Result<LocalSubagentSettings, String> {
    let path = subagent_settings_path();
    let mut map = read_json_object_or_empty(&path);
    map.insert("version".to_string(), json!(1));
    map.insert("builtInEnabled".to_string(), Value::Bool(enabled));
    map.insert(
        "maxConcurrent".to_string(),
        json!(max_concurrent.clamp(1, 8)),
    );
    if let Some(disabled) = disabled_built_ins {
        map.insert("disabledBuiltIns".to_string(), json!(disabled));
    } else if !map.contains_key("disabledBuiltIns") {
        map.insert("disabledBuiltIns".to_string(), json!([]));
    }
    write_json_pretty_atomic(&path, &Value::Object(map))?;
    Ok(read_subagent_settings_local())
}

pub fn list_subagent_profiles_local(cwd: Option<&str>) -> Vec<SubagentProfileRow> {
    let settings = read_subagent_settings_local();
    let mut items = vec![
        SubagentProfileRow {
            name: "explore".to_string(),
            display_name: Some("Explore".to_string()),
            description: "Fast read-only codebase exploration and search specialist".to_string(),
            scope: "builtin".to_string(),
            built_in: true,
            enabled: !settings.disabled_built_ins.iter().any(|n| n == "explore"),
            model: None,
        },
        SubagentProfileRow {
            name: "plan".to_string(),
            display_name: Some("Plan".to_string()),
            description: "Architecture and implementation planning specialist".to_string(),
            scope: "builtin".to_string(),
            built_in: true,
            enabled: !settings.disabled_built_ins.iter().any(|n| n == "plan"),
            model: None,
        },
        SubagentProfileRow {
            name: "subagent".to_string(),
            display_name: Some("Subagent".to_string()),
            description: "General-purpose autonomous coding subagent".to_string(),
            scope: "builtin".to_string(),
            built_in: true,
            enabled: !settings.disabled_built_ins.iter().any(|n| n == "subagent"),
            model: None,
        },
    ];

    let global_dir = pi_agent_dir().join("agents");
    items.extend(scan_agent_profile_dir(&global_dir, "user"));

    if let Some(c) = cwd.filter(|s| !s.is_empty()) {
        let proj_dir = Path::new(c).join(".pi").join("agents");
        items.extend(scan_agent_profile_dir(&proj_dir, "project"));
    }

    items
}

fn scan_agent_profile_dir(dir: &Path, scope: &str) -> Vec<SubagentProfileRow> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(item) = parse_subagent_profile_md(&path, scope) {
            out.push(item);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn parse_subagent_profile_md(path: &Path, scope: &str) -> Option<SubagentProfileRow> {
    let raw = fs::read_to_string(path).ok()?;
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let (fm, _body) = split_frontmatter(&raw);

    let name = fm
        .get("name")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or(stem);
    let description = fm.get("description").cloned().unwrap_or_default();
    let enabled = fm
        .get("enabled")
        .map(|v| v != "false")
        .unwrap_or(true);
    let model = fm.get("model").cloned().filter(|s| !s.is_empty());

    Some(SubagentProfileRow {
        name: name.clone(),
        display_name: Some(name),
        description,
        scope: scope.to_string(),
        built_in: false,
        enabled,
        model,
    })
}

fn split_frontmatter(raw: &str) -> ( std::collections::HashMap<String, String>, String ) {
    let mut map = std::collections::HashMap::new();
    let trimmed = raw.trim_start_matches('\u{feff}');
    if let Some(after_open) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n")) {
        if let Some(end_idx) = after_open.find("\n---") {
            let fm_block = &after_open[..end_idx];
            let rest = &after_open[end_idx + 4..];
            let body = rest.strip_prefix('\r').unwrap_or(rest);
            let body = body.strip_prefix('\n').unwrap_or(body);
            for line in fm_block.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim().trim_matches(['"', '\'']).to_string();
                    let val = v.trim().trim_matches(['"', '\'']).to_string();
                    map.insert(key, val);
                }
            }
            return (map, body.to_string());
        }
    }
    (map, raw.to_string())
}

pub fn save_subagent_profile_local(cwd: Option<&str>, body: &Value) -> Result<(), String> {
    let scope = body
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let name = body
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Profile name is required".to_string())?;

    // Built-in profiles are toggled via `disabledBuiltIns` in `agents/settings.json`,
    // never copied out to a `.md` file (AGENTS.md hard rule).
    if scope == "builtin" || matches!(name, "explore" | "plan" | "subagent") && body.get("systemPrompt").is_none() {
        let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let mut cur = read_subagent_settings_local();
        cur.disabled_built_ins.retain(|n| n != name);
        if !enabled {
            cur.disabled_built_ins.push(name.to_string());
        }
        save_subagent_settings_local(cur.enabled, cur.max_concurrent, Some(cur.disabled_built_ins))?;
        return Ok(());
    }

    let dir = if scope == "project" {
        let c = cwd
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Project cwd required for project-scoped profile".to_string())?;
        Path::new(c).join(".pi").join("agents")
    } else {
        pi_agent_dir().join("agents")
    };

    let safe_name: String = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' })
        .collect();
    let file_path = dir.join(format!("{safe_name}.md"));

    let desc = body.get("description").and_then(Value::as_str).unwrap_or("");
    let enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let model = body.get("model").and_then(Value::as_str).unwrap_or("");
    let thinking = body.get("thinkingLevel").and_then(Value::as_str).unwrap_or("");
    let preset = body.get("toolPreset").and_then(Value::as_str).unwrap_or("default");
    let max_turns = body.get("maxTurns").and_then(Value::as_u64).unwrap_or(15);
    let prompt = body.get("systemPrompt").and_then(Value::as_str).unwrap_or("");

    let mut md = String::from("---\n");
    md.push_str(&format!("name: {name}\n"));
    if !desc.is_empty() {
        md.push_str(&format!("description: {desc}\n"));
    }
    md.push_str(&format!("enabled: {enabled}\n"));
    if !model.is_empty() {
        md.push_str(&format!("model: {model}\n"));
    }
    if !thinking.is_empty() {
        md.push_str(&format!("thinkingLevel: {thinking}\n"));
    }
    if !preset.is_empty() {
        md.push_str(&format!("toolPreset: {preset}\n"));
    }
    md.push_str(&format!("maxTurns: {max_turns}\n"));
    md.push_str("---\n\n");
    md.push_str(prompt.trim());
    md.push('\n');

    write_file_atomic(&file_path, md.as_bytes())
}

pub fn delete_subagent_profile_local(cwd: Option<&str>, scope: &str, name: &str) -> Result<(), String> {
    let dir = if scope == "project" {
        let c = cwd.filter(|s| !s.is_empty()).ok_or_else(|| "Project cwd required".to_string())?;
        Path::new(c).join(".pi").join("agents")
    } else {
        pi_agent_dir().join("agents")
    };
    let candidate = dir.join(format!("{name}.md"));
    if candidate.exists() {
        fs::remove_file(&candidate).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================================
// 6. Skills Frontmatter Toggle (`disable-model-invocation` in `SKILL.md`)
// ============================================================================

/// Surgical frontmatter edit of `disable-model-invocation` on a `SKILL.md` file,
/// matching `lib/skill-frontmatter.ts`.
pub fn toggle_skill_file_local(file_path: &Path, disable: bool) -> Result<(), String> {
    if file_path.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("Not a Markdown skill file".to_string());
    }
    let content = fs::read_to_string(file_path).map_err(|e| e.to_string())?;
    let updated = set_disable_model_invocation_frontmatter(&content, disable);
    if updated != content {
        write_file_atomic(file_path, updated.as_bytes())?;
    }
    Ok(())
}

pub fn set_disable_model_invocation_frontmatter(content: &str, disable: bool) -> String {
    let bom = if content.starts_with('\u{feff}') { "\u{feff}" } else { "" };
    let body = if !bom.is_empty() { &content[bom.len()..] } else { content };

    let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let fence_open = format!("---{newline}");
    if let Some(after_open) = body.strip_prefix(&fence_open) {
        let close_pat = format!("{newline}---");
        if let Some(close_pos) = after_open.find(&close_pat) {
            let fm_head = &after_open[..close_pos];
            let tail = &after_open[close_pos..];
            let mut lines: Vec<String> = fm_head.split(newline).map(String::from).collect();
            let key_idx = lines.iter().position(|l| {
                let t = l.trim_start();
                t.starts_with("disable-model-invocation:")
                    || t.starts_with("\"disable-model-invocation\":")
                    || t.starts_with("'disable-model-invocation':")
            });

            if disable {
                if let Some(ix) = key_idx {
                    lines[ix] = "disable-model-invocation: true".to_string();
                } else {
                    lines.insert(0, "disable-model-invocation: true".to_string());
                }
            } else if let Some(ix) = key_idx {
                lines.remove(ix);
            } else {
                return content.to_string();
            }

            let new_head = lines.join(newline);
            return format!("{bom}{fence_open}{new_head}{tail}");
        }
    }

    if disable {
        format!("{bom}---{newline}disable-model-invocation: true{newline}---{newline}{body}")
    } else {
        content.to_string()
    }
}

// ============================================================================
// 7. MCP Config (`~/.pi/agent/mcp.json` & `<cwd>/.pi/mcp.json`) & Project Trust
// ============================================================================

pub fn global_mcp_config_path() -> PathBuf {
    pi_agent_dir().join("mcp.json")
}

pub fn project_mcp_config_path(cwd: &str) -> PathBuf {
    Path::new(cwd).join(".pi").join("mcp.json")
}

pub fn read_mcp_servers_local(cwd: Option<&str>) -> Vec<McpServerRow> {
    let mut entries = Vec::new();
    entries.extend(parse_mcp_file(&global_mcp_config_path(), "global", true));
    if let Some(c) = cwd.filter(|s| !s.is_empty()) {
        let trusted = is_project_trusted_local(c);
        entries.extend(parse_mcp_file(&project_mcp_config_path(c), "project", trusted));
    }
    entries
}

fn parse_mcp_file(path: &Path, scope: &str, trusted: bool) -> Vec<McpServerRow> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(val) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let Some(servers) = val.get("mcpServers").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, cfg) in servers {
        let enabled = cfg.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let url = cfg.get("url").and_then(Value::as_str).map(String::from);
        let transport = if url.is_some() {
            "http".to_string()
        } else {
            "stdio".to_string()
        };
        out.push(McpServerRow {
            name: name.clone(),
            scope: scope.to_string(),
            transport,
            status: "configured".to_string(),
            enabled,
            tools_count: 0,
            trusted,
            needs_auth: false,
            error: None,
        });
    }
    out
}

pub fn mutate_mcp_config_local(cwd: Option<&str>, body: &Value) -> Result<(), String> {
    let scope = body.get("scope").and_then(Value::as_str).unwrap_or("global");
    let path = if scope == "project" {
        let c = cwd
            .or_else(|| body.get("cwd").and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "Project cwd required for project MCP config".to_string())?;
        project_mcp_config_path(c)
    } else {
        global_mcp_config_path()
    };

    let mut root = read_json_object_or_empty(&path);
    let mut servers = root
        .get("mcpServers")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let action = body.get("action").and_then(Value::as_str).unwrap_or("update");
    let name = body
        .get("name")
        .or_else(|| body.get("serverName"))
        .and_then(Value::as_str)
        .unwrap_or("");

    match action {
        "remove" | "delete" => {
            servers.remove(name);
        }
        "add" => {
            if let Some(cfg) = body.get("config").or_else(|| body.get("server")) {
                servers.insert(name.to_string(), cfg.clone());
            }
        }
        "update" | _ => {
            let mut entry = servers
                .get(name)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(patch) = body.get("patch").and_then(Value::as_object) {
                for (k, v) in patch {
                    if k == "enabled" && v.as_bool() == Some(true) && scope == "global" {
                        entry.remove("enabled");
                    } else if k == "exposure" && v.as_str() == Some("codemode") && scope == "global" {
                        entry.remove("exposure");
                    } else {
                        entry.insert(k.clone(), v.clone());
                    }
                }
            }
            if let Some(en) = body.get("enabled").and_then(Value::as_bool) {
                if en && scope == "global" {
                    entry.remove("enabled");
                } else {
                    entry.insert("enabled".to_string(), Value::Bool(en));
                }
            }
            if let Some(exp) = body.get("exposure").and_then(Value::as_str) {
                if exp == "codemode" && scope == "global" {
                    entry.remove("exposure");
                } else {
                    entry.insert("exposure".to_string(), Value::String(exp.to_string()));
                }
            }
            servers.insert(name.to_string(), Value::Object(entry));
        }
    }

    root.insert("mcpServers".to_string(), Value::Object(servers));
    write_json_pretty_atomic(&path, &Value::Object(root))
}

/// Project trust store (`~/.pi/agent/trust.json`).
pub fn trust_store_path() -> PathBuf {
    pi_agent_dir().join("trust.json")
}

pub fn is_project_trusted_local(cwd: &str) -> bool {
    let map = read_json_object_or_empty(&trust_store_path());
    let resolved = fs::canonicalize(cwd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| cwd.to_string());
    if let Some(entries) = map.get("projects").and_then(Value::as_object) {
        if let Some(v) = entries.get(&resolved).or_else(|| entries.get(cwd)) {
            if let Some(b) = v.as_bool() {
                return b;
            }
            if let Some(b) = v.get("trusted").or_else(|| v.get("decision")).and_then(Value::as_bool) {
                return b;
            }
        }
    }
    if let Some(v) = map.get(&resolved).or_else(|| map.get(cwd)) {
        if let Some(b) = v.as_bool() {
            return b;
        }
    }
    true
}

pub fn set_project_trust_local(cwd: &str, trusted: bool) -> Result<(), String> {
    let path = trust_store_path();
    let mut map = read_json_object_or_empty(&path);
    let resolved = fs::canonicalize(cwd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| cwd.to_string());
    let mut projects = map
        .get("projects")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    projects.insert(resolved.clone(), Value::Bool(trusted));
    map.insert("projects".to_string(), Value::Object(projects));
    map.insert(resolved, Value::Bool(trusted));
    write_json_pretty_atomic(&path, &Value::Object(map))
}

// ============================================================================
// 8. Native Git Worktrees & Branches (`git worktree` / `git branch` CLI)
// ============================================================================

#[derive(Debug, Clone)]
pub struct LocalWorktreeSnapshot {
    pub is_git: bool,
    pub current_worktree_path: Option<String>,
    pub worktrees: Vec<WorktreeInfo>,
    pub branches: Vec<String>,
    pub remote_branches: Vec<String>,
}

pub fn inspect_git_worktrees_local(cwd: &str) -> LocalWorktreeSnapshot {
    let dir = Path::new(cwd);
    if !dir.exists() {
        return LocalWorktreeSnapshot {
            is_git: false,
            current_worktree_path: None,
            worktrees: Vec::new(),
            branches: Vec::new(),
            remote_branches: Vec::new(),
        };
    }

    let wt_out = crate::client::hidden_command("git")
        .args(["-C", cwd, "worktree", "list", "--porcelain"])
        .output();
    let Ok(output) = wt_out else {
        return LocalWorktreeSnapshot {
            is_git: false,
            current_worktree_path: None,
            worktrees: Vec::new(),
            branches: Vec::new(),
            remote_branches: Vec::new(),
        };
    };
    if !output.status.success() {
        return LocalWorktreeSnapshot {
            is_git: false,
            current_worktree_path: None,
            worktrees: Vec::new(),
            branches: Vec::new(),
            remote_branches: Vec::new(),
        };
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_head = String::new();
    let mut cur_branch: Option<String> = None;
    let mut is_bare = false;
    let mut is_detached = false;

    for line in text.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(p) = cur_path.take() {
                let is_main = worktrees.is_empty();
                let _ = (&cur_head, is_bare, is_detached);
                worktrees.push(WorktreeInfo {
                    path: p,
                    branch: cur_branch.take(),
                    is_main,
                    dirty: false,
                });
            }
            is_bare = false;
            is_detached = false;
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            cur_path = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("HEAD ") {
            cur_head = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("branch ") {
            let b = rest.trim().strip_prefix("refs/heads/").unwrap_or(rest.trim());
            cur_branch = Some(b.to_string());
        } else if line == "bare" {
            is_bare = true;
        } else if line == "detached" {
            is_detached = true;
        }
    }

    let canonical_cwd = fs::canonicalize(dir)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| cwd.to_string());
    let current_worktree_path = worktrees
        .iter()
        .find(|w| canonical_cwd.starts_with(&w.path) || cwd.starts_with(&w.path))
        .map(|w| w.path.clone())
        .or_else(|| worktrees.first().map(|w| w.path.clone()));

    let branches = crate::client::hidden_command("git")
        .args(["-C", cwd, "for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let remote_branches = crate::client::hidden_command("git")
        .args(["-C", cwd, "for-each-ref", "--format=%(refname:short)", "refs/remotes/origin/"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter_map(|s| s.strip_prefix("origin/"))
                .filter(|s| !s.is_empty() && *s != "HEAD" && !branches.iter().any(|b| b == *s))
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    LocalWorktreeSnapshot {
        is_git: true,
        current_worktree_path,
        worktrees,
        branches,
        remote_branches,
    }
}

pub fn switch_git_branch_local(cwd: &str, branch: &str) -> Result<String, String> {
    let out = crate::client::hidden_command("git")
        .args(["-C", cwd, "checkout", branch])
        .output()
        .map_err(|e| format!("git checkout failed: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git checkout {branch} exited with {}", out.status)
        } else {
            stderr
        });
    }
    Ok(branch.to_string())
}

pub fn create_git_worktree_local(cwd: &str, branch: &str) -> Result<(String, String), String> {
    let base = Path::new(cwd);
    let parent = base.parent().unwrap_or(base);
    let repo_name = base
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("worktree");
    let safe_branch: String = branch
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let target_dir = parent.join(format!("{repo_name}-{safe_branch}"));
    let target_str = target_dir.to_string_lossy().into_owned();

    // Check if branch exists locally
    let branch_exists = crate::client::hidden_command("git")
        .args(["-C", cwd, "show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let out = if branch_exists {
        crate::client::hidden_command("git")
            .args(["-C", cwd, "worktree", "add", &target_str, branch])
            .output()
    } else {
        crate::client::hidden_command("git")
            .args(["-C", cwd, "worktree", "add", "-b", branch, &target_str])
            .output()
    }
    .map_err(|e| format!("git worktree add failed: {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok((target_str, branch.to_string()))
}

pub fn remove_git_worktree_local(cwd: &str, worktree_path: &str, force: bool) -> Result<(), String> {
    let mut cmd = crate::client::hidden_command("git");
    cmd.args(["-C", cwd, "worktree", "remove"]);
    if force {
        cmd.arg("--force");
    }
    cmd.arg(worktree_path);
    let out = cmd.output().map_err(|e| format!("git worktree remove failed: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surgical_skill_frontmatter_toggles_disable_model_invocation() {
        let initial = "---\nname: demo-skill\ndescription: A test skill\n---\n# Demo\nBody text\n";
        let disabled = set_disable_model_invocation_frontmatter(initial, true);
        assert!(disabled.contains("disable-model-invocation: true"));
        assert!(disabled.contains("name: demo-skill"));

        let reenabled = set_disable_model_invocation_frontmatter(&disabled, false);
        assert!(!reenabled.contains("disable-model-invocation"));
        assert_eq!(reenabled, initial);
    }

    #[test]
    fn atomic_file_write_and_json_roundtrip() {
        let tmp_dir = std::env::temp_dir().join(format!("pi-gpui-store-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let file_path = tmp_dir.join("test.json");
        let payload = json!({ "version": 1, "ok": true });
        write_json_pretty_atomic(&file_path, &payload).unwrap();

        let read_back = read_json_object_or_empty(&file_path);
        assert_eq!(read_back.get("ok").and_then(Value::as_bool), Some(true));

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
