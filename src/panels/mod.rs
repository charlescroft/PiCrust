pub mod artifact_panel;
pub mod file_explorer;
pub mod file_viewer;
pub mod inspector_panels;
pub mod terminal_panel;

use crate::app_state::{AppState, RightPanelTab};
use crate::i18n::{AppLocale, tr};
use crate::panels::artifact_panel::ArtifactPanelView;
use crate::panels::file_explorer::FileExplorerPanel;
use crate::panels::file_viewer::FileViewerView;
use crate::panels::inspector_panels::render_inspector_panel;
use crate::panels::terminal_panel::TerminalHostView;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div,
};

pub struct WorkbenchPanel {
    focus_handle: FocusHandle,
    app_state: Entity<AppState>,
    artifact_view: Entity<ArtifactPanelView>,
    file_explorer: Entity<FileExplorerPanel>,
    file_viewer: Entity<FileViewerView>,
    terminal: Entity<TerminalHostView>,
}

impl EventEmitter<PanelEvent> for WorkbenchPanel {}

impl Focusable for WorkbenchPanel {
    fn focus_handle(&self, _cx: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for WorkbenchPanel {
    fn panel_name(&self) -> &'static str {
        "WorkbenchPanel"
    }
}

impl Panel for WorkbenchPanel {
    fn title(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let file_name = state
            .open_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|n| SharedString::from(n.to_string()));
        tab_title(state.right_panel_tab, file_name, state.locale)
    }
}

fn truncate_tab_name(name: &str, max_chars: usize) -> String {
    let mut chars = name.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

fn tab_title(tab: RightPanelTab, file_name: Option<SharedString>, locale: AppLocale) -> SharedString {
    match tab {
        RightPanelTab::Artifact => SharedString::from(tr(locale, "panel.artifact")),
        RightPanelTab::Files => SharedString::from(tr(locale, "panel.files")),
        RightPanelTab::FileViewer => file_name
            .map(|n| SharedString::from(truncate_tab_name(n.as_ref(), 20)))
            .unwrap_or_else(|| SharedString::from(tr(locale, "panel.filePreview"))),
        RightPanelTab::Terminal => SharedString::from(tr(locale, "panel.terminal")),
        RightPanelTab::Inspector => SharedString::from(tr(locale, "panel.session")),
    }
}

fn tab_icon(tab: RightPanelTab) -> IconName {
    match tab {
        RightPanelTab::Artifact => IconName::Frame,
        RightPanelTab::Files => IconName::Folder,
        RightPanelTab::FileViewer => IconName::FileText,
        RightPanelTab::Terminal => IconName::SquareTerminal,
        RightPanelTab::Inspector => IconName::Inspector,
    }
}

impl WorkbenchPanel {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let artifact_view = cx.new(|cx| ArtifactPanelView::new(app_state.clone(), window, cx));
        let file_explorer = cx.new(|cx| FileExplorerPanel::new(app_state.clone(), cx));
        let file_viewer = cx.new(|cx| FileViewerView::new(app_state.clone(), window, cx));
        let terminal = cx.new(|cx| TerminalHostView::new(app_state.clone(), cx));

        cx.observe(&app_state, |_, _, cx| cx.notify()).detach();

        Self {
            focus_handle: cx.focus_handle(),
            app_state,
            artifact_view,
            file_explorer,
            file_viewer,
            terminal,
        }
    }
}

impl Render for WorkbenchPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let tab = state.right_panel_tab;
        let locale = state.locale;
        let open_tabs = state.open_right_tabs.clone();
        let file_name = state
            .open_file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|n| SharedString::from(n.to_string()));

        // The artifact panel holds no WebView: its tabs are Source and Diff only.

        let selected_idx = open_tabs.iter().position(|&t| t == tab).unwrap_or(0);
        let app_close = self.app_state.clone();
        let app_tab_click = self.app_state.clone();
        let open_tabs_for_click = open_tabs.clone();

        let tabs: Vec<Tab> = open_tabs
            .iter()
            .enumerate()
            .map(|(idx, &t)| {
                let app_rm = self.app_state.clone();
                Tab::new()
                    .max_w(gpui_kit::px(180.))
                    .prefix(Icon::new(tab_icon(t)).xsmall())
                    .label(tab_title(t, file_name.clone(), locale))
                    .suffix(
                        Button::new(("wb-tab-close", idx))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .tooltip("Close tab")
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                app_rm.update(cx, |s, cx| {
                                    s.close_right_tab(t, cx);
                                });
                            }),
                    )
            })
            .collect();

        let tab_bar = TabBar::new("workbench-folder-tabs")
            .folder()
            .small()
            .selected_index(selected_idx)
            .on_click(move |ix, _, cx| {
                if let Some(&target_tab) = open_tabs_for_click.get(*ix) {
                    app_tab_click.update(cx, |s, cx| {
                        s.open_right_tab(target_tab, cx);
                    });
                }
            })
            .children(tabs)
            .suffix(
                div().px_1().child(
                    Button::new("wb-close")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip("Close Panel")
                        .on_click(move |_, _, cx| {
                            app_close.update(cx, |s, cx| {
                                s.right_panel_open = false;
                                cx.notify();
                            });
                        }),
                ),
            );

        let body = match tab {
            RightPanelTab::Artifact => div().size_full().child(self.artifact_view.clone()).into_any_element(),
            RightPanelTab::Files => div().size_full().child(self.file_explorer.clone()).into_any_element(),
            RightPanelTab::FileViewer => div().size_full().child(self.file_viewer.clone()).into_any_element(),
            RightPanelTab::Terminal => div().size_full().child(self.terminal.clone()).into_any_element(),
            RightPanelTab::Inspector => {
                render_inspector_panel(&self.app_state, window, cx).into_any_element()
            }
        };

        v_flex()
            .size_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(tab_bar)
            .child(div().flex_1().min_h_0().size_full().child(body))
    }
}
