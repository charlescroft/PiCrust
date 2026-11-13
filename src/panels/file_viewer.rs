use crate::app_state::AppState;
use crate::i18n::{AppLocale, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffState};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::text::{TextView, TextViewState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement, SharedString, Styled as _, Window,
    div, img, px,
};
use gpui_kit_markdown_extras::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilePreviewMode {
    Diff,
    Rendered,
    Source,
}

pub struct FileViewerView {
    app_state: Entity<AppState>,
    editor: Entity<EditorState>,
    markdown_state: Option<Entity<TextViewState>>,
    diff_state: Option<Entity<DiffState>>,
    mode: FilePreviewMode,
    loaded_path: Option<PathBuf>,
    loaded_snapshot: SharedString,
    loaded_patch: Option<SharedString>,
}

impl gpui_kit::Render for FileViewerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_viewer(window, cx)
    }
}

impl FileViewerView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| EditorState::new(window, cx));
        Self {
            app_state,
            editor,
            markdown_state: None,
            diff_state: None,
            mode: FilePreviewMode::Source,
            loaded_path: None,
            loaded_snapshot: SharedString::default(),
            loaded_patch: None,
        }
    }

    pub fn render_viewer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let locale = state.locale;
        let path = state.open_file_path.clone();
        let content = state.open_file_content.clone();
        let patch = state.open_file_diff_patch.clone();
        let cwd_root = state.active_cwd.clone();

        if path != self.loaded_path || content != self.loaded_snapshot || patch != self.loaded_patch {
            self.loaded_path = path.clone();
            self.loaded_snapshot = content.clone();
            self.loaded_patch = patch.clone();

            let lang = path
                .as_deref()
                .and_then(language_from_path)
                .unwrap_or("text");
            self.editor = cx.new(|cx| EditorState::new(window, cx).language(lang));
            self.editor.update(cx, |ed, cx| {
                ed.set_value(content.as_ref(), window, cx);
            });

            // One presentation only: unified. No split/unified switching in the panel.
            self.diff_state = patch
                .as_ref()
                .and_then(|p| DiffFile::parse(p.as_ref()).ok())
                .map(|files| {
                    cx.new(|cx| {
                        let mut state = DiffState::new(files, cx);
                        state.set_mode(DiffMode::Unified, cx);
                        state
                    })
                });

            if self.diff_state.is_some() {
                self.mode = FilePreviewMode::Diff;
            } else if path.as_deref().is_some_and(is_markdown_path) {
                let text = content.as_ref();
                self.markdown_state =
                    Some(cx.new(|cx| TextViewState::markdown(text, cx).selectable(true)));
                self.mode = FilePreviewMode::Rendered;
            } else {
                self.markdown_state = None;
                self.mode = FilePreviewMode::Source;
            }
        }

        let Some(ref file_path) = path else {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .p_6()
                .child(
                    Empty::new().header(
                        EmptyHeader::new()
                            .media(EmptyMedia::new().child(Icon::new(IconName::FileText)))
                            .title(EmptyTitle::new().child(tr(locale, "panel.noFileSelected")))
                            .description(EmptyDescription::new().child(
                                tr(locale, "panel.noFileSelectedDesc"),
                            )),
                    ),
                )
                .into_any_element();
        };

        let rel_path = file_path
            .strip_prefix(&cwd_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file_path.to_string_lossy().into_owned());
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("File")
            .to_string();

        let is_md = is_markdown_path(file_path);
        let is_img = is_image_path(file_path);
        let has_diff = self.diff_state.is_some();

        let app_mention = self.app_state.clone();
        let rel_for_mention = rel_path.clone();

        let mut header_actions = h_flex().items_center().gap_1p5();
        if has_diff {
            header_actions = header_actions.child(
                Button::new("fv-diff")
                    .ghost()
                    .xsmall()
                    .h(px(24.))
                    .selected(self.mode == FilePreviewMode::Diff)
                    .label(tr(locale, "chat.diff"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = FilePreviewMode::Diff;
                        cx.notify();
                    })),
            );
        }
        if is_md {
            header_actions = header_actions.child(
                Button::new("fv-rendered")
                    .ghost()
                    .xsmall()
                    .h(px(24.))
                    .selected(self.mode == FilePreviewMode::Rendered)
                    .label(tr(locale, "chat.preview"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = FilePreviewMode::Rendered;
                        cx.notify();
                    })),
            );
        }
        header_actions = header_actions
            .child(
                Button::new("fv-source")
                    .ghost()
                    .xsmall()
                    .h(px(24.))
                    .selected(self.mode == FilePreviewMode::Source)
                    .label(tr(locale, "chat.source"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = FilePreviewMode::Source;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("fv-mention")
                    .ghost()
                    .xsmall()
                    .h(px(24.))
                    .icon(Icon::new(crate::icons::AtSignIcon))
                    .label(tr(locale, "panel.mention"))
                    .tooltip(tr(locale, "panel.mentionTooltip"))
                    .on_click(move |_, _, cx| {
                        app_mention.update(cx, |s, cx| {
                            s.insert_composer_mention(&rel_for_mention, cx);
                        });
                    }),
            )
            .when_open_btn(Some(file_path.clone()), locale);

        let body = if self.mode == FilePreviewMode::Diff {
            if let Some(ref ds) = self.diff_state {
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    // The Source tab is an Editor whose gutter starts at its 6px
                    // content padding; match it so numbers share one column.
                    .pl(px(6.))
                    .child(
                        Diff::new(ds)
                            .header_visible(false)
                            .line_number(true)
                            .size_full(),
                    )
                    .into_any_element()
            } else {
                div().flex_1().into_any_element()
            }
        } else if is_img {
            div()
                .flex_1()
                .min_h_0()
                .size_full()
                .p_4()
                .flex()
                .items_center()
                .justify_center()
                .child(img(file_path.clone()).max_w_full().max_h_full())
                .into_any_element()
        } else if is_md && self.mode == FilePreviewMode::Rendered {
            if let Some(ref md_state) = self.markdown_state {
                let view = with_plugins(TextView::new(md_state).selectable(true).scrollable(true));
                div()
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_4()
                    .child(view)
                    .into_any_element()
            } else {
                div().flex_1().into_any_element()
            }
        } else {
            div()
                .flex_1()
                .min_h_0()
                .size_full()
                .child(Editor::new(&self.editor).size_full())
                .into_any_element()
        };

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .truncate()
                            .child(SharedString::from(format!("{file_name} · {rel_path}"))),
                    )
                    .child(header_actions),
            )
            .child(body)
            .into_any_element()
    }
}

