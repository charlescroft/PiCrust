# Pi Agent Desktop — GPUI Kit Migration (`pi-gpui/`)

This directory contains the isolated migration of Pi Agent Desktop to **Rust + GPUI Kit (`0.9.1`)**, separated completely from the existing Next.js / Tauri codebase.

## Directory Layout

```text
pi-gpui/
├── Cargo.toml                  # Release-optimized GPUI + syntect + xterm + lazy webview manifest
├── Cargo.lock
├── patches/                    # Local Cargo patches for rustc 1.97.1 compatibility
├── sidecar/
│   ├── next-server-shim.ts     # Minimal NextRequest / NextResponse shim over Web Request/Response
│   ├── router-utils.mjs        # Dynamic/catchall segment matcher & header adapter
│   ├── router-utils.test.mjs   # Unit tests for sidecar routing
│   ├── build-sidecar.mjs       # esbuild bundler collecting ../app/api/**/route.ts
│   └── api-sidecar.mjs         # Pure node:http JSON/SSE sidecar server (replaces Next.js runtime)
└── src/
    ├── main.rs                 # App init, native menus, window geometry persistence (SIZE|POSITION|MAXIMIZED)
    ├── actions.rs              # Global shortcuts & GPUI actions
    ├── app_state.rs            # Shared AppState entity, SSE & sidecar state synchronization
    ├── client/
    │   ├── mod.rs              # Typed reqwest PiDaemonClient on single-thread Tokio I/O driver
    │   ├── types.rs            # Shared API & UI models
    │   ├── sse.rs              # eventsource-stream SSE consumer
    │   ├── sidecar.rs          # SidecarProcess lifecycle wrapper
    │   ├── session_controller.rs # SessionTreeIndex (0ms branch nav), monotonic load_request_id, artifact extraction
    │   ├── native_index.rs     # Phase 3 native `ignore` + `nucleo-matcher` @file search & JSONL session scanner
    │   └── local_store.rs      # Phase 6.1 atomic local CRUD for sessions, settings, models, tasks, skills, MCP, worktrees
    ├── engine/
    │   ├── mod.rs              # Phase 6.2 & 6.3 native execution engine exports
    │   ├── llm_stream.rs       # Direct Rust reqwest SSE streaming (OpenAI/Anthropic/Google/Responses)
    │   ├── native_tools.rs     # Native Rust read/write/edit/bash execution & unified diff builder
    │   ├── quickjs_sandbox.rs  # In-process JavaScriptCore sandbox with pi.read/write/edit host bindings
    │   ├── scheduler.rs        # Native 5-field cron parser & multi-window scheduler lease lock
    │   └── worker_manager.rs   # WorkerLivenessState & Scale-to-Zero RPC worker reaper (180s idle TTL)
    ├── workspace/
    │   ├── mod.rs              # TitleBar + Sidebar + DockArea + WorkbenchPanel + StatusBar
    │   ├── title_bar.rs        # Brand, cwd/branch chip, view switcher, theme & panel toggles
    │   ├── sidebar.rs          # New Session / Chat, Views, Pinned, Project groups, Chats, Settings
    │   └── status_bar.rs       # Branch, Terminal/Files/Inspector toggles, context progress, token/cost stats
    ├── chat/
    │   ├── mod.rs              # ChatPanel (MessageScroller + BranchNavigator + FindBar + Questionnaire + Composer)
    │   ├── message_row.rs      # User/Assistant/Notice rows, TextView (stream_fade + math/mermaid), Diff cards
    │   ├── composer.rs         # Auto-grow Textarea, outbox queue, @file fuzzy popup, model/thinking/tool selectors
    │   ├── questionnaire.rs    # Extension UI & ask_user_question with multi-select, custom input, and preview
    │   ├── branch_nav.rs       # In-session navigate_tree branch switcher
    │   └── find_bar.rs         # Conversation search using TextViewState::set_range_highlights
    ├── panels/
    │   ├── mod.rs              # Right WorkbenchPanel (Artifact / Files / Terminal / Inspector)
    │   ├── artifact_panel.rs   # 3-tier engine: TextView (MD/Mermaid/Code) -> Native SVG -> Lazy gpui-webview (HTML/React)
    │   ├── file_explorer.rs    # Tree-based project file explorer
    │   ├── file_viewer.rs      # Syntect-highlighted Editor viewer
    │   ├── terminal_panel.rs   # Native gpui_xterm + portable-pty terminal (replaces node-pty + xterm.js)
    │   └── inspector_panels.rs # Session, tool definitions, and token/context usage inspector
    ├── pages/
    │   ├── tasks_page.rs       # Live tasks across projects
    │   ├── scheduled_page.rs   # Cron/interval scheduled tasks & manual trigger
    │   └── artifacts_page.rs   # Cross-session artifacts gallery
    └── settings/
        ├── mod.rs              # Searchable 6-page Settings dialog with busy-close guard
        ├── general_page.rs     # Theme, rendering backend, sidecar/network/relay
        ├── models_page.rs      # Enabled models switches, catalog refresh, models.json info
        ├── mcp_page.rs         # MCP server list, test action, project trust toggle
        └── extensions_page.rs  # Subagents, Skills, Plugins pages
```

## Build & Run

### 1. Rust GPUI Client (`pi-gpui`)

```bash
cd pi-gpui
cargo test
cargo run --release
```

Without a running sidecar, `pi-gpui` scans `~/.pi/agent/sessions/` natively and starts in **Offline Preview** mode (allowing full UI, Markdown/Math/Mermaid, Diff, `@file` fuzzy search, File Explorer, and native `gpui_xterm` Terminal testing).

### 2. Node API Sidecar (when repo `node_modules` is installed)

From the repository root (after `npm ci` has populated `node_modules`):

```bash
node pi-gpui/sidecar/build-sidecar.mjs
node pi-gpui/sidecar/api-sidecar.mjs
```

When `pi-gpui/dist-sidecar/server.cjs` exists, launching `pi-gpui` automatically spawns `api-sidecar.mjs` on `127.0.0.1:38471` with per-process `x-pi-desktop-token` and `x-pi-desktop-instance` headers and shuts it down cleanly on exit.

### 3. Sidecar Router Unit Tests

```bash
node --test pi-gpui/sidecar/router-utils.test.mjs
```
