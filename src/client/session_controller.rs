use crate::client::types::*;
use chrono::{DateTime, Local};
use gpui_kit::{SharedString, Task};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn current_local_hhmm() -> SharedString {
    SharedString::from(Local::now().format("%H:%M").to_string())
}

pub fn format_compact_relative_time(raw: Option<&str>) -> SharedString {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return SharedString::default();
    };
    let ts_ms = if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        dt.timestamp_millis()
    } else if let Ok(num) = s.parse::<i64>() {
        if num > 10_000_000_000 { num } else { num * 1000 }
    } else {
        return SharedString::from(s.to_string());
    };

    let now_ms = Local::now().timestamp_millis();
    let age_ms = (now_ms - ts_ms).max(0);
    let minutes = age_ms / 60_000;
    if minutes < 1 {
        return SharedString::from("now");
    }
    if minutes < 60 {
        return SharedString::from(format!("{minutes}m"));
    }
    let hours = age_ms / 3_600_000;
    if hours < 24 {
        return SharedString::from(format!("{hours}h"));
    }
    let days = age_ms / 86_400_000;
    if days < 7 {
        return SharedString::from(format!("{days}d"));
    }
    if days < 30 {
        return SharedString::from(format!("{}w", days / 7));
    }
    if days < 365 {
        return SharedString::from(format!("{}mo", (days / 30).max(1)));
    }
    SharedString::from(format!("{}y", days / 365))
}

fn parse_timestamp_millis(val: Option<&Value>) -> Option<i64> {
    let v = val?;
    if let Some(ms) = v.as_i64() {
        return Some(if ms > 10_000_000_000 { ms } else { ms * 1000 });
    }
    if let Some(f) = v.as_f64() {
        let ms = f as i64;
        return Some(if ms > 10_000_000_000 { ms } else { ms * 1000 });
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                return Some(dt.timestamp_millis());
            }
        }
    }
    None
}

pub fn format_timestamp_value(val: Option<&Value>) -> String {
    let Some(v) = val else {
        return String::new();
    };
    if let Some(ms) = v.as_i64() {
        let millis = if ms > 10_000_000_000 { ms } else { ms * 1000 };
        if let Some(dt) = DateTime::from_timestamp_millis(millis) {
            return dt.with_timezone(&Local).format("%H:%M").to_string();
        }
    }
    if let Some(f) = v.as_f64() {
        let ms = f as i64;
        let millis = if ms > 10_000_000_000 { ms } else { ms * 1000 };
        if let Some(dt) = DateTime::from_timestamp_millis(millis) {
            return dt.with_timezone(&Local).format("%H:%M").to_string();
        }
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
            return dt.with_timezone(&Local).format("%H:%M").to_string();
        }
        return trimmed.to_string();
    }
    String::new()
}

pub struct SessionController {
    pub load_request_id: u64,
    pub load_task: Option<Task<()>>,
    pub sse_task: Option<Task<()>>,
    pub send_in_flight: bool,
    pub next_local_msg_id: u64,
    pub next_queue_id: u64,
}

impl Default for SessionController {
    fn default() -> Self {
        Self {
            load_request_id: 0,
            load_task: None,
            sse_task: None,
            send_in_flight: false,
            next_local_msg_id: 1,
            next_queue_id: 1,
        }
    }
}

impl SessionController {
    pub fn next_load_id(&mut self) -> u64 {
        self.load_request_id += 1;
        self.load_task = None;
        self.load_request_id
    }

    pub fn alloc_msg_id(&mut self) -> u64 {
        let id = self.next_local_msg_id;
        self.next_local_msg_id += 1;
        id
    }

    pub fn alloc_queue_id(&mut self) -> u64 {
        let id = self.next_queue_id;
        self.next_queue_id += 1;
        id
    }

    pub fn stop_sse(&mut self) {
        self.sse_task = None;
    }
}

/// Full `.jsonl` tree index for 0ms local branch navigation and sibling resolution (Phase 4.1).
#[derive(Debug, Clone, Default)]
pub struct SessionTreeIndex {
    pub entries: Vec<Value>,
    pub by_id: std::collections::HashMap<String, usize>,
    pub children_by_parent: std::collections::HashMap<Option<String>, Vec<String>>,
    pub latest_leaf_id: Option<String>,
}

