use crate::client::types::{ChatFontSize, SendShortcut};
use crate::i18n::AppLocale;
use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeMode, ThemeSet};
use gpui_kit::{App, SharedString};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::OnceLock;

const THEME_FILES: &[&str] = &[
    include_str!("../themes/adventure.json"),
    include_str!("../themes/alduin.json"),
    include_str!("../themes/asciinema.json"),
    include_str!("../themes/aurora.json"),
    include_str!("../themes/ayu.json"),
    include_str!("../themes/catppuccin.json"),
    include_str!("../themes/everforest.json"),
    include_str!("../themes/fahrenheit.json"),
    include_str!("../themes/flexoki.json"),
    include_str!("../themes/gruvbox.json"),
    include_str!("../themes/harper.json"),
    include_str!("../themes/hybrid.json"),
    include_str!("../themes/jellybeans.json"),
    include_str!("../themes/kibble.json"),
    include_str!("../themes/macos-classic.json"),
    include_str!("../themes/mellifluous.json"),
    include_str!("../themes/molokai.json"),
    include_str!("../themes/solarized.json"),
    include_str!("../themes/spaceduck.json"),
    include_str!("../themes/tokyonight.json"),
    include_str!("../themes/twilight.json"),
];

pub const DEFAULT_LIGHT_NAME: &str = "Default Light";
pub const DEFAULT_DARK_NAME: &str = "Default Dark";

pub fn platform_system_font() -> SharedString {
    #[cfg(target_os = "macos")]
    {
        ".AppleSystemUIFont".into()
    }
    #[cfg(not(target_os = "macos"))]
    {
        ".SystemUIFont".into()
    }
}

pub struct ThemeCatalog {
    pub light: BTreeMap<String, ThemeConfig>,
    pub dark: BTreeMap<String, ThemeConfig>,
}

pub fn catalog() -> &'static ThemeCatalog {
    static CATALOG: OnceLock<ThemeCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut light = BTreeMap::new();
        let mut dark = BTreeMap::new();
        for raw in THEME_FILES {
            if let Ok(set) = serde_json::from_str::<ThemeSet>(raw) {
                for cfg in set.themes {
                    let name = cfg.name.to_string();
                    if cfg.mode.is_dark() {
                        dark.insert(name, cfg);
                    } else {
                        light.insert(name, cfg);
                    }
                }
            }
        }
        ThemeCatalog { light, dark }
    })
}

pub fn light_theme_names() -> Vec<SharedString> {
    let mut names = vec![SharedString::from(DEFAULT_LIGHT_NAME)];
    for k in catalog().light.keys() {
        if k != DEFAULT_LIGHT_NAME {
            names.push(SharedString::from(k.clone()));
        }
    }
    names
}

pub fn dark_theme_names() -> Vec<SharedString> {
    let mut names = vec![SharedString::from(DEFAULT_DARK_NAME)];
    for k in catalog().dark.keys() {
        if k != DEFAULT_DARK_NAME {
            names.push(SharedString::from(k.clone()));
        }
    }
    names
}

/// Legacy in-memory shape kept for callers that read a whole settings snapshot.
#[derive(Debug, Clone)]
pub struct AppearancePrefs {
    pub dark_mode: bool,
    pub light_theme: String,
    pub dark_theme: String,
    pub chat_font_size: String,
    pub locale: String,
    pub send_shortcut: String,
    pub user_collapse_chars: usize,
    pub notify_enabled: bool,
    pub notify_sound: bool,
    pub thinking_expanded_default: bool,
    pub diff_view_mode: String,
    pub core_mode: String,
    pub core_url: String,
}

