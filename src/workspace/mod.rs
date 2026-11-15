pub mod sidebar;
pub mod status_bar;
pub mod title_bar;

use crate::actions::*;
use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::chat::ChatPanel;
use crate::client::types::WorkspaceKind;
use crate::pages::artifacts_page::render_artifacts_page;
use crate::pages::scheduled_page::ScheduledPageView;
use crate::pages::tasks_page::render_tasks_page;
use crate::panels::WorkbenchPanel;
use crate::settings::add_dialog::SettingsAddDialogView;
use crate::settings::render_settings_overlay;
use crate::workspace::sidebar::{InlineRenameState, render_sidebar};
use crate::workspace::status_bar::render_status_bar;
use crate::workspace::title_bar::render_title_bar;
use gpui_kit::base::motion::{Transition, transition};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Render, Styled as _, Window, div,
    px,
};
use std::time::Duration;

pub struct WorkspaceView {
    focus_handle: FocusHandle,
    pub app_state: Entity<AppState>,
    chat_panel: Entity<ChatPanel>,
    workbench_panel: Entity<WorkbenchPanel>,
    scheduled_page: Entity<ScheduledPageView>,
    settings_add_dialog: Entity<SettingsAddDialogView>,
    artifacts_search_input: Entity<gpui_kit::component::input::InputState>,
    rename_state: Option<InlineRenameState>,
    resizing_right: bool,
}

impl Focusable for WorkspaceView {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl WorkspaceView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_panel = cx.new(|cx| ChatPanel::new(app_state.clone(), window, cx));
        let workbench_panel = cx.new(|cx| WorkbenchPanel::new(app_state.clone(), window, cx));
        let scheduled_page = cx.new(|cx| ScheduledPageView::new(app_state.clone(), window, cx));
        let settings_add_dialog =
            cx.new(|cx| SettingsAddDialogView::new(app_state.clone(), window, cx));

        let initial_locale = app_state.read(cx).locale;
        let artifacts_search_input = cx.new(|cx| {
            gpui_kit::component::input::InputState::new(window, cx)
                .placeholder(crate::i18n::tr(initial_locale, "page.searchArtifacts"))
        });
        let app_search = app_state.clone();
        let input_for_search = artifacts_search_input.clone();
        cx.subscribe(&artifacts_search_input, move |_, _, ev: &gpui_kit::component::input::InputEvent, cx| {
            if matches!(ev, gpui_kit::component::input::InputEvent::Change) {
                let val = input_for_search.read(cx).value().to_string();
                app_search.update(cx, |s, cx| {
                    s.set_artifacts_search_query(val, cx);
                });
            }
        })
        .detach();

        cx.observe(&app_state, |_, _, cx| cx.notify()).detach();

        Self {
            focus_handle: cx.focus_handle(),
            app_state,
            chat_panel,
            workbench_panel,
            scheduled_page,
            settings_add_dialog,
            artifacts_search_input,
            rename_state: None,
            resizing_right: false,
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let active_page = state.active_page.clone();
        let sidebar_collapsed = state.sidebar_collapsed;
        let right_open = state.right_panel_open;
        let right_target_w = f32::from(state.right_panel_width);

        // Smooth animated widths without remounting center content (prevents any flash)
        let easing = cx.theme().motion_tokens().easing_move.clone();
        let left_anim_w = transition(
            "left-sidebar-width",
            if sidebar_collapsed { 0.0 } else { 268.0 },
            Transition::new(Duration::from_millis(180)).easing(easing.clone()),
            window,
            cx,
        );
        let right_anim_w = if self.resizing_right && right_open {
            right_target_w
        } else {
            transition(
                "right-panel-width",
                if right_open { right_target_w } else { 0.0 },
                Transition::new(Duration::from_millis(200)).easing(easing),
                window,
                cx,
            )
        };

        let chat_view = div()
            .absolute()
            .inset_0()
            .child(self.chat_panel.clone());

        let page_overlay = match active_page {
            WorkspacePage::Chat => None,
            WorkspacePage::Tasks => {
                Some(render_tasks_page(&self.app_state, window, cx).into_any_element())
            }
            WorkspacePage::Scheduled => Some(self.scheduled_page.clone().into_any_element()),
            WorkspacePage::Artifacts => {
                Some(render_artifacts_page(&self.app_state, &self.artifacts_search_input, window, cx).into_any_element())
            }
        };

        let center_view = div()
            .size_full()
            .relative()
            .overflow_hidden()
            .child(chat_view)
            .children(page_overlay.map(|p| {
                div()
                    .absolute()
                    .inset_0()
                    .bg(cx.theme().background)
                    .occlude()
                    .child(p)
            }));

        let sidebar_el = if left_anim_w > 0.5 {
            Some(
                div()
                    .w(px(left_anim_w))
                    .h_full()
                    .overflow_hidden()
                    .flex_shrink_0()
                    .child(
                        div()
                            .w(px(268.))
                            .h_full()
                            .child(render_sidebar(&self.app_state, &mut self.rename_state, window, cx)),
                    ),
            )
        } else {
            None
        };

        let right_panel_el = if right_anim_w > 0.5 {
            let show_handle = right_open && (right_anim_w - right_target_w).abs() < 2.0;
            Some(
                h_flex()
                    .h_full()
                    .flex_shrink_0()
                    .when_resize_handle(show_handle, cx.listener(|this, _, _, _| {
                        this.resizing_right = true;
                    }))
                    .child(
                        div()
                            .w(px(right_anim_w))
                            .h_full()
                            .overflow_hidden()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .w(px(right_target_w))
                                    .h_full()
                                    .child(self.workbench_panel.clone()),
                            ),
                    ),
            )
        } else {
            None
        };

