//! @file basic.rs
//! @brief Standalone ANSI terminal example using GPUI Kit.
//! @author modolet <y@xxyx.io>
//! @date 2026-09-07

use gpui_kit::{AppContext, WindowOptions, component::Root};
use gpui_xterm::{TerminalConfig, TerminalView};
use std::io::{Cursor, sink};

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        cx.open_window(WindowOptions::default(), |window, cx| {
            let terminal = cx.new(|cx| {
                TerminalView::new(
                    sink(),
                    Cursor::new(b"GPUI Kit terminal\r\n\x1b[32mANSI colors\x1b[0m\r\nSelect text and use the context menu to copy.\r\n"),
                    TerminalConfig::default(),
                    cx,
                )
            });
            cx.new(|cx| Root::new(terminal, window, cx))
        })
        .expect("failed to open terminal window");
    });
}
