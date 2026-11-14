# Pi Agent Desktop (`pi-gpui/`) — Implementation Status Report

Reference Specification: [`docs/gpui-migration-guide.md`](../../docs/gpui-migration-guide.md)

---

## Executive Summary

All core architecture phases (**Phase 0**, **Phase 0.5**, **Phase 1**, **Phase 2**, **Phase 3**, **Phase 4.1**, **Phase 5**, and **Phase 6**) have been implemented inside `pi-gpui/` with **35/35 Rust unit tests** (`cargo test --offline`) and **5/5 sidecar router tests** (`node --test pi-gpui/sidecar/router-utils.test.mjs`) passing.

---

## Phase-by-Phase Audit vs `docs/gpui-migration-guide.md`

### Phase 0 & Phase 0.5: Repository Isolation, Dependencies & Runtime Architecture

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§0.1** | Strict isolation inside `pi-gpui/`; zero modifications to root `app/`, `components/`, `lib/`, `src-tauri/` | **Done** | `pi-gpui/` (own `.git` repo) |
| **§0.2** | `gpui-kit = "0.9.1"` (`gpui-fast`, `syntect` only; never `tree-sitter-languages`), `gpui-kit-markdown-extras = "0.3.0"`, `gpui_xterm = "0.3.0"`, `gpui-webview = "0.9.1"`, `codegen-units = 1`, `lto = "thin"`, `strip = "symbols"` | **Done** | `pi-gpui/Cargo.toml` |
| **§0.5** | Eliminate multi-threaded Tokio redundancy (`rt-multi-thread`); run `reqwest` on a single-threaded `current_thread` Tokio I/O driver thread (`512 KB` stack) while GPUI uses `smol` | **Done** | `pi-gpui/Cargo.toml`, `pi-gpui/src/client/mod.rs` (`tokio_runtime()`) |
| **§2.2 / §3.4** | Bounded LRU caches (`MAX_CACHED_MESSAGE_ROWS = 300`, `MAX_WARM_SESSIONS = 6`, per-session `ListState` isolation via `replace_fresh`), lazy `gpui-webview` creation & immediate destruction on close/tab switch | **Done** | `pi-gpui/src/chat/mod.rs`, `pi-gpui/src/panels/artifact_panel.rs` |

---

### Phase 1: Pure `node:http` JSON/SSE Sidecar & Rust Client Shell

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§3.1.1** | Pure `node:http` JSON/SSE server (`api-sidecar.mjs`), `next-server-shim.ts`, dynamic route matcher (`router-utils.mjs`), `esbuild` bundler (`build-sidecar.mjs`) | **Done** | `pi-gpui/sidecar/*` (5/5 tests pass) |
| **§3.1.2** | Window geometry limited to `SIZE \| POSITION \| MAXIMIZED` persisted to `~/.pi/agent/gpui-settings.json`, per-process `x-pi-desktop-token` & `x-pi-desktop-instance` | **Done** | `pi-gpui/src/main.rs`, `pi-gpui/src/client/sidecar.rs` |
| **§3.1.3** | Main workbench layout (`TitleBar` + left `Sidebar` + center `ChatPanel` + right `WorkbenchPanel` + `StatusBar`) | **Done** | `pi-gpui/src/workspace/*` |

---

### Phase 2: Streaming Conversation, Markdown/Math/Mermaid, Diff & Extension UI

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§3.2.1** | `SessionController`: monotonic `load_request_id`, single-send debouncing + input restore on error, `seedStreamingSnapshot`, SSE stream reconciliation | **Done** | `pi-gpui/src/client/session_controller.rs`, `pi-gpui/src/app_state.rs` |
| **§3.2.2** | Virtualized `ConversationScrollerState` (`ListAlignment::Bottom`, `800px` overdraw), `TextView` with `markdown_extras` (KaTeX + Mermaid), `Diff` cards, collapsible Thinking & Modified Files cards, `ConversationNavigator` rail | **Done** | `pi-gpui/src/chat/mod.rs`, `pi-gpui/src/chat/message_row.rs` |
| **§3.2.3** | Composer auto-grow `InputState`, Outbox queue, root-first `@` / `+` directory browser (with parent-directory navigation + fuzzy search fallback + deferred cursor-to-end positioning), `rpiv-todos` live card, `Questionnaire` (`ask_user_question` & primitive `select`/`input`/`confirm`) | **Done** | `pi-gpui/src/chat/composer.rs`, `pi-gpui/src/chat/questionnaire.rs` |

---

### Phase 3: Native Rust Offloading (`gpui_xterm`, `@file` Index, JSONL Scanner)

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§3.3.1** | Native `gpui_xterm` + `portable-pty` terminal panel (eliminating `node-pty` and `xterm.js`) | **Done** | `pi-gpui/src/panels/terminal_panel.rs` |
| **§3.3.2** | Native Rust `ignore::WalkBuilder` + `nucleo-matcher` file search and native `~/.pi/agent/sessions/**/*.jsonl` scanner (respecting `user_home()`) | **Done** | `pi-gpui/src/client/native_index.rs` |
| **§3.3.3** | Right-hand `FileExplorer` (with `@` mention button) and `FileViewer` (`[Diff \| Source]` tabs with `syntect` highlighting and live `preview_font_size`) | **Done** | `pi-gpui/src/panels/file_explorer.rs`, `pi-gpui/src/panels/file_viewer.rs` |

---

