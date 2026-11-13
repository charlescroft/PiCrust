use crate::app_state::{AppState, RightPanelTab};
use crate::client::types::WorkspaceKind;
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::separator::Separator;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

pub fn render_status_bar(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let state = app_state.read(cx);
    let locale = state.locale;
    let sidecar_ok = state.sidecar_connected;
    let is_running = state.is_agent_running;
    let branch = state.active_branch.clone();
    let cwd_display = if state.active_workspace_kind == WorkspaceKind::Chat {
        SharedString::from(tr(locale, "status.chatWorkspace"))
    } else {
        SharedString::from(state.active_cwd.to_string_lossy().into_owned())
    };

    let status_color = if sidecar_ok || is_running {
        cx.theme().success
    } else {
        cx.theme().danger
    };
    let status_tooltip = if is_running {
        tr(locale, "status.running")
    } else if sidecar_ok {
        tr(locale, "status.connected")
    } else {
        tr(locale, "status.disconnected")
    };

    let core_url = state.client.base_url.clone();
    let core_endpoint = format!("/api/agent/{}/events", state.active_session_id.as_deref().unwrap_or("{id}"));
    let app_for_reconnect = app_state.clone();
    let app_for_term = app_state.clone();
    let app_for_settings = app_state.clone();

    let mut bar = StatusBar::new()
        .py_1()
        .left(
            h_flex()
                .id("sb-pi-status")
                .flex_shrink_0()
                .px_1p5()
                .py_0p5()
                .gap_1p5()
                .items_center()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().secondary))
                .tooltip({
                    let core_url = core_url.clone();
                    let core_endpoint = core_endpoint.clone();
                    let status_tooltip = status_tooltip;
                    move |w, cx| {
                        Tooltip::element({
                            let core_url = core_url.clone();
                            let core_endpoint = core_endpoint.clone();
                            move |_, cx| {
                                v_flex()
                                    .gap_1()
                                    .text_xs()
                                    .child(
                                        div()
                                            .font_semibold()
                                            .child(SharedString::from(status_tooltip)),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(format!("Core: {core_url}"))),
                                    )
                                    .child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(SharedString::from(format!(
                                                "API: {core_endpoint}"
                                            ))),
                                    )
                            }
                        })
                        .build(w, cx)
                    }
                })
                .on_click(move |_, _, cx| {
                    app_for_reconnect.update(cx, |s, cx| s.refresh_from_sidecar(cx));
                })
                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(status_color))
                .child(
                    div()
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().foreground)
                        .child("Pi Agent"),
                ),
        )
        .left(Separator::vertical())
        .left(
            h_flex()
                .items_center()
                .gap_1p5()
                .max_w(px(360.))
                .child(Icon::new(IconName::Folder).xsmall())
                .child(
                    div()
                        .text_xs()
                        .truncate()
                        .text_color(cx.theme().muted_foreground)
                        .child(cwd_display),
                ),
        );

    if let Some(branch_name) = branch {
        let branches = get_local_git_branches(&state.active_cwd);
        let app_switch_branch = app_state.clone();
        let current_branch = branch_name.to_string();

        let branch_button = Button::new("sb-branch-btn")
            .ghost()
            .small()
            .h(px(26.))
            .icon(Icon::new(crate::icons::GitBranchIcon))
            .label(branch_name)
            .tooltip(tr(locale, "status.switchBranch"))
            .dropdown_menu_with_anchor(gpui_kit::Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu.label(tr(locale, "status.branchesHeading"));
                for b in &branches {
                    let b_name = b.clone();
                    let is_current = b_name == current_branch;
                    let app_sw = app_switch_branch.clone();
                    menu = menu.item(
                        PopupMenuItem::new(SharedString::from(b_name.clone()))
                            .checked(is_current)
                            .on_click(move |_, _, cx| {
                                let target = b_name.clone();
                                app_sw.update(cx, |s, cx| {
                                    s.switch_git_branch(target, cx);
                                });
                            }),
                    );
                }
                menu
            });

        bar = bar.left(Separator::vertical()).left(branch_button);
    }

    bar.right(
        h_flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("sb-terminal")
                    .ghost()
                    .small()
                    .h(px(26.))
                    .icon(IconName::SquareTerminal)
                    .label(tr(locale, "status.terminal"))
                    .on_click(move |_, _, cx| {
                        app_for_term.update(cx, |s, cx| {
                            s.toggle_right_tab(RightPanelTab::Terminal, cx);
                        });
                    }),
            )
            .child(
                Button::new("sb-settings")
                    .ghost()
                    .small()
                    .h(px(26.))
                    .icon(IconName::Settings)
                    .label(tr(locale, "status.settings"))
                    .on_click(move |_, _, cx| {
                        app_for_settings.update(cx, |s, cx| {
                            s.open_settings(cx);
                        });
                    }),
            ),
    )
}

fn get_local_git_branches(cwd: &std::path::Path) -> Vec<String> {
    use std::process::Command;
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(&["branch", "--format=%(refname:short)"])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