impl SessionTreeIndex {
    pub fn from_jsonl_file(path: &Path) -> Self {
        let Ok(file) = File::open(path) else {
            return Self::default();
        };
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                entries.push(val);
            }
        }
        Self::from_entries(entries)
    }

    pub fn from_entries(entries: Vec<Value>) -> Self {
        let mut by_id = std::collections::HashMap::new();
        let mut children_by_parent: std::collections::HashMap<Option<String>, Vec<String>> =
            std::collections::HashMap::new();
        let mut latest_leaf_id = None;

        for (idx, val) in entries.iter().enumerate() {
            let entry_type = val.get("type").and_then(Value::as_str).unwrap_or("");
            if entry_type == "session" || entry_type == "session_info" {
                continue;
            }
            if let Some(eid) = val.get("id").and_then(Value::as_str) {
                let id_str = eid.to_string();
                by_id.insert(id_str.clone(), idx);
                let parent_id = val
                    .get("parentId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(id_str.clone());
                latest_leaf_id = Some(id_str);
            }
        }

        Self {
            entries,
            by_id,
            children_by_parent,
            latest_leaf_id,
        }
    }

    /// Walks down from `start_entry_id` taking the latest child at each fork to reach a leaf.
    pub fn resolve_leaf_from(&self, start_entry_id: &str) -> String {
        let mut cur = start_entry_id.to_string();
        let mut visited = std::collections::HashSet::new();
        while visited.insert(cur.clone()) {
            let Some(children) = self.children_by_parent.get(&Some(cur.clone())) else {
                break;
            };
            let Some(last_child) = children.last() else {
                break;
            };
            cur = last_child.clone();
        }
        cur
    }

    /// Returns the ordered entry index chain from root to `leaf_id`, plus any `BranchSibling`s
    /// at the most recent fork point along that active branch.
    pub fn build_branch_chain_and_siblings(
        &self,
        target_leaf_id: Option<&str>,
    ) -> (Vec<usize>, Vec<BranchSibling>) {
        let leaf_id = target_leaf_id
            .map(String::from)
            .or_else(|| self.latest_leaf_id.clone());
        let Some(leaf_id) = leaf_id else {
            return ((0..self.entries.len()).collect(), Vec::new());
        };
        let Some(&start_idx) = self.by_id.get(&leaf_id) else {
            return ((0..self.entries.len()).collect(), Vec::new());
        };

        let mut chain = Vec::new();
        let mut cur_idx = start_idx;
        let mut seen = std::collections::HashSet::new();
        while seen.insert(cur_idx) {
            chain.push(cur_idx);
            let parent_id = self.entries[cur_idx]
                .get("parentId")
                .and_then(Value::as_str);
            if let Some(pid) = parent_id {
                if let Some(&p_idx) = self.by_id.get(pid) {
                    cur_idx = p_idx;
                    continue;
                }
            }
            break;
        }
        chain.reverse();

        // Find the deepest fork point along this chain where `children_by_parent` has > 1 child
        let mut siblings = Vec::new();
        for &idx in chain.iter().rev() {
            let val = &self.entries[idx];
            let Some(eid) = val.get("id").and_then(Value::as_str) else {
                continue;
            };
            let parent_key = val
                .get("parentId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(String::from);
            if let Some(child_ids) = self.children_by_parent.get(&parent_key) {
                if child_ids.len() > 1 {
                    siblings = child_ids
                        .iter()
                        .enumerate()
                        .map(|(i, cid)| {
                            let summary = self.entry_preview_label(cid, i + 1);
                            BranchSibling {
                                entry_id: cid.clone(),
                                label: summary,
                                active: cid == eid,
                            }
                        })
                        .collect();
                    break;
                }
            }
        }

        (chain, siblings)
    }

    fn entry_preview_label(&self, entry_id: &str, branch_num: usize) -> String {
        let Some(&idx) = self.by_id.get(entry_id) else {
            return format!("Branch {branch_num}");
        };
        let val = &self.entries[idx];
        let ts = format_timestamp_value(val.get("timestamp"));
        let snippet = val
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| {
                if let Some(s) = c.as_str() {
                    Some(s.to_string())
                } else if let Some(arr) = c.as_array() {
                    arr.iter().find_map(|part| {
                        part.get("text")
                            .and_then(Value::as_str)
                            .map(ToString::to_string)
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let clean: String = snippet
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .chars()
            .take(28)
            .collect();
        match (clean.is_empty(), ts.is_empty()) {
            (false, false) => format!("#{branch_num} · {clean} ({ts})"),
            (false, true) => format!("#{branch_num} · {clean}"),
            (true, false) => format!("#{branch_num} ({ts})"),
            (true, true) => format!("Branch {branch_num}"),
        }
    }

    pub fn extract_messages_for_leaf(
        &self,
        target_leaf_id: Option<&str>,
        next_id: &mut u64,
    ) -> (Vec<ChatMessage>, Vec<BranchSibling>) {
        if self.entries.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let (ordered_indices, siblings) = if let Some(tid) = target_leaf_id {
            let resolved_leaf = self.resolve_leaf_from(tid);
            self.build_branch_chain_and_siblings(Some(&resolved_leaf))
        } else {
            // Show all messages across branches concatenated in chronological order
            // so earlier conversation history is never hidden behind a branch switcher.
            let mut last_ts: i64 = 0;
            let mut indexed: Vec<(i64, usize)> = self
                .entries
                .iter()
                .enumerate()
                .map(|(idx, val)| {
                    let ts = parse_timestamp_millis(
                        val.get("timestamp")
                            .or_else(|| val.get("message").and_then(|m| m.get("timestamp"))),
                    );
                    if let Some(t) = ts {
                        last_ts = t;
                        (t, idx)
                    } else {
                        (last_ts, idx)
                    }
                })
                .collect();
            indexed.sort_by_key(|&(ts, idx)| (ts, idx));
            (indexed.into_iter().map(|(_, idx)| idx).collect(), Vec::new())
        };

        let mut messages = Vec::new();
        let mut entry_ids = Vec::new();
        let mut current_model: Option<String> = None;

        for idx in ordered_indices {
            let val = &self.entries[idx];
            let entry_type = val.get("type").and_then(Value::as_str).unwrap_or("");
            if entry_type == "model_change" {
                if let Some(mid) = val.get("modelId").and_then(Value::as_str) {
                    current_model = Some(mid.to_string());
                }
                continue;
            }
            if entry_type == "compaction" {
                let summary = val
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let tokens_before = val.get("tokensBefore").and_then(Value::as_u64);
                let ts = val.get("timestamp").cloned().unwrap_or(Value::Null);
                let eid = val
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                entry_ids.push(Value::String(eid));
                messages.push(serde_json::json!({
                    "role": "custom",
                    "customType": "compaction",
                    "content": summary,
                    "details": { "tokensBefore": tokens_before },
                    "timestamp": ts,
                }));
                continue;
            }
            if entry_type == "branch_summary" {
                let summary = val
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if !summary.is_empty() {
                    let ts = val.get("timestamp").cloned().unwrap_or(Value::Null);
                    let eid = val
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    entry_ids.push(Value::String(eid));
                    messages.push(serde_json::json!({
                        "role": "custom",
                        "customType": "branch_summary",
                        "content": summary,
                        "timestamp": ts,
                    }));
                }
                continue;
            }
            if entry_type == "message" {
                if let Some(mut msg) = val.get("message").cloned() {
                    if let Some(obj) = msg.as_object_mut() {
                        if !obj.contains_key("timestamp") {
                            if let Some(ts) = val.get("timestamp").cloned() {
                                obj.insert("timestamp".to_string(), ts);
                            }
                        }
                        if !obj.contains_key("model") {
                            if let Some(ref m) = current_model {
                                obj.insert("model".to_string(), Value::String(m.clone()));
                            }
                        }
                    }
                    let eid = val
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    entry_ids.push(Value::String(eid));
                    messages.push(msg);
                }
            }
        }

        let payload = serde_json::json!({
            "entryIds": entry_ids,
            "messages": messages,
        });
        (parse_session_messages(&payload, next_id), siblings)
    }
}

/// Reads a `.jsonl` session file directly on disk, walking the active `parentId` -> `leafId`
/// branch chain so switched branches are coherent and model changes stay accurate.
pub fn parse_session_jsonl_file(path: &Path, next_id: &mut u64) -> Vec<ChatMessage> {
    let index = SessionTreeIndex::from_jsonl_file(path);
    index.extract_messages_for_leaf(None, next_id).0
}

pub fn parse_session_jsonl_with_siblings(
    path: &Path,
    target_leaf_id: Option<&str>,
    next_id: &mut u64,
) -> (Vec<ChatMessage>, Vec<BranchSibling>) {
    let index = SessionTreeIndex::from_jsonl_file(path);
    index.extract_messages_for_leaf(target_leaf_id, next_id)
}

/// Computes cumulative session stats + current post-compaction context usage from a `.jsonl` file.
pub fn compute_stats_from_jsonl(path: &Path, context_window: u64) -> SessionStats {
    let Ok(file) = File::open(path) else {
        return SessionStats::default();
    };
    let reader = BufReader::new(file);
    let mut entries: Vec<Value> = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
            entries.push(val);
        }
    }
    if entries.is_empty() {
        return SessionStats::default();
    }

    let mut by_id: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut last_node_idx: Option<usize> = None;
    for (idx, val) in entries.iter().enumerate() {
        if let Some(eid) = val.get("id").and_then(Value::as_str) {
            by_id.insert(eid.to_string(), idx);
            last_node_idx = Some(idx);
        }
    }
    let ordered_indices: Vec<usize> = if let Some(mut cur_idx) = last_node_idx {
        let mut chain = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while seen.insert(cur_idx) {
            chain.push(cur_idx);
            if let Some(pid) = entries[cur_idx].get("parentId").and_then(Value::as_str) {
                if let Some(&p_idx) = by_id.get(pid) {
                    cur_idx = p_idx;
                    continue;
                }
            }
            break;
        }
        chain.reverse();
        chain
    } else {
        (0..entries.len()).collect()
    };

    let mut stats = SessionStats::default();
    let mut last_ctx_tokens: Option<u64> = None;

    for idx in ordered_indices {
        let val = &entries[idx];
        let etype = val.get("type").and_then(Value::as_str).unwrap_or("");
        if etype == "compaction" {
            last_ctx_tokens = None;
            continue;
        }
        if etype == "message" {
            if let Some(msg) = val.get("message") {
                if let Some(usage) = msg.get("usage") {
                    let inp = usage.get("input").and_then(Value::as_u64).unwrap_or(0);
                    let out = usage.get("output").and_then(Value::as_u64).unwrap_or(0);
                    let cr = usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                    let cw = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                    let cost = usage
                        .get("cost")
                        .and_then(|c| c.get("total"))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    stats.input_tokens += inp;
                    stats.output_tokens += out;
                    stats.cache_read_tokens += cr;
                    stats.cache_write_tokens += cw;
                    stats.cost_usd += cost;
                    if msg.get("role").and_then(Value::as_str) == Some("assistant") {
                        let turn_ctx = inp + out + cr + cw;
                        if turn_ctx > 0 {
                            last_ctx_tokens = Some(turn_ctx);
                        }
                    }
                }
            }
        }
    }

    if let Some(ctx_tok) = last_ctx_tokens {
        let win = context_window.max(1);
        stats.context_tokens = Some(ctx_tok);
        stats.context_window = Some(win);
        stats.context_used_pct = ((ctx_tok as f64 / win as f64) * 100.0).clamp(0.0, 100.0) as f32;
        stats.has_context_usage = true;
    }
    stats
}

/// Computes current context tokens from a `/api/sessions/[id]` payload's active context messages.
pub fn compute_context_tokens_from_payload(payload: &Value) -> Option<u64> {
    let messages = payload
        .get("messages")
        .or_else(|| payload.get("context").and_then(|c| c.get("messages")))
        .and_then(Value::as_array)?;
    let mut last_ctx_tokens: Option<u64> = None;
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        let custom_type = msg.get("customType").and_then(Value::as_str).unwrap_or("");
        if role == "compactionSummary" || (role == "custom" && custom_type == "compaction") {
            last_ctx_tokens = None;
            continue;
        }
        if role == "assistant" {
            if let Some(usage) = msg.get("usage") {
                let inp = usage.get("input").and_then(Value::as_u64).unwrap_or(0);
                let out = usage.get("output").and_then(Value::as_u64).unwrap_or(0);
                let cr = usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
                let cw = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
                let total = inp + out + cr + cw;
                if total > 0 {
                    last_ctx_tokens = Some(total);
                }
            }
        }
    }
    last_ctx_tokens
}

pub fn parse_compaction_summary(summary: &str) -> (String, Vec<String>, Vec<String>) {
    let mut read_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut body = String::new();
    let mut cursor = 0usize;

    while cursor < summary.len() {
        let next_read = summary[cursor..].find("<read-files>").map(|i| (cursor + i, true));
        let next_mod = summary[cursor..].find("<modified-files>").map(|i| (cursor + i, false));
        let next = match (next_read, next_mod) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((start, is_read)) = next else {
            body.push_str(&summary[cursor..]);
            break;
        };
        body.push_str(&summary[cursor..start]);
        let (open_tag, close_tag) = if is_read {
            ("<read-files>", "</read-files>")
        } else {
            ("<modified-files>", "</modified-files>")
        };
        let content_start = start + open_tag.len();
        if let Some(rel_end) = summary[content_start..].find(close_tag) {
            let inner = &summary[content_start..content_start + rel_end];
            let target = if is_read {
                &mut read_files
            } else {
                &mut modified_files
            };
            for line in inner.lines() {
                let t = line.trim();
                if !t.is_empty() {
                    target.push(t.to_string());
                }
            }
            cursor = content_start + rel_end + close_tag.len();
        } else {
            body.push_str(&summary[start..]);
            break;
        }
    }

    (body.trim().to_string(), read_files, modified_files)
}

pub const IMPLICIT_CONTINUE_PROMPT: &str =
    "Continue directly from where your previous response was interrupted. Do not repeat anything you already output.";

/// Removes an invalid failed turn (the failed assistant error entry, its
/// immediately preceding user entry, and any orphan descendants) from a session
/// `.jsonl` file, returning the `parentId` to roll back to in the session tree.
pub fn prune_failed_turn_from_jsonl(
    path: &Path,
    user_entry_id: Option<&str>,
    failed_assistant_entry_id: Option<&str>,
) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut parsed_lines: Vec<(String, Option<Value>)> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let val = serde_json::from_str::<Value>(trimmed).ok();
        parsed_lines.push((trimmed.to_string(), val));
    }
    if parsed_lines.is_empty() {
        return None;
    }

    let mut target_user_id = user_entry_id.map(str::to_string);
    let mut target_assistant_id = failed_assistant_entry_id.map(str::to_string);

    // Fallback: if caller didn't have entry IDs, locate the trailing error assistant
    // entry and its parent user entry directly from the JSONL tail.
    if target_assistant_id.is_none() {
        if let Some(last_err) = parsed_lines.iter().rev().find_map(|(_, val)| {
            let v = val.as_ref()?;
            let msg = v.get("message")?;
            if v.get("type").and_then(Value::as_str) == Some("message")
                && msg.get("role").and_then(Value::as_str) == Some("assistant")
                && msg.get("stopReason").and_then(Value::as_str) == Some("error")
            {
                v.get("id").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        }) {
            target_assistant_id = Some(last_err);
        }
    }

    if target_user_id.is_none() {
        if let Some(ref aid) = target_assistant_id {
            if let Some(parent_id) = parsed_lines.iter().find_map(|(_, val)| {
                let v = val.as_ref()?;
                if v.get("id").and_then(Value::as_str) == Some(aid.as_str()) {
                    v.get("parentId").and_then(Value::as_str).map(str::to_string)
                } else {
                    None
                }
            }) {
                // Only treat parent as the failed user message if its role is "user".
                let parent_is_user = parsed_lines.iter().any(|(_, val)| {
                    val.as_ref().is_some_and(|v| {
                        v.get("id").and_then(Value::as_str) == Some(parent_id.as_str())
                            && v.get("message")
                                .and_then(|m| m.get("role"))
                                .and_then(Value::as_str)
                                == Some("user")
                    })
                });
                if parent_is_user {
                    target_user_id = Some(parent_id);
                }
            }
        }
    }

    let anchor_id = target_user_id.as_deref().or(target_assistant_id.as_deref());
    let rollback_parent_id = anchor_id.and_then(|aid| {
        parsed_lines.iter().find_map(|(_, val)| {
            let v = val.as_ref()?;
            if v.get("id").and_then(Value::as_str) == Some(aid) {
                v.get("parentId").and_then(Value::as_str).map(str::to_string)
            } else {
                None
            }
        })
    });

    let mut remove_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(uid) = target_user_id {
        remove_ids.insert(uid);
    }
    if let Some(aid) = target_assistant_id {
        remove_ids.insert(aid);
    }
    if remove_ids.is_empty() {
        return rollback_parent_id;
    }

    // Transitively collect any child entries (e.g. context_edit) hanging off removed nodes.
    loop {
        let mut added = false;
        for (_, val) in &parsed_lines {
            let Some(v) = val.as_ref() else { continue };
            let Some(id) = v.get("id").and_then(Value::as_str) else { continue };
            if remove_ids.contains(id) {
                continue;
            }
            if let Some(pid) = v.get("parentId").and_then(Value::as_str) {
                if remove_ids.contains(pid) {
                    remove_ids.insert(id.to_string());
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
    }

    let kept: Vec<&str> = parsed_lines
        .iter()
        .filter_map(|(line, val)| {
            if let Some(id) = val.as_ref().and_then(|v| v.get("id")).and_then(Value::as_str) {
                if remove_ids.contains(id) {
                    return None;
                }
            }
            Some(line.as_str())
        })
        .collect();

    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    let _ = crate::client::local_store::write_file_atomic(path, out.as_bytes());
    rollback_parent_id
}

/// Converts a `/api/sessions/[id]` JSON payload into UI-ready `ChatMessage` rows,
/// merging consecutive assistant steps of the same turn into a single turn with
/// `process_steps` (matching `ChatWindow.tsx`'s `ProcessDetailsGroup` + `getFinalSplit`).
pub fn parse_session_messages(payload: &Value, next_id: &mut u64) -> Vec<ChatMessage> {
    let mut out: Vec<ChatMessage> = Vec::new();
    let messages = payload
        .get("messages")
        .or_else(|| payload.get("context").and_then(|c| c.get("messages")))
        .and_then(Value::as_array);

    let Some(items) = messages else {
        return out;
    };

    let entry_ids = payload
        .get("entryIds")
        .or_else(|| payload.get("context").and_then(|c| c.get("entryIds")))
        .and_then(Value::as_array);

    let mut prev_was_implicit_continue = false;

    for (idx, msg) in items.iter().enumerate() {
        let role_str = msg.get("role").and_then(Value::as_str).unwrap_or("assistant");
        if role_str == "system" {
            continue;
        }
        if role_str == "toolResult" {
            let tool_call_id = msg.get("toolCallId").and_then(Value::as_str).unwrap_or("");
            let is_error = msg.get("isError").and_then(Value::as_bool).unwrap_or(false);
            let output_text = flatten_content_text(msg.get("content"));
            let details_val = msg.get("details");
            let subagent_sid = extract_subagent_session_id(details_val);
            'outer: for prev in out.iter_mut().rev() {
                for tc in prev.tool_calls.iter_mut().rev() {
                    if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                        tc.output = SharedString::from(output_text.clone());
                        tc.is_error = is_error;
                        tc.is_running = false;
                        if subagent_sid.is_some() {
                            tc.subagent_session_id = subagent_sid.clone();
                        }
                        if tc.diff_patch.is_none() && tc.name.as_ref() == "edit" {
                            tc.diff_patch = build_diff_from_tool("edit", None, details_val);
                        }
                        break;
                    }
                }
                for step in prev.process_steps.iter_mut().rev() {
                    if let ProcessStep::ToolCall(tc) = step {
                        if tc.id.as_ref() == tool_call_id || tool_call_id.is_empty() {
                            tc.output = SharedString::from(output_text.clone());
                            tc.is_error = is_error;
                            tc.is_running = false;
                            if subagent_sid.is_some() {
                                tc.subagent_session_id = subagent_sid.clone();
                            }
                            if tc.diff_patch.is_none() && tc.name.as_ref() == "edit" {
                                tc.diff_patch = build_diff_from_tool("edit", None, details_val);
                            }
                            break 'outer;
                        }
                    }
                }
            }
            continue;
        }

        let custom_type = msg
            .get("customType")
            .and_then(Value::as_str)
            .unwrap_or("");
        let is_compaction = role_str == "compactionSummary"
            || (role_str == "custom" && custom_type == "compaction");
        let is_branch_summary = role_str == "branchSummary"
            || (role_str == "custom" && custom_type == "branch_summary");

        if is_compaction {
            let raw_summary = msg
                .get("summary")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| flatten_content_text(msg.get("content")));
            let tokens_before = msg
                .get("tokensBefore")
                .and_then(Value::as_u64)
                .or_else(|| {
                    msg.get("details")
                        .and_then(|d| d.get("tokensBefore"))
                        .and_then(Value::as_u64)
                });
            let (summary_markdown, read_files, modified_files) =
                parse_compaction_summary(&raw_summary);
            let step = CompactionStep {
                summary_markdown,
                tokens_before,
                read_files,
                modified_files,
                is_running: false,
            };

            // Attach inside the current Assistant turn ONLY when compaction happened mid-request
            // (i.e. an assistant continuation or implicit continue follows this compaction),
            // so post-turn / manual compaction after a finished turn renders as its own standalone row.
            let next_continues_assistant_turn = items[idx + 1..]
                .iter()
                .find(|next| next.get("role").and_then(Value::as_str) != Some("system"))
                .is_some_and(|next| {
                    let next_role = next.get("role").and_then(Value::as_str).unwrap_or("");
                    let next_hidden = next.get("hidden").and_then(Value::as_bool) == Some(true)
                        || flatten_content_text(next.get("content")).trim()
                            == IMPLICIT_CONTINUE_PROMPT;
                    next_role == "assistant" || (next_role == "user" && next_hidden)
                });
            if next_continues_assistant_turn {
                if let Some(prev) = out.last_mut() {
                    if prev.role == MessageRole::Assistant {
                        if !prev.content.trim().is_empty() {
                            let intermediate = std::mem::take(&mut prev.content);
                            prev.process_steps
                                .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                        }
                        prev.process_steps.push(ProcessStep::Compaction(step));
                        continue;
                    }
                }
            }

            let entry_id = entry_ids
                .and_then(|arr| arr.get(idx))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(|s| SharedString::from(s.to_string()));
            let timestamp = format_timestamp_value(msg.get("timestamp"));
            let id = *next_id;
            *next_id += 1;
            out.push(ChatMessage {
                id,
                entry_id,
                role: MessageRole::System,
                model_label: None,
                timestamp: SharedString::from(timestamp),
                thinking: SharedString::default(),
                process_steps: vec![ProcessStep::Compaction(step)],
                process_message_count: 1,
                thinking_ms: None,
                thinking_expanded: false,
                content: raw_summary,
                tool_calls: Vec::new(),
                attachments: Vec::new(),
                is_streaming: false,
                turn_duration_ms: None,
                token_summary: None,
                turn_usage: None,
                stop_reason: None,
                error_message: None,
            });
            continue;
        }

        let role = match role_str {
            "user" => MessageRole::User,
            _ if is_branch_summary => MessageRole::System,
            "custom" | "notice" => MessageRole::Notice,
            _ => MessageRole::Assistant,
        };

        if role == MessageRole::User {
            let is_hidden_continue = msg.get("hidden").and_then(Value::as_bool) == Some(true)
                || flatten_content_text(msg.get("content")).trim() == IMPLICIT_CONTINUE_PROMPT;
            if is_hidden_continue {
                prev_was_implicit_continue = true;
                continue;
            }
            prev_was_implicit_continue = false;
        }

        let entry_id = entry_ids
            .and_then(|arr| arr.get(idx))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| SharedString::from(s.to_string()));

        let model_label = msg
            .get("model")
            .and_then(Value::as_str)
            .or_else(|| msg.get("modelId").and_then(Value::as_str))
            .map(|s| SharedString::from(s.to_string()));

        let timestamp = format_timestamp_value(msg.get("timestamp"));
        let stop_reason = msg
            .get("stopReason")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|s| SharedString::from(s.to_string()));
        let error_message = msg
            .get("errorMessage")
            .or_else(|| msg.get("error"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(|s| SharedString::from(s.to_string()));
        let msg_usage = if role == MessageRole::Assistant {
            extract_turn_usage(msg)
        } else {
            None
        };
        let (text, thinking, tool_calls, mut steps) = parse_message_content(msg.get("content"));
        let mut msg_attachments = extract_image_attachments_from_content(msg.get("content"));

        if role == MessageRole::Assistant {
            // Merge consecutive Assistant entries within the same turn into ONE unified Assistant row,
            // unless the model switched mid-turn.
            if let Some(prev) = out.last_mut() {
                let same_model = match (&prev.model_label, &model_label) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                };
                if prev.role == MessageRole::Assistant && same_model {
                    prev.attachments.append(&mut msg_attachments);
                    let prev_had_error = prev.error_message.is_some()
                        || prev.stop_reason.as_deref() == Some("error");
                    let keep_prev_prefix = prev_had_error || prev_was_implicit_continue;
                    prev_was_implicit_continue = false;
                    // If previous assistant step had intermediate text before tool calls (and wasn't continued/interrupted),
                    // move it to thinking/process; if continuing a turn, keep it as the answer prefix.
                    if !keep_prev_prefix
                        && !prev.content.trim().is_empty()
                        && (!text.trim().is_empty() || !tool_calls.is_empty())
                    {
                        let intermediate = std::mem::take(&mut prev.content);
                        prev.process_steps
                            .push(ProcessStep::Thinking(SharedString::from(intermediate)));
                    }
                    if !thinking.trim().is_empty() {
                        if !prev.thinking.is_empty() {
                            let mut combined = prev.thinking.to_string();
                            combined.push_str("\n\n");
                            combined.push_str(&thinking);
                            prev.thinking = SharedString::from(combined);
                        } else {
                            prev.thinking = SharedString::from(thinking);
                        }
                    }
                    prev.process_steps.append(&mut steps);
                    prev.tool_calls.extend(tool_calls);
                    prev.process_message_count += 1;
                    if let Some(u) = msg_usage {
                        merge_turn_usage(&mut prev.turn_usage, u);
                    }
                    // If a previous entry in this turn had a transient error and a new entry
                    // followed (e.g. retry or tool continuation), archive the earlier error
                    // into process_steps so it stays at its chronological position rather than
                    // persisting at the bottom of the finished turn.
                    if let Some(prev_err) = prev.error_message.take() {
                        prev.process_steps
                            .push(ProcessStep::Thinking(SharedString::from(format!("⚠️ {prev_err}"))));
                    }
                    if stop_reason.is_some() {
                        prev.stop_reason = stop_reason;
                    }
                    if error_message.is_some() {
                        prev.error_message = error_message;
                    }
                    if !text.trim().is_empty() {
                        if !prev.content.is_empty() {
                            prev.content.push_str("\n\n");
                        }
                        prev.content.push_str(&text);
                    }
                    if !timestamp.is_empty() {
                        prev.timestamp = SharedString::from(timestamp);
                    }
                    if prev.model_label.is_none() && model_label.is_some() {
                        prev.model_label = model_label;
                    }
                    if entry_id.is_some() {
                        prev.entry_id = entry_id;
                    }
                    continue;
                }
            }
        }

        let id = *next_id;
        *next_id += 1;

        out.push(ChatMessage {
            id,
            entry_id,
            role,
            model_label,
            timestamp: SharedString::from(timestamp),
            thinking: SharedString::from(thinking),
            process_steps: steps,
            process_message_count: 1,
            thinking_ms: None,
            thinking_expanded: false,
            content: text,
            tool_calls,
            attachments: msg_attachments,
            is_streaming: false,
            turn_duration_ms: None,
            token_summary: None,
            turn_usage: msg_usage,
            stop_reason,
            error_message,
        });
    }

    out
}

