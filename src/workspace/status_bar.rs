use crate::app_state::{AppState, RightPanelTab};
use crate::client::types::WorkspaceKind;
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::separator::Separator;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
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
    let branch = if state.active_workspace_kind == WorkspaceKind::Chat {
        None
    } else {
        state.active_branch.clone()
    };
    let cwd_display = if state.active_is_temp_chat {
        SharedString::from(tr(locale, "status.tempChatWorkspace"))
    } else if state.active_workspace_kind == WorkspaceKind::Chat {
        SharedString::from(tr(locale, "status.chatWorkspace"))
    } else {
        SharedString::from(format_compact_cwd_path(&state.active_cwd))
    };

    let status_color = if sidecar_ok || is_running {
        cx.theme().success
    } else if state.selected_model.is_some() {
        cx.theme().accent
    } else {
        cx.theme().danger
    };
    let status_tooltip = if is_running {
        tr(locale, "status.running")
    } else if sidecar_ok {
        tr(locale, "status.connected")
    } else if state.selected_model.is_some() {
        "Native Rust Engine"
    } else {
        tr(locale, "status.disconnected")
    };

    let core_url = state.client.base_url.clone();
    let core_endpoint = format!("/api/agent/{}/events", state.active_session_id.as_deref().unwrap_or("{id}"));
    let app_for_reconnect = app_state.clone();
    let app_for_term = app_state.clone();
    let app_for_settings = app_state.clone();
    let app_for_agy = app_state.clone();
    let agy_quota_badge: Option<(Option<i32>, String)> = state.agy_link_state.as_ref().map(|agy| {
        if let Some(active) = agy.accounts.iter().find(|a| a.active).or_else(|| agy.accounts.first()) {
            let name = active.email.as_deref().unwrap_or(&active.alias).to_string();
            let pct = active.five_hour_pct.or(active.weekly_pct);
            (pct, name)
        } else {
            (None, "AGY".to_string())
        }
    });

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
                        .child("PiCrust"),
                ),
        )
        .left(Separator::vertical())
        .left(
            h_flex()
                .items_center()
                .gap_1p5()
                .max_w(px(360.))
                .child({
                    let is_temp = state.is_active_temp_chat();
                    let is_chat = state.active_workspace_kind == WorkspaceKind::Chat
                        || crate::client::local_store::is_chat_workspace_cwd(&state.active_cwd);
                    if is_temp {
                        Icon::new(crate::icons::ClockIcon).xsmall().into_any_element()
                    } else if is_chat {
                        Icon::new(IconName::Bot).xsmall().into_any_element()
                    } else {
                        Icon::new(IconName::Folder).xsmall().into_any_element()
                    }
                })
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
            .dropdown_menu_with_anchor(gpui_kit::Anchor::BottomLeft, move |mut menu, window, cx| {
                menu = menu
                    .min_w(px(340.))
                    .max_w(px(400.))
                    .max_h(px(440.));

                // 1. Current Branch Card
                let cur = current_branch.clone();
                menu = menu.item(
                    PopupMenuItem::element(move |_, cx| {
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .w_full()
                            .p_2p5()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .gap_1p5()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Icon::new(crate::icons::GitBranchIcon)
                                                    .small()
                                                    .text_color(cx.theme().primary),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "status.currentBranch")),
                                            ),
                                    )
                                    .child(Tag::primary().small().child("HEAD")),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_sm()
                                    .font_semibold()
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .text_color(cx.theme().foreground)
                                    .child(SharedString::from(cur.clone())),
                            )
                    })
                    .disabled(true),
                );

                menu = menu.separator();
                menu = menu.label(tr(locale, "status.branchesHeading"));

                for b in &branches {
                    let b_name = b.clone();
                    let is_current = b_name == current_branch;
                    let app_sw = app_switch_branch.clone();
                    let app_pull = app_switch_branch.clone();
                    let app_push = app_switch_branch.clone();
                    let app_del = app_switch_branch.clone();
                    let b_sw = b_name.clone();
                    let b_pull = b_name.clone();
                    let b_push = b_name.clone();
                    let b_del = b_name.clone();

                    let sub_menu = PopupMenu::build(window, cx, move |sub, _, _| {
                        let mut sub = sub;
                        if !is_current {
                            let a_sw = app_sw.clone();
                            let tgt = b_sw.clone();
                            sub = sub.item(
                                PopupMenuItem::new(tr(locale, "status.gitCheckout"))
                                    .icon(Icon::new(crate::icons::GitBranchIcon))
                                    .on_click(move |_, _, cx| {
                                        a_sw.update(cx, |s, cx| s.switch_git_branch(tgt.clone(), cx));
                                    }),
                            );
                        }
                        let a_pull = app_pull.clone();
                        let b_p = b_pull.clone();
                        sub = sub.item(
                            PopupMenuItem::new(tr(locale, "status.gitPull"))
                                .icon(IconName::ArrowDown)
                                .on_click(move |_, _, cx| {
                                    a_pull.update(cx, |s, cx| s.git_pull(Some(b_p.clone()), cx));
                                }),
                        );
                        let a_push = app_push.clone();
                        let b_pu = b_push.clone();
                        sub = sub.item(
                            PopupMenuItem::new(tr(locale, "status.gitPush"))
                                .icon(IconName::ArrowUp)
                                .on_click(move |_, _, cx| {
                                    a_push.update(cx, |s, cx| s.git_push(Some(b_pu.clone()), cx));
                                }),
                        );
                        if !is_current {
                            let a_del = app_del.clone();
                            let b_d = b_del.clone();
                            sub = sub.separator().item(
                                PopupMenuItem::new(tr(locale, "status.gitDelete"))
                                    .icon(Icon::new(crate::icons::TrashIcon))
                                    .on_click(move |_, _, cx| {
                                        a_del.update(cx, |s, cx| s.delete_git_branch(b_d.clone(), cx));
                                    }),
                            );
                        }
                        sub
                    });

                    let label_str = if is_current {
                        format!("{} (HEAD)", b_name)
                    } else {
                        b_name.clone()
                    };

                    menu = menu.item(
                        PopupMenuItem::submenu(SharedString::from(label_str), sub_menu)
                            .checked(is_current),
                    );
                }
                menu
            });

        let app_graph = app_state.clone();
        let graph_button = Button::new("sb-git-graph-btn")
            .ghost()
            .small()
            .h(px(26.))
            .icon(IconName::Network)
            .label(tr(locale, "git.graph"))
            .tooltip(tr(locale, "git.graphTooltip"))
            .on_click(move |_, _, cx| {
                app_graph.update(cx, |s, cx| {
                    s.toggle_git_graph(cx);
                });
            });

        bar = bar.left(Separator::vertical()).left(branch_button).left(graph_button);
    }

    bar.right(
        h_flex()
            .items_center()
            .gap_2()
            .child({
                let app_agy = app_for_agy.clone();
                if let Some((pct_opt, account_name)) = agy_quota_badge {
                    let tip = match pct_opt {
                        Some(p) => format!("Antigravity: {account_name} ({p}%) — Click to manage"),
                        None => format!("Antigravity: {account_name} — Click to manage"),
                    };
                    h_flex()
                        .id("sb-agy-link")
                        .h(px(26.))
                        .px_2()
                        .gap_1p5()
                        .items_center()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().secondary))
                        .tooltip(move |w, cx| Tooltip::new(tip.clone()).build(w, cx))
                        .on_click(move |_, _, cx| {
                            app_agy.update(cx, |s, cx| {
                                s.open_agy_link_modal(cx);
                            });
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_medium()
                                .text_color(cx.theme().muted_foreground)
                                .child("AGY"),
                        )
                        .when_some(pct_opt, |el, pct| {
                            let pct_clamped = pct.clamp(0, 100);
                            let ring_color = if pct_clamped >= 66 {
                                cx.theme().success
                            } else if pct_clamped >= 33 {
                                cx.theme().warning
                            } else {
                                cx.theme().danger
                            };
                            el.child(
                                ProgressCircle::new("sb-agy-circle")
                                    .value((pct_clamped as f32).max(4.0))
                                    .size(px(14.))
                                    .color(ring_color),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(cx.theme().foreground)
                                    .child(format!("{pct_clamped}%")),
                            )
                        })
                        .when(pct_opt.is_none(), |el| {
                            el.child(Icon::new(IconName::Bot).xsmall())
                        })
                        .into_any_element()
                } else {
                    Button::new("sb-agy-link-loading")
                        .ghost()
                        .small()
                        .h(px(26.))
                        .tooltip("Loading Antigravity (agy-link) Pool…")
                        .on_click(move |_, _, cx| {
                            app_agy.update(cx, |s, cx| {
                                s.open_agy_link_modal(cx);
                            });
                        })
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    Spinner::new()
                                        .icon(Icon::new(crate::icons::LoaderCircleIcon))
                                        .xsmall()
                                        .color(cx.theme().muted_foreground),
                                )
                                .child("Loading AGY"),
                        )
                        .into_any_element()
                }
            })
            .child(div().w(px(1.)).h(px(14.)).bg(cx.theme().border))
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

