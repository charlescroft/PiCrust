use crate::app_state::{AppState, RightPanelTab, WorkspacePage};
use crate::client::types::{ArtifactKind, ArtifactRecord};
use crate::i18n::{AppLocale, format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _,
    Window, div, img, px,
};
use gpui_webview::WebView;
use std::path::{Path, PathBuf};

const MAX_TEXT_PREVIEW_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewKind {
    Html,
    WebUrl,
    Pdf,
    Svg,
    Image,
    Markdown,
    Office,
    Binary,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadedArtifactKey {
    identifier: String,
    version: usize,
    content_len: usize,
    file_path: Option<String>,
    kind: PreviewKind,
}

pub struct ArtifactPanelView {
    app_state: Entity<AppState>,
    loaded_key: Option<LoadedArtifactKey>,
    active_kind: PreviewKind,
    active_disk_path: Option<PathBuf>,
    code_editor: Entity<EditorState>,
    markdown_content: Option<SharedString>,
    webview: Option<Entity<WebView>>,
    webview_error: Option<String>,
}

impl gpui_kit::Render for ArtifactPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_artifact(window, cx)
    }
}

impl ArtifactPanelView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let code_editor = cx.new(|cx| EditorState::new(window, cx));
        cx.observe(&app_state, |this, app, cx| {
            let s = app.read(cx);
            let modal_open = s.settings_open
                || s.notes_modal_open
                || s.credentials_modal_open
                || s.show_new_session_questionnaire
                || s.find_open
                || s.agy_link_modal_open
                || s.pending_ui_request.is_some();
            if !s.right_panel_open || s.right_panel_tab != RightPanelTab::Artifact {
                this.release_webview();
            } else if let Some(ref wv) = this.webview {
                wv.update(cx, |w, _| {
                    if modal_open {
                        w.hide();
                    } else {
                        w.show();
                    }
                });
            }
            cx.notify();
        })
        .detach();
        Self {
            app_state,
            loaded_key: None,
            active_kind: PreviewKind::Code,
            active_disk_path: None,
            code_editor,
            markdown_content: None,
            webview: None,
            webview_error: None,
        }
    }

    fn release_webview(&mut self) {
        if self.webview.is_some() || self.webview_error.is_some() {
            self.webview = None;
            self.webview_error = None;
            // Force re-sync when panel re-opens on the same artifact.
            self.loaded_key = None;
        }
    }

    fn sync_artifact(
        &mut self,
        art: &ArtifactRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut kind = resolve_preview_kind(art);
        let key = LoadedArtifactKey {
            identifier: art.identifier.clone(),
            version: art.version,
            content_len: art.content.len(),
            file_path: art.file_path.clone(),
            kind,
        };
        if self.loaded_key.as_ref() == Some(&key) {
            return;
        }
        self.loaded_key = Some(key);

        let disk_path = resolve_disk_path(art);
        self.active_disk_path = disk_path.clone();

        // Fetch inline artifact content if this is a session artifact without disk path.
        if disk_path.is_none() && art.content.is_empty() {
            if let Some(ref sid) = art.session_id {
                let app = self.app_state.clone();
                let sid_c = sid.clone();
                let aid_c = art.identifier.clone();
                cx.defer(move |cx| {
                    app.update(cx, |s, cx| {
                        s.load_artifact_versions(&sid_c, &aid_c, cx);
                    });
                });
            }
        }

        // Load text content lazily only for text-based preview kinds.
        let mut text_content = if !art.content.is_empty() {
            Some(art.content.clone())
        } else if matches!(
            kind,
            PreviewKind::Html | PreviewKind::Svg | PreviewKind::Markdown | PreviewKind::Code
        ) {
            match disk_path.as_deref() {
                Some(path) => match read_text_preview(path) {
                    TextRead::Text(s) => Some(s),
                    TextRead::Binary => {
                        kind = PreviewKind::Binary;
                        None
                    }
                    TextRead::Missing => None,
                },
                None => None,
            }
        } else {
            None
        };

        self.active_kind = kind;

        match kind {
            PreviewKind::Html => {
                self.markdown_content = None;
                let builder = if let Some(ref path) = disk_path {
                    wry::WebViewBuilder::new().with_url(&file_path_to_url(path))
                } else {
                    let html = text_content.take().unwrap_or_default();
                    wry::WebViewBuilder::new().with_html(&html)
                };
                self.build_webview(builder, window, cx);
            }
            PreviewKind::Pdf => {
                self.markdown_content = None;
                #[cfg(target_os = "linux")]
                {
                    self.webview = None;
                    self.webview_error = None;
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if let Some(ref path) = disk_path {
                        let builder = wry::WebViewBuilder::new().with_url(&file_path_to_url(path));
                        self.build_webview(builder, window, cx);
                    } else {
                        self.webview = None;
                        self.webview_error = Some("PDF file path missing".to_string());
                    }
                }
            }
            PreviewKind::Svg => {
                self.markdown_content = None;
                if let Some(svg) = text_content.take() {
                    let html = wrap_svg_html(&svg);
                    let builder = wry::WebViewBuilder::new().with_html(&html);
                    self.build_webview(builder, window, cx);
                } else if let Some(ref path) = disk_path {
                    let builder = wry::WebViewBuilder::new().with_url(&file_path_to_url(path));
                    self.build_webview(builder, window, cx);
                } else {
                    self.webview = None;
                    self.webview_error = None;
                }
            }
            PreviewKind::Markdown => {
                self.webview = None;
                self.webview_error = None;
                let md = text_content.unwrap_or_default();
                self.markdown_content = Some(SharedString::from(md));
            }
            PreviewKind::Code => {
                self.webview = None;
                self.webview_error = None;
                self.markdown_content = None;
                let lang = infer_editor_language(art, disk_path.as_deref());
                let code = text_content.unwrap_or_default();
                self.code_editor = cx.new(|cx| EditorState::new(window, cx).language(lang));
                self.code_editor.update(cx, |ed, cx| {
                    ed.set_value(&code, window, cx);
                });
            }
            PreviewKind::WebUrl => {
                self.markdown_content = None;
                if let Some(url) = art.file_path.as_deref() {
                    let builder = wry::WebViewBuilder::new().with_url(url);
                    self.build_webview(builder, window, cx);
                } else {
                    self.webview = None;
                    self.webview_error = Some("Missing URL".to_string());
                }
            }
            PreviewKind::Office => {
                self.markdown_content = None;
                const MAX_OFFICE_BYTES: u64 = 20 * 1024 * 1024;
                if let Some(ref path) = disk_path {
                    let meta_ok = std::fs::metadata(path)
                        .map(|m| m.len() <= MAX_OFFICE_BYTES)
                        .unwrap_or(false);
                    if meta_ok {
                        if let Ok(bytes) = std::fs::read(path) {
                            let ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            let b64 = base64_encode(&bytes);
                            let html = wrap_office_html(&ext, &b64);
                            let builder = wry::WebViewBuilder::new().with_html(&html);
                            self.build_webview(builder, window, cx);
                        } else {
                            self.webview = None;
                            self.webview_error = None;
                        }
                    } else {
                        self.webview = None;
                        self.webview_error = None;
                    }
                } else {
                    self.webview = None;
                    self.webview_error = None;
                }
            }
            PreviewKind::Image | PreviewKind::Binary => {
                self.webview = None;
                self.webview_error = None;
                self.markdown_content = None;
            }
        }
    }

    fn build_webview(
        &mut self,
        builder: wry::WebViewBuilder<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.webview = None;
        self.webview_error = None;
        match WebView::build(builder, window, cx) {
            Ok(wv) => {
                let entity = cx.new(|_| wv);
                self.webview = Some(entity);
            }
            Err(err) => {
                self.webview_error = Some(err.to_string());
            }
        }
    }

    pub fn render_artifact(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (locale, active_art, preview_font_sz, cwd_root, modal_overlay_open) = {
            let state = self.app_state.read(cx);
            let active_art = state
                .active_artifact_id
                .as_ref()
                .and_then(|id| state.artifacts.iter().find(|a| &a.identifier == id))
                .or_else(|| state.artifacts.first())
                .cloned();
            let modal_open = state.settings_open
                || state.notes_modal_open
                || state.credentials_modal_open
                || state.show_new_session_questionnaire
                || state.find_open
                || state.agy_link_modal_open
                || state.pending_ui_request.is_some();
            (
                state.locale,
                active_art,
                state.preview_font_size,
                state.active_cwd.clone(),
                modal_open,
            )
        };

        let Some(art) = active_art else {
            self.release_webview();
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .p_6()
                .child(
                    Empty::new().header(
                        EmptyHeader::new()
                            .media(EmptyMedia::new().child(Icon::new(IconName::Frame)))
                            .title(EmptyTitle::new().child(tr(locale, "panel.noArtifactSelected")))
                            .description(EmptyDescription::new().child(
                                tr(locale, "panel.noArtifactSelectedDesc"),
                            )),
                    ),
                )
                .into_any_element();
        };

        self.sync_artifact(&art, window, cx);
        if let Some(ref wv) = self.webview {
            wv.update(cx, |w, _| {
                if modal_overlay_open {
                    w.hide();
                } else {
                    w.show();
                }
            });
        }

        let mention_target = art
            .file_path
            .as_deref()
            .unwrap_or(art.identifier.as_str());
        let mention_ref = Path::new(mention_target)
            .strip_prefix(&cwd_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| mention_target.to_string());
        let app_mention = self.app_state.clone();
        let open_art = art.clone();

        let title_text = if art.file_path.is_some() && art.version <= 1 {
            art.title.clone()
        } else {
            format!("{} (v{})", art.title, format_commas(art.version))
        };

        let subtitle_text = preview_subtitle(&art, self.active_kind);

        let header = h_flex()
            .px_3()
            .py_1p5()
            .border_b_1()
            .border_color(cx.theme().border)
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .truncate()
                            .child(title_text),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(subtitle_text),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Button::new("art-mention")
                            .outline()
                            .xsmall()
                            .h(px(24.))
                            .label(tr(locale, "panel.mention"))
                            .tooltip(tr(locale, "panel.mentionTooltip"))
                            .on_click(move |_, _, cx| {
                                app_mention.update(cx, |s, cx| {
                                    s.insert_composer_mention(&mention_ref, cx);
                                    if s.active_page != WorkspacePage::Chat {
                                        s.active_page = WorkspacePage::Chat;
                                        cx.notify();
                                    }
                                });
                            }),
                    )
                    .child(
                        Button::new("art-open-external")
                            .outline()
                            .xsmall()
                            .h(px(24.))
                            .icon(Icon::new(IconName::ExternalLink))
                            .label(tr(locale, "panel.openExternal"))
                            .tooltip(tr(locale, "panel.openExternalTooltip"))
                            .on_click(move |_, _, _| {
                                open_artifact_externally(&open_art);
                            }),
                    ),
            );

        let content = self.render_preview_body(&art, locale, preview_font_sz, modal_overlay_open, cx);

        v_flex()
            .size_full()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .text_size(px(preview_font_sz))
                    .child(content),
            )
            .into_any_element()
    }

    fn render_preview_body(
        &self,
        art: &ArtifactRecord,
        locale: AppLocale,
        preview_font_sz: f32,
        modal_overlay_open: bool,
        cx: &Context<Self>,
    ) -> gpui_kit::AnyElement {
        match self.active_kind {
            PreviewKind::Html | PreviewKind::WebUrl | PreviewKind::Svg => {
                if modal_overlay_open {
                    div().size_full().into_any_element()
                } else if let Some(ref wv) = self.webview {
                    div().size_full().child(wv.clone()).into_any_element()
                } else {
                    render_unsupported_card(
                        locale,
                        self.webview_error
                            .as_deref()
                            .unwrap_or_else(|| tr(locale, "panel.artifactWebViewFailed")),
                        art.clone(),
                        cx,
                    )
                }
            }
            PreviewKind::Pdf => {
                #[cfg(target_os = "linux")]
                {
                    render_unsupported_card(
                        locale,
                        tr(locale, "panel.artifactPdfLinuxUnsupported"),
                        art.clone(),
                        cx,
                    )
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if modal_overlay_open {
                        div().size_full().into_any_element()
                    } else if let Some(ref wv) = self.webview {
                        div().size_full().child(wv.clone()).into_any_element()
                    } else {
                        render_unsupported_card(
                            locale,
                            self.webview_error
                                .as_deref()
                                .unwrap_or_else(|| tr(locale, "panel.artifactWebViewFailed")),
                            art.clone(),
                            cx,
                        )
                    }
                }
            }
            PreviewKind::Image => {
                if let Some(ref path) = self.active_disk_path {
                    div()
                        .size_full()
                        .p_4()
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .child(img(path.clone()).max_w_full().max_h_full())
                        .into_any_element()
                } else {
                    render_unsupported_card(
                        locale,
                        tr(locale, "panel.artifactBinaryUnsupported"),
                        art.clone(),
                        cx,
                    )
                }
            }
            PreviewKind::Markdown => {
                if let Some(ref md) = self.markdown_content {
                    div()
                        .size_full()
                        .overflow_y_scrollbar()
                        .p_4()
                        .text_size(px(preview_font_sz))
                        .child(TextView::markdown("artifact-markdown-preview", md.clone()))
                        .into_any_element()
                } else {
                    div().size_full().into_any_element()
                }
            }
            PreviewKind::Office => {
                if modal_overlay_open {
                    div().size_full().into_any_element()
                } else if let Some(ref wv) = self.webview {
                    div().size_full().child(wv.clone()).into_any_element()
                } else {
                    render_unsupported_card(
                        locale,
                        tr(locale, "panel.artifactOfficeUnsupported"),
                        art.clone(),
                        cx,
                    )
                }
            }
            PreviewKind::Binary => render_unsupported_card(
                locale,
                tr(locale, "panel.artifactBinaryUnsupported"),
                art.clone(),
                cx,
            ),
            PreviewKind::Code => div()
                .size_full()
                .child(
                    Editor::new(&self.code_editor)
                        .readonly(true)
                        .appearance(false)
                        .bordered(false)
                        .text_size(px(preview_font_sz))
                        .size_full(),
                )
                .into_any_element(),
        }
    }
}

