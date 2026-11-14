use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::client::session_controller::format_compact_relative_time;
use crate::client::types::ArtifactKind;
use crate::i18n::{format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
    StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

pub fn render_artifacts_page(
    app_state: &Entity<AppState>,
    search_input: &Entity<InputState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let (all_artifacts, sessions, locale, search_query, type_filter, page_index) = {
        let state = app_state.read(cx);
        (
            state.artifacts.clone(),
            state.sessions.clone(),
            state.locale,
            state.artifacts_search_query.trim().to_lowercase(),
            state.artifacts_type_filter.clone(),
            state.artifacts_page_index,
        )
    };

    let filtered_artifacts: Vec<_> = all_artifacts
        .into_iter()
        .filter(|art| {
            let matches_type = match type_filter.as_str() {
                "html" => matches!(art.kind, ArtifactKind::Html),
                "react" => matches!(art.kind, ArtifactKind::React),
                "svg" => matches!(art.kind, ArtifactKind::Svg),
                "markdown" => matches!(art.kind, ArtifactKind::Markdown),
                "code" => matches!(art.kind, ArtifactKind::Code),
                _ => true,
            };
            if !matches_type {
                return false;
            }

            if search_query.is_empty() {
                return true;
            }
            art.title.to_lowercase().contains(&search_query)
                || art.identifier.to_lowercase().contains(&search_query)
                || art.content.to_lowercase().contains(&search_query)
        })
        .collect();

    let app_tab1 = app_state.clone();
    let app_tab2 = app_state.clone();
    let app_tab3 = app_state.clone();
    let app_tab4 = app_state.clone();
    let app_tab5 = app_state.clone();
    let app_tab6 = app_state.clone();

    let filter_bar = h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex_1()
                .max_w(px(360.))
                .child(Input::new(search_input).h(px(32.))),
        )
        .child(
            TabBar::new("art-type-tabs")
                .segmented()
                .h(px(32.))
                .child(
                    Tab::new()
                        .label(tr(locale, "page.allTypes"))
                        .selected(type_filter == "all")
                        .on_click(move |_, _, cx| {
                            app_tab1.update(cx, |s, cx| s.set_artifacts_type_filter("all".to_string(), cx));
                        }),
                )
                .child(
                    Tab::new()
                        .label("HTML")
                        .selected(type_filter == "html")
                        .on_click(move |_, _, cx| {
                            app_tab2.update(cx, |s, cx| s.set_artifacts_type_filter("html".to_string(), cx));
                        }),
                )
                .child(
                    Tab::new()
                        .label("React")
                        .selected(type_filter == "react")
                        .on_click(move |_, _, cx| {
                            app_tab3.update(cx, |s, cx| s.set_artifacts_type_filter("react".to_string(), cx));
                        }),
                )
                .child(
                    Tab::new()
                        .label("SVG")
                        .selected(type_filter == "svg")
                        .on_click(move |_, _, cx| {
                            app_tab4.update(cx, |s, cx| s.set_artifacts_type_filter("svg".to_string(), cx));
                        }),
                )
                .child(
                    Tab::new()
                        .label("Markdown")
                        .selected(type_filter == "markdown")
                        .on_click(move |_, _, cx| {
                            app_tab5.update(cx, |s, cx| s.set_artifacts_type_filter("markdown".to_string(), cx));
                        }),
                )
                .child(
                    Tab::new()
                        .label("Code")
                        .selected(type_filter == "code")
                        .on_click(move |_, _, cx| {
                            app_tab6.update(cx, |s, cx| s.set_artifacts_type_filter("code".to_string(), cx));
                        }),
                ),
        );

    let mut inner = v_flex()
        .size_full()
        .max_w(px(900.))
        .mx_auto()
        .gap_4()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(tr(locale, "page.artifactsTitle")))
                .child(
                    Tag::secondary().small().child(format!(
                        "{} {}",
                        format_commas(filtered_artifacts.len()),
                        tr(locale, "page.artifactsItems")
                    )),
                ),
        )
        .child(filter_bar);

    if filtered_artifacts.is_empty() {
        inner = inner.child(
            div().flex_1().flex().items_center().justify_center().child(
                Empty::new().header(
                    EmptyHeader::new()
                        .media(EmptyMedia::new().child(Icon::new(IconName::Frame)))
                        .title(EmptyTitle::new().child(tr(locale, "page.noArtifacts")))
                        .description(EmptyDescription::new().child(tr(locale, "page.noArtifactsDesc"))),
                ),
            ),
        );
    } else {
        const PAGE_SIZE: usize = 8;
        let total_count = filtered_artifacts.len();
        let total_pages = (total_count + PAGE_SIZE - 1) / PAGE_SIZE;
        let current_page = page_index.min(total_pages.saturating_sub(1));
        let start_ix = current_page * PAGE_SIZE;
        let end_ix = (start_ix + PAGE_SIZE).min(total_count);
        let page_items = &filtered_artifacts[start_ix..end_ix];

        let mut grid = v_flex()
            .id("art-page-grid")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_2p5();

        for (page_offset, art) in page_items.iter().enumerate() {
            let global_idx = start_ix + page_offset;
            let aid = art.identifier.clone();
            let sid_opt = art.session_id.clone();
            let app_open = app_state.clone();
            let type_icon = match art.kind {
                ArtifactKind::Html | ArtifactKind::React | ArtifactKind::Svg => IconName::Frame,
                ArtifactKind::Markdown => IconName::FileText,
                ArtifactKind::Code | ArtifactKind::Mermaid => IconName::File,
            };

            let session_title = sid_opt
                .as_deref()
                .and_then(|sid| sessions.iter().find(|s| s.id == sid))
                .map(|s| s.display_title().to_string());
            let rel_time = format_compact_relative_time(art.updated_at.as_deref());

            let mut meta_parts = vec![
                art.identifier.clone(),
                format!("v{}", format_commas(art.version)),
            ];
            if let Some(st) = session_title {
                if !st.is_empty() {
                    meta_parts.push(st);
                }
            }
            if !rel_time.is_empty() {
                meta_parts.push(rel_time.to_string());
            }

            grid = grid.child(
                h_flex()
                    .w_full()
                    .p_3p5()
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .hover(|s| s.bg(cx.theme().secondary.opacity(0.45)))
                    .justify_between()
                    .items_center()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(Icon::new(type_icon).small().text_color(cx.theme().primary))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_medium()
                                            .truncate()
                                            .text_color(cx.theme().foreground)
                                            .child(SharedString::from(art.title.clone())),
                                    )
                                    .child(Tag::secondary().small().child(art.kind.label())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(meta_parts.join(" · "))),
                            ),
                    )
                    .child(
                        Button::new(("art-page-open", global_idx))
                            .outline()
                            .small()
                            .h(px(28.))
                            .label(tr(locale, "chat.preview"))
                            .on_click(move |_, _, cx| {
                                let aid_clone = aid.clone();
                                let sid_clone = sid_opt.clone();
                                app_open.update(cx, |s, cx| {
                                    s.active_artifact_id = Some(aid_clone.clone());
                                    if let Some(ref sid) = sid_clone {
                                        if s.active_session_id.as_deref() != Some(sid.as_str()) {
                                            s.select_session(sid.clone(), cx);
                                        } else {
                                            s.active_page = WorkspacePage::Chat;
                                        }
                                    } else {
                                        s.active_page = WorkspacePage::Chat;
                                    }
                                    s.open_right_tab(RightPanelTab::Artifact, cx);
                                });
                            }),
                    ),
            );
        }
        inner = inner.child(grid);

        if total_pages > 1 {
            let app_page_prev = app_state.clone();
            let app_page_next = app_state.clone();
            let pagination = h_flex()
                .w_full()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} ({} - {})",
                            tr(locale, "page.pageOf")
                                .replacen("{}", &(current_page + 1).to_string(), 1)
                                .replacen("{}", &total_pages.to_string(), 1),
                            format_commas(start_ix + 1),
                            format_commas(end_ix),
                        )),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(
                            Button::new("art-prev-page")
                                .outline()
                                .small()
                                .h(px(28.))
                                .icon(IconName::ChevronLeft)
                                .disabled(current_page == 0)
                                .label(tr(locale, "page.prevPage"))
                                .on_click(move |_, _, cx| {
                                    app_page_prev.update(cx, |s, cx| {
                                        if s.artifacts_page_index > 0 {
                                            s.set_artifacts_page_index(s.artifacts_page_index - 1, cx);
                                        }
                                    });
                                }),
                        )
                        .child(
                            Button::new("art-next-page")
                                .outline()
                                .small()
                                .h(px(28.))
                                .icon(IconName::ChevronRight)
                                .disabled(current_page + 1 >= total_pages)
                                .label(tr(locale, "page.nextPage"))
                                .on_click(move |_, _, cx| {
                                    app_page_next.update(cx, |s, cx| {
                                        if s.artifacts_page_index + 1 < total_pages {
                                            s.set_artifacts_page_index(s.artifacts_page_index + 1, cx);
                                        }
                                    });
                                }),
                        ),
                );
            inner = inner.child(pagination);
        }
    }

    div().size_full().p_6().child(inner)
}