pub fn extract_turn_usage(msg: &Value) -> Option<TurnUsage> {
    let usage = msg.get("usage")?;
    let input_tokens = usage.get("input").and_then(Value::as_u64).unwrap_or(0);
    let output_tokens = usage.get("output").and_then(Value::as_u64).unwrap_or(0);
    let cache_read_tokens = usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0);
    let cache_write_tokens = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
    let total_tokens = usage
        .get("totalTokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens + cache_read_tokens + cache_write_tokens);
    let cost_usd = usage
        .get("cost")
        .and_then(|c| c.get("total"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if total_tokens == 0 && cost_usd == 0.0 {
        return None;
    }
    Some(TurnUsage {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_tokens,
        total_tokens,
        cost_usd,
    })
}

pub fn merge_turn_usage(target: &mut Option<TurnUsage>, add: TurnUsage) {
    match target {
        Some(existing) => {
            existing.input_tokens += add.input_tokens;
            existing.output_tokens += add.output_tokens;
            existing.cache_read_tokens += add.cache_read_tokens;
            existing.cache_write_tokens += add.cache_write_tokens;
            existing.total_tokens += add.total_tokens;
            existing.cost_usd += add.cost_usd;
        }
        None => *target = Some(add),
    }
}

pub fn image_mime_for_path(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())?;
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        _ => None,
    }
}

