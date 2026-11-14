use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::i18n::{format_commas, tr};
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _, Window, div};

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
        let mut grid = v_flex().gap_2();
        for (idx, art) in artifacts.iter().enumerate() {
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
                        Button::new(("art-page-open", idx))
                            .outline()
                            .small()
                            .label(tr(locale, "chat.preview"))
                            .on_click(move |_, _, cx| {
                                let aid_clone = aid.clone();
                                let sid_clone = sid_opt.clone();
                                app_open.update(cx, |s, cx| {
                                    s.mark_artifact_read(&aid_clone, cx);
                                    s.active_artifact_id = Some(aid_clone.clone());
                                    if let Some(sid) = sid_clone {
                                        s.select_session(sid, cx);
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
    }

    page
}