impl AppearancePrefs {
    fn from_settings(s: &crate::app_settings::AppSettings) -> Self {
        Self {
            dark_mode: s.appearance.dark_mode,
            light_theme: s.appearance.light_theme.clone(),
            dark_theme: s.appearance.dark_theme.clone(),
            chat_font_size: s.appearance.chat_font_size.clone(),
            locale: s.appearance.locale.clone(),
            send_shortcut: s.chat.send_shortcut.clone(),
            user_collapse_chars: s.chat.user_collapse_chars,
            notify_enabled: s.notifications.enabled,
            notify_sound: s.notifications.sound,
            thinking_expanded_default: s.chat.thinking_expanded_default,
            diff_view_mode: s.chat.diff_view_mode.clone(),
            core_mode: s.core.mode.clone(),
            core_url: s.core.url.clone(),
        }
    }

    fn apply_to(&self, s: &mut crate::app_settings::AppSettings) {
        s.appearance.dark_mode = self.dark_mode;
        s.appearance.light_theme = self.light_theme.clone();
        s.appearance.dark_theme = self.dark_theme.clone();
        s.appearance.chat_font_size = self.chat_font_size.clone();
        s.appearance.locale = self.locale.clone();
        s.chat.send_shortcut = self.send_shortcut.clone();
        s.chat.user_collapse_chars = self.user_collapse_chars;
        s.chat.thinking_expanded_default = self.thinking_expanded_default;
        s.chat.diff_view_mode = self.diff_view_mode.clone();
        s.notifications.enabled = self.notify_enabled;
        s.notifications.sound = self.notify_sound;
        s.core.mode = self.core_mode.clone();
        s.core.url = self.core_url.clone();
    }
}

impl Default for AppearancePrefs {
    fn default() -> Self {
        Self::from_settings(&crate::app_settings::AppSettings::default())
    }
}

pub fn load_appearance_prefs() -> AppearancePrefs {
    AppearancePrefs::from_settings(&crate::app_settings::load_app_settings())
}

pub fn save_appearance_prefs(prefs: &AppearancePrefs) {
    crate::app_settings::update_app_settings(|settings| prefs.apply_to(settings));
}

pub fn font_size_from_str(s: &str) -> ChatFontSize {
    match s {
        "small" => ChatFontSize::Small,
        "large" => ChatFontSize::Large,
        _ => ChatFontSize::Medium,
    }
}

pub fn font_size_to_str(sz: ChatFontSize) -> &'static str {
    match sz {
        ChatFontSize::Small => "small",
        ChatFontSize::Medium => "medium",
        ChatFontSize::Large => "large",
    }
}

pub fn locale_from_prefs(s: &str) -> AppLocale {
    AppLocale::from_code(s)
}

pub fn send_shortcut_from_prefs(s: &str) -> SendShortcut {
    SendShortcut::from_str(s)
}

pub fn apply_appearance(
    dark_mode: bool,
    light_name: &str,
    dark_name: &str,
    cx: &mut App,
) {
    let cat = catalog();
    let light_cfg = cat
        .light
        .get(light_name)
        .cloned()
        .unwrap_or_else(|| {
            let mut cfg = ThemeConfig::default();
            cfg.is_default = true;
            cfg.name = DEFAULT_LIGHT_NAME.into();
            cfg.mode = ThemeMode::Light;
            cfg
        });
    let dark_cfg = cat
        .dark
        .get(dark_name)
        .cloned()
        .unwrap_or_else(|| {
            let mut cfg = ThemeConfig::default();
            cfg.is_default = true;
            cfg.name = DEFAULT_DARK_NAME.into();
            cfg.mode = ThemeMode::Dark;
            cfg
        });

    let target_mode = if dark_mode {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };

    Theme::update(cx, |theme| {
        theme.light_theme = Rc::new(light_cfg);
        theme.dark_theme = Rc::new(dark_cfg);
    });
    Theme::change(target_mode, None, cx);
    // Keep system UI font (e.g. .AppleSystemUIFont on macOS) across all themes
    Theme::update(cx, |theme| {
        theme.font_family = platform_system_font();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_official_gpui_kit_themes() {
        let cat = catalog();
        assert!(!cat.light.is_empty(), "expected light themes");
        assert!(!cat.dark.is_empty(), "expected dark themes");
        assert!(cat.light.contains_key("Ayu Light"));
        assert!(cat.dark.contains_key("Catppuccin Mocha"));
    }
}