fn extension_for_image_mime(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        _ => "png",
    }
}

pub fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u8> {
        match b {
            b'A'..=b'Z' => Some(b - b'A'),
            b'a'..=b'z' => Some(b - b'a' + 26),
            b'0'..=b'9' => Some(b - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if clean.is_empty() {
        return Some(Vec::new());
    }
    let mut out = Vec::with_capacity((clean.len() / 4) * 3);
    for chunk in clean.chunks(4) {
        if chunk.len() < 2 {
            return None;
        }
        let v0 = val(chunk[0])? as u32;
        let v1 = val(chunk[1])? as u32;
        let p2 = chunk.get(2).copied().unwrap_or(b'=');
        let p3 = chunk.get(3).copied().unwrap_or(b'=');
        let v2 = if p2 == b'=' { 0 } else { val(p2)? as u32 };
        let v3 = if p3 == b'=' { 0 } else { val(p3)? as u32 };
        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
        out.push(((triple >> 16) & 0xff) as u8);
        if p2 != b'=' {
            out.push(((triple >> 8) & 0xff) as u8);
        }
        if p3 != b'=' {
            out.push((triple & 0xff) as u8);
        }
    }
    Some(out)
}

pub fn materialize_image_to_cache(data_b64: &str, mime: &str) -> Option<std::path::PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let bytes = decode_base64(data_b64)?;
    if bytes.is_empty() {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    mime.hash(&mut hasher);
    bytes.hash(&mut hasher);
    let hash = hasher.finish();
    let ext = extension_for_image_mime(mime);
    let dir = std::env::temp_dir().join("picrust-image-cache");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("img-{hash:016x}.{ext}"));
    if !path.exists() {
        let _ = std::fs::write(&path, &bytes);
    }
    Some(path)
}

pub fn extract_image_attachments_from_content(content: Option<&Value>) -> Vec<AttachmentItem> {
    let Some(arr) = content.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, block) in arr.iter().enumerate() {
        let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
        if kind == "image" {
            let data = block
                .get("data")
                .and_then(Value::as_str)
                .or_else(|| block.get("source").and_then(|s| s.get("data")).and_then(Value::as_str));
            let mime = block
                .get("mimeType")
                .or_else(|| block.get("mime_type"))
                .and_then(Value::as_str)
                .or_else(|| {
                    block
                        .get("source")
                        .and_then(|s| s.get("media_type"))
                        .and_then(Value::as_str)
                })
                .unwrap_or("image/png");
            if let Some(b64) = data {
                if let Some(path) = materialize_image_to_cache(b64, mime) {
                    let size_bytes = std::fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| SharedString::from(s.to_string()))
                        .unwrap_or_else(|| SharedString::from(format!("image-{idx}.png")));
                    out.push(AttachmentItem {
                        name,
                        mime: SharedString::from(mime.to_string()),
                        size_bytes,
                        local_path: Some(path),
                        base64_data: None,
                    });
                }
            }
        } else if kind == "image_url" {
            let url = block
                .get("image_url")
                .and_then(|u| u.get("url"))
                .or_else(|| block.get("url"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if let Some(rest) = url.strip_prefix("data:") {
                if let Some((meta, b64)) = rest.split_once(";base64,") {
                    let mime = if meta.starts_with("image/") { meta } else { "image/png" };
                    if let Some(path) = materialize_image_to_cache(b64, mime) {
                        let size_bytes = std::fs::metadata(&path).map(|m| m.len() as usize).unwrap_or(0);
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| SharedString::from(s.to_string()))
                            .unwrap_or_else(|| SharedString::from(format!("image-{idx}.png")));
                        out.push(AttachmentItem {
                            name,
                            mime: SharedString::from(mime.to_string()),
                            size_bytes,
                            local_path: Some(path),
                            base64_data: None,
                        });
                    }
                }
            }
        }
    }
    out
}

pub fn parse_message_content(
    content: Option<&Value>,
) -> (String, String, Vec<ToolCallItem>, Vec<ProcessStep>) {
    let Some(content) = content else {
        return (String::new(), String::new(), Vec::new(), Vec::new());
    };
    if let Some(s) = content.as_str() {
        return (s.to_string(), String::new(), Vec::new(), Vec::new());
    }
    let Some(arr) = content.as_array() else {
        return (String::new(), String::new(), Vec::new(), Vec::new());
    };

    let mut text = String::new();
    let mut thinking = String::new();
    let mut tools = Vec::new();
    let mut steps = Vec::new();

    for block in arr {
        let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text" => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(t);
                }
            }
            "thinking" => {
                if let Some(t) = block.get("thinking").and_then(Value::as_str) {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        if !thinking.is_empty() {
                            thinking.push('\n');
                        }
                        thinking.push_str(trimmed);
                        steps.push(ProcessStep::Thinking(SharedString::from(trimmed.to_string())));
                    }
                }
            }
            "toolCall" | "tool_use" => {
                let id = block
                    .get("toolCallId")
                    .or_else(|| block.get("id"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("toolName")
                    .or_else(|| block.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_string();
                let args_val = block.get("input").or_else(|| block.get("arguments"));
                let arguments = flatten_tool_args(args_val);
                let diff_patch = build_diff_from_tool(&name, args_val, None);
                let tc = ToolCallItem {
                    id: SharedString::from(id),
                    name: SharedString::from(name),
                    arguments,
                    output: SharedString::default(),
                    diff_patch,
                    subagent_session_id: None,
                    is_running: false,
                    is_error: false,
                    expanded: false,
                };
                steps.push(ProcessStep::ToolCall(tc.clone()));
                tools.push(tc);
            }
            _ => {}
        }
    }

    (text, thinking, tools, steps)
}

pub fn flatten_tool_args(args: Option<&Value>) -> Vec<(SharedString, SharedString)> {
    let Some(obj) = args.and_then(Value::as_object) else {
        return Vec::new();
    };
    obj.iter()
        .map(|(k, v)| {
            let val_str = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (SharedString::from(k.clone()), SharedString::from(val_str))
        })
        .collect()
}

