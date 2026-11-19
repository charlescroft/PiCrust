#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app_settings;
mod app_state;
mod chat;
mod client;
mod core_probe;
pub mod engine;
mod icons;
mod i18n;
mod pages;
mod panels;
mod provider_catalog;
mod settings;
mod themes;
mod workspace;

use crate::actions::{NewSession, OpenSettings, QuitApp, ToggleTheme, register_keybindings};
use crate::app_state::AppState;
use crate::workspace::WorkspaceView;
use gpui_kit::assets::{Assets, icon_assets};
use gpui_kit::component::TitleBar;
use gpui_kit::{
    AppContext as _, AssetSource, Bounds, Menu, MenuItem, Result as GpuiResult, SharedString,
    WindowBounds, WindowOptions, application, init, open_window, point, px, size,
};
use std::borrow::Cow;

// Icons this client uses that are absent from the component's default set:
// the asset source only serves what these lists embed.
icon_assets!(ExtraIcons, [Zap, Shield, Database, Pencil, Trash, AtSign, LoaderCircle, GitBranch, Archive, Clock, Play, KeyRound]);

struct PiAssets;

impl AssetSource for PiAssets {
    fn load(&self, path: &str) -> GpuiResult<Option<Cow<'static, [u8]>>> {
        // Brand mark for the custom title bar. Serving it through the asset
        // source keeps it available in the portable Windows build, where no
        // `assets/` directory is shipped next to the executable. SVG lets the
        // mark follow the active theme's foreground colour.
        if path == "picrust-mark.svg" {
            return Ok(Some(Cow::Borrowed(
                include_bytes!("../assets/picrust-mark.svg").as_slice(),
            )));
        }
        if let Ok(Some(bytes)) = ExtraIcons.load(path) {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> GpuiResult<Vec<SharedString>> {
        let mut items = Assets.list(path)?;
        items.extend(ExtraIcons.list(path)?);
        Ok(items)
    }
}

/// Window geometry lives in the same JSON file as every other client setting.
fn load_window_bounds() -> WindowBounds {
    let settings = app_settings::load_app_settings();
    if let Some(geo) = settings.window {
        if geo.width >= 640.0 && geo.height >= 480.0 {
            let b = Bounds {
                origin: point(px(geo.x), px(geo.y)),
                size: size(px(geo.width), px(geo.height)),
            };
            return if geo.maximized {
                WindowBounds::Maximized(b)
            } else {
                WindowBounds::Windowed(b)
            };
        }
    }
    WindowBounds::Windowed(Bounds {
        origin: point(px(80.), px(60.)),
        size: size(px(1360.), px(860.)),
    })
}

fn save_window_bounds(bounds: WindowBounds) {
    // Persist strictly SIZE | POSITION | MAXIMIZED (never VISIBLE or DECORATIONS)
    let (rect, maximized) = match bounds {
        WindowBounds::Maximized(b) => (b, true),
        WindowBounds::Fullscreen(b) => (b, true),
        WindowBounds::Windowed(b) => (b, false),
    };
    app_settings::update_app_settings(|settings| {
        settings.window = Some(app_settings::WindowSettings {
            x: f32::from(rect.origin.x),
            y: f32::from(rect.origin.y),
            width: f32::from(rect.size.width),
            height: f32::from(rect.size.height),
            maximized,
        });
    });
}

fn open_main_window(
    app_state: gpui_kit::Entity<AppState>,
    cx: &mut gpui_kit::App,
) -> gpui_kit::AnyWindowHandle {
    let window_bounds = load_window_bounds();
    let options = WindowOptions {
        window_bounds: Some(window_bounds),
        window_min_size: Some(size(px(760.), px(520.))),
        ..TitleBar::window_options()
    };

    let (window_handle, _workspace) = open_window(options, cx, |window, cx| {
        let ui_sz = app_state.read(cx).ui_font_size.clamp(5.0, 21.0);
        window.set_rem_size(px(ui_sz));
        let is_system_dark = matches!(
            window.appearance(),
            gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
        );
        app_state.update(cx, |s, cx| {
            if s.theme_mode == "system" && s.dark_mode != is_system_dark {
                s.dark_mode = is_system_dark;
                themes::apply_appearance(
                    s.dark_mode,
                    &s.light_theme_name,
                    &s.dark_theme_name,
                    cx,
                );
            }
        });

        let state = app_state.clone();
        window.on_window_should_close(cx, |window, _| {
            save_window_bounds(window.window_bounds());
            true
        });
        cx.new(|cx| {
            cx.observe_window_bounds(window, |_, win, _| {
                save_window_bounds(win.window_bounds());
            })
            .detach();
            let act_state = state.clone();
            cx.observe_window_activation(window, move |_, win, cx| {
                if win.is_window_active() {
                    act_state.update(cx, |s, cx| {
                        s.on_window_activated(cx);
                    });
                }
            })
            .detach();
            let app_state_appearance = state.clone();
            cx.observe_window_appearance(window, move |_, win, cx| {
                let is_system_dark = matches!(
                    win.appearance(),
                    gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
                );
                app_state_appearance.update(cx, |s, cx| {
                    if s.theme_mode == "system" && s.dark_mode != is_system_dark {
                        s.dark_mode = is_system_dark;
                        s.persist_and_apply_appearance(cx);
                        cx.notify();
                    }
                });
            })
            .detach();
            WorkspaceView::new(state, window, cx)
        })
    })
    .expect("failed to open PiCrust GPUI window");
    window_handle
}

fn main() {
    let app = application().with_assets(PiAssets);
    app.on_reopen(|cx| {
        if let Some(win) = cx.windows().into_iter().next() {
            let _ = win.update(cx, |_, window, _| {
                window.activate_window();
            });
        } else {
            let app_state = cx.new(|cx| AppState::new(cx));
            let wh = open_main_window(app_state, cx);
            let _ = wh.update(cx, |_, window, _| {
                window.activate_window();
            });
        }
    });
    app.run(|cx| {
        init(cx);
        #[cfg(target_os = "windows")]
        cx.set_app_identity("works.earendil.picrust", "PiCrust");
        register_keybindings(cx);
        let prefs = themes::load_appearance_prefs();
        themes::apply_appearance(prefs.dark_mode, &prefs.light_theme, &prefs.dark_theme, cx);

        cx.set_menus(vec![
            Menu {
                name: "PiCrust".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("Settings...", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("Quit PiCrust", QuitApp),
                ],
            },
            Menu {
                name: "File".into(),
                disabled: false,
                items: vec![MenuItem::action("New Session", NewSession)],
            },
            Menu {
                name: "View".into(),
                disabled: false,
                items: vec![MenuItem::action("Toggle Light / Dark Theme", ToggleTheme)],
            },
        ]);

        let app_state = cx.new(|cx| AppState::new(cx));
        let window_handle = open_main_window(app_state, cx);

        cx.on_app_quit({
            move |cx| {
                let _ = window_handle.update(cx, |_, window, _| {
                    save_window_bounds(window.window_bounds());
                });
                async {}
            }
        })
        .detach();

        let _ = window_handle.update(cx, |_, window, _| {
            window.activate_window();
        });
    });
}
