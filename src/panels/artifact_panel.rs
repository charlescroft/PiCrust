use crate::app_state::AppState;
use crate::client::types::{ArtifactKind, ArtifactRecord};
use crate::i18n::{format_commas, tr};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::diff::{Diff, DiffFile, DiffState};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::{
    ActiveTheme as _, Icon, IconName, Selectable as _, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _,
    Window, div, px,
};
use std::collections::HashMap;

/// The artifact panel only compares two things: the artifact's own source and
/// what changed since the previous revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactViewMode {
    Source,
    Diff,
}

pub struct ArtifactPanelView {
    app_state: Entity<AppState>,
    mode: ArtifactViewMode,
    loaded_key: Option<(String, usize, ArtifactViewMode)>,
    code_editor: Entity<EditorState>,
    /// Lazily built only while the Diff tab shows a version with a predecessor.
    diff_states: HashMap<String, Entity<DiffState>>,
}

impl gpui_kit::Render for ArtifactPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_artifact(window, cx)
    }
}

impl ArtifactPanelView {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let code_editor = cx.new(|cx| EditorState::new(window, cx));
        Self {
            app_state,
            mode: ArtifactViewMode::Source,
            loaded_key: None,
            code_editor,
            diff_states: HashMap::new(),
        }
    }

    fn sync_artifact(
        &mut self,
        art: &ArtifactRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = (art.identifier.clone(), art.version, self.mode);
        if self.loaded_key.as_ref() == Some(&key) {
            return;
        }
        self.loaded_key = Some(key);

        if self.mode == ArtifactViewMode::Source {
            let lang = art
                .language
                .as_deref()
                .unwrap_or_else(|| match art.kind {
                    ArtifactKind::Html => "html",
                    ArtifactKind::React => "tsx",
                    ArtifactKind::Svg => "xml",
                    ArtifactKind::Mermaid => "markdown",
                    ArtifactKind::Markdown => "markdown",
                    ArtifactKind::Code => "text",
                });
            self.code_editor = cx.new(|cx| EditorState::new(window, cx).language(lang));
            self.code_editor.update(cx, |ed, cx| {
                ed.set_value(&art.content, window, cx);
            });
            // Bounded cache: one diff per artifact, dropped when the panel moves on.
            self.diff_states.clear();
            return;
        }

        // Diff tab: build (or rebuild) the patch for this revision only.
        let cache_key = format!("{}:{}", art.identifier, art.version);
        if self.diff_states.contains_key(&cache_key) {
            self.diff_states.retain(|k, _| k == &cache_key);
            return;
        }
        self.diff_states.clear();
        if let Some(previous) = art.previous_content.as_deref() {
            if previous != art.content {
                let patch = build_unified_patch(&art.title, previous, &art.content);
                if let Ok(files) = DiffFile::parse(&patch) {
                    let state = cx.new(|cx| DiffState::new(files, cx));
                    cx.observe(&state, |_, _, cx| cx.notify()).detach();
                    self.diff_states.insert(cache_key, state);
                }
            }
        }
    }

    pub fn render_artifact(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.app_state.read(cx);
        let locale = state.locale;
        let active_art = state
            .active_artifact_id
            .as_ref()
            .and_then(|id| state.artifacts.iter().find(|a| &a.identifier == id))
            .or_else(|| state.artifacts.first())
            .cloned();

        let Some(art) = active_art else {
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

        let header = h_flex()
            .px_3()
            .py_1p5()
            .border_b_1()
            .border_color(cx.theme().border)
            .justify_between()
            .items_center()
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .truncate()
                            .child(format!("{} (v{})", art.title, format_commas(art.version))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(art.kind.label()),
                    ),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(
                        Button::new("art-source-tab")
                            .ghost()
                            .small()
                            .h(px(24.))
                            .selected(self.mode == ArtifactViewMode::Source)
                            .label(tr(locale, "panel.artifactSource"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = ArtifactViewMode::Source;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("art-diff-tab")
                            .ghost()
                            .small()
                            .h(px(24.))
                            .selected(self.mode == ArtifactViewMode::Diff)
                            .label(tr(locale, "panel.artifactDiff"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.mode = ArtifactViewMode::Diff;
                                cx.notify();
                            })),
                    ),
            );

        let content = if self.mode == ArtifactViewMode::Source {
            div()
                .flex_1()
                .min_h_0()
                .child(Editor::new(&self.code_editor).size_full())
                .into_any_element()
        } else {
            let cache_key = format!("{}:{}", art.identifier, art.version);
            match self.diff_states.get(&cache_key) {
                Some(diff_state) => div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    // Match the Editor's 6px content padding so the Source and Diff
                    // tabs put their line numbers in the same column.
                    .pl(px(6.))
                    .child(
                        Diff::new(diff_state)
                            .header_visible(false)
                            .line_number(true)
                            .size_full(),
                    )
                    .into_any_element(),
                None => div()
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(locale, "panel.artifactNoDiff"))
                    .into_any_element(),
            }
        };

        v_flex().size_full().child(header).child(content).into_any_element()
    }
}

/// Builds a single-file unified diff (`--- a/<title>` / `+++ b/<title>`) from two
/// revisions, using a small LCS so only real line changes appear.
fn build_unified_patch(title: &str, previous: &str, current: &str) -> String {
    let a: Vec<&str> = previous.lines().collect();
    let b: Vec<&str> = current.lines().collect();

    // Classic LCS table; artifacts are small enough for this to stay cheap.
    let n = a.len();
    let m = b.len();
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut body = String::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut old_start = 1usize;
    let mut new_start = 1usize;
    let mut old_count = 0usize;
    let mut new_count = 0usize;
    let mut hunk = String::new();
    let mut flush_hunk = |hunk: &mut String, old_start: usize, old_count: usize, new_start: usize, new_count: usize| {
        if !hunk.is_empty() {
            body.push_str(&format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"));
            body.push_str(hunk);
            hunk.clear();
        }
    };

    let mut pending_hunk_started = false;
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            if pending_hunk_started {
                hunk.push(' ');
                hunk.push_str(a[i]);
                hunk.push('\n');
                old_count += 1;
                new_count += 1;
            }
            i += 1;
            j += 1;
            continue;
        }
        if !pending_hunk_started {
            pending_hunk_started = true;
            old_start = i + 1;
            new_start = j + 1;
            old_count = 0;
            new_count = 0;
        }
        if j < m && (i >= n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            hunk.push('+');
            hunk.push_str(b[j]);
            hunk.push('\n');
            new_count += 1;
            j += 1;
        } else if i < n {
            hunk.push('-');
            hunk.push_str(a[i]);
            hunk.push('\n');
            old_count += 1;
            i += 1;
        }
    }
    flush_hunk(&mut hunk, old_start, old_count, new_start, new_count);

    if body.is_empty() {
        return String::new();
    }
    format!("--- a/{title}\n+++ b/{title}\n{body}")
}