fn render_unsupported_card(
    locale: AppLocale,
    message: &str,
    art: ArtifactRecord,
    _cx: &Context<ArtifactPanelView>,
) -> gpui_kit::AnyElement {
    let ext = extract_extension(&art).unwrap_or_default().to_uppercase();
    let title = if ext.is_empty() {
        art.title.clone()
    } else {
        format!("{} ({})", art.title, ext)
    };
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_4()
        .p_6()
        .child(
            Empty::new().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(IconName::File)))
                    .title(EmptyTitle::new().child(title))
                    .description(EmptyDescription::new().child(SharedString::from(message.to_string()))),
            ),
        )
        .child(
            Button::new("art-unsupported-open")
                .primary()
                .small()
                .icon(Icon::new(IconName::ExternalLink))
                .label(tr(locale, "panel.openExternal"))
                .on_click(move |_, _, _| {
                    open_artifact_externally(&art);
                }),
        )
        .into_any_element()
}

fn extract_extension(art: &ArtifactRecord) -> Option<String> {
    let from_path = art
        .file_path
        .as_deref()
        .and_then(|p| Path::new(p).extension())
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    if from_path.as_ref().is_some_and(|s| !s.is_empty()) {
        return from_path;
    }
    let from_id = Path::new(&art.identifier)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    if from_id.as_ref().is_some_and(|s| !s.is_empty()) {
        return from_id;
    }
    let from_title = Path::new(&art.title)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    if from_title.as_ref().is_some_and(|s| !s.is_empty()) {
        return from_title;
    }
    art.language
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

fn resolve_preview_kind(art: &ArtifactRecord) -> PreviewKind {
    if art
        .file_path
        .as_deref()
        .is_some_and(|p| p.starts_with("http://") || p.starts_with("https://"))
    {
        return PreviewKind::WebUrl;
    }
    if let Some(ext) = extract_extension(art) {
        match ext.as_str() {
            "html" | "htm" => return PreviewKind::Html,
            "pdf" => return PreviewKind::Pdf,
            "svg" => return PreviewKind::Svg,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "icns" | "avif"
            | "tiff" | "tif" => return PreviewKind::Image,
            "md" | "markdown" | "mdx" => return PreviewKind::Markdown,
            "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods" | "ppt" | "pptx"
            | "odp" => return PreviewKind::Office,
            "zip" | "tar" | "gz" | "7z" | "rar" | "dmg" | "exe" | "bin" | "wasm" | "dll"
            | "so" | "dylib" | "mp3" | "mp4" | "wav" | "mov" | "avi" | "mkv" | "sqlite"
            | "db" | "parquet" => return PreviewKind::Binary,
            _ => {}
        }
    }
    match art.kind {
        ArtifactKind::Html => PreviewKind::Html,
        ArtifactKind::Svg => PreviewKind::Svg,
        ArtifactKind::Markdown => PreviewKind::Markdown,
        ArtifactKind::React | ArtifactKind::Mermaid | ArtifactKind::Code => PreviewKind::Code,
    }
}

fn resolve_disk_path(art: &ArtifactRecord) -> Option<PathBuf> {
    if let Some(ref p) = art.file_path {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let id_pb = PathBuf::from(&art.identifier);
    if id_pb.is_absolute() && id_pb.is_file() {
        return Some(id_pb);
    }
    None
}

enum TextRead {
    Text(String),
    Binary,
    Missing,
}

fn read_text_preview(path: &Path) -> TextRead {
    let Ok(bytes) = std::fs::read(path) else {
        return TextRead::Missing;
    };
    let slice = if bytes.len() > MAX_TEXT_PREVIEW_BYTES {
        &bytes[..MAX_TEXT_PREVIEW_BYTES]
    } else {
        &bytes[..]
    };
    if slice.contains(&0) {
        return TextRead::Binary;
    }
    match String::from_utf8(slice.to_vec()) {
        Ok(mut text) => {
            if bytes.len() > MAX_TEXT_PREVIEW_BYTES {
                text.push_str("\n\n… (truncated)");
            }
            TextRead::Text(text)
        }
        Err(_) => TextRead::Binary,
    }
}

fn infer_editor_language(art: &ArtifactRecord, disk_path: Option<&Path>) -> &'static str {
    let ext = disk_path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .or_else(|| art.language.as_deref())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" => "python",
        "go" => "go",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "css" | "scss" => "css",
        "html" | "htm" => "html",
        "xml" | "svg" => "xml",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" => "cpp",
        "java" => "java",
        "swift" => "swift",
        "md" | "markdown" | "mdx" => "markdown",
        _ => match art.kind {
            ArtifactKind::React => "tsx",
            ArtifactKind::Mermaid | ArtifactKind::Markdown => "markdown",
            _ => "text",
        },
    }
}

