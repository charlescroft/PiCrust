use crate::app_state::AppState;
use crate::client::types::{ExtensionUiRequest, QuestionChoice, QuestionDefinition};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::InputState;
use gpui_kit::component::questionnaire::{
    Questionnaire, QuestionnaireActions, QuestionnaireChoice, QuestionnaireChoiceDefinition,
    QuestionnaireChoices, QuestionnaireDescription, QuestionnaireError, QuestionnaireEvent,
    QuestionnaireInput, QuestionnaireInputDefinition, QuestionnaireItem,
    QuestionnaireItemDefinition, QuestionnaireNext, QuestionnairePrevious, QuestionnaireProgress,
    QuestionnaireSkip, QuestionnaireState, QuestionnaireSubmit, QuestionnaireTitle,
};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use serde_json::{Map, Value, json};

pub struct QuestionnaireView {
    app_state: Entity<AppState>,
    request: ExtensionUiRequest,
    state: Entity<QuestionnaireState>,
    /// The rendered questions (extension `select`/`input`/`confirm` folded into one).
    questions: Vec<QuestionDefinition>,
    /// True when this dialog answers an `ask_user_question` tool call, whose result
    /// expects `{ answers: [{ questionIndex, optionIndexes, text }], cancelled }`.
    #[allow(dead_code)]
    canonical: bool,
}

impl gpui_kit::Render for QuestionnaireView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_overlay(window, cx)
    }
}

