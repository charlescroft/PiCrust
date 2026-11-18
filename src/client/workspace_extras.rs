use crate::client::types::WorkspaceKind;
use crate::client::local_store::{is_chat_workspace_cwd, pi_agent_dir, write_file_atomic};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// 1. Notes (`~/.pi/agent/picrust/notes/*.md` & `<cwd>/.picrust/notes/*.md`)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteScope {
    Global,
    Project,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEntry {
    pub name: String,
    pub path: PathBuf,
    pub modified_ms: i64,
}

pub fn sanitize_note_filename(title: &str) -> String {
    let trimmed = title.trim();
    let without_ext = trimmed.strip_suffix(".md").unwrap_or(trimmed);
    let mut cleaned = String::with_capacity(without_ext.len());
    for ch in without_ext.chars() {
        if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control() {
            continue;
        }
        if ch.is_whitespace() {
            if !cleaned.ends_with('-') && !cleaned.is_empty() {
                cleaned.push('-');
            }
        } else {
            cleaned.push(ch);
        }
    }
    let cleaned = cleaned.trim_matches('-');
    if cleaned.is_empty() {
        "untitled.md".to_string()
    } else {
        format!("{cleaned}.md")
    }
}

pub fn project_notes_available(workspace_kind: WorkspaceKind, cwd: &Path) -> bool {
    workspace_kind == WorkspaceKind::Project && !is_chat_workspace_cwd(cwd)
}

pub fn notes_dir(scope: NoteScope, cwd: &Path, workspace_kind: WorkspaceKind) -> Option<PathBuf> {
    match scope {
        NoteScope::Global => Some(pi_agent_dir().join("picrust").join("notes")),
        NoteScope::Project => {
            if project_notes_available(workspace_kind, cwd) {
                Some(cwd.join(".picrust").join("notes"))
            } else {
                None
            }
        }
    }
}

