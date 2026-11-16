use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::client::session_controller::format_compact_relative_time;
use crate::client::types::WorkspaceKind;
use crate::i18n::{AppLocale, format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, PathPromptOptions, Render, SharedString, StatefulInteractiveElement as _,
    Styled, Window, div, px,
};
use std::path::PathBuf;

#[derive(Clone)]
struct DragProjectGroup {
    name: String,
}

impl Render for DragProjectGroup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .px_3()
            .py_1p5()
            .gap_2()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().primary)
            .bg(cx.theme().popover)
            .shadow_md()
            .text_xs()
            .font_medium()
            .text_color(cx.theme().foreground)
            .child(Icon::new(IconName::Folder).xsmall().text_color(cx.theme().primary))
            .child(SharedString::from(self.name.clone()))
    }
}

pub struct InlineRenameState {
    pub session_id: String,
    pub input: Entity<InputState>,
}

pub fn render_sidebar(
    app_state: &Entity<AppState>,
    rename_state: &mut Option<InlineRenameState>,
    window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement + use<> {
    let state = app_state.read(cx);
    let active_page = state.active_page.clone();
    let active_sid = state.active_session_id.clone();
    let renaming_sid = state.renaming_session_id.clone();

    match (&renaming_sid, &rename_state) {
        (Some(sid), Some(existing)) if &existing.session_id == sid => {}
        (Some(sid), _) => {
            let initial = state
                .sessions
                .iter()
                .find(|s| &s.id == sid)
                .map(|s| s.display_title().to_string())
                .unwrap_or_default();
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Session name...")
                    .default_value(initial)
            });
            let app_commit = app_state.clone();
            let sid_commit = sid.clone();
            cx.subscribe(&input, move |_, st, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let val = st.read(cx).value().to_string();
                    app_commit.update(cx, |s, cx| {
                        s.rename_session_by_id(sid_commit.clone(), val, cx);
                    });
                }
            })
            .detach();
            *rename_state = Some(InlineRenameState {
                session_id: sid.clone(),
                input,
            });
        }
        (None, Some(_)) => {
            *rename_state = None;
        }
        (None, None) => {}
    }

    let state = app_state.read(cx);
    let locale = state.locale;
    let projects_collapsed = state.projects_section_collapsed;
    let chats_collapsed = state.chats_section_collapsed;
    let unread_ids = &state.unread_session_ids;
    let running_ids = &state.running_session_ids;

    let pinned: Vec<_> = state
        .sessions
        .iter()
        .filter(|s| s.pinned && !s.archived && !s.is_subagent())
        .map(|s| {
            let is_run = s.is_running || running_ids.contains(&s.id);
            let is_unr = unread_ids.contains(&s.id);
            let rel_time = format_compact_relative_time(s.updated_at.as_deref());
            (s.id.clone(), s.display_title(), is_run, is_unr, s.pinned, rel_time)
        })
        .collect();

    let chats: Vec<_> = state
        .sessions
        .iter()
        .filter(|s| {
            s.workspace_kind == Some(WorkspaceKind::Chat)
                && !s.pinned
                && !s.archived
                && !s.is_subagent()
        })
        .map(|s| {
            let is_run = s.is_running || running_ids.contains(&s.id);
            let is_unr = unread_ids.contains(&s.id);
            let rel_time = format_compact_relative_time(s.updated_at.as_deref());
            (s.id.clone(), s.display_title(), is_run, is_unr, s.pinned, rel_time)
        })
        .collect();

    let grouped = state.grouped_project_sessions();
    let mut project_groups: Vec<(
        String,
        bool,
        Option<PathBuf>,
        Vec<(String, SharedString, bool, bool, bool, SharedString)>,
    )> = grouped
        .into_iter()
        .map(|(proj, list)| {
            let is_proj_collapsed = state.collapsed_projects.contains(&proj);
            let proj_cwd = list
                .first()
                .and_then(|s| s.cwd.as_ref())
                .map(PathBuf::from);
            let rows = list
                .into_iter()
                .map(|s| {
                    let is_run = s.is_running || running_ids.contains(&s.id);
                    let is_unr = unread_ids.contains(&s.id);
                    let rel_time = format_compact_relative_time(s.updated_at.as_deref());
                    (s.id.clone(), s.display_title(), is_run, is_unr, s.pinned, rel_time)
                })
                .collect();
            (proj, is_proj_collapsed, proj_cwd, rows)
        })
        .collect();
    if !state.project_order.is_empty() {
        let order = &state.project_order;
        project_groups.sort_by(|a, b| {
            let pos_a = order.iter().position(|n| n == &a.0).unwrap_or(usize::MAX);
            let pos_b = order.iter().position(|n| n == &b.0).unwrap_or(usize::MAX);
            pos_a.cmp(&pos_b).then_with(|| a.0.cmp(&b.0))
        });
    }

    let app_new_proj = app_state.clone();
    let app_tasks = app_state.clone();
    let app_sched = app_state.clone();
    let app_arts = app_state.clone();

    let tasks_count = state
        .tasks
        .iter()
        .filter(|t| t.status == "running" || t.status == "waiting")
        .count();
    let sched_count = state.scheduled_tasks.len();
    let arts_count = state.artifacts.len();

    let mut scroll_body = v_flex()
        .id("sidebar-tree-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px_2()
        .py_1()
        .gap_1()
        // Top-level Views aligned uniformly at px_2p5
        .child(build_nav_row(
            "nav-tasks",
            Icon::new(IconName::CircleCheck).small(),
            tr(locale, "sidebar.tasks").to_string(),
            active_page == WorkspacePage::Tasks,
            cx,
            state.has_unread_completed_tasks,
            move |_, _, cx| {
                app_tasks.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Tasks;
                    s.mark_tasks_read(cx);
                    s.refresh_tasks_from_sidecar(cx);
                });
            },
        ))
        .child(build_nav_row(
            "nav-scheduled",
            Icon::new(IconName::Calendar).small(),
            tr(locale, "sidebar.scheduled").to_string(),
            active_page == WorkspacePage::Scheduled,
            cx,
            false,
            move |_, _, cx| {
                app_sched.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Scheduled;
                    s.refresh_scheduled_tasks(cx);
                    cx.notify();
                });
            },
        ))
        .child(build_nav_row(
            "nav-artifacts",
            Icon::new(IconName::Frame).small(),
            tr(locale, "sidebar.artifacts").to_string(),
            active_page == WorkspacePage::Artifacts,
            cx,
            false,
            move |_, _, cx| {
                app_arts.update(cx, |s, cx| {
                    s.active_page = WorkspacePage::Artifacts;
                    s.refresh_artifacts_from_sidecar(cx);
                    cx.notify();
                });
            },
        ));

    // Pinned section (uniform text_sm, color differentiation)
    if !pinned.is_empty() {
        scroll_body = scroll_body.child(
            div()
                .px_2p5()
                .pt_2()
                .pb_0p5()
                .text_sm()
                .font_medium()
                .text_color(cx.theme().muted_foreground)
                .child(tr(locale, "sidebar.pinned")),
        );
        for (idx, (sid, title, running, unread, is_pinned, rel_time)) in
            pinned.into_iter().enumerate()
        {
            scroll_body = scroll_body.child(build_session_row(
                ("pinned", idx),
                sid,
                title,
                running,
                unread,
                is_pinned,
                false,
                rel_time,
                active_sid.as_deref(),
                active_page == WorkspacePage::Chat,
                rename_state.as_ref(),
                app_state,
                false,
                locale,
                cx,
            ));
        }
    }

    // Top-level Chats section (aligned left with Tasks/Scheduled/Artifacts, animated expand/collapse, no guide lines)
    let chats_count = chats.len();
    let app_chats_fold = app_state.clone();
    let app_chats_menu = app_state.clone();
    let app_new_chat = app_state.clone();

    let chats_header = h_flex()
        .id("chats-section-hdr")
        .w_full()
        .h(px(30.))
        .px_2p5()
        .mt_2()
        .items_center()
        .justify_between()
        .gap_1p5()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
        .group("chats-section-hdr")
        .on_click(move |_, _, cx| {
            app_chats_fold.update(cx, |s, cx| {
                s.chats_section_collapsed = !s.chats_section_collapsed;
                cx.notify();
            });
        })
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(if chats_collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                )
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                        .child(if chats_collapsed && chats_count > 0 {
                            format!("{} ({})", tr(locale, "sidebar.chats"), format_commas(chats_count))
                        } else {
                            tr(locale, "sidebar.chats").to_string()
                        }),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_0p5()
                .opacity(0.)
                .group_hover("chats-section-hdr", |s| s.opacity(1.))
                .child(
                    Button::new("chats-section-menu")
                        .ghost()
                        .w(px(22.))
                        .h(px(22.))
                        .icon(IconName::Ellipsis)
                        .tooltip(tr(locale, "sidebar.chatsMenu"))
                        .dropdown_menu(move |menu, _, _| {
                            let c1 = app_chats_menu.clone();
                            let c2 = app_chats_menu.clone();
                            menu.item(
                                PopupMenuItem::new(tr(locale, "sidebar.collapseChats"))
                                    .disabled(chats_collapsed)
                                    .on_click(move |_, _, cx| {
                                        c1.update(cx, |s, cx| {
                                            s.chats_section_collapsed = true;
                                            cx.notify();
                                        });
                                    }),
                            )
                            .item(
                                PopupMenuItem::new(tr(locale, "sidebar.expandChats"))
                                    .disabled(!chats_collapsed)
                                    .on_click(move |_, _, cx| {
                                        c2.update(cx, |s, cx| {
                                            s.chats_section_collapsed = false;
                                            cx.notify();
                                        });
                                    }),
                            )
                        }),
                )
                .child(
                    Button::new("chats-section-add")
                        .ghost()
                        .w(px(22.))
                        .h(px(22.))
                        .icon(IconName::Plus)
                        .tooltip(tr(locale, "sidebar.newChat"))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            app_new_chat.update(cx, |s, cx| {
                                s.chats_section_collapsed = false;
                                s.start_new_composer(WorkspaceKind::Chat, cx);
                            });
                        }),
                ),
        );

    scroll_body = scroll_body.child(chats_header);

    if !chats_collapsed {
        let mut chats_list = v_flex().w_full().gap_0p5();
        if chats.is_empty() {
            chats_list = chats_list.child(
                div()
                    .px_2p5()
                    .pl_6()
                    .py_1()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(locale, "sidebar.noChats")),
            );
        } else {
            for (idx, (sid, title, running, unread, is_pinned, rel_time)) in
                chats.into_iter().enumerate()
            {
                chats_list = chats_list.child(build_session_row(
                    ("chat-sess", idx),
                    sid,
                    title,
                    running,
                    unread,
                    is_pinned,
                    false,
                    rel_time,
                    active_sid.as_deref(),
                    active_page == WorkspacePage::Chat,
                    rename_state.as_ref(),
                    app_state,
                    true,
                    locale,
                    cx,
                ));
            }
        }
        scroll_body = scroll_body.child(chats_list);
    }

    // Top-level Projects section (aligned left with Tasks & Chats, animated expand/collapse, no guide lines)
    let proj_count = project_groups.len();
    let app_proj_fold = app_state.clone();
    let app_proj_menu = app_state.clone();
    let app_proj_add = app_state.clone();

    let projects_header = h_flex()
        .id("projects-section-hdr")
        .w_full()
        .h(px(30.))
        .px_2p5()
        .mt_2()
        .items_center()
        .justify_between()
        .gap_1p5()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
        .group("projects-section-hdr")
        .on_click(move |_, _, cx| {
            app_proj_fold.update(cx, |s, cx| {
                s.projects_section_collapsed = !s.projects_section_collapsed;
                cx.notify();
            });
        })
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(if projects_collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronDown
                    })
                    .xsmall(),
                )
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} ({})", tr(locale, "sidebar.projects"), format_commas(proj_count))),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_0p5()
                .opacity(0.)
                .group_hover("projects-section-hdr", |s| s.opacity(1.))
                .child(
                    Button::new("projects-section-menu")
                        .ghost()
                        .w(px(22.))
                        .h(px(22.))
                        .icon(IconName::Ellipsis)
                        .tooltip(tr(locale, "sidebar.projectsMenu"))
                        .dropdown_menu(move |menu, _, _| {
                            let c = app_proj_menu.clone();
                            let e = app_proj_menu.clone();
                            menu.item(
                                PopupMenuItem::new(tr(locale, "sidebar.collapseAllProjects")).on_click(
                                    move |_, _, cx| {
                                        c.update(cx, |s, cx| s.collapse_all_projects(cx));
                                    },
                                ),
                            )
                            .item(
                                PopupMenuItem::new(tr(locale, "sidebar.expandAllProjects")).on_click(
                                    move |_, _, cx| {
                                        e.update(cx, |s, cx| {
                                            s.projects_section_collapsed = false;
                                            s.expand_all_projects(cx);
                                        });
                                    },
                                ),
                            )
                        }),
                )
                .child(
                    Button::new("projects-section-add")
                        .ghost()
                        .w(px(22.))
                        .h(px(22.))
                        .icon(IconName::Plus)
                        .tooltip(tr(locale, "sidebar.addProject"))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            let prompt_text = tr(locale, "sidebar.selectProjectFolder").to_string();
                            let rx = cx.prompt_for_paths(PathPromptOptions {
                                files: false,
                                directories: true,
                                multiple: false,
                                prompt: Some(prompt_text.into()),
                            });
                            let app_p = app_proj_add.clone();
                            cx.spawn(async move |cx| {
                                if let Ok(Ok(Some(paths))) = rx.await {
                                    if let Some(dir) = paths.into_iter().next() {
                                        let _ = app_p.update(cx, |s, cx| {
                                            s.add_project_path(dir, cx);
                                        });
                                    }
                                }
                            })
                            .detach();
                        }),
                ),
        );

    scroll_body = scroll_body.child(projects_header);

    if !projects_collapsed {
        let mut projects_list = v_flex().w_full().gap_0p5();

        for (p_idx, (proj_name, proj_collapsed, proj_cwd, rows)) in
            project_groups.into_iter().enumerate()
        {
            let app_toggle = app_state.clone();
            let proj_key = proj_name.clone();
            let row_count = rows.len();
            let proj_running_count = rows.iter().filter(|r| r.2).count();
            let proj_unread_count = rows.iter().filter(|r| r.3).count();

            let label = SharedString::from(proj_name.clone());

            let app_menu = app_state.clone();
            let app_ctx = app_state.clone();
            let app_drop = app_state.clone();
            let proj_for_drag = proj_name.clone();
            let proj_for_drop = proj_name.clone();
            let proj_for_menu = proj_name.clone();
            let proj_for_ctx = proj_name.clone();
            let cwd_for_menu = proj_cwd.clone();
            let cwd_for_ctx = proj_cwd.clone();
            let proj_group = SharedString::from(format!("proj-row-{p_idx}"));
            let proj_group_hover = proj_group.clone();
            let proj_tip = label.clone();

            let proj_row = h_flex()
                .id(("proj-row", p_idx))
                .w_full()
                .h(px(30.))
                .px_2p5()
                .items_center()
                .justify_between()
                .gap_1p5()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
                .group(proj_group.clone())
                .on_drag(DragProjectGroup { name: proj_for_drag }, |drag, _, _, cx| {
                    let d = drag.clone();
                    cx.new(|_| d)
                })
                .drag_over::<DragProjectGroup>(|style, _, _, cx| {
                    style.border_t_2().border_color(cx.theme().primary)
                })
                .on_drop(move |drag: &DragProjectGroup, _, cx| {
                    let from_name = drag.name.clone();
                    let to_name = proj_for_drop.clone();
                    app_drop.update(cx, |s, cx| {
                        s.reorder_project_group(&from_name, &to_name, cx);
                    });
                })
                .on_click(move |_, _, cx| {
                    app_toggle.update(cx, |s, cx| {
                        if s.collapsed_projects.contains(&proj_key) {
                            s.collapsed_projects.remove(&proj_key);
                        } else {
                            s.collapsed_projects.insert(proj_key.clone());
                        }
                        cx.notify();
                    });
                })
                .context_menu(move |menu, _, _| {
                    populate_project_popup_menu(menu, &app_ctx, &proj_for_ctx, &cwd_for_ctx, locale)
                })
                .child(
                    h_flex()
                        .id(("proj-lbl", p_idx))
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .tooltip(move |w, cx| {
                            let tip = proj_tip.clone();
                            Tooltip::element(move |_, _| {
                                div()
                                    .max_w(px(240.))
                                    .whitespace_normal()
                                    .text_xs()
                                    .child(tip.clone())
                            })
                            .build(w, cx)
                        })
                        .child(
                            Icon::new(if proj_collapsed {
                                IconName::FolderClosed
                            } else {
                                IconName::FolderOpen
                            })
                            .small(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .font_medium()
                                .child(label),
                        ),
                )
                .when_proj_activity(proj_collapsed, proj_running_count, proj_unread_count, cx)
                .child(
                    div()
                        .flex_shrink_0()
                        .opacity(0.)
                        .group_hover(proj_group_hover.clone(), |s| s.opacity(1.))
                        .child(
                            Button::new(("proj-menu-btn", p_idx))
                                .ghost()
                                .w(px(22.))
                                .h(px(22.))
                                .icon(IconName::Ellipsis)
                                .tooltip(tr(locale, "sidebar.projectActions"))
                                .dropdown_menu(move |menu, _, _| {
                                    populate_project_popup_menu(
                                        menu,
                                        &app_menu,
                                        &proj_for_menu,
                                        &cwd_for_menu,
                                        locale,
                                    )
                                }),
                        ),
                );

            projects_list = projects_list.child(proj_row);

            if !proj_collapsed {
                let mut children_col = v_flex().w_full().gap_0p5();
                for (s_idx, (sid, title, running, unread, is_pinned, rel_time)) in
                    rows.into_iter().enumerate()
                {
                    children_col = children_col.child(build_session_row(
                        ("proj-sess", p_idx * 1000 + s_idx),
                        sid,
                        title,
                        running,
                        unread,
                        is_pinned,
                        false,
                        rel_time,
                        active_sid.as_deref(),
                        active_page == WorkspacePage::Chat,
                        rename_state.as_ref(),
                        app_state,
                        true,
                        locale,
                        cx,
                    ));
                }
                projects_list = projects_list.child(children_col);
            }
        }

        scroll_body = scroll_body.child(projects_list);
    }

    let archived_list: Vec<_> = state
        .sessions
        .iter()
        .filter(|s| !s.is_subagent() && (s.archived || state.archived_session_ids.contains(&s.id)))
        .map(|s| {
            let is_run = s.is_running || running_ids.contains(&s.id);
            let is_unr = unread_ids.contains(&s.id);
            let rel_time = format_compact_relative_time(s.updated_at.as_deref());
            (s.id.clone(), s.display_title(), is_run, is_unr, s.pinned, rel_time)
        })
        .collect();

    if !archived_list.is_empty() {
        scroll_body = scroll_body.child(
            v_flex()
                .w_full()
                .gap_1()
                .pt_3()
                .child(
                    h_flex()
                        .px_2p5()
                        .py_1()
                        .items_center()
                        .gap_1p5()
                        .child(Icon::new(crate::icons::ArchiveIcon).xsmall().text_color(cx.theme().muted_foreground))
                        .child(
                            div()
                                .text_xs()
                                .font_semibold()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} ({})", tr(locale, "sidebar.archivedGroup"), format_commas(archived_list.len()))),
                        ),
                )
                .children(archived_list.into_iter().enumerate().map(|(a_idx, (sid, title, is_run, is_unr, is_pin, rel_time))| {
                    build_session_row(
                        ("arch-sess", a_idx),
                        sid,
                        title,
                        is_run,
                        is_unr,
                        is_pin,
                        true,
                        rel_time,
                        active_sid.as_deref(),
                        active_page == WorkspacePage::Chat,
                        rename_state.as_ref(),
                        app_state,
                        false,
                        locale,
                        cx,
                    )
                })),
        );
    }

    v_flex()
        .w(px(268.))
        .h_full()
        .bg(cx.theme().sidebar)
        .border_r_1()
        .border_color(cx.theme().sidebar_border)
        .child(
            div().px_3().pt_3().pb_2().child({
                let app_chat = app_new_proj.clone();
                let app_subproj = app_new_proj.clone();
                let app_temp = app_new_proj.clone();
                Button::new("new-session-btn")
                    .primary()
                    .small()
                    .h(px(30.))
                    .w_full()
                    .icon(IconName::Plus)
                    .label(format!("{}  ▾", tr(locale, "sidebar.newSession")))
                    .dropdown_menu(move |menu, _, _| {
                        let app_c = app_chat.clone();
                        let app_p = app_subproj.clone();
                        let app_t = app_temp.clone();
                        menu.item(
                            PopupMenuItem::new(tr(locale, "sidebar.newStandaloneChat"))
                                .on_click(move |_, _, cx| {
                                    app_c.update(cx, |s, cx| {
                                        s.chats_section_collapsed = false;
                                        s.start_new_composer(WorkspaceKind::Chat, cx);
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr(locale, "sidebar.newSubproject"))
                                .on_click(move |_, _, cx| {
                                    let prompt_text =
                                        tr(locale, "sidebar.selectProjectFolder").to_string();
                                    let rx = cx.prompt_for_paths(PathPromptOptions {
                                        files: false,
                                        directories: true,
                                        multiple: false,
                                        prompt: Some(prompt_text.into()),
                                    });
                                    let app_p2 = app_p.clone();
                                    cx.spawn(async move |cx| {
                                        if let Ok(Ok(Some(paths))) = rx.await {
                                            if let Some(dir) = paths.into_iter().next() {
                                                let _ = app_p2.update(cx, |s, cx| {
                                                    s.add_project_path(dir, cx);
                                                });
                                            }
                                        }
                                    })
                                    .detach();
                                }),
                        )
                        .item(
                            PopupMenuItem::new(tr(locale, "sidebar.newTempChat"))
                                .on_click(move |_, _, cx| {
                                    app_t.update(cx, |s, cx| {
                                        s.chats_section_collapsed = false;
                                        s.start_new_temp_chat(cx);
                                    });
                                }),
                        )
                    })
            }),
        )
        .child(scroll_body)
}

fn build_nav_row(
    id: &'static str,
    leading: impl IntoElement,
    label: String,
    active: bool,
    cx: &gpui_kit::App,
    has_unread: bool,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    h_flex()
        .id(id)
        .w_full()
        .h(px(30.))
        .px_2p5()
        .gap_2()
        .items_center()
        .rounded_md()
        .cursor_pointer()
        .bg(if active {
            cx.theme().sidebar_accent
        } else {
            cx.theme().transparent
        })
        .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.75)))
        .on_click(on_click)
        .child(leading)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(if active {
                    cx.theme().sidebar_accent_foreground
                } else {
                    cx.theme().sidebar_foreground
                })
                .child(SharedString::from(label)),
        )
        .children(has_unread.then(|| {
            div()
                .w(px(6.))
                .h(px(6.))
                .flex_shrink_0()
                .rounded_full()
                .bg(cx.theme().success)
        }))
        .into_any_element()
}

