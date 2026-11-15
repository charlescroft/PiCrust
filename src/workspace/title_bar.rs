use crate::app_state::{AppState, RightPanelTab};
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, IconName, StyledExt as _, TitleBar, h_flex};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};

pub fn render_title_bar(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let state = app_state.read(cx);
    let is_dark = state.dark_mode;
    let locale = state.locale;
    let sidebar_collapsed = state.sidebar_collapsed;
    let right_open = state.right_panel_open;

    let app_for_sidebar = app_state.clone();
    let app_for_find = app_state.clone();
    let app_for_right = app_state.clone();
    let app_for_theme = app_state.clone();

    TitleBar::new()
        .h(px(40.))
        .py_1p5()
        .bg(cx.theme().title_bar)
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            h_flex()
                .id("tb-left-group")
                .items_center()
                .gap_3()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("toggle-sidebar")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .icon(if sidebar_collapsed {
                            IconName::PanelLeftOpen
                        } else {
                            IconName::PanelLeftClose
                        })
                        .tooltip(tr(locale, "title.toggleSidebar"))
                        .on_click(move |_, _, cx| {
                            app_for_sidebar.update(cx, |s, cx| {
                                s.sidebar_collapsed = !s.sidebar_collapsed;
                                cx.notify();
                            });
                        }),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_1p5()
                        .child(
                            svg()
                                .path("picrust-mark.svg")
                                .size(px(18.))
                                .flex_shrink_0()
                                .text_color(cx.theme().foreground),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child("PiCrust"),
                        ),
                ),
        )
        .child(
            h_flex()
                .id("tb-right-group")
                .items_center()
                .gap_2()
                .pr_2p5()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("find-in-chat")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .icon(IconName::Search)
                        .tooltip(tr(locale, "title.findInChat"))
                        .on_click(move |_, _, cx| {
                            app_for_find.update(cx, |s, cx| {
                                s.find_open = !s.find_open;
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new("toggle-theme")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .icon(if is_dark { IconName::Sun } else { IconName::Moon })
                        .tooltip(tr(locale, "title.toggleTheme"))
                        .on_click(move |_, window, cx| {
                            app_for_theme.update(cx, |s, cx| {
                                s.toggle_theme(window, cx);
                            });
                        }),
                )
                .child(
                    Button::new("toggle-right-panel")
                        .ghost()
                        .w(px(30.))
                        .h(px(30.))
                        .icon(if right_open {
                            IconName::PanelRightClose
                        } else {
                            IconName::PanelRightOpen
                        })
                        .tooltip(tr(locale, "title.toggleRightPanel"))
                        .on_click(move |_, _, cx| {
                            app_for_right.update(cx, |s, cx| {
                                if s.right_panel_open {
                                    s.right_panel_open = false;
                                    cx.notify();
                                } else {
                                    let tab = s
                                        .open_right_tabs
                                        .last()
                                        .copied()
                                        .unwrap_or(RightPanelTab::Inspector);
                                    s.open_right_tab(tab, cx);
                                }
                            });
                        }),
                ),
        )
}