pub fn list_notes(scope: NoteScope, cwd: &Path, workspace_kind: WorkspaceKind) -> Vec<NoteEntry> {
    let Some(dir) = notes_dir(scope, cwd, workspace_kind) else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        let modified_ms = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        out.push(NoteEntry {
            name,
            path,
            modified_ms,
        });
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn read_note(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

pub fn save_note(path: &Path, content: &str) -> Result<(), String> {
    write_file_atomic(path, content.as_bytes())
}

pub fn delete_note(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================================
// 2. Credentials (`~/.pi/agent/picrust/credentials/`)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub auto_discover: bool,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CredentialsIndex {
    #[serde(default)]
    items: Vec<CredentialMeta>,
}

pub fn credentials_dir() -> PathBuf {
    pi_agent_dir().join("picrust").join("credentials")
}

pub fn credential_secret_path(name: &str) -> PathBuf {
    credentials_dir().join("secrets").join(format!("{name}.txt"))
}

pub fn is_valid_credential_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn read_credentials_index() -> CredentialsIndex {
    let path = credentials_dir().join("index.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return CredentialsIndex::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_credentials_index(idx: &CredentialsIndex) -> Result<(), String> {
    let path = credentials_dir().join("index.json");
    let bytes = serde_json::to_vec_pretty(idx).map_err(|e| e.to_string())?;
    write_file_atomic(&path, &bytes)
}

pub fn list_credentials() -> Vec<CredentialMeta> {
    let mut items = read_credentials_index().items;
    items.sort_by(|a, b| a.name.cmp(&b.name));
    items
}

pub fn read_credential_secret(name: &str) -> Result<String, String> {
    if !is_valid_credential_name(name) {
        return Err("Invalid credential name".to_string());
    }
    let path = credential_secret_path(name);
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(raw.trim_matches(|c| c == '\r' || c == '\n').to_string())
}

pub fn save_credential(
    name: &str,
    description: &str,
    secret: &str,
    auto_discover: bool,
) -> Result<CredentialMeta, String> {
    let name = name.trim();
    if !is_valid_credential_name(name) {
        return Err("Name must match ^[A-Za-z_][A-Za-z0-9_]*$".to_string());
    }
    let single_line_secret = secret
        .replace('\r', "")
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let secret_path = credential_secret_path(name);
    write_file_atomic(&secret_path, single_line_secret.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&secret_path, fs::Permissions::from_mode(0o600));
    }

    let mut idx = read_credentials_index();
    let now_ms = Utc::now().timestamp_millis();
    let meta = CredentialMeta {
        name: name.to_string(),
        description: description.trim().to_string(),
        auto_discover,
        updated_at: now_ms,
    };
    if let Some(existing) = idx.items.iter_mut().find(|i| i.name == name) {
        *existing = meta.clone();
    } else {
        idx.items.push(meta.clone());
    }
    write_credentials_index(&idx)?;
    Ok(meta)
}

pub fn delete_credential(name: &str) -> Result<(), String> {
    if !is_valid_credential_name(name) {
        return Err("Invalid credential name".to_string());
    }
    let secret_path = credential_secret_path(name);
    if secret_path.exists() {
        fs::remove_file(&secret_path).map_err(|e| e.to_string())?;
    }
    let mut idx = read_credentials_index();
    idx.items.retain(|i| i.name != name);
    write_credentials_index(&idx)?;
    Ok(())
}

pub fn format_credential_mention(meta: &CredentialMeta) -> String {
    let path = credential_secret_path(&meta.name);
    let desc = if meta.description.is_empty() {
        String::new()
    } else {
        format!("（{}）", meta.description)
    };
    format!(
        "[凭证 {}{}：单行文件位于 `{}`，需要时请用 read 工具读取或通过命令读取，切勿在回复中输出明文]",
        meta.name,
        desc,
        path.display()
    )
}

pub fn auto_discover_prompt_block() -> Option<String> {
    let items: Vec<CredentialMeta> = list_credentials()
        .into_iter()
        .filter(|c| c.auto_discover)
        .collect();
    if items.is_empty() {
        return None;
    }
    let mut lines = vec![
        "[Available Credentials Index — never echo secret values in replies; read the single-line file via `read` tool only when needed:]".to_string(),
    ];
    for item in items {
        let path = credential_secret_path(&item.name);
        if item.description.is_empty() {
            lines.push(format!("- {}: `{}`", item.name, path.display()));
        } else {
            lines.push(format!(
                "- {} ({}): `{}`",
                item.name,
                item.description,
                path.display()
            ));
        }
    }
    Some(lines.join("\n"))
}

// ============================================================================
// 3. Workflows (`~/.pi/agent/prompts/*.md` & `<cwd>/.pi/prompts/*.md`)
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowScope {
    Global,
    Project,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowStep {
    pub title: String,
    pub instruction: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowDef {
    pub name: String,
    pub scope: WorkflowScope,
    pub path: PathBuf,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub raw_content: String,
    pub has_args: bool,
}

pub fn workflows_dir(
    scope: WorkflowScope,
    cwd: &Path,
    workspace_kind: WorkspaceKind,
) -> Option<PathBuf> {
    match scope {
        WorkflowScope::Global => Some(pi_agent_dir().join("prompts")),
        WorkflowScope::Project => {
            if project_notes_available(workspace_kind, cwd) {
                Some(cwd.join(".pi").join("prompts"))
            } else {
                None
            }
        }
    }
}

pub fn sanitize_workflow_name(name: &str) -> String {
    let trimmed = name.trim().strip_suffix(".md").unwrap_or(name.trim());
    let mut cleaned = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            cleaned.push(ch);
        } else if ch.is_whitespace() && !cleaned.ends_with('-') && !cleaned.is_empty() {
            cleaned.push('-');
        }
    }
    let cleaned = cleaned.trim_matches('-');
    if cleaned.is_empty() {
        "workflow".to_string()
    } else {
        cleaned.to_string()
    }
}

pub fn parse_workflow_md(
    name: String,
    scope: WorkflowScope,
    path: PathBuf,
    content: &str,
) -> WorkflowDef {
    let mut description = String::new();
    let mut body = content;

    if let Some(rest) = content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n")) {
        if let Some(end_idx) = rest.find("\n---") {
            let fm = &rest[..end_idx];
            for line in fm.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    if k.trim() == "description" {
                        description = v
                            .trim()
                            .trim_matches(|c| c == '"' || c == '\'')
                            .to_string();
                    }
                }
            }
            let after_fm = &rest[end_idx + 4..];
            body = after_fm.trim_start_matches(['\r', '\n']);
        }
    }

    let mut steps: Vec<WorkflowStep> = Vec::new();
    let mut cur_title: Option<String> = None;
    let mut cur_lines: Vec<String> = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(title) = parse_step_heading(trimmed) {
            if let Some(prev_title) = cur_title.take() {
                steps.push(WorkflowStep {
                    title: prev_title,
                    instruction: cur_lines.join("\n").trim().to_string(),
                });
                cur_lines.clear();
            }
            cur_title = Some(title);
        } else if cur_title.is_some() {
            cur_lines.push(line.to_string());
        }
    }
    if let Some(prev_title) = cur_title.take() {
        steps.push(WorkflowStep {
            title: prev_title,
            instruction: cur_lines.join("\n").trim().to_string(),
        });
    }

    // Fallback: parse numbered list if no `## Step N:` headings were found
    if steps.is_empty() {
        for line in body.lines() {
            let trimmed = line.trim();
            if let Some(rest) = strip_numbered_prefix(trimmed) {
                steps.push(WorkflowStep {
                    title: rest.to_string(),
                    instruction: rest.to_string(),
                });
            }
        }
    }

    let has_args = body.contains("$ARGUMENTS") || body.contains("$1") || body.contains("$2");

    WorkflowDef {
        name,
        scope,
        path,
        description,
        steps,
        raw_content: content.to_string(),
        has_args,
    }
}