fn split_lines_exact(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

pub fn build_diff_from_tool(
    name: &str,
    args: Option<&Value>,
    details: Option<&Value>,
) -> Option<SharedString> {
    if name != "edit" {
        return None;
    }
    if let Some(patch) = details
        .and_then(|d| d.get("patch").or_else(|| d.get("diff")))
        .and_then(Value::as_str)
        .filter(|p| !p.trim().is_empty())
    {
        if patch.contains("--- ") && patch.contains("+++ ") {
            return Some(SharedString::from(patch.to_string()));
        }
        let path = args
            .and_then(|a| a.get("path").or_else(|| a.get("file_path")))
            .and_then(Value::as_str)
            .unwrap_or("file");
        return Some(SharedString::from(format!(
            "--- a/{path}\n+++ b/{path}\n{patch}"
        )));
    }
    let obj = args?.as_object()?;
    let path = obj
        .get("path")
        .or_else(|| obj.get("file_path"))
        .and_then(Value::as_str)
        .unwrap_or("file");

    let edits: Vec<&Value> = obj
        .get("edits")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().collect())
        .unwrap_or_default();

    let mut patch = String::new();
    if !edits.is_empty() {
        for item in edits {
            let Some(eobj) = item.as_object() else { continue };
            let before = eobj
                .get("oldText")
                .or_else(|| eobj.get("old_string"))
                .and_then(Value::as_str)?;
            let after = eobj
                .get("newText")
                .or_else(|| eobj.get("new_string"))
                .and_then(Value::as_str)?;
            append_unified_hunk(&mut patch, path, before, after);
        }
    } else {
        let before = obj
            .get("oldText")
            .or_else(|| obj.get("old_string"))
            .and_then(Value::as_str)?;
        let after = obj
            .get("newText")
            .or_else(|| obj.get("new_string"))
            .and_then(Value::as_str)?;
        append_unified_hunk(&mut patch, path, before, after);
    }

    if patch.is_empty() {
        None
    } else {
        Some(SharedString::from(patch))
    }
}

fn append_unified_hunk(out: &mut String, path: &str, before: &str, after: &str) {
    let a = split_lines_exact(before);
    let b = split_lines_exact(after);
    let mut start = 0usize;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        start += 1;
    }
    let mut end = 0usize;
    while end < a.len() - start
        && end < b.len() - start
        && a[a.len() - 1 - end] == b[b.len() - 1 - end]
    {
        end += 1;
    }

    let ctx_before = start.min(3);
    let ctx_after = end.min(3);
    let a_start = start - ctx_before;
    let a_end = a.len() - end + ctx_after;
    let b_start = start - ctx_before;
    let b_end = b.len() - end + ctx_after;

    if out.is_empty() {
        out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
    }
    out.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        a_start + 1,
        (a_end - a_start).max(1),
        b_start + 1,
        (b_end - b_start).max(1)
    ));
    for line in &a[a_start..start] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    for line in &a[start..a.len() - end] {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in &b[start..b.len() - end] {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    for line in &a[a.len() - end..a_end] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
}

pub fn flatten_content_text(content: Option<&Value>) -> String {
    let Some(c) = content else {
        return String::new();
    };
    if let Some(s) = c.as_str() {
        return s.to_string();
    }
    if let Some(arr) = c.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(t) = item.get("text").and_then(Value::as_str) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }
        }
        return out;
    }
    c.to_string()
}

pub fn extract_artifacts_from_markdown(
    markdown: &str,
    session_id: Option<&str>,
) -> Vec<ArtifactRecord> {
    let mut artifacts = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_start) = markdown[cursor..].find(":::artifact{") {
        let start = cursor + rel_start;
        let Some(rel_header_end) = markdown[start..].find('}') else {
            break;
        };
        let header = &markdown[start + 12..start + rel_header_end];
        let body_start = start + rel_header_end + 1;
        let Some(rel_close) = markdown[body_start..].find("\n:::") else {
            break;
        };
        let body = markdown[body_start..body_start + rel_close].trim();
        cursor = body_start + rel_close + 4;

        let identifier = parse_attr(header, "identifier")
            .unwrap_or_else(|| format!("artifact-{}", artifacts.len() + 1));
        let title = parse_attr(header, "title").unwrap_or_else(|| identifier.clone());
        let kind_str = parse_attr(header, "type").unwrap_or_else(|| "code".to_string());
        let kind = match kind_str.as_str() {
            "html" => ArtifactKind::Html,
            "react" | "jsx" | "tsx" => ArtifactKind::React,
            "svg" => ArtifactKind::Svg,
            "mermaid" => ArtifactKind::Mermaid,
            "markdown" | "md" => ArtifactKind::Markdown,
            _ => ArtifactKind::Code,
        };

        let (language, inner_code) = strip_outer_fence(body);
        artifacts.push(ArtifactRecord {
            identifier,
            title,
            kind,
            language,
            file_path: None,
            content: inner_code,
            previous_content: None,
            version: 1,
            session_id: session_id.map(String::from),
            updated_at: None,
            favorite: false,
        });
    }

    artifacts
}

fn parse_attr(header: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = header.find(&needle)? + needle.len();
    let end = header[start..].find('"')?;
    Some(header[start..start + end].to_string())
}

fn strip_outer_fence(body: &str) -> (Option<String>, String) {
    let trimmed = body.trim();
    if let Some(after_ticks) = trimmed.strip_prefix("```") {
        if let Some(newline) = after_ticks.find('\n') {
            let lang = after_ticks[..newline].trim();
            let rest = &after_ticks[newline + 1..];
            let code = rest.strip_suffix("```").unwrap_or(rest).trim_end();
            return (
                if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                },
                code.to_string(),
            );
        }
    }
    (None, trimmed.to_string())
}

pub fn parse_agy_link_widget_lines(lines: &[String]) -> Option<crate::client::types::AgyLinkState> {
    let raw = lines.iter().find(|l| !l.trim().is_empty())?;
    serde_json::from_str::<crate::client::types::AgyLinkState>(raw).ok()
}

pub fn parse_subagent_async_json_widget(lines: &[String]) -> Option<Vec<SubagentRunInfo>> {
    const PREFIX: &str = "PI_SUBAGENT_ASYNC_JSON:";
    let raw_line = lines.iter().find_map(|l| l.strip_prefix(PREFIX))?;
    let val: Value = serde_json::from_str(raw_line).ok()?;
    let runs_arr = val.get("runs")?.as_array()?;
    let mut out = Vec::with_capacity(runs_arr.len());
    for r in runs_arr {
        let id = r.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let kind = r.get("kind").and_then(Value::as_str).unwrap_or("subagent").to_string();
        let label = r.get("label").and_then(Value::as_str).unwrap_or("subagent").to_string();
        let state = r.get("state").and_then(Value::as_str).unwrap_or("running").to_string();
        let started_at = r.get("startedAt").and_then(Value::as_u64);
        let ended_at = r.get("endedAt").and_then(Value::as_u64);
        let model = r.get("model").and_then(Value::as_str).map(String::from);
        let thinking = r.get("thinking").and_then(Value::as_str).map(String::from);
        let activity = r.get("activity");
        let current_tool = activity
            .and_then(|a| a.get("currentTool"))
            .and_then(Value::as_str)
            .map(String::from);
        let turn_count = activity.and_then(|a| a.get("turnCount")).and_then(Value::as_u64);
        let tool_count = activity.and_then(|a| a.get("toolCount")).and_then(Value::as_u64);
        let progress_text = r.get("progressText").and_then(Value::as_str).map(String::from);
        let run_session_id = r
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let mut children = Vec::new();
        if let Some(c_arr) = r.get("children").and_then(Value::as_array) {
            for c in c_arr {
                let child_sid = c
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from);
                children.push(SubagentChildInfo {
                    id: c.get("id").and_then(Value::as_str).unwrap_or("").to_string(),
                    label: c.get("label").and_then(Value::as_str).unwrap_or("step").to_string(),
                    agent: c.get("agent").and_then(Value::as_str).map(String::from),
                    state: c.get("state").and_then(Value::as_str).unwrap_or("running").to_string(),
                    model: c.get("model").and_then(Value::as_str).map(String::from),
                    current_tool: c
                        .get("activity")
                        .and_then(|a| a.get("currentTool"))
                        .and_then(Value::as_str)
                        .map(String::from),
                    progress_text: c.get("progressText").and_then(Value::as_str).map(String::from),
                    session_id: child_sid,
                });
            }
        }
        let source = r.get("source").and_then(Value::as_str).map(String::from);
        let log_path = r
            .get("logPath")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let output_tail = r
            .get("outputTail")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        out.push(SubagentRunInfo {
            id,
            kind,
            label,
            state,
            started_at,
            ended_at,
            model,
            thinking,
            current_tool,
            turn_count,
            tool_count,
            progress_text,
            session_id: run_session_id,
            source,
            log_path,
            output_tail,
            children,
        });
    }
    Some(out)
}

/// Parses a `background-tasks` extension widget payload (JSON array or `{ tasks | runs: [...] }`)
/// into unified `SubagentRunInfo` entries with `kind = "background_task"` by default.
pub fn parse_background_tasks_widget(lines: &[String]) -> Option<Vec<SubagentRunInfo>> {
    let joined = lines.join("\n");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    let val: Value = serde_json::from_str(trimmed).ok()?;
    let arr = val
        .as_array()
        .or_else(|| val.get("tasks").and_then(Value::as_array))
        .or_else(|| val.get("runs").and_then(Value::as_array))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("bg-task")
            .to_string();
        let kind = item
            .get("kind")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("background_task")
            .to_string();
        let label = item
            .get("label")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&id)
            .to_string();
        let state = item
            .get("state")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("running")
            .to_string();
        let started_at = item.get("startedAt").and_then(Value::as_u64);
        let ended_at = item.get("endedAt").and_then(Value::as_u64);
        let progress_text = item
            .get("progressText")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let source = item
            .get("source")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let log_path = item
            .get("logPath")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let output_tail = item
            .get("outputTail")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
            .unwrap_or_default();
        let session_id = item
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        out.push(SubagentRunInfo {
            id,
            kind,
            label,
            state,
            started_at,
            ended_at,
            model: None,
            thinking: None,
            current_tool: None,
            turn_count: None,
            tool_count: None,
            progress_text,
            session_id,
            source,
            log_path,
            output_tail,
            children: Vec::new(),
        });
    }
    Some(out)
}

