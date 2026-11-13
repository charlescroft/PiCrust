use crate::app_state::AppState;
use crate::client::types::TaskSnapshot;
use crate::i18n::{format_commas, tr};
use gpui_kit::component::badge::Badge;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

pub fn render_tasks_page(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::workspace::WorkspaceView>,
) -> impl IntoElement {
    let state = app_state.read(cx);
    let tasks = state.tasks.clone();
    let locale = state.locale;
    let app_refresh = app_state.clone();

    let mut page = v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_lg().font_semibold().child(tr(locale, "page.tasksTitle")))
                .child(
                    Button::new("tasks-refresh")
                        .outline()
                        .small()
                        .h(px(28.))
                        .icon(IconName::RefreshCw)
                        .label(tr(locale, "page.refresh"))
                        .on_click(move |_, _, cx| {
                            app_refresh.update(cx, |s, cx| s.refresh_tasks_from_sidecar(cx));
                        }),
                ),
        );

    if tasks.is_empty() {
        page = page.child(
            div().flex_1().flex().items_center().justify_center().child(
                Empty::new().header(
                    EmptyHeader::new()
                        .media(EmptyMedia::new().child(Icon::new(IconName::LayoutDashboard)))
                        .title(EmptyTitle::new().child(tr(locale, "page.noTasks")))
                        .description(EmptyDescription::new().child(tr(locale, "page.noTasksDesc"))),
                ),
            ),
        );
    } else {
        let waiting: Vec<&TaskSnapshot> = tasks.iter().filter(|t| t.status == "waiting").collect();
        let running: Vec<&TaskSnapshot> = tasks.iter().filter(|t| t.status == "running").collect();
        let recent: Vec<&TaskSnapshot> = tasks.iter().filter(|t| t.status == "recent").take(5).collect();

        let mut list = v_flex()
            .id("tasks-scroll-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .gap_4();

        let groups = [
            (tr(locale, "tasks.waiting"), waiting, "waiting"),
            (tr(locale, "tasks.running"), running, "running"),
            (tr(locale, "tasks.recent"), recent, "recent"),
        ];

        for (group_title, items, kind) in groups {
            if items.is_empty() {
                continue;
            }
            let mut section = v_flex().gap_2().child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{group_title} ({})", format_commas(items.len()))),
            );
            for (idx, t) in items.into_iter().enumerate() {
                let sid = t.session_id.clone();
                let app_open = app_state.clone();
                let badge_color = match kind {
                    "waiting" => cx.theme().warning,
                    "running" => cx.theme().primary,
                    _ => cx.theme().success,
                };
                let step_label = t
                    .current_step
                    .clone()
                    .or_else(|| t.last_run_status.clone())
                    .unwrap_or_else(|| t.status.clone());
                let row_id = SharedString::from(format!("task-{kind}-{idx}"));

                section = section.child(
                    h_flex()
                        .p_3()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().secondary)
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(if kind == "running" {
                                            Spinner::new()
                                                .xsmall()
                                                .color(cx.theme().primary)
                                                .into_any_element()
                                        } else {
                                            Badge::new().dot().color(badge_color).into_any_element()
                                        })
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_sm()
                                                .font_medium()
                                                .child(SharedString::from(t.title.clone())),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(Tag::secondary().small().child(SharedString::from(t.project.clone())))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(SharedString::from(step_label)),
                                        ),
                                ),
                        )
                        .child(
                            Button::new(row_id)
                                .ghost()
                                .small()
                                .h(px(28.))
                                .label(tr(locale, "tasks.open"))
                                .on_click(move |_, _, cx| {
                                    app_open.update(cx, |s, cx| s.select_session(sid.clone(), cx));
                                }),
                        ),
                );
            }
            list = list.child(section);
        }
        page = page.child(list);
    }

    page
}
