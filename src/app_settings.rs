//! JSON configuration for the Pi Agent desktop client.
//!
//! Every client-side preference lives in one file so it can be inspected,
//! versioned, or copied between machines:
//!
//! `~/.pi/agent/gpui-settings.json`
//!
//! The file is the single source of truth: `load_app_settings()` reads it at
//! startup, every setter writes it back, and the earlier
//! `gpui-appearance.json` is imported once when the new file does not exist yet.

use crate::client::native_index::user_home;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE_NAME: &str = "gpui-settings.json";
const LEGACY_FILE_NAME: &str = "gpui-appearance.json";

fn default_send_shortcut() -> String {
    "enter".to_string()
}
fn default_user_collapse_chars() -> usize {
    300
}
fn default_true() -> bool {
    true
}
fn default_diff_view_mode() -> String {
    "unified".to_string()
}
fn default_core_mode() -> String {
    "auto".to_string()
}
fn default_font_size() -> String {
    "medium".to_string()
}
fn default_locale() -> String {
    "en".to_string()
}
fn default_light_theme() -> String {
    "Ayu Light".to_string()
}
fn default_dark_theme() -> String {
    "Catppuccin Mocha".to_string()
}
fn default_probe_interval() -> u64 {
    1500
}

/// Theme and typography block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    #[serde(default = "default_true")]
    pub dark_mode: bool,
    #[serde(default = "default_light_theme")]
    pub light_theme: String,
    #[serde(default = "default_dark_theme")]
    pub dark_theme: String,
    #[serde(default = "default_font_size")]
    pub chat_font_size: String,
    #[serde(default = "default_locale")]
    pub locale: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            light_theme: default_light_theme(),
            dark_theme: default_dark_theme(),
            chat_font_size: default_font_size(),
            locale: default_locale(),
        }
    }
}

/// Composer and transcript behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    #[serde(default = "default_send_shortcut")]
    pub send_shortcut: String,
    /// Character count above which a long user message starts collapsed; 0 disables it.
    #[serde(default = "default_user_collapse_chars")]
    pub user_collapse_chars: usize,
    #[serde(default)]
    pub thinking_expanded_default: bool,
    #[serde(default = "default_diff_view_mode")]
    pub diff_view_mode: String,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            send_shortcut: default_send_shortcut(),
            user_collapse_chars: default_user_collapse_chars(),
            thinking_expanded_default: false,
            diff_view_mode: default_diff_view_mode(),
        }
    }
}

/// Desktop notification behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: true,
        }
    }
}

/// Which core (HTTP API) the client talks to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSettings {
    /// `auto` probes the documented local addresses, `manual` uses `url`.
    #[serde(default = "default_core_mode")]
    pub mode: String,
    #[serde(default)]
    pub url: String,
    #[serde(default = "default_probe_interval")]
    pub probe_interval_ms: u64,
}

impl Default for CoreSettings {
    fn default() -> Self {
        Self {
            mode: default_core_mode(),
            url: String::new(),
            probe_interval_ms: default_probe_interval(),
        }
    }
}

/// Saved main-window geometry (size, position, maximized flag).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WindowSettings {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

/// The whole client configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Bumped when a future version needs to migrate fields.
    #[serde(default = "current_version")]
    pub version: u32,
    pub appearance: AppearanceSettings,
    pub chat: ChatSettings,
    pub notifications: NotificationSettings,
    pub core: CoreSettings,
    #[serde(default)]
    pub window: Option<WindowSettings>,
}

fn current_version() -> u32 {
    1
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: current_version(),
            appearance: AppearanceSettings::default(),
            chat: ChatSettings::default(),
            notifications: NotificationSettings::default(),
            core: CoreSettings::default(),
            window: None,
        }
    }
}

fn settings_dir() -> PathBuf {
    user_home().join(".pi").join("agent")
}

pub fn settings_path() -> PathBuf {
    settings_dir().join(SETTINGS_FILE_NAME)
}

fn legacy_path() -> PathBuf {
    settings_dir().join(LEGACY_FILE_NAME)
}

/// Shape of the pre-JSON-file `gpui-appearance.json`.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct LegacyPrefs {
    dark_mode: Option<bool>,
    light_theme: Option<String>,
    dark_theme: Option<String>,
    chat_font_size: Option<String>,
    locale: Option<String>,
    send_shortcut: Option<String>,
    user_collapse_chars: Option<usize>,
    notify_enabled: Option<bool>,
    notify_sound: Option<bool>,
    thinking_expanded_default: Option<bool>,
    diff_view_mode: Option<String>,
    core_mode: Option<String>,
    core_url: Option<String>,
}

fn import_legacy(path: &Path) -> AppSettings {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return AppSettings::default();
    };
    let Ok(legacy) = serde_json::from_str::<LegacyPrefs>(&raw) else {
        return AppSettings::default();
    };
    let mut settings = AppSettings::default();
    if let Some(v) = legacy.dark_mode {
        settings.appearance.dark_mode = v;
    }
    if let Some(v) = legacy.light_theme {
        settings.appearance.light_theme = v;
    }
    if let Some(v) = legacy.dark_theme {
        settings.appearance.dark_theme = v;
    }
    if let Some(v) = legacy.chat_font_size {
        settings.appearance.chat_font_size = v;
    }
    if let Some(v) = legacy.locale {
        settings.appearance.locale = v;
    }
    if let Some(v) = legacy.send_shortcut {
        settings.chat.send_shortcut = v;
    }
    if let Some(v) = legacy.user_collapse_chars {
        settings.chat.user_collapse_chars = v;
    }
    if let Some(v) = legacy.thinking_expanded_default {
        settings.chat.thinking_expanded_default = v;
    }
    if let Some(v) = legacy.diff_view_mode {
        settings.chat.diff_view_mode = v;
    }
    if let Some(v) = legacy.notify_enabled {
        settings.notifications.enabled = v;
    }
    if let Some(v) = legacy.notify_sound {
        settings.notifications.sound = v;
    }
    if let Some(v) = legacy.core_mode {
        settings.core.mode = v;
    }
    if let Some(v) = legacy.core_url {
        settings.core.url = v;
    }
    settings
}

/// Reads the configuration file. A missing file is imported from the legacy
/// appearance file once (and written out), so the JSON file becomes the source
/// of truth from then on.
pub fn load_app_settings() -> AppSettings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<AppSettings>(&raw) {
            Ok(mut parsed) => {
                if parsed.version == 0 {
                    parsed.version = current_version();
                }
                parsed
            }
            Err(_) => AppSettings::default(),
        },
        Err(_) => {
            let imported = import_legacy(&legacy_path());
            save_app_settings(&imported);
            imported
        }
    }
}

pub fn save_app_settings(settings: &AppSettings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, format!("{json}\n"));
    }
}

/// Reads the file, applies `f`, and writes it back — used by single-setting
/// setters so unrelated keys survive untouched.
pub fn update_app_settings(f: impl FnOnce(&mut AppSettings)) {
    let mut settings = load_app_settings();
    f(&mut settings);
    save_app_settings(&settings);
}
