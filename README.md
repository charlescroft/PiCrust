<div align="center">

<img src="assets/icon.png" alt="PiCrust Icon" width="128" height="128" style="margin-bottom: 12px;" />

# PiCrust

**The crispy, ultra-fast, GPU-native Rust crust baked for Pi Agent — bite into a slice of pure coding speed.**

<br/>

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GUI-GPUI%200.9.1-blue.svg?style=flat-square)](https://github.com/zed-industries/zed)
[![Platform](https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%2F%20Intel)-black.svg?style=flat-square&logo=apple)](https://apple.com)
[![Bundle Size](https://img.shields.io/badge/Bundle%20Size-~26%20MB-brightgreen.svg?style=flat-square)](#benchmark--performance)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=flat-square)](#license)

[English](README.md) | [简体中文](README.zh-CN.md)

</div>

---

## Overview

**PiCrust** is a next-generation desktop workbench for agentic AI coding. While modern AI code editors (Cursor, Windsurf, VS Code forks) rely on heavy Chromium/Electron runtimes that consume **1GB–2GB of memory** on startup, PiCrust is built from the ground up in **pure Rust** using Zed's high-performance **GPUI** framework.

It delivers **fluid GPU-accelerated rendering**, hardware-accelerated typography, native PTY terminal virtualization, an integrated **Git Graph** DAG visualizer, and a full-featured agentic coding environment—all packed into a tiny **26 MB standalone bundle** that operates comfortably within **~300 MB – 700 MB of RAM** under active developer workflows (inclusive of syntax trees, terminal scrollback, and diff buffers), compared to 1.5 GB – 3.5 GB+ for Electron-based solutions.

> **What is PiCrust?**  
> Just like the crispy, golden crust that holds a pie together, **PiCrust** provides a rock-solid, ultra-fast, GPU-accelerated native Rust shell for your AI coding agent. It cuts through latency like a hot knife through butter, giving you instant 0ms responses without the bloat of web technologies.

---

## Key Highlights & Competitive Edge

| Feature | PiCrust | Electron / Chromium IDEs | VS Code Extensions |
| :--- | :--- | :--- | :--- |
| **GUI Framework** | **GPUI 0.9.1 (Metal / GPU Native)** | Chromium WebView / DOM | Webview Iframe inside Electron |
| **Active Working Memory** | **~300 MB – 700 MB** | 1.5 GB – 3.5 GB+ | Bound to host editor (~1.5 GB+) |
| **Bundle Size** | **~26 MB (Standalone `.app`)** | 250 MB – 450 MB | Requires host IDE install |
| **Scroll & Render** | **GPU Virtualized (High Refresh Rate)** | DOM Repaint / Jitter | DOM Reflow / Event lag |
| **Local-First Speed** | **0ms Session & Branch Switch** | Multi-second reload | Re-render IPC latency |
| **Git Graph** | **Native Cubic Bézier DAG built-in** | Webview plugin required | Heavy Webview extension |
| **Terminal** | **Native `gpui_xterm` + `portable-pty`** | `xterm.js` via Node IPC | Shared IDE terminal |
| **Offline Resilience**| **Local JSONL, models, tools & cron** | Online portal dependency | Limited offline capability |

---

## Features

### Agentic Conversation & Composer
- **High-Performance Message Scroller**: Bounded virtualized list with `ConversationScrollerState` supporting hundreds of turns without frame drops.
- **Rich Content Rendering**: Hardware-accelerated typography, LaTeX math formulas via KaTeX, Mermaid diagrams (Flowcharts, Sequence, Class, State, Gantt), syntax-highlighted code blocks with copy actions, and unified/split diff cards.
- **Collapsible Thinking Steps**: Dynamic reasoning inspection with automatic folding for settled historical turns.
- **Smart Composer**: Auto-expanding textarea, Outbox queuing, single-send debouncing, and `@file` fuzzy search powered by `nucleo-matcher` + git-aware directory traversal.
- **Interactive Questionnaire UI**: Native UI for `ask_user_question` with single-select, multi-select, custom text entries, and real-time preview Monospace boxes.
- **Session Branching**: 0ms in-session branch switching (`navigate_tree`) and instant session forking from any message.

### Built-in Native Git Graph
- **Complete vscode-git-graph Migration**: Dedicated Graph panel and status bar integration right next to the active branch indicator.
- **Topological DAG Engine**: Real-time git commit DAG visualization with smooth cubic Bézier curves, persistent branch lane colors, and deduplicated parallel tracks.
- **Ref Badges**: Visual tags for `HEAD`, local branches, remote branches (`origin/*`), and annotated/lightweight tags.
- **Interactive Commit Inspector**: Full-screen overlay commit detail modal with author, timestamp, parent links, and full commit message.
- **Inline Numstat & Diff Preview**: Fixed-height scrollable file change list with `+added / -deleted` badges and an integrated multi-tab Diff viewer (Unified & Split modes).
- **Direct Actions**: Checkout commit/branch, create branch from commit, create tag, and copy SHA.

### Workbench & Developer Tools
- **Native PTY Terminal**: High-throughput terminal emulator powered by `gpui_xterm` and `portable-pty`, freeing you from `node-pty` memory overhead.
- **Tree-Based File Explorer & File Viewer**: Native file tree with fast ignored path filtering, syntect code highlighting, and instant file previews.
- **Artifacts Gallery**: Cross-session index for HTML, React, SVG, Mermaid, and Code artifacts with full offline persistence.
- **Timezone-Aware Scheduled Tasks**: Embedded 5-field cron engine supporting global timezones (`Asia/Tokyo +9`, `America/New_York -5`, etc.), multi-window lease locking, and upcoming run previews.

### Architecture & Stability
- **Single-Threaded Tokio Runtime**: Dedicated single-thread Tokio runtime (512 KB stack) strictly driving HTTP/SSE I/O, leaving GPUI on `smol` with zero thread contention.
- **Self-Healing Sidecar Watchdog**: Automatic process lifecycle management with parent PID tracking, dynamic port allocation, and automatic loopback authentication.
- **Dual Engine Architecture**: Supports direct native LLM streaming (OpenAI, Anthropic, Gemini, Responses) and background sidecar mode.

---

## Core Runtime & Extension Dependencies

PiCrust is designed as the native desktop frontend and execution workbench for the **Pi coding agent ecosystem** (`@earendil-works/pi-coding-agent`, `pi` CLI / `omp`).

### Pi Core Runtime (`pi` / `omp`)
- **Shared Session Store**: Automatically discovers and synchronizes with sessions stored in `~/.pi/agent/sessions/<encoded-cwd>/`.
- **Configuration & Auth**: Seamlessly integrates with standard Pi configuration files (`~/.pi/agent/settings.json`, `models.json`, and `auth.json`).
- **Live Terminal & CLI Interop**: Changes made in the `pi` terminal CLI or TUI reflect in PiCrust in real time (<300ms) via the active JSONL watcher.

### Interactive Task List (`todo`)
- **Extension Dependency**: Integrates with the `@juicesharp/rpiv-todo` extension.
- **Real-Time Task Overlay**: Intercepts `todo` tool invocations (`create`, `update`, `list`, `get`, `delete`, `clear`) and parses live progress envelopes.
- **Status Lifecycle**: Visualizes task state transitions (`pending` → `in_progress` → `completed` / `deleted`) with active spinner labels.

### Subagent Fleet Orchestration (`subagents`)
- **Extension Dependency**: Powered by the `pi-subagents` extension (`subagent`, `subagent_supervisor`, `contact_supervisor`, `bg_wait`).
- **Async Execution & Review Fanout**: Tracks concurrent child agent tasks, background job lifecycles, and interactive supervisor requests.
- **Custom Agent Profiles**: Loads and manages agent profiles defined in `~/.pi/agent/agents/*.md` and project-local `.pi/agents/*.md`.

---

## Installation & Getting Started

### Prerequisites
- macOS 13.0 (Ventura) or newer (Apple Silicon & Intel supported)
- [Rust](https://rustup.rs/) `1.80+` (stable)
- Node.js `18+` (only required when building the optional API sidecar bundle)

### Quick Start (Build & Run)

```bash
# Clone the repository
git clone https://github.com/charlescroft/picrust.git
cd picrust/pi-gpui

# Run test suite (38 unit & integration tests)
cargo test --offline

# Build and run optimized release binary
cargo run --release
```

### Packaging macOS `.app` Release Bundle

To produce a self-contained, signed macOS application bundle:

```bash
# From repository root
./pi-gpui/bundle-macos.sh
```

The output bundle will be generated at:
```text
pi-gpui/dist-macos/PiCrust.app  (~26 MB)
```
You can drag `PiCrust.app` into `/Applications` and launch it directly.

---

## Global Keyboard Shortcuts

| macOS Shortcut | Windows / Linux Shortcut | Action |
| :--- | :--- | :--- |
| `Cmd + N` | `Ctrl + N` | Start a new session / chat |
| `Cmd + F` | `Ctrl + F` | Open in-conversation search & command palette |
| `Cmd + B` | `Ctrl + B` | Toggle left sidebar |
| `Cmd + J` | `Ctrl + J` | Toggle right workbench panel |
| `Cmd + Shift + T` | `Ctrl + Shift + T` | Open native PTY terminal |
| `Cmd + ,` | `Ctrl + ,` | Open Settings modal |
| `Esc` | `Esc` | Stop running agent / close active modal dialog |
| `Enter` / `Cmd + Enter` | `Enter` / `Ctrl + Enter` | Submit prompt (configurable in Settings › General) |

---

## Configuration & Customization

PiCrust stores all client preferences in a single local JSON file:
- **macOS / Linux**: `~/.pi/agent/picrust-settings.json`
- **Windows**: `%USERPROFILE%\.pi\agent\picrust-settings.json`

Key configuration options include:
- **Theme & Appearance**: Dark/Light/System theme mode, official GPUI kit color schemes (Twilight, Solarized, Gruvbox, etc.), chat font size, UI font size.
- **Composer Shortcuts**: `enter`, `cmd-enter`, or `ctrl-enter` for prompt submission.
- **Core Connection**: `auto` (automatic sidecar discovery) or `manual` (custom remote core endpoint).
- **Thinking Defaults**: Configurable default expansion state and model-linked reasoning levels.

---

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                       PiCrust                               │
│                                                             │
│  ┌───────────────────────┐       ┌───────────────────────┐  │
│  │     WorkspaceView     │       │     ChatPanel         │  │
│  │  - TitleBar           │       │  - MessageScroller    │  │
│  │  - SessionSidebar     │──────>│  - Composer & Outbox  │  │
│  │  - StatusBar          │       │  - BranchNavigator    │  │
│  │  - RightWorkbench     │       │  - Questionnaire      │  │
│  └───────────────────────┘       └───────────────────────┘  │
│             │                                │              │
│  ┌──────────▼────────────┐       ┌───────────▼───────────┐  │
│  │   WorkbenchPanels     │       │     GitGraphView      │  │
│  │  - ArtifactGallery    │       │  - Cubic Bezier DAG   │  │
│  │  - FileExplorer       │       │  - Commit Inspector   │  │
│  │  - NativeTerminal     │       │  - Diff Numstat Modal │  │
│  └───────────────────────┘       └───────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   Core Engine                         │  │
│  │  - NativeIndex: Ignore + Nucleo-matcher search        │  │
│  │  - LocalStore: Atomic JSON / JSONL state persistence  │  │
│  │  - Scheduler: 5-field Cron engine with Timezones      │  │
│  │  - Single-threaded Tokio I/O driver (reqwest SSE)     │  │
│  └───────────────────────────────────────────────────────┘  │
└──────────────────────────────▲──────────────────────────────┘
                               │ Loopback IPC (Auto-Port)
┌──────────────────────────────▼──────────────────────────────┐
│            Isolated Node.js Sidecar (api-sidecar.mjs)       │
│    - 111 bundled API routes                                 │
│    - Watchdog auto-termination on parent exit               │
└─────────────────────────────────────────────────────────────┘
```

---

## Contributing

We welcome contributions! Please follow these guidelines:
1. Ensure all code remains isolated inside `pi-gpui/`.
2. Verify all tests pass before submitting a PR:
   ```bash
   cargo test --offline
   ```
3. Maintain compliance with `docs/ui-design-system.md` (mutual-exclusion controls, bounded virtualization, unified icons).

---

## License

PiCrust is open-sourced under the [Apache License 2.0](LICENSE-APACHE).
