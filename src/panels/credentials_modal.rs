use crate::app_state::AppState;
use crate::client::workspace_extras::{
    CredentialMeta, credential_secret_path, delete_credential, format_credential_mention,
    list_credentials, read_credential_secret, save_credential,
};
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, StyledExt as _, WindowExt as _,
    h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, px,
};

pub struct CredentialsModal {
    app_state: Entity<AppState>,
    items: Vec<CredentialMeta>,
    selected_name: Option<String>,
    name_input: Entity<InputState>,
    desc_input: Entity<InputState>,
    secret_input: Entity<InputState>,
    auto_discover: bool,
    secret_masked: bool,
}

impl CredentialsModal {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let locale = app_state.read(cx).locale;
        let name_ph = tr(locale, "credentials.namePlaceholder");
        let desc_ph = tr(locale, "credentials.descPlaceholder");
        let val_ph = tr(locale, "credentials.valuePlaceholder");
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(name_ph));
        let desc_input = cx.new(|cx| InputState::new(window, cx).placeholder(desc_ph));
        let secret_input = cx.new(|cx| {
            let mut st = InputState::new(window, cx).placeholder(val_ph);
            st.set_masked(true, window, cx);
            st
        });
        let mut this = Self {
            app_state,
            items: Vec::new(),
            selected_name: None,
            name_input,
            desc_input,
            secret_input,
            auto_discover: false,
            secret_masked: true,
        };
        this.reload(window, cx);
        this
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.items = list_credentials();
        if let Some(first) = self.items.first().cloned() {
            if self
                .selected_name
                .as_ref()
                .is_none_or(|n| !self.items.iter().any(|c| &c.name == n))
            {
                self.select_item(&first, window, cx);
            }
        } else {
            self.start_new(window, cx);
        }
    }

    fn select_item(&mut self, meta: &CredentialMeta, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_name = Some(meta.name.clone());
        self.auto_discover = meta.auto_discover;
        let secret = read_credential_secret(&meta.name).unwrap_or_default();
        self.name_input
            .update(cx, |s, cx| s.set_value(&meta.name, window, cx));
        self.desc_input
            .update(cx, |s, cx| s.set_value(&meta.description, window, cx));
        self.secret_input
            .update(cx, |s, cx| s.set_value(&secret, window, cx));
    }

    fn start_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_name = None;
        self.auto_discover = false;
        self.name_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.desc_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.secret_input
            .update(cx, |s, cx| s.set_value("", window, cx));
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.app_state.read(cx).locale;
        let name = self.name_input.read(cx).value().trim().to_string();
        let desc = self.desc_input.read(cx).value().trim().to_string();
        let secret = self.secret_input.read(cx).value().to_string();
        match save_credential(&name, &desc, &secret, self.auto_discover) {
            Ok(meta) => {
                if let Some(prev) = self.selected_name.as_ref() {
                    if prev != &meta.name {
                        let _ = delete_credential(prev);
                    }
                }
                self.selected_name = Some(meta.name);
                self.reload(window, cx);
                self.app_state.update(cx, |s, cx| {
                    s.show_success_toast(tr(locale, "credentials.saved"), cx);
                });
            }
            Err(err) => {
                self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            }
        }
        cx.notify();
    }

    fn delete_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let locale = self.app_state.read(cx).locale;
        let Some(name) = self.selected_name.take() else {
            return;
        };
        match delete_credential(&name) {
            Ok(()) => {
                self.reload(window, cx);
                self.app_state.update(cx, |s, cx| {
                    s.show_success_toast(tr(locale, "credentials.deleted"), cx);
                });
            }
            Err(err) => {
                self.app_state.update(cx, |s, cx| s.show_error_toast(err, cx));
            }
        }
        cx.notify();
    }

    fn insert_into_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
        let Some(name) = self.selected_name.clone() else {
            return;
        };
        let Some(meta) = self.items.iter().find(|c| c.name == name).cloned() else {
            return;
        };
        let mention = format_credential_mention(&meta);
        self.app_state.update(cx, |s, cx| {
            s.pending_composer_paste = Some(mention);
            s.close_credentials_modal(cx);
        });
    }
}

