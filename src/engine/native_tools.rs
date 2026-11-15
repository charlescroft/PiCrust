//! Native Rust execution of core built-in tools (`pi-gpui/src/engine/native_tools.rs`).
//!
//! Implements `read`, `write`, `edit`, `bash`, and `todo` directly in Rust without
//! spawning a Node sidecar process.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::llm_stream::NativeToolDefinition;

/// Maximum lines returned by `read` before truncation.
pub const MAX_READ_LINES: usize = 2000;
/// Maximum byte size returned by `read` or `bash` before truncation (50 KB).
pub const MAX_OUTPUT_BYTES: usize = 50 * 1024;

/// Result of executing a native built-in tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeToolResult {
    pub output: String,
    pub is_error: bool,
    pub diff: Option<String>,
    pub details: Option<Value>,
}

/// Returns the JSON Schema definitions for the core native tools.
pub fn builtin_tool_definitions() -> Vec<NativeToolDefinition> {
    vec![
        NativeToolDefinition {
            name: "read".to_string(),
            description: "Read the contents of a text file (truncated to 2000 lines or 50KB)."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the file to read" },
                    "offset": { "type": "integer", "description": "1-indexed start line" },
                    "limit": { "type": "integer", "description": "Max lines to read" }
                },
                "required": ["path"]
            }),
        },
        NativeToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file atomically, creating parent directories."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to write" },
                    "content": { "type": "string", "description": "Full file content" }
                },
                "required": ["path", "content"]
            }),
        },
        NativeToolDefinition {
            name: "edit".to_string(),
            description: "Replace exact text regions in a file and produce a unified diff."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to edit" },
                    "oldText": { "type": "string", "description": "Exact text to replace" },
                    "newText": { "type": "string", "description": "Replacement text" },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "oldText": { "type": "string" },
                                "newText": { "type": "string" }
                            },
                            "required": ["oldText", "newText"]
                        }
                    }
                },
                "required": ["path"]
            }),
        },
        NativeToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command in the workspace directory.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command" },
                    "timeout": { "type": "number", "description": "Timeout in seconds" }
                },
                "required": ["command"]
            }),
        },
    ]
}

fn resolve_tool_path(cwd: &Path, raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return crate::client::native_index::user_home().join(rest);
    }
    let p = Path::new(trimmed);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// Executes `read` with `offset` (1-indexed), `limit`, and 2000-line / 50KB truncation.
pub fn execute_read_tool(cwd: &Path, args: &Value) -> NativeToolResult {
    let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
        return NativeToolResult {
            output: "Missing required parameter: path".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    };
    let path = resolve_tool_path(cwd, raw_path);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return NativeToolResult {
                output: format!("Failed to read {}: {e}", path.display()),
                is_error: true,
                diff: None,
                details: None,
            };
        }
    };

    let all_lines: Vec<&str> = content.lines().collect();
    let total_lines = all_lines.len();
    let offset_1 = args
        .get("offset")
        .and_then(Value::as_u64)
        .map(|n| n.max(1) as usize)
        .unwrap_or(1);
    let start_idx = (offset_1 - 1).min(total_lines);
    let max_lines = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| (n as usize).min(MAX_READ_LINES))
        .unwrap_or(MAX_READ_LINES);

    let mut out = String::new();
    let mut lines_taken = 0usize;
    let mut truncated_by_bytes = false;

    for line in all_lines.iter().skip(start_idx).take(max_lines) {
        if out.len() + line.len() + 1 > MAX_OUTPUT_BYTES && lines_taken > 0 {
            truncated_by_bytes = true;
            break;
        }
        if lines_taken > 0 {
            out.push('\n');
        }
        out.push_str(line);
        lines_taken += 1;
    }

    let remaining = total_lines.saturating_sub(start_idx + lines_taken);
    if remaining > 0 || truncated_by_bytes {
        let next_offset = start_idx + lines_taken + 1;
        out.push_str(&format!(
            "\n\n[{remaining} more lines in file. Use offset={next_offset} to continue.]"
        ));
    }

    NativeToolResult {
        output: out,
        is_error: false,
        diff: None,
        details: None,
    }
}