fn preview_subtitle(art: &ArtifactRecord, kind: PreviewKind) -> String {
    let ext = extract_extension(art).map(|s| s.to_ascii_uppercase());
    match (kind, ext) {
        (PreviewKind::Html, _) => "HTML".to_string(),
        (PreviewKind::WebUrl, _) => "Web".to_string(),
        (PreviewKind::Pdf, _) => "PDF".to_string(),
        (PreviewKind::Svg, _) => "SVG".to_string(),
        (PreviewKind::Image, Some(e)) => e,
        (PreviewKind::Image, None) => "Image".to_string(),
        (PreviewKind::Markdown, _) => "Markdown".to_string(),
        (PreviewKind::Office, Some(e)) => format!("Office · {e}"),
        (PreviewKind::Office, None) => "Office".to_string(),
        (PreviewKind::Binary, Some(e)) => e,
        (PreviewKind::Binary, None) => "Binary".to_string(),
        (PreviewKind::Code, Some(e)) => e,
        (PreviewKind::Code, None) => art.kind.label().to_string(),
    }
}

fn file_path_to_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::with_capacity(raw.len() + 16);
    if raw.starts_with('/') {
        encoded.push_str("file://");
    } else {
        encoded.push_str("file:///");
    }
    for byte in raw.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'/'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b':' => encoded.push(*byte as char),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