        let win_w = window.viewport_size().width;

        div()
            .id("pi-workspace-root")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if this.resizing_right {
                    let next_w = (win_w - ev.position.x).clamp(px(280.), px(760.));
                    this.app_state.update(cx, |s, cx| {
                        s.right_panel_width = next_w;
                        cx.notify();
                    });
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| {
                    this.resizing_right = false;
                }),
            )
            .on_action(cx.listener(|this, _: &NewSession, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.start_new_composer(WorkspaceKind::Project, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.sidebar_collapsed = !s.sidebar_collapsed;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &ToggleRightPanel, _, cx| {
                this.app_state.update(cx, |s, cx| {
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
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.toggle_right_tab(RightPanelTab::Terminal, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &OpenFind, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.find_open = !s.find_open;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    if !s.settings_busy {
                        if s.settings_open {
                            s.settings_open = false;
                            cx.notify();
                        } else {
                            s.open_settings(cx);
                        }
                    }
                });
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.toggle_theme(window, cx);
                });
            }))
            .on_action(cx.listener(|this, _: &SwitchToChat, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Chat;
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &SwitchToTasks, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Tasks;
                    s.refresh_tasks_from_sidecar(cx);
                });
            }))
            .on_action(cx.listener(|this, _: &SwitchToScheduled, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Scheduled;
                    s.refresh_scheduled_tasks(cx);
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _: &SwitchToArtifacts, _, cx| {
                this.app_state.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Artifacts;
                    cx.notify();
                });
            }))
            .on_action(|_: &QuitApp, _, cx| {
                cx.quit();
            })
            .child(
                v_flex()
                    .size_full()
                    .child(render_title_bar(&self.app_state, window, cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .children(sidebar_el)
                            .child(div().flex_1().min_w_0().h_full().child(center_view))
                            .children(right_panel_el),
                    )
                    .child(render_status_bar(&self.app_state, window, cx)),
            )
            .child(render_settings_overlay(
                &self.app_state,
                &self.settings_add_dialog,
                window,
                cx,
            ))
            .child(crate::panels::agy_link_modal::render_agy_link_overlay(
                &self.app_state,
                window,
                cx,
            ))
    }
}

trait ResizeHandleExt {
    fn when_resize_handle(
        self,
        show: bool,
        on_down: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut gpui_kit::App) + 'static,
    ) -> Self;
}

impl<E: ParentElement> ResizeHandleExt for E {
    fn when_resize_handle(
        self,
        show: bool,
        on_down: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut gpui_kit::App) + 'static,
    ) -> Self {
        if !show {
            return self;
        }
        self.child(
            div()
                .id("right-panel-resize-handle")
                .w(px(5.))
                .h_full()
                .cursor_col_resize()
                .on_mouse_down(MouseButton::Left, on_down),
        )
    }
}