impl Render for CredentialsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.app_state.read(cx).locale;
        let app_close = self.app_state.clone();
        let current_name = self.name_input.read(cx).value().trim().to_string();
        let path_preview = if current_name.is_empty() {
            String::new()
        } else {
            credential_secret_path(&current_name)
                .to_string_lossy()
                .into_owned()
        };

        div()
            .id("credentials-modal-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .bg(cx.theme().background.opacity(0.65))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                app_close.update(cx, |s, cx| s.close_credentials_modal(cx));
            })
            .child(
                v_flex()
                    .id("credentials-modal-card")
                    .w(px(760.))
                    .h(px(520.))
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    // Header
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_2p5()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(cx.theme().foreground)
                                    .child(tr(locale, "credentials.title")),
                            )
                            .child(
                                Button::new("credentials-close-btn")
                                    .ghost()
                                    .small()
                                    .h(px(26.))
                                    .w(px(26.))
                                    .icon(IconName::Close)
                                    .on_click({
                                        let app = self.app_state.clone();
                                        move |_, _, cx| {
                                            app.update(cx, |s, cx| s.close_credentials_modal(cx));
                                        }
                                    }),
                            ),
                    )
                    // Body
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .child(
                                v_flex()
                                    .w(px(210.))
                                    .h_full()
                                    .flex_shrink_0()
                                    .border_r_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary.opacity(0.25))
                                    .child(
                                        div().p_2().child(
                                            Button::new("credentials-new-btn")
                                                .outline()
                                                .small()
                                                .w_full()
                                                .h(px(28.))
                                                .label(tr(locale, "credentials.new"))
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.start_new(window, cx);
                                                    cx.notify();
                                                })),
                                        ),
                                    )
                                    .child(
                                        v_flex()
                                            .id("credentials-list-scroll")
                                            .flex_1()
                                            .min_h_0()
                                            .overflow_y_scroll()
                                            .px_2()
                                            .pb_2()
                                            .gap_1()
                                            .children(if self.items.is_empty() {
                                                vec![div()
                                                    .px_2()
                                                    .py_3()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "credentials.empty"))
                                                    .into_any_element()]
                                            } else {
                                                self.items
                                                    .iter()
                                                    .enumerate()
                                                    .map(|(idx, item)| {
                                                        let is_sel = self.selected_name.as_ref()
                                                            == Some(&item.name);
                                                        let item_c = item.clone();
                                                        v_flex()
                                                            .id(("cred-item", idx))
                                                            .px_2p5()
                                                            .py_1p5()
                                                            .rounded_md()
                                                            .cursor_pointer()
                                                            .bg(if is_sel {
                                                                cx.theme().accent
                                                            } else {
                                                                cx.theme().background.opacity(0.0)
                                                            })
                                                            .hover(|s| {
                                                                s.bg(cx.theme().accent.opacity(0.6))
                                                            })
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.select_item(
                                                                        &item_c, window, cx,
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            ))
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_medium()
                                                                    .truncate()
                                                                    .text_color(if is_sel {
                                                                        cx.theme().accent_foreground
                                                                    } else {
                                                                        cx.theme().foreground
                                                                    })
                                                                    .child(SharedString::from(
                                                                        item.name.clone(),
                                                                    )),
                                                            )
                                                            .when(!item.description.is_empty(), |el| {
                                                                el.child(
                                                                    div()
                                                                        .text_xs()
                                                                        .truncate()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        )
                                                                        .child(SharedString::from(
                                                                            item.description
                                                                                .clone(),
                                                                        )),
                                                                )
                                                            })
                                                            .into_any_element()
                                                    })
                                                    .collect()
                                            }),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .p_4()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_medium()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "credentials.name")),
                                            )
                                            .child(Input::new(&self.name_input).small()),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_medium()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "credentials.description")),
                                            )
                                            .child(Input::new(&self.desc_input).small()),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_medium()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "credentials.value")),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .child(Input::new(&self.secret_input).small()),
                                                    )
                                                    .child(
                                                        Button::new("cred-toggle-mask")
                                                            .outline()
                                                            .small()
                                                            .h(px(28.))
                                                            .w(px(28.))
                                                            .icon(if self.secret_masked {
                                                                IconName::Eye
                                                            } else {
                                                                IconName::EyeOff
                                                            })
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.secret_masked =
                                                                        !this.secret_masked;
                                                                    let masked = this.secret_masked;
                                                                    this.secret_input.update(
                                                                        cx,
                                                                        |s, cx| {
                                                                            s.set_masked(
                                                                                masked, window, cx,
                                                                            );
                                                                        },
                                                                    );
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_medium()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(tr(locale, "credentials.path")),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .truncate()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(SharedString::from(path_preview)),
                                            ),
                                    )
                                    .child(
                                        Checkbox::new("cred-auto-discover")
                                            .checked(self.auto_discover)
                                            .label(tr(locale, "credentials.autoDiscover"))
                                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                                this.auto_discover = *checked;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    // Footer
                    .child(
                        h_flex()
                            .w_full()
                            .px_4()
                            .py_2p5()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cred-delete-btn")
                                    .danger()
                                    .small()
                                    .h(px(28.))
                                    .label(tr(locale, "credentials.delete"))
                                    .disabled(self.selected_name.is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.delete_current(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("cred-save-btn")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .label(tr(locale, "credentials.save"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_current(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("cred-insert-btn")
                                    .primary()
                                    .small()
                                    .h(px(28.))
                                    .label(tr(locale, "credentials.insertChat"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.insert_into_chat(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}