pub fn strip_ansi_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for inner in chars.by_ref() {
                    if ('@'..='~').contains(&inner) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn parse_todo_widget_lines(lines: &[String]) -> Vec<TodoWidgetItem> {
    let mut parsed = Vec::new();
    for (i, raw_line) in lines.iter().enumerate() {
        let plain = strip_ansi_escapes(raw_line);
        let trimmed = plain.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip rpiv-todo header ("● Todos (1/3)", "○ Todos ...") and overflow/collapsed hint lines
        if trimmed.starts_with("● Todos")
            || trimmed.starts_with("○ Todos")
            || trimmed.starts_with("● 待办")
            || trimmed.starts_with("○ 待办")
            || trimmed.starts_with("Todos (")
            || trimmed.contains("to expand")
            || trimmed.ends_with("collapsed")
        {
            continue;
        }
        let body = trimmed
            .trim_start_matches("├─")
            .trim_start_matches("└─")
            .trim_start_matches('│')
            .trim();
        if body.is_empty() || body.starts_with('+') && body.contains("more") {
            continue;
        }
        let done = body.starts_with("[x]")
            || body.starts_with("[X]")
            || body.starts_with('✓')
            || body.starts_with('✔');
        let cleaned = body
            .trim_start_matches("[x]")
            .trim_start_matches("[X]")
            .trim_start_matches("[ ]")
            .trim_start_matches('✓')
            .trim_start_matches('✔')
            .trim_start_matches('◐')
            .trim_start_matches('●')
            .trim_start_matches('○')
            .trim_start_matches('-')
            .trim();
        if !cleaned.is_empty() {
            parsed.push(TodoWidgetItem {
                id: SharedString::from(format!("todo-{i}")),
                content: SharedString::from(cleaned.to_string()),
                done,
            });
        }
    }
    parsed
}

pub fn parse_todo_tool_details(details: &Value) -> Option<Vec<TodoWidgetItem>> {
    let tasks = details.get("tasks").and_then(Value::as_array)?;
    let mut out = Vec::new();
    for t in tasks {
        let status = t.get("status").and_then(Value::as_str).unwrap_or("pending");
        if status == "deleted" {
            continue;
        }
        let id = t
            .get("id")
            .and_then(|v| {
                v.as_u64()
                    .map(|n| n.to_string())
                    .or_else(|| v.as_str().map(String::from))
            })
            .unwrap_or_else(|| (out.len() + 1).to_string());
        let subject = t
            .get("subject")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if subject.is_empty() {
            continue;
        }
        let active_form = t
            .get("activeForm")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let label = if status == "in_progress" {
            if let Some(af) = active_form {
                format!("{subject} ({af})")
            } else {
                subject.to_string()
            }
        } else {
            subject.to_string()
        };
        out.push(TodoWidgetItem {
            id: SharedString::from(id),
            content: SharedString::from(label),
            done: status == "completed",
        });
    }
    Some(out)
}

/// Applies optimistic todo mutations from a `todo` tool call's arguments immediately on start.
pub fn apply_optimistic_todo_args(todos: &mut Vec<TodoWidgetItem>, args: &Value) {
    let action = args.get("action").and_then(Value::as_str).unwrap_or("");
    match action {
        "clear" => todos.clear(),
        "create" => {
            if let Some(arr) = args.get("tasks").and_then(Value::as_array) {
                for item in arr {
                    if let Some(subj) = item.get("subject").and_then(Value::as_str) {
                        let next_id = (todos.len() + 1).to_string();
                        todos.push(TodoWidgetItem {
                            id: SharedString::from(next_id),
                            content: SharedString::from(subj.trim().to_string()),
                            done: false,
                        });
                    }
                }
            } else if let Some(subj) = args.get("subject").and_then(Value::as_str) {
                let next_id = (todos.len() + 1).to_string();
                todos.push(TodoWidgetItem {
                    id: SharedString::from(next_id),
                    content: SharedString::from(subj.trim().to_string()),
                    done: false,
                });
            }
        }
        "update" => {
            let target_id = args.get("id").and_then(|v| {
                v.as_u64()
                    .map(|n| n.to_string())
                    .or_else(|| v.as_str().map(String::from))
            });
            if let Some(tid) = target_id {
                let status = args.get("status").and_then(Value::as_str);
                if status == Some("deleted") {
                    todos.retain(|t| t.id.as_ref() != tid);
                    return;
                }
                if let Some(item) = todos
                    .iter_mut()
                    .find(|t| t.id.as_ref() == tid || t.content.as_ref().starts_with(&format!("#{tid} ")))
                {
                    if let Some(st) = status {
                        item.done = st == "completed";
                    }
                    if let Some(subj) = args.get("subject").and_then(Value::as_str) {
                        item.content = SharedString::from(subj.trim().to_string());
                    }
                }
            }
        }
        "delete" => {
            let target_id = args.get("id").and_then(|v| {
                v.as_u64()
                    .map(|n| n.to_string())
                    .or_else(|| v.as_str().map(String::from))
            });
            if let Some(tid) = target_id {
                todos.retain(|t| t.id.as_ref() != tid);
            }
        }
        _ => {}
    }
}

/// Parses `@juicesharp/rpiv-todo` text envelope output when `details.tasks` is stripped over SSE.
pub fn apply_todo_tool_output_text(todos: &mut Vec<TodoWidgetItem>, output: &str) {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return;
    }
    if trimmed == "No tasks" || trimmed.starts_with("Cleared ") {
        todos.clear();
        return;
    }
    let mut list_items = Vec::new();
    let mut is_list = true;
    for line in trimmed.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if let Some(rest) = l.strip_prefix('[') {
            if let Some(close) = rest.find("] #") {
                let status = &rest[..close];
                let after = &rest[close + 3..];
                let mut parts = after.splitn(2, ' ');
                let id = parts.next().unwrap_or("").trim();
                let subject = parts.next().unwrap_or("").trim();
                if !id.is_empty() && !subject.is_empty() {
                    list_items.push(TodoWidgetItem {
                        id: SharedString::from(id.to_string()),
                        content: SharedString::from(subject.to_string()),
                        done: status == "completed",
                    });
                    continue;
                }
            }
        }
        is_list = false;
        break;
    }
    if is_list && !list_items.is_empty() {
        *todos = list_items;
        return;
    }

    for line in trimmed.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("Created #") {
            if let Some((id_str, subj_part)) = rest.split_once(": ") {
                let subj = subj_part
                    .strip_suffix(" (pending)")
                    .or_else(|| subj_part.strip_suffix(" (in_progress)"))
                    .or_else(|| subj_part.strip_suffix(" (completed)"))
                    .unwrap_or(subj_part)
                    .trim();
                let id = id_str.trim();
                if let Some(existing) = todos.iter_mut().find(|t| t.id.as_ref() == id) {
                    existing.content = SharedString::from(subj.to_string());
                } else if let Some(opt_match) = todos.iter_mut().find(|t| t.content.as_ref() == subj) {
                    opt_match.id = SharedString::from(id.to_string());
                } else {
                    todos.push(TodoWidgetItem {
                        id: SharedString::from(id.to_string()),
                        content: SharedString::from(subj.to_string()),
                        done: false,
                    });
                }
            }
        } else if let Some(rest) = l.strip_prefix("Updated #") {
            let id = rest.split_whitespace().next().unwrap_or("").trim();
            if let Some(item) = todos.iter_mut().find(|t| t.id.as_ref() == id) {
                if rest.contains("→ completed)") || rest.contains("-> completed)") {
                    item.done = true;
                } else if rest.contains("→ pending)") || rest.contains("→ in_progress)") {
                    item.done = false;
                } else if rest.contains("→ deleted)") {
                    let id_own = id.to_string();
                    todos.retain(|t| t.id.as_ref() != id_own);
                }
            }
        } else if let Some(rest) = l.strip_prefix("Deleted #") {
            let id = rest.split(':').next().unwrap_or("").trim();
            todos.retain(|t| t.id.as_ref() != id);
        }
    }
}

pub fn extract_model_and_thinking_from_jsonl(
    path: &Path,
) -> (Option<(String, String)>, Option<ThinkingLevel>) {
    let Ok(file) = File::open(path) else {
        return (None, None);
    };
    let reader = BufReader::new(file);
    let mut model: Option<(String, String)> = None;
    let mut thinking: Option<ThinkingLevel> = None;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let etype = val.get("type").and_then(Value::as_str).unwrap_or("");
        if etype == "model_change" {
            let prov = val.get("provider").and_then(Value::as_str);
            let mid = val.get("modelId").or_else(|| val.get("model")).and_then(Value::as_str);
            if let (Some(p), Some(m)) = (prov, mid) {
                model = Some((p.to_string(), m.to_string()));
            }
            if let Some(tl) = val.get("thinkingLevel").and_then(Value::as_str) {
                thinking = ThinkingLevel::parse(tl);
            }
        } else if etype == "message" {
            if let Some(msg) = val.get("message") {
                if msg.get("role").and_then(Value::as_str) == Some("assistant") {
                    let prov = msg.get("provider").and_then(Value::as_str);
                    let mid = msg.get("model").or_else(|| msg.get("modelId")).and_then(Value::as_str);
                    if let (Some(p), Some(m)) = (prov, mid) {
                        model = Some((p.to_string(), m.to_string()));
                    }
                    if let Some(tl) = msg.get("thinkingLevel").and_then(Value::as_str) {
                        if let Some(parsed) = ThinkingLevel::parse(tl) {
                            thinking = Some(parsed);
                        }
                    }
                }
            }
        }
    }
    (model, thinking)
}

pub fn extract_todos_from_jsonl(path: &Path) -> Vec<TodoWidgetItem> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut latest: Option<Vec<TodoWidgetItem>> = None;
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if !trimmed.contains("todo") {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let etype = val.get("type").and_then(Value::as_str).unwrap_or("");
        if etype == "custom"
            && val.get("customType").and_then(Value::as_str) == Some("rpiv-todo-state")
        {
            if let Some(data) = val.get("data").or_else(|| val.get("details")) {
                if let Some(parsed) = parse_todo_tool_details(data) {
                    latest = Some(parsed);
                }
            }
        } else if etype == "message" {
            if let Some(msg) = val.get("message") {
                if msg.get("role").and_then(Value::as_str) == Some("toolResult") {
                    if let Some(details) = msg.get("details") {
                        if let Some(parsed) = parse_todo_tool_details(details) {
                            latest = Some(parsed);
                        }
                    }
                }
            }
        }
    }
    latest.unwrap_or_default()
}

