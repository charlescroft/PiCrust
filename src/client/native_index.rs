use crate::client::types::{SessionSummary, WorkspaceKind};
use ignore::WalkBuilder;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Respects `PI_HOME` / `HOME` / `USERPROFILE` rather than hardcoding a single platform API.
pub fn user_home() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.is_empty() {
            return PathBuf::from(profile);
        }
    }
    PathBuf::from(".")
}

pub fn pi_sessions_root() -> PathBuf {
    if let Ok(dir) = std::env::var("PI_SESSIONS_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    user_home().join(".pi").join("agent").join("sessions")
}

#[derive(Debug, Clone)]
pub struct FileMatch {
    pub relative_path: String,
    pub score: u32,
}

/// Phase 3 native `@file` fuzzy search powered by `ignore` (gitignored-aware walk)
/// + `nucleo-matcher` (zero-allocation fuzzy scorer).
pub fn fuzzy_search_files(root: &Path, query: &str, limit: usize) -> Vec<FileMatch> {
    if !root.exists() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(8))
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | ".next" | ".next-desktop" | "dist"
            )
        })
        .build();

    for entry in walker.flatten() {
        if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                paths.push(rel.to_string_lossy().replace('\\', "/"));
                if paths.len() >= 25_000 {
                    break;
                }
            }
        }
    }

    if query.trim().is_empty() {
        return paths
            .into_iter()
            .take(limit)
            .map(|relative_path| FileMatch {
                relative_path,
                score: 0,
            })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<FileMatch> = paths
        .into_iter()
        .filter_map(|rel| {
            let haystack = Utf32Str::new(&rel, &mut buf);
            pattern
                .score(haystack, &mut matcher)
                .map(|score| FileMatch {
                    relative_path: rel,
                    score,
                })
        })
        .collect();

    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.relative_path.cmp(&b.relative_path)));
    scored.truncate(limit);
    scored
}

/// Phase 3 native JSONL session scanner: reads `~/.pi/agent/sessions/<encoded-cwd>/*.jsonl`
/// without waiting on Node.js.
pub fn scan_local_sessions(root: &Path) -> Vec<SessionSummary> {
    if !root.exists() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let Ok(project_dirs) = fs::read_dir(root) else {
        return results;
    };

    for dir_entry in project_dirs.flatten() {
        let dir_path = dir_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&dir_path) else {
            continue;
        };
        for file_entry in files.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some(summary) = parse_session_jsonl_summary(&file_path) {
                results.push(summary);
            }
        }
    }

    results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    results
}