fn parse_step_heading(line: &str) -> Option<String> {
    let rest = line.strip_prefix("## ")?;
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("step ") || rest.starts_with("步骤") {
        if let Some((_, title)) = rest.split_once(':').or_else(|| rest.split_once('：')) {
            let t = title.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        return Some(rest.trim().to_string());
    }
    None
}

fn strip_numbered_prefix(line: &str) -> Option<&str> {
    let mut digits = 0;
    for b in line.bytes() {
        if b.is_ascii_digit() {
            digits += 1;
        } else if b == b'.' && digits > 0 {
            let rest = line[digits + 1..].trim();
            return (!rest.is_empty()).then_some(rest);
        } else {
            return None;
        }
    }
    None
}

pub fn serialize_workflow_md(description: &str, steps: &[WorkflowStep]) -> String {
    let mut out = String::new();
    let desc = description.trim();
    if !desc.is_empty() {
        out.push_str("---\ndescription: ");
        out.push_str(desc);
        out.push_str("\n---\n");
    }
    for (idx, step) in steps.iter().enumerate() {
        if idx > 0 || !out.is_empty() {
            if !out.ends_with("\n\n") && !out.is_empty() {
                out.push('\n');
            }
        }
        out.push_str(&format!("## Step {}: {}\n", idx + 1, step.title.trim()));
        let inst = step.instruction.trim();
        if !inst.is_empty() {
            out.push_str(inst);
            out.push('\n');
        }
    }
    out
}

pub fn list_workflows(
    scope: WorkflowScope,
    cwd: &Path,
    workspace_kind: WorkspaceKind,
) -> Vec<WorkflowDef> {
    let Some(dir) = workflows_dir(scope, cwd, workspace_kind) else {
        return Vec::new();
    };
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workflow")
            .to_string();
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        out.push(parse_workflow_md(name, scope, path, &raw));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn save_workflow(
    scope: WorkflowScope,
    cwd: &Path,
    workspace_kind: WorkspaceKind,
    name: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let dir = workflows_dir(scope, cwd, workspace_kind)
        .ok_or_else(|| "Project workspace is not active".to_string())?;
    let safe = sanitize_workflow_name(name);
    let path = dir.join(format!("{safe}.md"));
    write_file_atomic(&path, content.as_bytes())?;
    Ok(path)
}

pub fn delete_workflow(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn build_workflow_execution_prompt(def: &WorkflowDef, args: &str) -> String {
    let args_trimmed = args.trim();
    let arg_parts: Vec<&str> = if args_trimmed.is_empty() {
        Vec::new()
    } else {
        args_trimmed.split_whitespace().collect()
    };

    let substitute = |input: &str| -> String {
        let mut s = input.replace("$ARGUMENTS", args_trimmed);
        for idx in (1..=9).rev() {
            let placeholder = format!("${idx}");
            let val = arg_parts.get(idx - 1).copied().unwrap_or("");
            s = s.replace(&placeholder, val);
        }
        s
    };

    if def.steps.is_empty() {
        return substitute(&def.raw_content);
    }

    let mut out = String::new();
    out.push_str(&format!("请严格按照工作流 `{}` 执行以下任务：\n", def.name));
    if !def.description.is_empty() {
        out.push_str(&format!("目标：{}\n", substitute(&def.description)));
    }
    out.push_str(
        "\n【强制执行协议】\n\
         1. 第一步必须先调用 `todo` 工具，将下面所有 Step 1 ~ Step N 写入任务清单；\n\
         2. 严格按顺序逐个执行：必须完成并验证当前 Step 后，更新 `todo` 状态，再开始下一步；严禁跳步、合并未完成步骤或跳过失败步骤；\n\
         3. 任意一步失败时立刻停止后续步骤，并明确汇报卡在哪一步及失败原因。\n\n",
    );
    for (idx, step) in def.steps.iter().enumerate() {
        out.push_str(&format!("## Step {}: {}\n", idx + 1, substitute(&step.title)));
        if !step.instruction.is_empty() {
            out.push_str(&substitute(&step.instruction));
            out.push('\n');
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_note_filenames_safely() {
        assert_eq!(sanitize_note_filename("架构 备忘/v1:test"), "架构-备忘v1test.md");
        assert_eq!(sanitize_note_filename("notes.md"), "notes.md");
        assert_eq!(sanitize_note_filename("///"), "untitled.md");
    }

    #[test]
    fn validates_credential_names() {
        assert!(is_valid_credential_name("STEAMDECK_PASS"));
        assert!(is_valid_credential_name("_TOKEN_1"));
        assert!(!is_valid_credential_name("123_BAD"));
        assert!(!is_valid_credential_name("BAD-NAME"));
        assert!(!is_valid_credential_name(""));
    }

    #[test]
    fn workflow_markdown_roundtrip_and_prompt_wrapper() {
        let steps = vec![
            WorkflowStep {
                title: "运行单元测试".to_string(),
                instruction: "执行 `cargo test`".to_string(),
            },
            WorkflowStep {
                title: "发布 ($1)".to_string(),
                instruction: "推送 $ARGUMENTS".to_string(),
            },
        ];
        let md = serialize_workflow_md("发布前检查", &steps);
        let parsed = parse_workflow_md(
            "release-check".to_string(),
            WorkflowScope::Global,
            PathBuf::from("/tmp/release-check.md"),
            &md,
        );
        assert_eq!(parsed.description, "发布前检查");
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].title, "运行单元测试");
        assert!(parsed.has_args);

        let prompt = build_workflow_execution_prompt(&parsed, "v1.4.0 --dry-run");
        assert!(prompt.contains("todo"));
        assert!(prompt.contains("## Step 2: 发布 (v1.4.0)"));
        assert!(prompt.contains("推送 v1.4.0 --dry-run"));
    }
}
