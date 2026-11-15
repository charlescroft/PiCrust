pub mod add_dialog;
pub mod extensions_page;
pub mod general_page;
pub mod mcp_page;
pub mod models_page;

use crate::app_state::AppState;
use crate::i18n::{AppLocale, tr};
use crate::settings::add_dialog::SettingsAddDialogView;
use crate::settings::extensions_page::{
    antigravity_setting_page, plugins_setting_page, skills_setting_page, subagents_setting_page,
};
use crate::settings::general_page::general_setting_page;
use crate::settings::mcp_page::mcp_setting_page;
use crate::settings::models_page::models_setting_page;
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::group_box::GroupBoxVariant;
use gpui_kit::component::setting::{SelectIndex, Settings};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, PathPromptOptions, SharedString, Styled as _, Window, div, px,
};
use std::path::PathBuf;
use std::time::Duration;

pub const SETTINGS_DROPDOWN_W: gpui_kit::Pixels = px(260.);
pub const SETTINGS_BTN_W: gpui_kit::Pixels = px(124.);

/// Unified config file path field: styled read-only path box + Open/Reveal file button (+ optional Browse).
pub fn config_path_field(
    id_prefix: &'static str,
    path: PathBuf,
    display_text: String,
    allow_browse: bool,
    locale: AppLocale,
    app_state: Entity<AppState>,
    cx: &gpui_kit::App,
) -> AnyElement {
    let open_path = path.clone();
    let app_open = app_state.clone();
    let mut row = h_flex()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_1p5()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h(px(28.))
                .px_2p5()
                .flex()
                .items_center()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().input)
                .bg(cx.theme().background)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate()
                .child(SharedString::from(display_text)),
        )
        .child(
            Button::new((id_prefix, 1usize))
                .outline()
                .small()
                .h(px(28.))
                .w(px(100.))
                .icon(IconName::FolderOpen)
                .label(tr(locale, "settings.openFile"))
                .on_click(move |_, _, cx| {
                    if open_path.exists() {
                        app_open.update(cx, |s, cx| {
                            s.open_file(open_path.clone(), cx);
                            s.settings_open = false;
                        });
                    } else if let Some(parent) = open_path.parent() {
                        let _ = open::that(parent);
                    }
                }),
        );

    if allow_browse {
        let app_browse = app_state;
        row = row.child(
            Button::new((id_prefix, 2usize))
                .outline()
                .small()
                .h(px(28.))
                .w(px(100.))
                .icon(IconName::Search)
                .label(tr(locale, "settings.browseFile"))
                .on_click(move |_, _, cx| {
                    let rx = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some("Select Config File".into()),
                    });
                    let app_b = app_browse.clone();
                    cx.spawn(async move |cx| {
                        if let Ok(Ok(Some(paths))) = rx.await {
                            if let Some(p) = paths.into_iter().next() {
                                let _ = app_b.update(cx, |s, cx| {
                                    s.open_file(p, cx);
                                    s.settings_open = false;
                                });
                            }
                        }
                    })
                    .detach();
                }),
        );
    }

    row.into_any_element()
}

