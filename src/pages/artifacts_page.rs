use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::i18n::{format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px};

pub fn render_artifacts_page(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let state = app_state.read(cx);
    let artifacts = state.artifacts.clone();
    let locale = state.locale;

    let mut page = v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(tr(locale, "page.artifactsTitle")))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} {}",
                            format_commas(artifacts.len()),
                            tr(locale, "page.artifactsItems")
                        )),
                ),
        );

    if artifacts.is_empty() {
        page = page.child(
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
        let total_count = artifacts.len();
        let total_pages = (total_count + PAGE_SIZE - 1) / PAGE_SIZE;
        let current_page = state.artifacts_page_index.min(total_pages.saturating_sub(1));
        let start_ix = current_page * PAGE_SIZE;
        let end_ix = (start_ix + PAGE_SIZE).min(total_count);
        let page_items = &artifacts[start_ix..end_ix];

        let mut grid = v_flex().id("art-page-grid").flex_1().min_h_0().overflow_y_scroll().gap_2();
        for (page_offset, art) in page_items.iter().enumerate() {
            let global_idx = start_ix + page_offset;
            let aid = art.identifier.clone();
            let sid_opt = art.session_id.clone();
            let is_unread = state.is_artifact_unread(art);
            let app_open = app_state.clone();
            grid = grid.child(
                h_flex()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().secondary)
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .children((is_unread).then(|| {
                                        div()
                                            .size_2()
                                            .rounded_full()
                                            .bg(cx.theme().primary)
                                    }))
                                    .child(div().text_sm().font_medium().child(SharedString::from(art.title.clone())))
                                    .child(Tag::secondary().small().child(art.kind.label())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(format!(
                                        "{} · v{}",
                                        art.identifier,
                                        format_commas(art.version)
                                    ))),
                            ),
                    )
                    .child(
                        Button::new(("art-page-open", global_idx))
                            .outline()
                            .small()
                            .label(tr(locale, "chat.preview"))
                            .on_click(move |_, _, cx| {
                                let aid_clone = aid.clone();
                                let sid_clone = sid_opt.clone();
                                app_open.update(cx, |s, cx| {
                                    s.mark_artifact_read(&aid_clone, cx);
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
        page = page.child(grid);

        if total_pages > 1 {
            let app_page_prev = app_state.clone();
            let app_page_next = app_state.clone();
            let pagination = h_flex()
                .w_full()
                .py_2()
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
            page = page.child(pagination);
        }
    }

    page
}
