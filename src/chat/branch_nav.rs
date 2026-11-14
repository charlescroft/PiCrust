use crate::app_state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, h_flex};
use gpui_kit::{Context, Entity, IntoElement, ParentElement as _, Styled as _, Window, div};

pub fn render_branch_nav(
    app_state: &Entity<AppState>,
    _window: &mut Window,
    cx: &mut Context<crate::chat::ChatPanel>,
) -> impl IntoElement {
    let siblings = app_state.read(cx).branch_siblings.clone();
    if siblings.len() <= 1 {
        return div().into_any_element();
    }

    let active_idx = siblings.iter().position(|b| b.active).unwrap_or(0);
    let prev_id = active_idx
        .checked_sub(1)
        .and_then(|i| siblings.get(i))
        .map(|b| b.entry_id.clone());
    let next_id = siblings.get(active_idx + 1).map(|b| b.entry_id.clone());

    let app_prev = app_state.clone();
    let app_next = app_state.clone();

    h_flex()
        .w_full()
        .px_3()
        .py_1()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("Branch {}/{}", active_idx + 1, siblings.len())),
        )
        .child(
            h_flex()
                .gap_1()
                .child(
                    Button::new("branch-prev")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronLeft)
                        .disabled(prev_id.is_none())
                        .on_click(move |_, _, cx| {
                            if let Some(target) = prev_id.clone() {
                                app_prev.update(cx, |s, cx| {
                                    s.navigate_to_branch_entry(&target, cx);
                                });
                            }
                        }),
                )
                .child(
                    Button::new("branch-next")
                        .ghost()
                        .xsmall()
                        .icon(IconName::ChevronRight)
                        .disabled(next_id.is_none())
                        .on_click(move |_, _, cx| {
                            if let Some(target) = next_id.clone() {
                                app_next.update(cx, |s, cx| {
                                    s.navigate_to_branch_entry(&target, cx);
                                });
                            }
                        }),
                ),
        )
        .into_any_element()
}