impl QuestionnaireView {
    pub fn new(
        app_state: Entity<AppState>,
        request: ExtensionUiRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut item_defs = Vec::new();
        // Deduplicate identical questions within the request to prevent duplicate items
        let mut questions: Vec<QuestionDefinition> = Vec::new();
        for q in &request.questions {
            let q_trimmed = q.question.trim();
            let h_trimmed = q.header.trim();
            if !questions.iter().any(|existing| {
                existing.question.trim().eq_ignore_ascii_case(q_trimmed)
                    && existing.header.trim().eq_ignore_ascii_case(h_trimmed)
            }) {
                questions.push(q.clone());
            }
        }
        // Extension dialog primitives (`ui.select` / `ui.input` / `ui.confirm`) arrive as
        // one request each; present them in the same questionnaire surface.
        if questions.is_empty() {
            questions.push(dialog_question_from_request(&request));
        }
        for q in &questions {
            let ph = request
                .placeholder
                .clone()
                .unwrap_or_else(|| "Type something.".to_string());
            let prefill = request.prefill.clone();
            let custom_input = cx.new(|cx| {
                let mut st = InputState::new(window, cx).placeholder(ph);
                if let Some(val) = prefill {
                    st.set_value(val, window, cx);
                }
                st
            });
            let choices: Vec<QuestionnaireChoiceDefinition> = q
                .options
                .iter()
                .map(|opt| {
                    let mut def = QuestionnaireChoiceDefinition::new(
                        opt.id.clone(),
                        opt.label.clone(),
                    );
                    if let Some(desc) = &opt.description {
                        def = def.with_description(desc.clone());
                    }
                    def
                })
                .collect();

            let mut item = QuestionnaireItemDefinition::new(q.id.clone(), q.question.clone())
                .with_description(q.header.clone())
                .with_multiple(q.multi_select)
                .with_choices(choices)
                .with_input(QuestionnaireInputDefinition::new(
                    custom_input,
                    "Type something.",
                ));
            if !q.options.is_empty() {
                item = item.with_required(true);
            }
            item_defs.push(item);
        }

        // Fallback single confirmation item if an extension sent a simple confirm/input prompt
        if item_defs.is_empty() {
            let fallback_input = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Type response...")
            });
            item_defs.push(
                QuestionnaireItemDefinition::new(
                    "response",
                    request
                        .title
                        .clone()
                        .unwrap_or_else(|| "Extension Request".to_string()),
                )
                .with_description(request.message.clone().unwrap_or_default())
                .with_choices([
                    QuestionnaireChoiceDefinition::new("confirm", "Confirm"),
                    QuestionnaireChoiceDefinition::new("cancel", "Cancel"),
                ])
                .with_input(QuestionnaireInputDefinition::new(
                    fallback_input,
                    "Custom value",
                )),
            );
        }

        let q_state = cx.new(|cx| {
            QuestionnaireState::new(item_defs, cx).expect("valid questionnaire schema")
        });

        let app_submit = app_state.clone();
        let req_id = request.request_id.clone();
        let method = request.method.clone();
        let questions_for_submit = questions.clone();
        let canonical = app_state.read(cx).questionnaire_request_id.as_deref() == Some(req_id.as_str());
        cx.subscribe(&q_state, move |_, _, event: &QuestionnaireEvent, cx| {
            let submission = match event {
                QuestionnaireEvent::Submit(submission)
                | QuestionnaireEvent::Completed(submission) => submission,
                _ => return,
            };
            // Index incoming answers by the question they belong to, in render order.
            let mut order: Vec<(String, Vec<usize>, Option<String>)> = Vec::new();
            for item in submission.items() {
                let name = item.name().to_string();
                let ans = item.answer();
                let indexes: Vec<usize> = ans
                    .choices()
                    .iter()
                    .filter_map(|c| c.as_ref().trim().parse::<usize>().ok())
                    .collect();
                let text = ans.freeform().map(|s| s.to_string());
                if let Some(slot) = order.iter_mut().find(|(n, _, _)| *n == name) {
                    slot.1 = indexes;
                    slot.2 = text;
                } else {
                    order.push((name, indexes, text));
                }
            }

            let payload = if canonical {
                let answers: Vec<Value> = order
                    .iter()
                    .enumerate()
                    .map(|(q_ix, (_, indexes, text))| {
                        if indexes.is_empty() {
                            json!({ "questionIndex": q_ix, "text": text.clone().unwrap_or_default() })
                        } else if let Some(t) = text.as_ref().filter(|t| !t.trim().is_empty()) {
                            json!({
                                "questionIndex": q_ix,
                                "optionIndexes": indexes,
                                "text": t,
                            })
                        } else {
                            json!({ "questionIndex": q_ix, "optionIndexes": indexes })
                        }
                    })
                    .collect();
                json!({
                    "toolCallId": Value::Null,
                    "answers": answers,
                    "cancelled": false,
                })
            } else if method == "select" {
                let picked = order.first().and_then(|(_, indexes, text)| {
                    text.as_ref()
                        .filter(|t| !t.trim().is_empty())
                        .cloned()
                        .or_else(|| {
                            indexes
                                .first()
                                .and_then(|ix| questions_for_submit.first().and_then(|q| q.options.get(*ix)))
                                .map(|o| o.label.clone())
                        })
                });
                match picked {
                    Some(v) => json!({ "value": v }),
                    None => json!({ "cancelled": true }),
                }
            } else if method == "input" || method == "editor" {
                let typed = order
                    .first()
                    .and_then(|(_, _, text)| text.clone())
                    .filter(|t| !t.trim().is_empty());
                match typed {
                    Some(v) => json!({ "value": v }),
                    None => json!({ "cancelled": true }),
                }
            } else if method == "confirm" {
                let confirmed = order
                    .first()
                    .map(|(_, indexes, text)| {
                        indexes.first() == Some(&0)
                            && text.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(true)
                    })
                    .unwrap_or(false);
                json!({ "confirmed": confirmed })
            } else {
                let mut answers = Map::new();
                for (name, _, text) in &order {
                    answers.insert(name.clone(), json!(text.clone().unwrap_or_default()));
                }
                Value::Object(answers)
            };

            app_submit.update(cx, |s, cx| {
                if let Some(sid) = s.active_session_id.clone() {
                    let client = s.client.clone();
                    let rid = req_id.clone();
                    cx.spawn(async move |_, _| {
                        let _ = client.respond_extension_ui(&sid, &rid, payload).await;
                    })
                    .detach();
                }
                s.complete_pending_ui_request(cx);
            });
        })
        .detach();

        Self {
            app_state,
            request,
            state: q_state,
            questions,
            canonical,
        }
    }

    pub fn request_id(&self) -> &str {
        &self.request.request_id
    }

    pub fn has_same_content(&self, req: &ExtensionUiRequest) -> bool {
        if self.request.method != req.method
            || self.request.title != req.title
            || self.request.message != req.message
            || self.request.options != req.options
            || self.request.questions.len() != req.questions.len()
        {
            return false;
        }
        self.request
            .questions
            .iter()
            .zip(req.questions.iter())
            .all(|(a, b)| a.question.trim().eq_ignore_ascii_case(b.question.trim()))
    }

    pub fn render_overlay(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_item_id = self.state.read(cx).current_item().cloned();
        let current_preview = current_item_id.as_ref().and_then(|item_id| {
            self.questions
                .iter()
                .find(|q| q.id == item_id.as_ref())
                .and_then(|q| {
                    q.options
                        .iter()
                        .find_map(|o| o.preview.as_ref().map(|p| (o.label.clone(), p.clone())))
                })
        });

        let mut form = Questionnaire::new(&self.state).child(QuestionnaireProgress::new(&self.state));

        if self.questions.is_empty() {
            form = form.child(
                QuestionnaireItem::new(&self.state, "response")
                    .child(QuestionnaireTitle::new(&self.state, "response"))
                    .child(QuestionnaireDescription::new(&self.state, "response"))
                    .child(
                        QuestionnaireChoices::new(&self.state, "response")
                            .child(QuestionnaireChoice::new(&self.state, "response", "confirm"))
                            .child(QuestionnaireChoice::new(&self.state, "response", "cancel"))
                            .child(QuestionnaireInput::new(&self.state, "response")),
                    )
                    .child(QuestionnaireError::new(&self.state, "response")),
            );
        } else {
            for q in &self.questions {
                let mut choices = QuestionnaireChoices::new(&self.state, q.id.clone());
                for opt in &q.options {
                    choices = choices.child(QuestionnaireChoice::new(
                        &self.state,
                        q.id.clone(),
                        opt.id.clone(),
                    ));
                }
                choices = choices.child(QuestionnaireInput::new(&self.state, q.id.clone()));

                form = form.child(
                    QuestionnaireItem::new(&self.state, q.id.clone())
                        .child(QuestionnaireTitle::new(&self.state, q.id.clone()))
                        .child(QuestionnaireDescription::new(&self.state, q.id.clone()))
                        .child(choices)
                        .child(QuestionnaireError::new(&self.state, q.id.clone())),
                );
            }
        }

        form = form.child(
            QuestionnaireActions::new(&self.state)
                .child(QuestionnairePrevious::new(&self.state))
                .child(QuestionnaireSkip::new(&self.state))
                .child(QuestionnaireNext::new(&self.state))
                .child(QuestionnaireSubmit::new(&self.state)),
        );

        let chat_width = self.app_state.read(cx).chat_content_width;
        let app_cancel = self.app_state.clone();
        let req_id = self.request.request_id.clone();

        v_flex()
            .w_full()
            .max_w(px(chat_width))
            .mx_auto()
            .p_3()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .child(
                                self.request
                                    .title
                                    .clone()
                                    .unwrap_or_else(|| "Extension Prompt".to_string()),
                            ),
                    )
                    .child(
                        Button::new("dismiss-questionnaire")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Close)
                            .on_click(move |_, _, cx| {
                                app_cancel.update(cx, |s, cx| {
                                    if let Some(sid) = s.active_session_id.clone() {
                                        let client = s.client.clone();
                                        let rid = req_id.clone();
                                        cx.spawn(async move |_, _| {
                                            let _ = client
                                                .respond_extension_ui(&sid, &rid, json!({ "cancelled": true }))
                                                .await;
                                        })
                                        .detach();
                                    }
                                    s.complete_pending_ui_request(cx);
                                });
                            }),
                    ),
            )
            .child(match current_preview {
                Some((label, preview_md)) => h_flex()
                    .w_full()
                    .gap_3()
                    .items_start()
                    .child(div().flex_1().child(form))
                    .child(
                        v_flex()
                            .w(px(320.))
                            .p_2()
                            .gap_1()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().secondary)
                            .child(
                                div()
                                    .text_xs()
                                    .font_medium()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(format!("Preview: {label}"))),
                            )
                            .child(TextView::markdown("q-preview", preview_md)),
                    )
                    .into_any_element(),
                None => form.into_any_element(),
            })
    }
}