/// Executes `write` atomically, creating parent directories and generating a unified diff.
pub fn execute_write_tool(cwd: &Path, args: &Value) -> NativeToolResult {
    let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
        return NativeToolResult {
            output: "Missing required parameter: path".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    };
    let Some(content) = args.get("content").and_then(Value::as_str) else {
        return NativeToolResult {
            output: "Missing required parameter: content".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    };
    let path = resolve_tool_path(cwd, raw_path);
    let old_content = std::fs::read_to_string(&path).unwrap_or_default();

    if let Err(e) = crate::client::local_store::write_file_atomic(&path, content.as_bytes()) {
        return NativeToolResult {
            output: format!("Failed to write {}: {e}", path.display()),
            is_error: true,
            diff: None,
            details: None,
        };
    }

    let diff = build_unified_diff(raw_path, &old_content, content);
    NativeToolResult {
        output: format!("Successfully wrote to {raw_path}"),
        is_error: false,
        diff: Some(diff.clone()),
        details: Some(json!({ "diff": diff })),
    }
}

/// Executes `edit` with single `(oldText, newText)` or batch `edits[]` replacements.
pub fn execute_edit_tool(cwd: &Path, args: &Value) -> NativeToolResult {
    let Some(raw_path) = args.get("path").and_then(Value::as_str) else {
        return NativeToolResult {
            output: "Missing required parameter: path".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    };
    let path = resolve_tool_path(cwd, raw_path);
    let original = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return NativeToolResult {
                output: format!("Failed to read {}: {e}", path.display()),
                is_error: true,
                diff: None,
                details: None,
            };
        }
    };

    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(arr) = args.get("edits").and_then(Value::as_array) {
        for item in arr {
            if let (Some(o), Some(n)) = (
                item.get("oldText").and_then(Value::as_str),
                item.get("newText").and_then(Value::as_str),
            ) {
                pairs.push((o.to_string(), n.to_string()));
            }
        }
    } else if let (Some(o), Some(n)) = (
        args.get("oldText").and_then(Value::as_str),
        args.get("newText").and_then(Value::as_str),
    ) {
        pairs.push((o.to_string(), n.to_string()));
    }

    if pairs.is_empty() {
        return NativeToolResult {
            output: "No edits specified".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    }

    let mut updated = original.clone();
    for (old_text, new_text) in &pairs {
        let count = updated.matches(old_text.as_str()).count();
        if count == 0 {
            return NativeToolResult {
                output: format!("oldText not found in {raw_path}"),
                is_error: true,
                diff: None,
                details: None,
            };
        }
        if count > 1 {
            return NativeToolResult {
                output: format!("oldText matched {count} locations in {raw_path}; must be unique"),
                is_error: true,
                diff: None,
                details: None,
            };
        }
        updated = updated.replacen(old_text.as_str(), new_text.as_str(), 1);
    }

    if let Err(e) = crate::client::local_store::write_file_atomic(&path, updated.as_bytes()) {
        return NativeToolResult {
            output: format!("Failed to write {}: {e}", path.display()),
            is_error: true,
            diff: None,
            details: None,
        };
    }

    let diff = build_unified_diff(raw_path, &original, &updated);
    NativeToolResult {
        output: format!(
            "Successfully replaced {} block(s) in {raw_path}.",
            pairs.len()
        ),
        is_error: false,
        diff: Some(diff.clone()),
        details: Some(json!({ "diff": diff })),
    }
}

/// Builds a compact unified diff header + hunks compatible with `gpui-component`'s `DiffFile::parse`.
pub fn build_unified_diff(display_path: &str, old_text: &str, new_text: &str) -> String {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let mut prefix = 0usize;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (old_lines.len().saturating_sub(prefix))
        && suffix < (new_lines.len().saturating_sub(prefix))
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let ctx_start = prefix.saturating_sub(3);
    let old_end = old_lines.len().saturating_sub(suffix);
    let new_end = new_lines.len().saturating_sub(suffix);
    let old_ctx_end = (old_end + 3).min(old_lines.len());
    let new_ctx_end = (new_end + 3).min(new_lines.len());

    let old_count = old_ctx_end.saturating_sub(ctx_start);
    let new_count = new_ctx_end.saturating_sub(ctx_start);

    let mut out = format!(
        "--- a/{display_path}\n+++ b/{display_path}\n@@ -{},{} +{},{} @@\n",
        ctx_start + 1,
        old_count,
        ctx_start + 1,
        new_count
    );

    for line in &old_lines[ctx_start..prefix] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }
    for line in &old_lines[prefix..old_end] {
        out.push('-');
        out.push_str(line);
        out.push('\n');
    }
    for line in &new_lines[prefix..new_end] {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    for line in &new_lines[new_end..new_ctx_end] {
        out.push(' ');
        out.push_str(line);
        out.push('\n');
    }

    out
}

/// Executes `bash` asynchronously via `smol::process::Command` with output tail truncation.
pub async fn execute_bash_tool(cwd: &Path, args: &Value) -> NativeToolResult {
    let Some(command) = args.get("command").and_then(Value::as_str) else {
        return NativeToolResult {
            output: "Missing required parameter: command".to_string(),
            is_error: true,
            diff: None,
            details: None,
        };
    };
    let timeout_secs = args.get("timeout").and_then(Value::as_u64).unwrap_or(120);

    let shell = if cfg!(windows) { "cmd" } else { "bash" };
    let flag = if cfg!(windows) { "/C" } else { "-c" };

    let mut cmd = smol::process::Command::new(shell);
    cmd.arg(flag)
        .arg(command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use smol::process::windows::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }

    let fut = cmd.output();
    let timed = futures::future::select(
        Box::pin(fut),
        Box::pin(smol::Timer::after(Duration::from_secs(timeout_secs))),
    )
    .await;

    match timed {
        futures::future::Either::Left((Ok(out), _)) => {
            let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                if !combined.is_empty() && !combined.ends_with('\n') {
                    combined.push('\n');
                }
                combined.push_str(&stderr);
            }
            if combined.len() > MAX_OUTPUT_BYTES {
                let start = combined.len() - MAX_OUTPUT_BYTES;
                let safe_start = combined
                    .char_indices()
                    .map(|(i, _)| i)
                    .find(|&i| i >= start)
                    .unwrap_or(0);
                combined = format!(
                    "[... truncated leading bytes ...]\n{}",
                    &combined[safe_start..]
                );
            }
            let code = out.status.code().unwrap_or(-1);
            let is_error = !out.status.success();
            if is_error {
                combined.push_str(&format!("\n\nCommand exited with code {code}"));
            }
            NativeToolResult {
                output: combined,
                is_error,
                diff: None,
                details: Some(json!({ "exitCode": code })),
            }
        }
        futures::future::Either::Left((Err(e), _)) => NativeToolResult {
            output: format!("Failed to spawn shell command: {e}"),
            is_error: true,
            diff: None,
            details: None,
        },
        futures::future::Either::Right(_) => NativeToolResult {
            output: format!("Command timed out after {timeout_secs} seconds"),
            is_error: true,
            diff: None,
            details: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_write_edit_and_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("pi-gpui-tools-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let w = execute_write_tool(
            &dir,
            &json!({
                "path": "hello.txt",
                "content": "line 1\nline 2\nline 3\n"
            }),
        );
        assert!(!w.is_error);
        assert!(w.diff.as_deref().unwrap_or("").contains("+line 2"));

        let e = execute_edit_tool(
            &dir,
            &json!({
                "path": "hello.txt",
                "oldText": "line 2",
                "newText": "line TWO"
            }),
        );
        assert!(!e.is_error);
        assert!(e.diff.as_deref().unwrap_or("").contains("-line 2"));
        assert!(e.diff.as_deref().unwrap_or("").contains("+line TWO"));

        let r = execute_read_tool(&dir, &json!({ "path": "hello.txt" }));
        assert!(!r.is_error);
        assert_eq!(r.output, "line 1\nline TWO\nline 3");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