pub fn get_local_git_branches(cwd: &std::path::Path) -> Vec<String> {
    let Ok(out) = crate::client::hidden_command("git")
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

pub fn format_compact_cwd_path(path: &std::path::Path) -> String {
    let home = crate::client::native_index::user_home();
    format_compact_cwd_path_with_home(path, Some(&home))
}

fn format_compact_cwd_path_with_home(
    path: &std::path::Path,
    home: Option<&std::path::Path>,
) -> String {
    let raw = path.to_string_lossy().into_owned();
    let sep = if raw.contains('\\') && !raw.contains('/') {
        '\\'
    } else {
        '/'
    };

    // Replace home prefix with `~` when applicable
    let display = if let Some(h) = home {
        let h_str = h.to_string_lossy();
        let h_norm = h_str.trim_end_matches(['/', '\\']);
        let raw_norm = raw.trim_end_matches(['/', '\\']);
        if !h_norm.is_empty() && raw_norm == h_norm {
            return "~".to_string();
        } else if !h_norm.is_empty()
            && raw.starts_with(h_norm)
            && raw
                .as_bytes()
                .get(h_norm.len())
                .is_some_and(|&b| b == b'/' || b == b'\\')
        {
            format!("~{}", &raw[h_norm.len()..])
        } else {
            raw
        }
    } else {
        raw
    };

    if display.chars().count() <= 34 {
        return display;
    }

    // Split into prefix (drive/root/`~`) and normal segments, handling both `/` and `\`
    let (prefix, rest) = if let Some(after_tilde) = display.strip_prefix('~') {
        ("~".to_string(), after_tilde.trim_start_matches(['/', '\\']))
    } else if display.starts_with("\\\\") {
        // Windows UNC: \\server\share\...
        ("\\\\".to_string(), display.trim_start_matches('\\'))
    } else if display.len() >= 2 && display.as_bytes()[1] == b':' {
        let drive = &display[..2];
        let after = display[2..].trim_start_matches(['/', '\\']);
        (format!("{drive}{sep}"), after)
    } else if display.starts_with('/') {
        ("/".to_string(), display.trim_start_matches('/'))
    } else {
        (String::new(), display.as_str())
    };

    let parts: Vec<&str> = rest
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect();

    if parts.len() <= 2 {
        return display;
    }

    let tail_two = format!("{}{sep}{}", parts[parts.len() - 2], parts[parts.len() - 1]);
    let candidate = if prefix.is_empty() {
        format!("…{sep}{tail_two}")
    } else if prefix == "~" {
        format!("~{sep}…{sep}{tail_two}")
    } else {
        format!("{prefix}…{sep}{tail_two}")
    };

    if candidate.chars().count() <= 40 {
        candidate
    } else {
        format!("…{sep}{}", parts[parts.len() - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn formats_compact_paths_across_platforms() {
        let home = Path::new("/Users/charles");
        assert_eq!(
            format_compact_cwd_path_with_home(Path::new("/Users/charles"), Some(home)),
            "~"
        );
        assert_eq!(
            format_compact_cwd_path_with_home(
                Path::new("/Users/charles/Documents/Projects/very-long-workspace/pi-agent-desktop"),
                Some(home)
            ),
            "~/…/very-long-workspace/pi-agent-desktop"
        );
        assert_eq!(
            format_compact_cwd_path_with_home(
                Path::new("/Volumes/Ext/dev/Rust/pi-agent-desktop"),
                Some(home)
            ),
            "/…/Rust/pi-agent-desktop"
        );
        assert_eq!(
            format_compact_cwd_path_with_home(
                Path::new(r"C:\Users\Charles\Documents\Projects\my-app\sub-dir"),
                None
            ),
            r"C:\…\my-app\sub-dir"
        );
    }
}
