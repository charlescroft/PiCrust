//! @file character_art.rs
//! @brief Visual regression sample for terminal artwork, line heights and fonts.
//! @author modolet <y@xxyx.io>
//! @date 2026-09-12

use gpui_kit::{AppContext, Bounds, WindowBounds, WindowOptions, component::Root, px, size};
use gpui_xterm::{ColorPalette, TerminalConfig, TerminalView};
use std::{
    borrow::Cow,
    io::{Cursor, sink},
};

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        // Optional font file lets the sample reproduce the embedding app's font.
        if let Ok(path) = std::env::var("GPUI_XTERM_FONT_FILE") {
            cx.text_system()
                .add_fonts(vec![Cow::Owned(std::fs::read(path).unwrap())])
                .unwrap();
        }
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1400.), px(1250.)),
                cx,
            ))),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            window.set_window_title("gpui_xterm character art");
            let font_family =
                std::env::var("GPUI_XTERM_FONT").unwrap_or_else(|_| "monospace".into());
            let line_height_multiplier = std::env::var("GPUI_XTERM_LINE_HEIGHT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1.2);
            let colors = if std::env::var("GPUI_XTERM_LIGHT").is_ok() {
                ColorPalette::builder()
                    .background(230, 230, 230)
                    .foreground(25, 25, 25)
                    .build()
            } else {
                ColorPalette::builder()
                    .background(36, 35, 55)
                    .foreground(208, 206, 225)
                    .build()
            };
            let terminal = cx.new(|cx| {
                TerminalView::new(
                    sink(),
                    Cursor::new(ART.replace('\n', "\r\n").into_bytes()),
                    TerminalConfig {
                        font_family,
                        font_size: px(18.),
                        line_height_multiplier,
                        colors,
                        padding: gpui_kit::Edges::all(px(16.)),
                        ..Default::default()
                    },
                    cx,
                )
            });
            cx.new(|cx| Root::new(terminal, window, cx))
        })
        .expect("failed to open character-art window");
    });
}

const ART: &str = "███████╗██╗ ██████╗ ██╗███████╗
██╔════╝██║██╔═══██╗██║██╔════╝
███████╗██║██║   ██║██║███████╗
╚════██║██║██║   ██║██║╚════██║
███████║██║╚██████╔╝██║███████║
╚══════╝╚═╝ ╚═════╝ ╚═╝╚══════╝

Build Type:       Release
Build Timestamp   260909_1503
Build Hash:       922abf1
Build Branch:     master
Version:          3.23.0

OIS:/$ fetch
                                                     :
                                               ,:++:      SIOIS N647 BOARD System Information
                                           .;+;++-       ========================================
                 =?????0?!;a?????0?=      ;+;.+++:=;.    MCU       : STM32N647
                ?0a!0000b?0?b0000!a0!    ,;  =+=;++,     Core      : ARM Cortex-M55
               ?000?b00;00000;00a?0000    .    =+-       RTOS      : FreeRTOS
             ,0000000+:0000000=+0000000.                 ----------------------------------------
             -0000000=                  :0000000:        CPUCLK    : 800 MHz
               00000;00.              .00!!0000=         SYSCLK    : 400 MHz
                00?b0000-            .0000?b00.          HCLK      : 200 MHz
                 +?000000:          -0000000;            PCLK1     : 200 MHz
                  b000000a:        =;000000!             PCLK2     : 200 MHz
                   ;0000+00a      +00:0000b              ----------------------------------------
                    -00=0000?    a0000:00;               Build Type      : Release
                     .:0000000. 0000000:=                Build Timestamp : 260909_1503
   c+000000?a;0000000;:000000?.;0000000.                 Build Hash      : 922abf1
  a00;0000b!00=0000?c00;0000!,00b?0000.                  Build Branch    : master
 !0000;00c?0000;00!a0000c00a=0000!a0?                    Version         : 3.23.0
0000000:;0000000c+?000000!=:000000?:                     ----------------------------------------

█▀▄▌▐ ░▒▓ ▁▂▃▄▅▆▇█ ▖▗▘▙▚▛▜▝▞▟
┌────┬────┐  ╔════╦════╗  ┏━━━━┳━━━━┓
│    │    │  ║    ║    ║  ┃    ┃    ┃
├────┼────┤  ╠════╬════╣  ┣━━━━╋━━━━┫
└────┴────┘  ╚════╩════╝  ┗━━━━┻━━━━┛
";