pub fn parse_session_jsonl_summary(path: &Path) -> Option<SessionSummary> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut created_at: Option<String> = None;
    let mut updated_at: Option<String> = None;
    let mut parent_session: Option<String> = None;
    let mut name: Option<String> = None;
    let mut first_message: Option<String> = None;
    let mut message_count: usize = 0;
    let mut is_subagent = false;
    let mut subagent_parent_id: Option<String> = None;

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let entry_type = val.get("type").and_then(Value::as_str).unwrap_or("");
        if let Some(ts) = val.get("timestamp").and_then(Value::as_str) {
            if created_at.is_none() {
                created_at = Some(ts.to_string());
            }
            updated_at = Some(ts.to_string());
        }

        match entry_type {
            "session" => {
                id = val.get("id").and_then(Value::as_str).map(String::from);
                cwd = val.get("cwd").and_then(Value::as_str).map(String::from);
                parent_session = val
                    .get("parentSession")
                    .and_then(Value::as_str)
                    .map(String::from);
            }
            "session_info" => {
                if let Some(n) = val.get("name").and_then(Value::as_str) {
                    name = Some(n.to_string());
                }
            }
            "custom" => {
                if val.get("customType").and_then(Value::as_str) == Some("pi-web:subagent") {
                    is_subagent = true;
                    if subagent_parent_id.is_none() {
                        subagent_parent_id = val
                            .get("data")
                            .and_then(|d| d.get("parentSessionId"))
                            .and_then(Value::as_str)
                            .map(String::from);
                    }
                }
            }
            "message" => {
                message_count += 1;
                if first_message.is_none() {
                    if let Some(msg) = val.get("message") {
                        if msg.get("role").and_then(Value::as_str) == Some("user") {
                            first_message = extract_text_content(msg.get("content"));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let id = id.or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(String::from)
    })?;

    let project_name = cwd.as_deref().and_then(|c| {
        Path::new(c)
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from)
    });

    let workspace_kind = cwd.as_deref().map(|c| {
        if c.contains("chat-workspace") || c.contains("chats/") {
            WorkspaceKind::Chat
        } else {
            WorkspaceKind::Project
        }
    });

    Some(SessionSummary {
        id,
        name,
        first_message,
        cwd,
        project_name,
        updated_at,
        created_at,
        parent_session,
        message_count,
        is_running: false,
        awaiting_input: false,
        pinned: false,
        archived: false,
        workspace_kind,
        relation: if is_subagent {
            Some(crate::client::types::SessionRelation {
                kind: Some("subagent".to_string()),
                parent_session_id: subagent_parent_id,
            })
        } else {
            None
        },
        path: Some(path.to_string_lossy().into_owned()),
    })
}

fn extract_text_content(content: Option<&Value>) -> Option<String> {
    let content = content?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        for item in arr {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(t) = item.get("text").and_then(Value::as_str) {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// Fast metadata-only signature for `~/.pi/agent/sessions/` so background polling
/// avoids re-opening and JSON-parsing every session file when nothing changed on disk.
pub fn sessions_dir_signature(root: &Path) -> (usize, u64) {
    let Ok(project_dirs) = fs::read_dir(root) else {
        return (0, 0);
    };
    let mut count = 0usize;
    let mut acc = 0u64;
    for dir_entry in project_dirs.flatten() {
        let dir_path = dir_entry.path();
        if !dir_path.is_dir() {
            continue;
        }
        let Ok(files) = fs::read_dir(&dir_path) else {
            continue;
        };
        for file_entry in files.flatten() {
            let file_path = file_entry.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            count += 1;
            if let Ok(meta) = file_entry.metadata() {
                acc = acc.wrapping_add(meta.len());
                if let Ok(mt) = meta.modified() {
                    if let Ok(d) = mt.duration_since(std::time::UNIX_EPOCH) {
                        acc = acc.wrapping_add(d.as_secs());
                    }
                }
            }
        }
    }
    (count, acc)
}

/// Fast local scanner across recent session files to extract artifact records.
pub fn scan_all_local_artifacts(sessions: &[SessionSummary]) -> Vec<crate::client::types::ArtifactRecord> {
    use std::io::BufRead as _;
    let mut all_artifacts: Vec<crate::client::types::ArtifactRecord> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut buf = Vec::with_capacity(8192);

    for sess in sessions.iter().take(30) {
        let Some(ref path_str) = sess.path else { continue; };
        let path = std::path::Path::new(path_str);
        let Ok(file) = std::fs::File::open(path) else { continue; };
        let mut reader = std::io::BufReader::new(file);
        loop {
            buf.clear();
            let Ok(n) = reader.read_until(b'\n', &mut buf) else { break; };
            if n == 0 {
                break;
            }
            if buf.windows(11).any(|w| w == b":::artifact") {
                let line = String::from_utf8_lossy(&buf);
                let sid = sess.id.as_str();
                let arts = crate::client::session_controller::extract_artifacts_from_markdown(&line, Some(sid));
                for mut art in arts {
                    if let Some(existing) = all_artifacts.iter_mut().find(|a| a.identifier == art.identifier) {
                        existing.version += 1;
                        existing.previous_content = Some(existing.content.clone());
                        existing.content = art.content;
                        existing.updated_at = sess.updated_at.clone();
                    } else if seen_ids.insert(art.identifier.clone()) {
                        art.updated_at = sess.updated_at.clone();
                        all_artifacts.push(art);
                    }
                }
            }
        }
    }
    all_artifacts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_jsonl_session_summary() {
        let dir = std::env::temp_dir().join(format!("pi-gpui-test-{}", std::process::id()));
        let sess_dir = dir.join("encoded-proj");
        fs::create_dir_all(&sess_dir).unwrap();
        let file_path = sess_dir.join("test.jsonl");
        let mut f = File::create(&file_path).unwrap();
        writeln!(
            f,
            r#"{{"type":"session","version":3,"id":"sess-1","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp/my-app"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"message","id":"m1","parentId":null,"timestamp":"2026-01-01T00:00:01Z","message":{{"role":"user","content":"Hello GPUI"}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"session_info","id":"si1","parentId":"m1","name":"GPUI Migration"}}"#
        )
        .unwrap();

        let scanned = scan_local_sessions(&dir);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].id, "sess-1");
        assert_eq!(scanned[0].name.as_deref(), Some("GPUI Migration"));
        assert_eq!(scanned[0].first_message.as_deref(), Some("Hello GPUI"));
        assert_eq!(scanned[0].project_name.as_deref(), Some("my-app"));
        assert_eq!(scanned[0].message_count, 1);

        let matches = fuzzy_search_files(&dir, "test.js", 10);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].relative_path.ends_with("test.jsonl"));

        let arts = scan_all_local_artifacts(&scanned);
        assert_eq!(arts.len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }
}