/// Turns an extension dialog primitive request into a single questionnaire question.
fn dialog_question_from_request(request: &ExtensionUiRequest) -> QuestionDefinition {
    let title = request
        .title
        .clone()
        .or_else(|| request.message.clone())
        .unwrap_or_else(|| "Extension Request".to_string());
    let options: Vec<QuestionChoice> = match request.method.as_str() {
        "confirm" => vec![
            QuestionChoice {
                id: "0".to_string(),
                label: "Confirm".to_string(),
                description: None,
                preview: None,
            },
            QuestionChoice {
                id: "1".to_string(),
                label: "Cancel".to_string(),
                description: None,
                preview: None,
            },
        ],
        _ => request
            .options
            .iter()
            .enumerate()
            .map(|(ix, label)| QuestionChoice {
                id: ix.to_string(),
                label: label.clone(),
                description: None,
                preview: None,
            })
            .collect(),
    };
    QuestionDefinition {
        id: "response".to_string(),
        header: request.method.clone(),
        question: title,
        multi_select: false,
        options,
    }
}

pub struct NewSessionQuestionnaireView {
    app_state: Entity<AppState>,
    locale: crate::i18n::AppLocale,
}

impl gpui_kit::Render for NewSessionQuestionnaireView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::client::types::WorkspaceKind;
        use crate::i18n::tr;
        use gpui_kit::component::Icon;

        let app_close = self.app_state.clone();
        let app_backdrop = self.app_state.clone();
        let app_opt_chat = self.app_state.clone();
        let app_opt_proj = self.app_state.clone();
        let app_opt_temp = self.app_state.clone();
        let locale = self.locale;

        let option_card = |id: &'static str,
                           icon: Icon,
                           title: String,
                           desc: String,
                           cx: &gpui_kit::App| {
            h_flex()
                .id(id)
                .w_full()
                .px_3()
                .py_2p5()
                .gap_3()
                .items_center()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .hover(|s| {
                    s.bg(cx.theme().accent)
                        .border_color(cx.theme().primary.opacity(0.6))
                })
                .cursor_pointer()
                .child(
                    h_flex()
                        .w(px(30.))
                        .h(px(30.))
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .child(icon),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(cx.theme().foreground)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_normal()
                                .child(desc),
                        ),
                )
                .child(
                    Icon::new(IconName::ChevronRight)
                        .xsmall()
                        .text_color(cx.theme().muted_foreground),
                )
        };

        div()
            .id("new-session-questionnaire-backdrop")
            .occlude()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.65))
            .on_scroll(|_, _, cx| {
                cx.stop_propagation();
            })
            .on_mouse_down(gpui_kit::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                app_backdrop.update(cx, |s, cx| {
                    s.show_new_session_questionnaire = false;
                    cx.notify();
                });
            })
            .child(
                v_flex()
                    .id("new-session-questionnaire-card")
                    .occlude()
                    .w(px(460.))
                    .max_w_full()
                    .p_4()
                    .gap_3()
                    .rounded_xl()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .on_scroll(|_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_semibold()
                                    .child(tr(locale, "sidebar.newSessionPromptTitle")),
                            )
                            .child(
                                Button::new("close-new-session-questionnaire")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .on_click(move |_, _, cx| {
                                        app_close.update(cx, |s, cx| {
                                            s.show_new_session_questionnaire = false;
                                            cx.notify();
                                        });
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "sidebar.newSessionPromptQuestion")),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .gap_2()
                            .child(
                                option_card(
                                    "new-sess-opt-standalone",
                                    Icon::new(IconName::Plus).small().text_color(cx.theme().primary),
                                    tr(locale, "sidebar.newStandaloneChat").to_string(),
                                    tr(locale, "sidebar.newStandaloneChatDesc").to_string(),
                                    cx,
                                )
                                .on_click(move |_, _, cx| {
                                    app_opt_chat.update(cx, |s, cx| {
                                        s.show_new_session_questionnaire = false;
                                        s.chats_section_collapsed = false;
                                        s.start_new_composer(WorkspaceKind::Chat, cx);
                                    });
                                }),
                            )
                            .child(
                                option_card(
                                    "new-sess-opt-project",
                                    Icon::new(IconName::FolderOpen).small().text_color(cx.theme().primary),
                                    tr(locale, "sidebar.newSubproject").to_string(),
                                    tr(locale, "sidebar.newSubprojectDesc").to_string(),
                                    cx,
                                )
                                .on_click(move |_, _, cx| {
                                    app_opt_proj.update(cx, |s, cx| {
                                        s.show_new_session_questionnaire = false;
                                        cx.notify();
                                    });
                                    let prompt_text =
                                        tr(locale, "sidebar.selectProjectFolder").to_string();
                                    let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                                        files: false,
                                        directories: true,
                                        multiple: false,
                                        prompt: Some(prompt_text.into()),
                                    });
                                    let app_p = app_opt_proj.clone();
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
                            )
                            .child(
                                option_card(
                                    "new-sess-opt-temp",
                                    Icon::new(crate::icons::ClockIcon).small().text_color(cx.theme().primary),
                                    tr(locale, "sidebar.newTempChat").to_string(),
                                    tr(locale, "sidebar.newTempChatDesc").to_string(),
                                    cx,
                                )
                                .on_click(move |_, _, cx| {
                                    app_opt_temp.update(cx, |s, cx| {
                                        s.show_new_session_questionnaire = false;
                                        s.chats_section_collapsed = false;
                                        s.start_new_temp_chat(cx);
                                    });
                                }),
                            ),
                    ),
            )
    }
}

impl NewSessionQuestionnaireView {
    pub fn new(
        app_state: Entity<AppState>,
        locale: crate::i18n::AppLocale,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self { app_state, locale }
    }

    pub fn locale(&self) -> crate::i18n::AppLocale {
        self.locale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_rpc_extension_ui_request_with_id_field() {
        let raw = json!({
            "type": "extension_ui_request",
            "id": "req-123",
            "method": "select",
            "title": "[Header] Pick one?",
            "options": ["1. Alpha — first", "2. Beta — second"]
        });
        let req: ExtensionUiRequest = serde_json::from_value(raw).expect("valid ExtensionUiRequest");
        assert_eq!(req.request_id, "req-123");
        assert_eq!(req.method, "select");
        assert_eq!(req.options.len(), 2);
    }
}
