use crate::app_state::AppState;
use crate::i18n::tr;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::tree::{TreeItem, TreeState, tree};
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, SharedString, Styled as _,
    Window, div, px,
};
use std::path::{Path, PathBuf};

pub struct FileExplorerPanel {
    app_state: Entity<AppState>,
    tree_state: Entity<TreeState>,
    loaded_cwd: Option<PathBuf>,
}

enum FileNode {
    Dir {
        id: String,
        name: String,
        expanded: bool,
        children: Vec<FileNode>,
    },
    File {
        id: String,
        name: String,
    },
}

fn nodes_to_tree_items(nodes: Vec<FileNode>) -> Vec<TreeItem> {
    nodes
        .into_iter()
        .map(|n| match n {
            FileNode::Dir {
                id,
                name,
                expanded,
                children,
            } => TreeItem::new(id, name)
                .expanded(expanded)
                .children(nodes_to_tree_items(children)),
            FileNode::File { id, name } => TreeItem::new(id, name),
        })
        .collect()
}

impl gpui_kit::Render for FileExplorerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_explorer(window, cx)
    }
}

impl FileExplorerPanel {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let tree_state = cx.new(|cx| TreeState::new(cx));
        cx.observe(&tree_state, |_, _, cx| cx.notify()).detach();
        let mut panel = Self {
            app_state,
            tree_state,
            loaded_cwd: None,
        };
        panel.reload(false, cx);
        panel
    }

    pub fn reload(&mut self, force: bool, cx: &mut Context<Self>) {
        let cwd = self.app_state.read(cx).active_cwd.clone();
        if !force && self.loaded_cwd.as_ref() == Some(&cwd) {
            return;
        }
        self.loaded_cwd = Some(cwd.clone());
        let tree_state = self.tree_state.clone();
        cx.spawn(async move |_, cx| {
            let nodes = smol::unblock(move || build_tree_nodes(&cwd, 0)).await;
            let _ = tree_state.update(cx, |state, cx| {
                state.set_items(nodes_to_tree_items(nodes), cx);
            });
        })
        .detach();
    }

    pub fn render_explorer(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.reload(false, cx);
        let state = self.app_state.read(cx);
        let cwd_root = state.active_cwd.clone();
        let locale = state.locale;
        let app_for_rows = self.app_state.clone();

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_medium()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr(locale, "panel.filesHint")),
                    )
                    .child(
                        Button::new("fe-refresh")
                            .ghost()
                            .w(px(24.))
                            .h(px(24.))
                            .icon(IconName::RefreshCw)
                            .tooltip(tr(locale, "page.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reload(true, cx);
                            })),
                    ),
            )
            .child(
                div().flex_1().min_h_0().size_full().child(
                    tree(&self.tree_state, move |ix, entry, selected, _window, _cx| {
                        let item = entry.item();
                        let is_dir = entry.is_folder();
                        let icon = if !is_dir {
                            IconName::FileText
                        } else if entry.is_expanded() {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        };
                        let path_str = item.id.to_string();
                        let path_buf = PathBuf::from(&path_str);
                        let rel_path = path_buf
                            .strip_prefix(&cwd_root)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_else(|_| path_str.clone());
                        let app_mention = app_for_rows.clone();
                        let app_preview = app_for_rows.clone();
                        let rel_for_click = rel_path.clone();
                        let path_for_preview = path_buf.clone();

                        let mut row = h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_2()
                                    .items_center()
                                    .child(icon)
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(SharedString::from(item.label.clone())),
                                    ),
                            );

                        if !is_dir {
                            let rel_for_btn = rel_for_click.clone();
                            row = row.child(
                                Button::new(("fe-mention", ix))
                                    .ghost()
                                    .small()
                                    .w(px(26.))
                                    .h(px(24.))
                                    .flex_shrink_0()
                                    .label("@")
                                    .tooltip(tr(locale, "panel.mentionTooltip"))
                                    .on_click(move |_, _, cx| {
                                        cx.stop_propagation();
                                        app_mention.update(cx, |s, cx| {
                                            s.insert_composer_mention(&rel_for_btn, cx);
                                        });
                                    }),
                            );
                        }

                        ListItem::new(ix)
                            .cursor_pointer()
                            .selected(selected)
                            .pl(px(14.) * entry.depth() + px(10.))
                            .child(row)
                            .on_click(move |_, _, cx| {
                                if !is_dir {
                                    let p = path_for_preview.clone();
                                    app_preview.update(cx, |s, cx| {
                                        s.open_file(p, cx);
                                    });
                                }
                            })
                    })
                    .size_full(),
                ),
            )
    }
}

fn build_tree_nodes(dir: &Path, depth: usize) -> Vec<FileNode> {
    if depth > 4 || !dir.is_dir() {
        return Vec::new();
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | ".next" | ".next-desktop" | "dist" | ".DS_Store"
        ) {
            continue;
        }
        if path.is_dir() {
            dirs.push((name, path));
        } else {
            files.push((name, path));
        }
    }

    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items = Vec::new();
    for (name, path) in dirs {
        let children = build_tree_nodes(&path, depth + 1);
        items.push(FileNode::Dir {
            id: path.to_string_lossy().into_owned(),
            name,
            expanded: depth == 0,
            children,
        });
    }
    for (name, path) in files {
        items.push(FileNode::File {
            id: path.to_string_lossy().into_owned(),
            name,
        });
    }
    items
}