fn is_markdown_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("md" | "mdx" | "markdown")
    )
}

fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "svg")
    )
}

fn language_from_path(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Some("rust"),
        Some("ts" | "tsx") => Some("typescript"),
        Some("js" | "jsx" | "mjs" | "cjs") => Some("javascript"),
        Some("json") => Some("json"),
        Some("toml") => Some("toml"),
        Some("md") => Some("markdown"),
        Some("css") => Some("css"),
        Some("html") => Some("html"),
        Some("py") => Some("python"),
        Some("sh" | "bash" | "zsh") => Some("bash"),
        Some("yaml" | "yml") => Some("yaml"),
        _ => None,
    }
}

trait OpenExt {
    fn when_open_btn(self, path: Option<PathBuf>, locale: AppLocale) -> Self;
}

impl<E: ParentElement> OpenExt for E {
    fn when_open_btn(self, path: Option<PathBuf>, locale: AppLocale) -> Self {
        let Some(p) = path else {
            return self;
        };
        self.child(
            Button::new("reveal-file")
                .ghost()
                .xsmall()
                .h(px(24.))
                .icon(IconName::ExternalLink)
                .label(tr(locale, "panel.openFile"))
                .on_click(move |_, _, _| {
                    let _ = open::that(&p);
                }),
        )
    }
}