fn populate_project_popup_menu(
    menu: PopupMenu,
    app_state: &Entity<AppState>,
    proj_name: &str,
    proj_cwd: &Option<PathBuf>,
    locale: AppLocale,
) -> PopupMenu {
    let app_new = app_state.clone();
    let app_term = app_state.clone();
    let app_arch = app_state.clone();
    let cwd_new = proj_cwd.clone();
    let cwd_term = proj_cwd.clone();
    let cwd_open = proj_cwd.clone();
    let p_arch = proj_name.to_string();

    menu.item(
        PopupMenuItem::new(tr(locale, "sidebar.newSessionInProject"))
            .icon(IconName::Plus)
            .on_click(move |_, _, cx| {
                app_new.update(cx, |s, cx| {
                    if let Some(c) = &cwd_new {
                        s.active_cwd = c.clone();
                    }
                    s.start_new_composer(WorkspaceKind::Project, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(tr(locale, "sidebar.openTerminal"))
            .icon(IconName::SquareTerminal)
            .on_click(move |_, _, cx| {
                app_term.update(cx, |s, cx| {
                    if let Some(c) = &cwd_term {
                        s.active_cwd = c.clone();
                    }
                    s.open_right_tab(RightPanelTab::Terminal, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(tr(locale, "sidebar.revealFinder"))
            .icon(IconName::ExternalLink)
            .on_click(move |_, _, _| {
                if let Some(c) = &cwd_open {
                    let _ = open::that(c);
                }
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new(tr(locale, "sidebar.archiveProject"))
            .icon(IconName::FolderClosed)
            .on_click(move |_, _, cx| {
                app_arch.update(cx, |s, cx| {
                    s.archived_projects.insert(p_arch.clone());
                    cx.notify();
                });
            }),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_session_row(
    id_prefix: (&'static str, usize),
    sid: String,
    title: SharedString,
    running: bool,
    unread: bool,
    is_pinned: bool,
    is_archived: bool,
    rel_time: SharedString,
    active_sid: Option<&str>,
    on_chat_page: bool,
    rename_state: Option<&InlineRenameState>,
    app_state: &Entity<AppState>,
    indented: bool,
    locale: AppLocale,
    cx: &gpui_kit::App,
) -> AnyElement {
    let is_active = active_sid == Some(sid.as_str()) && on_chat_page;
    let is_renaming = rename_state.is_some_and(|r| r.session_id == sid);
    let is_temp = app_state.read(cx).temp_chat_ids.contains(&sid);

    if is_renaming {
        let input_entity = rename_state.unwrap().input.clone();
        let app_save = app_state.clone();
        let app_cancel = app_state.clone();
        let sid_save = sid.clone();
        let input_for_save = input_entity.clone();

        return h_flex()
            .id(("sess-ren", id_prefix.1))
            .w_full()
            .h(px(30.))
            .px_2p5()
            .when_indented(indented)
            .items_center()
            .gap_1()
            .rounded_md()
            .bg(cx.theme().sidebar_accent)
            .child(div().flex_1().min_w_0().child(Input::new(&input_entity).xsmall()))
            .child(
                Button::new((id_prefix.0, id_prefix.1 * 10 + 1))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Check)
                    .on_click(move |_, _, cx| {
                        let val = input_for_save.read(cx).value().to_string();
                        app_save.update(cx, |s, cx| {
                            s.rename_session_by_id(sid_save.clone(), val, cx);
                        });
                    }),
            )
            .child(
                Button::new((id_prefix.0, id_prefix.1 * 10 + 2))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .on_click(move |_, _, cx| {
                        app_cancel.update(cx, |s, cx| {
                            s.renaming_session_id = None;
                            cx.notify();
                        });
                    }),
            )
            .into_any_element();
    }

    let app_sel = app_state.clone();
    let app_menu = app_state.clone();
    let app_ctx = app_state.clone();
    let sid_click = sid.clone();
    let sid_ctx = sid.clone();
    let sid_m = sid.clone();
    let full_title = title.clone();
    let sess_group = SharedString::from(format!("sess-row-{}-{}", id_prefix.0, id_prefix.1));
    let sess_group_hover = sess_group.clone();
    let sess_group_time = sess_group.clone();
    let has_time = !rel_time.is_empty();

    h_flex()
        .id((id_prefix.0, id_prefix.1 * 100 + 7))
        .w_full()
        .h(px(30.))
        .px_2p5()
        .when_indented(indented)
        .items_center()
        .justify_between()
        .gap_2()
        .rounded_md()
        .cursor_pointer()
        .bg(if is_active {
            cx.theme().sidebar_accent
        } else {
            cx.theme().transparent
        })
        .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.75)))
        .group(sess_group.clone())
        .on_click(move |_, _, cx| {
            app_sel.update(cx, |s, cx| {
                if s.active_session_id.as_deref() != Some(sid_click.as_str()) || s.active_page != WorkspacePage::Chat {
                    s.select_session(sid_click.clone(), cx);
                }
            });
        })
        .context_menu(move |menu, _, _| {
            populate_session_popup_menu(menu, &app_ctx, &sid_ctx, is_pinned, is_archived, is_temp, locale)
        })
        .when_session_status_badge(running, unread, cx)
        .children(is_temp.then(|| {
            Icon::new(crate::icons::ClockIcon)
                .xsmall()
                .text_color(cx.theme().muted_foreground)
        }))
        .child(
            div()
                .id(("sess-title", id_prefix.1))
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(if is_active {
                    cx.theme().sidebar_accent_foreground
                } else {
                    cx.theme().sidebar_foreground
                })
                .tooltip(move |w, cx| {
                    let tip = full_title.clone();
                    Tooltip::element(move |_, _| {
                        div()
                            .max_w(px(240.))
                            .whitespace_normal()
                            .text_xs()
                            .child(tip.clone())
                    })
                    .build(w, cx)
                })
                .child(title),
        )
        .child(
            div()
                .relative()
                .flex_shrink_0()
                .min_w(px(32.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_end()
                .when_rel_time(has_time, rel_time, sess_group_time, cx)
                .child(
                    div()
                        .opacity(0.)
                        .group_hover(sess_group_hover, |s| s.opacity(1.))
                        .child(
                            Button::new((id_prefix.0, id_prefix.1))
                                .ghost()
                                .w(px(22.))
                                .h(px(22.))
                                .icon(IconName::Ellipsis)
                                .tooltip(tr(locale, "sidebar.sessionActions"))
                                .dropdown_menu(move |menu, _, _| {
                                    populate_session_popup_menu(
                                        menu,
                                        &app_menu,
                                        &sid_m,
                                        is_pinned,
                                        is_archived,
                                        is_temp,
                                        locale,
                                    )
                                }),
                        ),
                ),
        )
        .into_any_element()
}

trait SidebarActivityExt {
    fn when_indented(self, indented: bool) -> Self;
    fn when_session_status_badge(
        self,
        running: bool,
        unread: bool,
        cx: &gpui_kit::App,
    ) -> Self;
    fn when_proj_activity(
        self,
        collapsed: bool,
        running_count: usize,
        unread_count: usize,
        cx: &gpui_kit::App,
    ) -> Self;
    fn when_rel_time(
        self,
        show: bool,
        time: SharedString,
        group: SharedString,
        cx: &gpui_kit::App,
    ) -> Self;
}

impl<E: ParentElement + Styled> SidebarActivityExt for E {
    fn when_indented(self, indented: bool) -> Self {
        if indented {
            self.pl(px(22.))
        } else {
            self
        }
    }

    fn when_session_status_badge(self, running: bool, unread: bool, cx: &gpui_kit::App) -> Self {
        if running {
            self.child(
                div()
                    .flex_shrink_0()
                    .child(Spinner::new().icon(Icon::new(crate::icons::LoaderCircleIcon)).xsmall().color(cx.theme().primary)),
            )
        } else if unread {
            self.child(
                div()
                    .flex_shrink_0()
                    .w(px(7.))
                    .h(px(7.))
                    .rounded_full()
                    .bg(cx.theme().primary),
            )
        } else {
            self
        }
    }

    fn when_proj_activity(
        self,
        collapsed: bool,
        running_count: usize,
        unread_count: usize,
        cx: &gpui_kit::App,
    ) -> Self {
        if !collapsed || (running_count == 0 && unread_count == 0) {
            return self;
        }
        let mut row = h_flex().items_center().gap_1p5().flex_shrink_0();
        if running_count > 0 {
            row = row.child(
                h_flex()
                    .items_center()
                    .gap_0p5()
                    .text_xs()
                    .text_color(cx.theme().primary)
                    .child(Spinner::new().icon(Icon::new(crate::icons::LoaderCircleIcon)).xsmall().color(cx.theme().primary))
                    .child(running_count.to_string()),
            );
        }
        if unread_count > 0 {
            row = row.child(
                h_flex()
                    .items_center()
                    .gap_0p5()
                    .text_xs()
                    .text_color(cx.theme().primary)
                    .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(cx.theme().primary))
                    .child(unread_count.to_string()),
            );
        }
        self.child(row)
    }

    fn when_rel_time(
        self,
        show: bool,
        time: SharedString,
        group: SharedString,
        cx: &gpui_kit::App,
    ) -> Self {
        if !show {
            return self;
        }
        self.child(
            div()
                .absolute()
                .right_0()
                .whitespace_nowrap()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .group_hover(group, |s| s.opacity(0.))
                .child(time),
        )
    }
}

fn populate_session_popup_menu(
    menu: PopupMenu,
    app_state: &Entity<AppState>,
    sid: &str,
    is_pinned: bool,
    is_archived: bool,
    is_temp: bool,
    locale: AppLocale,
) -> PopupMenu {
    let app_ren = app_state.clone();
    let app_fork = app_state.clone();
    let app_pin = app_state.clone();
    let app_del = app_state.clone();
    let app_arch = app_state.clone();
    let app_keep = app_state.clone();
    let app_rel = app_state.clone();
    let sid_r = sid.to_string();
    let sid_f = sid.to_string();
    let sid_p = sid.to_string();
    let sid_d = sid.to_string();
    let sid_a = sid.to_string();
    let sid_k = sid.to_string();
    let sid_rel = sid.to_string();

    let mut menu = menu.item(
        PopupMenuItem::new(tr(locale, "sidebar.rename"))
            .icon(IconName::Settings2)
            .on_click(move |_, _, cx| {
                app_ren.update(cx, |s, cx| {
                    s.renaming_session_id = Some(sid_r.clone());
                    cx.notify();
                });
            }),
    )
    .item(
        PopupMenuItem::new(tr(locale, "sidebar.fork"))
            .icon(IconName::Copy)
            .on_click(move |_, _, cx| {
                app_fork.update(cx, |s, cx| {
                    s.fork_session_by_id(sid_f.clone(), cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(if is_pinned {
            tr(locale, "sidebar.unpin")
        } else {
            tr(locale, "sidebar.pin")
        })
        .icon(if is_pinned {
            IconName::StarOff
        } else {
            IconName::Star
        })
        .on_click(move |_, _, cx| {
            app_pin.update(cx, |s, cx| {
                s.toggle_pin_session(sid_p.clone(), cx);
            });
        }),
    )
    .separator()
    .item(
        PopupMenuItem::new(if is_archived {
            tr(locale, "sidebar.unarchiveSession")
        } else {
            tr(locale, "sidebar.archiveSession")
        })
        .icon(Icon::new(crate::icons::ArchiveIcon))
        .on_click(move |_, _, cx| {
            app_arch.update(cx, |s, cx| {
                s.toggle_archive_session(sid_a.clone(), cx);
            });
        }),
    )
    .item(
        PopupMenuItem::new(tr(locale, "sidebar.delete"))
            .icon(Icon::new(crate::icons::TrashIcon))
            .on_click(move |_, _, cx| {
                app_del.update(cx, |s, cx| {
                    s.delete_session_by_id(sid_d.clone(), cx);
                });
            }),
    );
    if is_temp {
        menu = menu
            .separator()
            .item(
                PopupMenuItem::new(tr(locale, "sidebar.keepTempChat"))
                    .icon(IconName::Check)
                    .on_click(move |_, _, cx| {
                        app_keep.update(cx, |s, cx| {
                            s.promote_temp_chat(sid_k.clone(), cx);
                        });
                    }),
            )
            .item(
                PopupMenuItem::new(tr(locale, "sidebar.releaseTempChat"))
                    .icon(Icon::new(crate::icons::TrashIcon))
                    .on_click(move |_, _, cx| {
                        app_rel.update(cx, |s, cx| {
                            s.release_temp_chat(sid_rel.clone(), cx);
                        });
                    }),
            );
    }
    menu
}
