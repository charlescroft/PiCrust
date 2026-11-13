use crate::app_state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, StyledExt as _, h_flex, v_flex};
use gpui_kit::{AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled as _, Window, div, px};
use gpui_xterm::{ColorPalette, ColorPaletteBuilder, TerminalConfig, TerminalView};
use parking_lot::Mutex;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::path::Path;
use std::sync::Arc;

pub struct TerminalSession {
    pub view: Entity<TerminalView>,
    _master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    pub cwd_label: String,
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        // Windows ConPTY does not support POSIX signal names; call kill() directly.
        let mut child = self.child.lock();
        let _ = child.kill();
    }
}

pub struct TerminalHostView {
    app_state: Entity<AppState>,
    session: Option<TerminalSession>,
    last_error: Option<String>,
    last_dark_mode: bool,
}

impl gpui_kit::Render for TerminalHostView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_terminal(window, cx)
    }
}

impl TerminalHostView {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let dark_mode = app_state.read(cx).dark_mode;
        Self {
            app_state,
            session: None,
            last_error: None,
            last_dark_mode: dark_mode,
        }
    }

    pub fn ensure_started(&mut self, cx: &mut Context<Self>) {
        if self.session.is_some() || self.last_error.is_some() {
            return;
        }
        self.spawn_terminal(cx);
    }

    pub fn spawn_terminal(&mut self, cx: &mut Context<Self>) {
        self.session = None;
        self.last_error = None;

        let cwd = self.app_state.read(cx).active_cwd.clone();
        let dark = self.app_state.read(cx).dark_mode;
        self.last_dark_mode = dark;

        match create_pty_session(&cwd, dark, cx) {
            Ok(sess) => {
                self.session = Some(sess);
            }
            Err(err) => {
                self.last_error = Some(err.to_string());
            }
        }
        cx.notify();
    }

    pub fn render_terminal(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.ensure_started(cx);
        let dark = self.app_state.read(cx).dark_mode;
        if dark != self.last_dark_mode {
            self.last_dark_mode = dark;
            if let Some(sess) = &self.session {
                let cfg = terminal_config(dark);
                sess.view.update(cx, |tv, cx| {
                    tv.update_config(cfg, cx);
                });
            }
        }

        let header = h_flex()
            .px_3()
            .py_1p5()
            .border_b_1()
            .border_color(cx.theme().border)
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        self.session
                            .as_ref()
                            .map(|s| format!("TERMINAL · {}", s.cwd_label))
                            .unwrap_or_else(|| "TERMINAL".to_string()),
                    ),
            )
            .child(
                Button::new("restart-terminal")
                    .ghost()
                    .xsmall()
                    .icon(IconName::RefreshCw)
                    .label("Restart")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.spawn_terminal(cx);
                    })),
            );

        let body = if let Some(sess) = &self.session {
            div().flex_1().min_h_0().w_full().child(sess.view.clone()).into_any_element()
        } else {
            div()
                .flex_1()
                .p_4()
                .text_xs()
                .text_color(cx.theme().danger)
                .child(
                    self.last_error
                        .clone()
                        .unwrap_or_else(|| "Terminal unavailable".to_string()),
                )
                .into_any_element()
        };

        v_flex().size_full().child(header).child(body)
    }
}

fn create_pty_session(
    cwd: &Path,
    dark: bool,
    cx: &mut Context<TerminalHostView>,
) -> anyhow::Result<TerminalSession> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut cmd = build_shell_command(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let master = Arc::new(Mutex::new(pair.master));
    let master_for_resize = master.clone();
    let child = Arc::new(Mutex::new(child));

    let config = terminal_config(dark);

    let view = cx.new(|cx| {
        TerminalView::new(writer, reader, config, cx).with_resize_callback(move |cols, rows| {
            let _ = master_for_resize.lock().resize(PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            });
        })
    });

    Ok(TerminalSession {
        view,
        _master: master,
        child,
        cwd_label: cwd.to_string_lossy().into_owned(),
    })
}

fn build_shell_command(cwd: &Path) -> CommandBuilder {
    #[cfg(target_os = "windows")]
    {
        let shell = ["pwsh.exe", "powershell.exe", "cmd.exe"]
            .into_iter()
            .find(|s| which_exists(s))
            .unwrap_or("cmd.exe");
        let mut cmd = CommandBuilder::new(shell);
        if cwd.exists() {
            cmd.cwd(cwd);
        }
        cmd
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        if cwd.exists() {
            cmd.cwd(cwd);
        }
        cmd
    }
}

#[cfg(target_os = "windows")]
fn which_exists(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).exists()))
        .unwrap_or(false)
}

fn terminal_config(dark: bool) -> TerminalConfig {
    TerminalConfig {
        cols: 80,
        rows: 24,
        font_family: default_terminal_font(),
        font_size: px(13.0),
        scrollback: 10_000,
        line_height_multiplier: 1.15,
        colors: terminal_palette(dark),
        ..Default::default()
    }
}

fn default_terminal_font() -> String {
    #[cfg(target_os = "macos")]
    {
        "Menlo".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "Cascadia Mono".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "monospace".to_string()
    }
}

fn terminal_palette(dark: bool) -> ColorPalette {
    if dark {
        ColorPaletteBuilder::new()
            .background(24, 24, 27)
            .foreground(244, 244, 245)
            .cursor(96, 165, 250)
            .build()
    } else {
        // High-contrast light terminal palette (>= 4.5:1 contrast)
        ColorPaletteBuilder::new()
            .background(250, 250, 250)
            .foreground(24, 24, 27)
            .cursor(37, 99, 235)
            .build()
    }
}