pub fn extract_subagent_session_id(details: Option<&Value>) -> Option<SharedString> {
    let d = details?;
    if d.get("kind").and_then(Value::as_str) == Some("pi-web-subagent") {
        return d
            .get("sessionId")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(|s| SharedString::from(s.to_string()));
    }
    d.get("results")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|r| r.get("sessionId"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(|s| SharedString::from(s.to_string()))
}

pub fn extract_todos_from_payload(payload: &Value) -> Option<Vec<TodoWidgetItem>> {
    let messages = payload
        .get("messages")
        .or_else(|| payload.get("context").and_then(|c| c.get("messages")))
        .and_then(Value::as_array)?;
    let mut latest: Option<Vec<TodoWidgetItem>> = None;
    let mut replayed: Vec<TodoWidgetItem> = Vec::new();
    let mut saw_replay = false;
    let mut todo_call_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for msg in messages {
        if let Some(details) = msg.get("details").or_else(|| msg.get("data")) {
            if let Some(parsed) = parse_todo_tool_details(details) {
                latest = Some(parsed.clone());
                replayed = parsed;
                saw_replay = true;
            }
        }
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("");
        if role == "assistant" {
            if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                for block in blocks {
                    let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
                    let name = block
                        .get("toolName")
                        .or_else(|| block.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if (kind == "toolCall" || kind == "tool_use") && name == "todo" {
                        if let Some(cid) = block
                            .get("toolCallId")
                            .or_else(|| block.get("id"))
                            .and_then(Value::as_str)
                        {
                            todo_call_ids.insert(cid.to_string());
                        }
                        if let Some(args) = block.get("input").or_else(|| block.get("arguments")) {
                            apply_optimistic_todo_args(&mut replayed, args);
                            saw_replay = true;
                        }
                    }
                }
            }
        } else if role == "toolResult" || role == "tool" {
            let is_error = msg.get("isError").and_then(Value::as_bool).unwrap_or(false);
            let tool_name = msg.get("toolName").and_then(Value::as_str).unwrap_or("");
            let call_id = msg.get("toolCallId").and_then(Value::as_str).unwrap_or("");
            if !is_error && (tool_name == "todo" || todo_call_ids.contains(call_id)) {
                let text = flatten_content_text(msg.get("content"));
                if !text.trim().is_empty() {
                    apply_todo_tool_output_text(&mut replayed, &text);
                    saw_replay = true;
                }
            }
        }
    }
    latest.or(if saw_replay { Some(replayed) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_artifact_blocks() {
        let md = r#"Here is a chart:
:::artifact{identifier="demo-svg" type="svg" title="Demo SVG"}
```svg
<svg viewBox="0 0 100 100"><circle cx="50" cy="50" r="40"/></svg>
```
:::
"#;
        let arts = extract_artifacts_from_markdown(md, Some("s1"));
        assert_eq!(arts.len(), 1);
        assert_eq!(arts[0].identifier, "demo-svg");
        assert_eq!(arts[0].kind, ArtifactKind::Svg);
        assert!(arts[0].content.contains("<circle"));
    }

    #[test]
    fn monotonic_request_id_increments() {
        let mut ctrl = SessionController::default();
        let a = ctrl.next_load_id();
        let b = ctrl.next_load_id();
        assert_eq!(a + 1, b);
    }

    #[test]
    fn parses_session_messages_with_tool_results_and_diff() {
        let payload = serde_json::json!({
            "entryIds": ["e1", "e2", "e3", "e4"],
            "messages": [
                { "role": "user", "content": "Edit main.rs" },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "content": [
                        { "type": "thinking", "thinking": "Inspecting file" },
                        {
                            "type": "toolCall",
                            "toolCallId": "tc1",
                            "toolName": "edit",
                            "input": { "path": "src/main.rs", "oldText": "fn a() {}", "newText": "fn b() {}" }
                        }
                    ]
                },
                {
                    "role": "toolResult",
                    "toolCallId": "tc1",
                    "isError": false,
                    "content": [{ "type": "text", "text": "File updated" }]
                },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "content": [
                        { "type": "thinking", "thinking": "Verifying edit" },
                        { "type": "text", "text": "Applying fix." }
                    ]
                }
            ]
        });

        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].role, MessageRole::User);
        assert_eq!(parsed[1].process_message_count, 2);
        assert_eq!(parsed[1].process_steps.len(), 3); // 2 thinking + 1 toolCall
        assert_eq!(parsed[1].content, "Applying fix.");
        assert_eq!(parsed[1].tool_calls.len(), 1);
        assert_eq!(parsed[1].tool_calls[0].output.as_ref(), "File updated");
        assert!(parsed[1].tool_calls[0].diff_patch.as_ref().unwrap().contains("-fn a() {}"));
        assert!(parsed[1].tool_calls[0].diff_patch.as_ref().unwrap().contains("+fn b() {}"));
    }

    #[test]
    fn merges_compaction_inside_assistant_turn_without_splitting_request() {
        let payload = serde_json::json!({
            "entryIds": ["u1", "a1", "c1", "a2"],
            "messages": [
                { "role": "user", "content": "Refactor module" },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "content": [
                        { "type": "thinking", "thinking": "Pre-compaction analysis" }
                    ]
                },
                {
                    "role": "custom",
                    "customType": "compaction",
                    "content": "Summary of prior context.\n\n<read-files>\nfoo.rs\n</read-files>\n<modified-files>\nbar.rs\n</modified-files>",
                    "details": { "tokensBefore": 120000 }
                },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "content": [
                        { "type": "thinking", "thinking": "Post-compaction analysis" },
                        { "type": "text", "text": "Done!" }
                    ]
                }
            ]
        });

        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].role, MessageRole::User);
        assert_eq!(parsed[1].role, MessageRole::Assistant);
        assert_eq!(parsed[1].process_steps.len(), 3);
        assert!(matches!(parsed[1].process_steps[0], ProcessStep::Thinking(_)));
        match &parsed[1].process_steps[1] {
            ProcessStep::Compaction(c) => {
                assert_eq!(c.summary_markdown, "Summary of prior context.");
                assert_eq!(c.tokens_before, Some(120000));
                assert_eq!(c.read_files, vec!["foo.rs"]);
                assert_eq!(c.modified_files, vec!["bar.rs"]);
            }
            _ => panic!("expected ProcessStep::Compaction"),
        }
        assert!(matches!(parsed[1].process_steps[2], ProcessStep::Thinking(_)));
        assert!(parsed[1].tool_calls.is_empty());
        assert_eq!(parsed[1].content, "Done!");
    }

    #[test]
    fn keeps_post_turn_manual_compaction_separate_from_finished_assistant_turn() {
        let payload = serde_json::json!({
            "entryIds": ["u1", "a1", "c1", "u2", "a2"],
            "messages": [
                { "role": "user", "content": "Explain architecture" },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "stopReason": "stop",
                    "content": [
                        { "type": "text", "text": "Here is the overview." }
                    ]
                },
                {
                    "role": "custom",
                    "customType": "compaction",
                    "content": "Manual compaction summary.",
                    "details": { "tokensBefore": 45000 }
                },
                { "role": "user", "content": "Next question" },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "stopReason": "stop",
                    "content": [
                        { "type": "text", "text": "Follow-up answer." }
                    ]
                }
            ]
        });

        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[0].role, MessageRole::User);
        assert_eq!(parsed[1].role, MessageRole::Assistant);
        assert_eq!(parsed[1].content, "Here is the overview.");
        assert!(parsed[1].process_steps.is_empty());
        assert_eq!(parsed[2].role, MessageRole::System);
        assert!(matches!(
            parsed[2].process_steps.first(),
            Some(ProcessStep::Compaction(c)) if c.summary_markdown == "Manual compaction summary."
        ));
        assert_eq!(parsed[3].role, MessageRole::User);
        assert_eq!(parsed[4].role, MessageRole::Assistant);
        assert_eq!(parsed[4].content, "Follow-up answer.");
    }

    #[test]
    fn parses_agy_link_widget_json() {
        let lines = vec![
            r#"{"version":"1.5.9","dormantReason":null,"activeAccountId":"acc_1","accounts":[{"id":"acc_1","alias":"Primary","email":"user@example.com","tier":"PRO/ULTRA","priority":1,"disabled":false,"active":true,"systemHome":true,"authRequired":false,"fiveHourPct":92,"fiveHourReset":"3h 10m","weeklyPct":85,"weeklyReset":"5d 2h","models":[{"name":"gemini-3.6-flash","percentage":92,"resetTime":"3h 10m"}]}],"updatedAt":1700000000000}"#.to_string(),
        ];
        let parsed = parse_agy_link_widget_lines(&lines).unwrap();
        assert_eq!(parsed.version.as_deref(), Some("1.5.9"));
        assert_eq!(parsed.accounts.len(), 1);
        assert_eq!(parsed.accounts[0].email.as_deref(), Some("user@example.com"));
        assert_eq!(parsed.accounts[0].five_hour_pct, Some(92));
        assert_eq!(parsed.accounts[0].models.len(), 1);
    }

    #[test]
    fn parses_pi_subagents_async_json_widget() {
        let lines = vec![
            "PI_SUBAGENT_ASYNC_JSON:{\"runs\":[{\"id\":\"r1\",\"kind\":\"workflow\",\"label\":\"reviewer, worker\",\"state\":\"running\",\"model\":\"anthropic/claude-sonnet-4-6\",\"activity\":{\"currentTool\":\"bash\",\"turnCount\":2,\"toolCount\":4},\"children\":[{\"id\":\"s1\",\"label\":\"reviewer\",\"agent\":\"reviewer\",\"state\":\"complete\"},{\"id\":\"s2\",\"label\":\"worker\",\"agent\":\"worker\",\"state\":\"running\",\"activity\":{\"currentTool\":\"edit\"}}]}]}".to_string()
        ];
        let runs = parse_subagent_async_json_widget(&lines).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].kind, "workflow");
        assert!(runs[0].is_active());
        assert_eq!(runs[0].current_tool.as_deref(), Some("bash"));
        assert_eq!(runs[0].children.len(), 2);
        assert_eq!(runs[0].children[1].current_tool.as_deref(), Some("edit"));
    }

    #[test]
    fn parses_rpiv_todo_widget_lines_with_ansi_and_tool_details() {
        let lines = vec![
            "\x1b[36m● Todos (1/3)\x1b[0m".to_string(),
            "\x1b[2m├─\x1b[0m \x1b[32m✓\x1b[0m #1 Inspect SSE stream".to_string(),
            "\x1b[2m├─\x1b[0m \x1b[33m◐\x1b[0m #2 Fix live streaming".to_string(),
            "\x1b[2m└─\x1b[0m ○ #3 Run cargo test".to_string(),
            String::new(),
        ];
        let parsed = parse_todo_widget_lines(&lines);
        assert_eq!(parsed.len(), 3);
        assert!(parsed[0].done);
        assert_eq!(parsed[0].content.as_ref(), "#1 Inspect SSE stream");
        assert!(!parsed[1].done);
        assert_eq!(parsed[1].content.as_ref(), "#2 Fix live streaming");
        assert!(!parsed[2].done);
        assert_eq!(parsed[2].content.as_ref(), "#3 Run cargo test");

        let details = serde_json::json!({
            "tasks": [
                { "id": 1, "subject": "Check SSE", "status": "completed" },
                { "id": 2, "subject": "Fix UI", "activeForm": "fixing UI", "status": "in_progress" },
                { "id": 3, "subject": "Old item", "status": "deleted" }
            ]
        });
        let from_details = parse_todo_tool_details(&details).unwrap();
        assert_eq!(from_details.len(), 2);
        assert!(from_details[0].done);
        assert_eq!(from_details[1].content.as_ref(), "Fix UI (fixing UI)");
    }

    #[test]
    fn parses_assistant_stop_reason_and_error_message() {
        let payload = serde_json::json!({
            "entryIds": ["u1", "a1"],
            "messages": [
                { "role": "user", "content": "Hello" },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4-6",
                    "content": [],
                    "stopReason": "error",
                    "errorMessage": "429 Too Many Requests: rate limit exceeded"
                }
            ]
        });
        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].stop_reason.as_deref(), Some("error"));
        assert_eq!(
            parsed[1].error_message.as_deref(),
            Some("429 Too Many Requests: rate limit exceeded")
        );
    }

    #[test]
    fn updates_todos_from_optimistic_args_and_envelope_text() {
        let mut todos = Vec::new();
        apply_optimistic_todo_args(
            &mut todos,
            &serde_json::json!({
                "action": "create",
                "tasks": [
                    { "subject": "First step" },
                    { "subject": "Second step" }
                ]
            }),
        );
        assert_eq!(todos.len(), 2);
        assert!(!todos[0].done);

        apply_todo_tool_output_text(
            &mut todos,
            "Updated #1 (in_progress → completed)\nCreated #3: Third step (pending)",
        );
        assert_eq!(todos.len(), 3);
        assert!(todos[0].done);
        assert_eq!(todos[2].content.as_ref(), "Third step");
    }

    #[test]
    fn replays_todos_from_payload_when_details_stripped_and_parses_subagent_session_id() {
        let payload = serde_json::json!({
            "entryIds": ["u1", "a1", "t1", "a2", "t2"],
            "messages": [
                { "role": "user", "content": "Track and delegate" },
                {
                    "role": "assistant",
                    "content": [{
                        "type": "toolCall",
                        "toolCallId": "tc-todo-1",
                        "toolName": "todo",
                        "input": { "action": "create", "tasks": [{ "subject": "Audit todo" }, { "subject": "Audit subagent" }] }
                    }]
                },
                {
                    "role": "toolResult",
                    "toolCallId": "tc-todo-1",
                    "toolName": "todo",
                    "isError": false,
                    "content": [{ "type": "text", "text": "Created #1: Audit todo (pending)\nCreated #2: Audit subagent (pending)" }]
                },
                {
                    "role": "assistant",
                    "content": [{
                        "type": "toolCall",
                        "toolCallId": "tc-agent-1",
                        "toolName": "Agent",
                        "input": { "subagent_type": "explore", "description": "Check subagent", "prompt": "Check subagent" }
                    }]
                },
                {
                    "role": "toolResult",
                    "toolCallId": "tc-agent-1",
                    "toolName": "Agent",
                    "isError": false,
                    "content": [{ "type": "text", "text": "Subagent child-abc completed." }],
                    "details": {
                        "kind": "pi-web-subagent",
                        "sessionId": "child-abc",
                        "profile": "explore",
                        "description": "Check subagent",
                        "status": "completed",
                        "runInBackground": false
                    }
                }
            ]
        });

        let replayed_todos = extract_todos_from_payload(&payload).unwrap();
        assert_eq!(replayed_todos.len(), 2);
        assert_eq!(replayed_todos[0].id.as_ref(), "1");
        assert_eq!(replayed_todos[1].content.as_ref(), "Audit subagent");

        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        let agent_tc = parsed[1]
            .tool_calls
            .iter()
            .find(|tc| tc.name.as_ref() == "Agent")
            .unwrap();
        assert_eq!(agent_tc.subagent_session_id.as_deref(), Some("child-abc"));
    }

    #[test]
    fn session_tree_index_resolves_branches_and_siblings() {
        let entries = vec![
            serde_json::json!({ "type": "session", "version": 3, "id": "sess-1" }),
            serde_json::json!({
                "type": "message",
                "id": "u1",
                "parentId": null,
                "message": { "role": "user", "content": "Hello world" }
            }),
            serde_json::json!({
                "type": "message",
                "id": "a1",
                "parentId": "u1",
                "message": { "role": "assistant", "content": "Reply 1" }
            }),
            serde_json::json!({
                "type": "message",
                "id": "u2-branch-a",
                "parentId": "a1",
                "message": { "role": "user", "content": "Option A" }
            }),
            serde_json::json!({
                "type": "message",
                "id": "a2-branch-a",
                "parentId": "u2-branch-a",
                "message": { "role": "assistant", "content": "Outcome A" }
            }),
            serde_json::json!({
                "type": "message",
                "id": "u2-branch-b",
                "parentId": "a1",
                "message": { "role": "user", "content": "Option B" }
            }),
            serde_json::json!({
                "type": "message",
                "id": "a2-branch-b",
                "parentId": "u2-branch-b",
                "message": { "role": "assistant", "content": "Outcome B" }
            }),
        ];

        let index = SessionTreeIndex::from_entries(entries);
        assert_eq!(index.resolve_leaf_from("u2-branch-a"), "a2-branch-a");
        assert_eq!(index.resolve_leaf_from("u2-branch-b"), "a2-branch-b");

        let mut next_id = 1;
        let (msgs_all, siblings_all) = index.extract_messages_for_leaf(None, &mut next_id);
        assert_eq!(msgs_all.len(), 6);
        assert_eq!(msgs_all[2].content.as_str(), "Option A");
        assert_eq!(msgs_all[3].content.as_str(), "Outcome A");
        assert_eq!(msgs_all[4].content.as_str(), "Option B");
        assert_eq!(msgs_all[5].content.as_str(), "Outcome B");
        assert!(siblings_all.is_empty());

        let (msgs_a, siblings_a) =
            index.extract_messages_for_leaf(Some("u2-branch-a"), &mut next_id);
        assert_eq!(msgs_a.len(), 4);
        assert_eq!(msgs_a[2].content.as_str(), "Option A");
        assert_eq!(msgs_a[3].content.as_str(), "Outcome A");
        assert_eq!(siblings_a.len(), 2);
        assert!(siblings_a[0].active);
        assert!(!siblings_a[1].active);
    }

    #[test]
    fn prunes_failed_turn_and_returns_rollback_parent() {
        let dir = std::env::temp_dir().join(format!("picrust-prune-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("session.jsonl");
        let lines = [
            serde_json::json!({ "type": "session", "id": "s1" }),
            serde_json::json!({ "type": "message", "id": "u1", "parentId": "s1", "message": { "role": "user", "content": "hi" } }),
            serde_json::json!({ "type": "message", "id": "a1", "parentId": "u1", "message": { "role": "assistant", "content": "hello" } }),
            serde_json::json!({ "type": "message", "id": "u2", "parentId": "a1", "message": { "role": "user", "content": "broken" } }),
            serde_json::json!({ "type": "message", "id": "a2", "parentId": "u2", "message": { "role": "assistant", "content": [], "stopReason": "error" } }),
            serde_json::json!({ "type": "context_edit", "id": "ce1", "parentId": "a2" }),
        ];
        let body = lines
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(&path, body).unwrap();

        let rollback = prune_failed_turn_from_jsonl(&path, Some("u2"), Some("a2"));
        assert_eq!(rollback.as_deref(), Some("a1"));

        let after = std::fs::read_to_string(&path).unwrap();
        let kept_ids: Vec<String> = after
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter_map(|v| v.get("id").and_then(Value::as_str).map(String::from))
            .collect();
        assert_eq!(kept_ids, vec!["s1", "u1", "a1"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn implicit_continue_merges_into_interrupted_assistant_turn() {
        let payload = serde_json::json!({
            "messages": [
                { "role": "user", "content": "Explain Rust lifetimes" },
                {
                    "role": "assistant",
                    "content": [{ "type": "text", "text": "Part 1: Lifetimes ensure" }],
                    "stopReason": "error",
                    "errorMessage": "network reset"
                },
                { "role": "user", "content": IMPLICIT_CONTINUE_PROMPT },
                {
                    "role": "assistant",
                    "content": [{ "type": "text", "text": "Part 2: references stay valid." }],
                    "stopReason": "stop"
                }
            ]
        });
        let mut next_id = 1;
        let parsed = parse_session_messages(&payload, &mut next_id);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].role, MessageRole::User);
        assert_eq!(parsed[1].role, MessageRole::Assistant);
        assert!(parsed[1].content.contains("Part 1: Lifetimes ensure"));
        assert!(parsed[1].content.contains("Part 2: references stay valid."));
        assert_eq!(parsed[1].stop_reason.as_deref(), Some("stop"));
        assert!(parsed[1].error_message.is_none());
    }

    #[test]
    fn parses_background_tasks_widget_json() {
        let lines = vec![
            r#"[{"id":"task-1","kind":"background_task","label":"task-1","state":"running","source":"agy","progressText":"building...","outputTail":["line 1","line 2"],"logPath":"/tmp/task-1.log","startedAt":1700000000}]"#.to_string(),
        ];
        let parsed = parse_background_tasks_widget(&lines).expect("should parse json array");
        assert_eq!(parsed.len(), 1);
        let run = &parsed[0];
        assert_eq!(run.id, "task-1");
        assert!(run.is_background_task());
        assert_eq!(run.state, "running");
        assert_eq!(run.source.as_deref(), Some("agy"));
        assert_eq!(run.progress_text.as_deref(), Some("building..."));
        assert_eq!(run.log_path.as_deref(), Some("/tmp/task-1.log"));
        assert_eq!(run.output_tail, vec!["line 1", "line 2"]);
    }
}