pub fn render_settings_overlay(
    app_state: &Entity<AppState>,
    add_dialog: &Entity<SettingsAddDialogView>,
    window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let (is_open, busy, locale, saved_flash) = {
        let state = app_state.read(cx);
        (
            state.settings_open,
            state.settings_busy,
            state.locale,
            state.settings_saved_flash,
        )
    };

    let presence = Presence::new("settings-modal-presence", is_open)
        .transition(
            Transition::new(Duration::from_millis(160))
                .easing(cx.theme().motion_tokens().easing_move.clone()),
        )
        .sample(window, cx);
    if !presence.should_render() {
        return div().into_any_element();
    }
    let progress = presence.progress;

    let app_close = app_state.clone();
    let app_footer_close = app_state.clone();
    let app_footer_save = app_state.clone();
    let app_backdrop_close = app_state.clone();

    let initial_page = app_state.read(cx).settings_initial_page.min(1);

    let pages = vec![
        general_setting_page(app_state.clone(), locale, cx),
        models_setting_page(app_state.clone(), locale, cx),
        mcp_setting_page(app_state.clone(), locale, cx),
        subagents_setting_page(app_state.clone(), locale, cx),
        skills_setting_page(app_state.clone(), locale, cx),
        plugins_setting_page(app_state.clone(), locale, cx),
        antigravity_setting_page(app_state.clone(), locale, cx),
    ];

    div()
        .id("settings-backdrop")
        .occlude()
        .absolute()
        .inset_0()
        .p_6()
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().background.opacity(0.72 * progress))
        .on_scroll(|_, _, cx| {
            cx.stop_propagation();
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            app_backdrop_close.update(cx, |s, cx| {
                if !s.settings_busy {
                    s.settings_open = false;
                    s.settings_add_kind = None;
                    cx.notify();
                }
            });
        })
        .child(
            v_flex()
                .id("settings-modal-card")
                .relative()
                .occlude()
                .w(px(1020.))
                .max_w_full()
                .h(px(700.))
                .max_h_full()
                .mt(px(14.0 * (1.0 - progress)))
                .opacity(progress)
                .rounded_xl()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .shadow_lg()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .on_scroll(|_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    h_flex()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .justify_between()
                        .items_center()
                        .child(div().text_sm().font_semibold().child(tr(locale, "settings.title")))
                        .child(
                            Button::new("close-settings-btn")
                                .ghost()
                                .small()
                                .icon(IconName::Close)
                                .disabled(busy)
                                .on_click(move |_, _, cx| {
                                    app_close.update(cx, |s, cx| {
                                        if !s.settings_busy {
                                            s.settings_open = false;
                                            s.settings_add_kind = None;
                                            cx.notify();
                                        }
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_hidden()
                        .child(
                            Settings::new(match initial_page {
                                1 => "picrust-settings-models",
                                6 => "picrust-settings-antigravity",
                                _ => "picrust-settings",
                            })
                                .default_selected_index(SelectIndex {
                                    page_ix: initial_page,
                                    group_ix: None,
                                })
                                .sidebar_width(px(200.))
                                .sidebar_size_range(px(170.)..px(250.))
                                .with_group_variant(GroupBoxVariant::Outline)
                                .pages(pages),
                        ),
                )
                .child(
                    h_flex()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .justify_end()
                        .gap_2()
                        .children(if saved_flash {
                            Some(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().success)
                                    .child(tr(locale, "settings.saved")),
                            )
                        } else {
                            None
                        })
                        .child(
                            Button::new("settings-footer-close")
                                .ghost()
                                .small()
                                .h(px(28.))
                                .w(SETTINGS_BTN_W)
                                .icon(IconName::Close)
                                .disabled(busy)
                                .label(tr(locale, "scheduled.cancel"))
                                .on_click(move |_, _, cx| {
                                    app_footer_close.update(cx, |s, cx| {
                                        if !s.settings_busy {
                                            s.settings_open = false;
                                            s.settings_add_kind = None;
                                            cx.notify();
                                        }
                                    });
                                }),
                        )
                        .child(
                            Button::new("settings-footer-save")
                                .primary()
                                .small()
                                .h(px(28.))
                                .w(SETTINGS_BTN_W)
                                .icon(IconName::Check)
                                .disabled(busy)
                                .label(tr(locale, "settings.saveSettings"))
                                .on_click(move |_, _, cx| {
                                    // Save applies pending edits and reloads settings in place:
                                    // the dialog stays open so further changes need no reopen.
                                    app_footer_save.update(cx, |s, cx| {
                                        s.refresh_settings_data(cx);
                                        s.refresh_from_sidecar(cx);
                                        s.settings_saved_flash = true;
                                        cx.notify();
                                    });
                                    let app_timer = app_footer_save.clone();
                                    cx.spawn(async move |cx| {
                                        smol::Timer::after(std::time::Duration::from_millis(2000)).await;
                                        let _ = app_timer.update(cx, |s, cx| {
                                            s.settings_saved_flash = false;
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                }),
                        ),
                )
                .child(add_dialog.clone()),
        )
        .children(busy.then(|| {
            div()
                .id("settings-blocking-overlay")
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(cx.theme().background.opacity(0.68))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .occlude()
                        .w(px(360.))
                        .p_5()
                        .gap_2p5()
                        .items_center()
                        .rounded_xl()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .shadow_lg()
                        .child(Spinner::new().large())
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(tr(locale, "settings.waitingCoreTitle")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_center()
                                .text_color(cx.theme().muted_foreground)
                                .child(tr(locale, "settings.waitingCoreDesc")),
                        ),
                )
        }))
        .into_any_element()
}