### Phase 4: Full Workbench Feature Parity

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§4.1** | **In-Session Branching (`SessionTreeIndex`) & Session Fork**: `SessionTreeIndex` over `.jsonl` entries (`parentId` tree + `leaf_id` resolution + `BranchSibling` computation), **0ms local branch switching** (`navigate_to_branch_entry`), user message edit (`edit_user_message`), and session fork (`fork_session_from_entry` / `fork_session_local`) | **Done** | `pi-gpui/src/client/session_controller.rs`, `pi-gpui/src/app_state.rs`, `pi-gpui/src/chat/branch_nav.rs`, `pi-gpui/src/chat/message_row.rs` |
| **§4.2** | Three-tier Artifact engine (`TextView` for Markdown/Mermaid/Code, native `gpui::img` for SVG/Images, lazy `gpui-webview` for HTML/PDF, external card for Office/Binary) | **Done** | `pi-gpui/src/panels/artifact_panel.rs` |
| **§4.3** | Full-screen `TasksPage`, `ScheduledPage` (`GroupBox::outline()` with `DatePicker` + `TimeField` and linked Model/Thinking), and `ArtifactsPage` (cross-session gallery with category filter + pagination) | **Done** | `pi-gpui/src/pages/*` |
| **§4.4** | Searchable 6-page `Settings` dialog (`General`, `Models`, `MCP`, `Subagents`, `Skills`, `Plugins`) with `settings_busy` full-screen blocking overlay and CLI install-command normalization (`pi install` / `npx skills add`) | **Done** | `pi-gpui/src/settings/*` |

---

### Phase 5: Hard Rules & Verification Matrix

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§5.1** | All 10 Hard Rules preserved (window state flags, `user_home()`, `enabledModels` minimal edits, `disabledBuiltIns` subagent rule, `settings_busy` guard, single-shot composer send, monotonic `load_request_id`, per-process desktop token, sandboxed preview) | **Done** | Verified across `pi-gpui/src/*` |
| **§5.2** | Automated test suite + macOS `.app` release bundle (`Pi Agent.app`, ~22 MB binary + icon + Info.plist) | **Done** | `pi-gpui/bundle-macos.sh`, `pi-gpui/dist-macos/Pi Agent.app` |

---

### Phase 6: Deep Native Rust Offloading & Scale-to-Zero RPC Worker

| Guide Section | Requirement | Status | Implementation Location |
| :--- | :--- | :---: | :--- |
| **§6.3.1** | **Phase 6.1 — Direct Local CRUD & Config Persistence (`local_store.rs`)**: Atomic write-temp-then-rename (`write_file_atomic`) for sessions (rename, delete with subagent cascade & fork reparenting, fork), `settings.json`, `pi-web/settings.json`, `models.json` (preserving redacted API keys), `scheduled-tasks.json` + run logs, `agents/settings.json` + `.md` profiles, `SKILL.md` surgical frontmatter edits, `mcp.json`, `trust.json`, and local `git worktree` commands | **Done** | `pi-gpui/src/client/local_store.rs`, wired into `AppState`, `ScheduledPage`, and `Settings` |
| **§6.3.2** | **Phase 6.2 — Native Rust LLM Streaming, Built-in Tools & In-Process JS Sandbox**: Direct `reqwest` SSE streaming (`OpenAiCompletions`, `OpenAiResponses`, `AnthropicMessages`, `GoogleGenerativeAi`), native `read`/`write`/`edit`/`bash` tools with unified diff generation (`similar::TextDiff`), and in-process `JavaScriptCore` sandbox (`quickjs_sandbox.rs`) exposing `console.log`, `pi.read`, `pi.write`, and `pi.edit` | **Done** | `pi-gpui/src/engine/llm_stream.rs`, `pi-gpui/src/engine/native_tools.rs`, `pi-gpui/src/engine/quickjs_sandbox.rs`, wired into `AppState::dispatch_single_prompt` |
| **§6.3.3** | **Phase 6.3 — Native Rust Cron Scheduler & Scale-to-Zero RPC Worker**: 5-field cron parser (`CronSchedule`), multi-window lease lock (`scheduler-lease.json`), `WorkerLivenessState` (`active_foreground_runs == 0 && active_subagent_runs == 0 && pending_ui_requests == 0 && active_mcp_sessions == 0`), and `ScaleToZeroWorkerManager` (180s idle TTL auto-reclaim) | **Done** | `pi-gpui/src/engine/scheduler.rs`, `pi-gpui/src/engine/worker_manager.rs`, `AppState::start_native_scheduler_and_worker_reaper` |

---

## Remaining Optional Parity Items (For Next-Stage Review)

1. **Interactive OAuth Login / Device-Code Flow UI in `Settings -> Models`**:
   - Currently API-key providers and local `~/.pi/agent/auth.json` credentials work out of the box; adding an interactive OAuth sign-in / paste-code modal in `ModelsPage` would complete browser OAuth onboarding from inside GPUI.
2. **Dedicated Sandboxed `Chats` Workspace Policy UI**:
   - Enforcing and surfacing the non-project `Chats` folder sandbox (`pi-web:chat-unlock` toggle and restricted tool preset badge) in the sidebar `Chats` section.
3. **One-Click Session Export to Standalone HTML**:
   - Adding a local Rust exporter that renders a session `.jsonl` into a self-contained HTML file via native save dialog.
4. **Completed Todo History Filtering**:
   - Optionally collapsing or hiding older completed `rpiv-todos` batches when a new todo list is created in long sessions.