fn wrap_svg_html(svg: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>\
        html,body{{margin:0;padding:16px;width:100%;height:100%;box-sizing:border-box;\
        display:flex;align-items:center;justify-content:center;background:transparent;}}\
        svg{{max-width:100%;max-height:100%;width:auto;height:auto;}}\
        </style></head><body>{svg}</body></html>"
    )
}

fn open_artifact_externally(art: &ArtifactRecord) {
    if let Some(path) = resolve_disk_path(art) {
        let _ = open::that_detached(path);
        return;
    }
    let ext = extract_extension(art).unwrap_or_else(|| match art.kind {
        ArtifactKind::Html => "html".to_string(),
        ArtifactKind::Svg => "svg".to_string(),
        ArtifactKind::Markdown => "md".to_string(),
        ArtifactKind::React => "tsx".to_string(),
        ArtifactKind::Mermaid => "mmd".to_string(),
        ArtifactKind::Code => "txt".to_string(),
    });
    let safe_id: String = art
        .identifier
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let filename = if safe_id.is_empty() {
        format!("pi-artifact.{ext}")
    } else {
        format!("pi-artifact-{safe_id}.{ext}")
    };
    let tmp_path = std::env::temp_dir().join(filename);
    if std::fs::write(&tmp_path, art.content.as_bytes()).is_ok() {
        let _ = open::that_detached(tmp_path);
    }
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn wrap_office_html(ext: &str, b64: &str) -> String {
    let script_body = match ext {
        "docx" => r#"
<script src="https://unpkg.com/jszip@3.10.1/dist/jszip.min.js"></script>
<script src="https://unpkg.com/docx-preview@0.3.3/dist/docx-preview.min.js"></script>
<script>
(async () => {
  try {
    const bin = Uint8Array.from(atob(window.__OFFICE_B64__), c => c.charCodeAt(0));
    await docx.renderAsync(bin.buffer, document.getElementById('root'));
  } catch (e) {
    document.getElementById('root').innerHTML = '<p style="color:#c00;padding:16px">Preview failed: ' + (e && e.message ? e.message : e) + '</p>';
  }
})();
</script>"#,
        "xlsx" | "xls" | "ods" => r#"
<script src="https://unpkg.com/xlsx@0.18.5/dist/xlsx.full.min.js"></script>
<script>
(() => {
  try {
    const bin = Uint8Array.from(atob(window.__OFFICE_B64__), c => c.charCodeAt(0));
    const wb = XLSX.read(bin, { type: 'array' });
    const tabs = document.createElement('div');
    tabs.style.cssText = 'display:flex;gap:6px;padding:8px 12px;border-bottom:1px solid #e5e7eb;flex-wrap:wrap;background:#f9fafb';
    const body = document.createElement('div');
    body.style.cssText = 'padding:12px;overflow:auto';
    const root = document.getElementById('root');
    root.innerHTML = '';
    root.appendChild(tabs);
    root.appendChild(body);
    const renderSheet = (name) => {
      body.innerHTML = XLSX.utils.sheet_to_html(wb.Sheets[name]);
    };
    wb.SheetNames.forEach((name, idx) => {
      const btn = document.createElement('button');
      btn.textContent = name;
      btn.style.cssText = 'padding:4px 10px;border-radius:6px;border:1px solid #d1d5db;background:#fff;cursor:pointer;font-size:12px';
      btn.onclick = () => renderSheet(name);
      tabs.appendChild(btn);
      if (idx === 0) renderSheet(name);
    });
  } catch (e) {
    document.getElementById('root').innerHTML = '<p style="color:#c00;padding:16px">Preview failed: ' + (e && e.message ? e.message : e) + '</p>';
  }
})();
</script>"#,
        _ => r#"
<script>
document.getElementById('root').innerHTML = '<p style="padding:16px;color:#6b7280">Use "Open Externally" to view this Office format.</p>';
</script>"#,
    };

    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8" />
<style>
  body {{ margin:0; font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; background:#fff; color:#111827; }}
  table {{ border-collapse:collapse; font-size:12px; }}
  td, th {{ border:1px solid #e5e7eb; padding:4px 8px; text-align:left; }}
</style>
</head>
<body>
<div id="root"><p style="padding:16px;color:#6b7280">Loading preview...</p></div>
<script>window.__OFFICE_B64__ = "{b64}";</script>
{script_body}
</body>
</html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_artifact(identifier: &str, kind: ArtifactKind, file_path: Option<&str>) -> ArtifactRecord {
        ArtifactRecord {
            identifier: identifier.to_string(),
            title: identifier.to_string(),
            kind,
            language: None,
            file_path: file_path.map(String::from),
            content: String::new(),
            previous_content: None,
            version: 1,
            session_id: None,
            updated_at: None,
            favorite: false,
        }
    }

    #[test]
    fn classifies_preview_kinds_across_formats() {
        assert_eq!(
            resolve_preview_kind(&sample_artifact("report.html", ArtifactKind::Html, None)),
            PreviewKind::Html
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("spec.pdf", ArtifactKind::Code, Some("/tmp/spec.pdf"))),
            PreviewKind::Pdf
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("photo.png", ArtifactKind::Code, Some("/tmp/photo.png"))),
            PreviewKind::Image
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("diagram.svg", ArtifactKind::Svg, None)),
            PreviewKind::Svg
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("deck.pptx", ArtifactKind::Code, Some("/tmp/deck.pptx"))),
            PreviewKind::Office
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("archive.zip", ArtifactKind::Code, Some("/tmp/archive.zip"))),
            PreviewKind::Binary
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("README.md", ArtifactKind::Markdown, None)),
            PreviewKind::Markdown
        );
        assert_eq!(
            resolve_preview_kind(&sample_artifact("main.rs", ArtifactKind::Code, None)),
            PreviewKind::Code
        );
    }

    #[test]
    fn encodes_file_urls_safely() {
        assert_eq!(
            file_path_to_url(Path::new("/tmp/my report #1.pdf")),
            "file:///tmp/my%20report%20%231.pdf"
        );
    }
}
