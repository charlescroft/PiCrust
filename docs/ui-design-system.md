# `pi-gpui` Whole-Application UI Design System & Specification

> **Single Source of Truth**: This document defines the complete UI architecture, component selection rules, visual tokens, surface hierarchy, and behavioral invariants for **every screen and component** in `pi-gpui`. Any agent implementing or modifying UI in `pi-gpui` MUST adhere strictly to this specification.

---

## 1. Core Foundations & Component Library

- **UI Framework**: `gpui-kit = "0.9.1"` (`gpui-fast`, `gpui-base`, `gpui-component`, `syntect` highlighter; never enable `tree-sitter-languages`).
- **Crate Patches**: `pi-gpui/patches/gpui-component` patches Unified Diff gutters so line numbers sit flush left in a single column matching the Source `Editor`.
- **Motion**: Modals and drawers use `Presence` (`160ms` fade/slide) and `Transition` (`200ms` panel width).
- **Virtualization**: Long lists (chat transcript, file trees, search palettes) MUST use virtualized `ListState` and bounded entity caches.

---

## 2. Visual Tokens, Sizing & Surface Hierarchy

### 2.1 Surface Hierarchy (Critical for Contrast)
Never stack identical surface colors on nested interactive containers:
- **Window / Main Canvas**: `cx.theme().background`
- **Sidebar**: `cx.theme().sidebar` with `sidebar_border`
- **Settings / Form Cards (`GroupBox::outline()`)**: Uses `group_box` / `background` card surface with `1px` `border` and `rounded_xl`.
  - **IMPORTANT**: Segmented `TabBar`s (`tab_bar_segmented`, which renders with `secondary` / muted pill track) **MUST** sit inside a `GroupBox::outline()` (or `background` surface), **NEVER** inside a `bg(cx.theme().secondary)` container—otherwise the segmented track background vanishes into the card!
- **Popovers / Menus / Preview Cards**: `cx.theme().popover` with `border` and `shadow_lg`.

### 2.2 Standardized Dimensions & Heights
Every sibling control in the same toolbar or form row MUST share an exact pixel height:

| Context | Standard Height | Standard Width | Examples |
| --- | --- | --- | --- |
| **Settings / Form Controls** | **`px(32.)`** (`h_8`) | **`px(260.)`** (`SETTINGS_DROPDOWN_W`) on right column | `Input`, `Button::outline()` dropdowns, `TabBar::segmented()`, `DatePicker`, `TimeField`, `Switch` row |
| **Form Action Buttons** | **`px(32.)`** | `px(100.)` (Cancel) / `px(124.)` (`SETTINGS_BTN_W`) | Modal/Editor footer actions, Settings Save/Update |
| **List / Card Action Buttons** | **`px(28.)`** | `px(124.)` or auto | Card inline actions, Pagination Prev/Next, Chat footer actions |
| **Compact Workbench Toolbars** | **`px(24.)`** | Auto | `FileViewer` (`fv-mode-tabs`, Mention, Open), `ArtifactPanel` (`art-mode-tabs`), Composer outbox delete |
| **Sidebar / Main Nav Rows** | **`px(30.)`** | `w_full()` inside `px(268.)` sidebar | Session rows, Project folder rows, Top nav rows |

### 2.3 Unified Icon Semantics
- **Delete / Remove**: Always `Icon::new(crate::icons::TrashIcon)` with `text_color(cx.theme().danger)` (sidebar menu, outbox, scheduled tasks, models, plugins, skills). Never use `IconName::Delete` (backspace icon).
- **Archive / Unarchive**: `Icon::new(crate::icons::ArchiveIcon)`
- **Git Branch**: `Icon::new(crate::icons::GitBranchIcon)`
- **Edit**: `Icon::new(crate::icons::PencilIcon)`
- **Running / Busy**: `Spinner::new()` (`LoaderCircleIcon` rotating)
- **Info / Details**: `IconName::Info`

---

## 3. Universal Control Selection & Linkage Rules

### 3.1 Mutual-Exclusion Rule (Strict)
1. **≤ 3 Mutually Exclusive Options → Segmented Group Tag (`TabBar::segmented()`)**
   - Container: `div().w(px(260.)).child(TabBar::new(id).segmented().w_full().h(px(32.)))`
   - Each tab: `Tab::new().flex_1().label(...).selected(...)` (Filling Space enabled; equal widths; distinct segmented track background).
   - Examples: Theme Mode (Dark / Light / System), Execution Target (New / Reuse), Tool Preset (Full / Default / Chat Only), Scope (Global / Project).
2. **> 3 Mutually Exclusive Options → Dropdown (`Button::outline().dropdown_caret(true).dropdown_menu(...)`)**
   - Dimensions: `.w_full().h(px(32.))` inside `w(px(260.))` container.
   - Menu items: `PopupMenuItem::new(...).checked(is_selected)`.
   - Examples: Project selector, Repeat frequency (Daily / Weekdays / Weekly / Once / Custom), Model selector, Thinking Level (>3 options).

### 3.2 Date & Time Controls
- Never use raw plain-text `Input`s for dates or times.
- **Date**: `DatePicker::new(&state)` (`DatePickerState::new(window, cx).date_format("%Y-%m-%d")`), height `px(32.)`.
- **Time**: `TimeField::new(&state)` (`TimeFieldState::new(window, cx).precision(TimePrecision::Minute)`), height `px(32.)`.

### 3.3 Model ↔ Thinking Level Linkage (Mandatory)
- Every UI that exposes both Model and Thinking Level (Composer, Scheduled Task Editor) **MUST** dynamically derive available Thinking options from the currently selected Model's `thinking_levels` (falling back to default model if "Default" is chosen).
- Changing the selected Model **MUST** validate and clamp/reset `thinking_level` if the previously selected level is unsupported by the new model—preventing invalid model/thinking combinations.
- If the selected model supports `≤ 2` thinking levels (+ Default = ≤ 3 total choices), render as a Segmented `TabBar`; otherwise render as a Dropdown.

---

## 4. Application Shell (`src/workspace/`)

### 4.1 Overall Layout
Outer vertical flex (`size_full`):
1. **Top `TitleBar`**: Window traffic lights padding, Sidebar collapse toggle, Conversation Search button (`Cmd+F` → `620px` `ConversationCommandPalette`), Right Workbench toggle, Dark/Light quick toggle.
2. **Main Horizontal Body** (`flex_1().min_h_0()`):
   - **Left `Sidebar`** (`w(px(268.))`, animated width `200ms`):
     - Top CTA: `Button::primary().small().h(px(30.)).w_full()` ("New Session")
     - Top Utility Nav (clean labels, **NO parenthesis counts**):
       - `Tasks` (shows green unread dot only when background task completes)
       - `Scheduled`
       - `Artifacts` (no unread dot)
     - `Pinned` sessions section
     - `Chats` (non-project workspace) section
     - `Projects` collapsible folders (**clean folder names, NO child session counts**; collapsed folders aggregate running `Spinner` / unread dots)
     - `Archived` collapsible group at bottom (context menu `Archive` / `Unarchive`)
   - **Center Viewport** (`flex_1().min_w_0()`):
     - **View Retention Rule**: `ChatPanel` remains mounted at all times and is hidden via `.hidden()` (`Display::None`) when `active_page != WorkspacePage::Chat`. Full-screen pages (`Tasks`, `Scheduled`, `Artifacts`) render as overlays so returning to Chat causes **zero remount or scroll jitter**.
   - **Right Workbench Panel** (resizable, `200ms` animated width):
     - Folder `TabBar` (`Artifact`, `FileViewer`, `FileExplorer`, `Terminal`) + close button.
3. **Bottom `StatusBar`**:
   - Left: Interactive Git Branch switcher button (`GitBranchIcon`, opens `Anchor::BottomLeft` upward `dropdown_menu` of local branches with checkmarks to `git checkout`).
   - Right: Core status indicator (tooltip shows live Core base URL + session API endpoint) and session stats.

---

## 5. Conversation & Chat View (`src/chat/`)

### 5.1 Virtualized Scroller Invariants (`ConversationScrollerState`)
- Uses `ListState::new(item_count, ListAlignment::Bottom, LIST_OVERDRAW).measure_all()` with `FollowMode::Tail`.
- **Synchronous Parse Fallback**: Because `TextViewState::markdown` parses inputs `> 4 KB` asynchronously (`rendered_text().source().is_empty()` on frame 0), `render_message_row` renders wrapped synchronous text on frame 0 while observing `TextViewState` to swap in rich Markdown as soon as the background parse commits. This guarantees **0px-collapse never occurs** and prevents frame-1 flashing of prior user messages.
- **Session Switch Stability**: `select_session` is a no-op if already viewing that session; async sidecar reconciliation matches and preserves existing `Message.id`s so `row_cache` is never unnecessarily invalidated.
- **Mutation-Guarded Pinning**: `sync_scroller_with_messages` only restores `pinned_offset` when messages actually mutated, never fighting active user scrolling.
- **Load Earlier History**: When `has_earlier_messages` is true, renders a `"Load earlier messages"` button above item `0`.

### 5.2 Message Cards (`message_row.rs`)
- **Max Content Width**: Configurable slider (`756px – 1512px`, default `840px`), centered (`mx_auto()`).
- **User Bubble**: Truncates past configurable threshold (default `300` chars) with Expand/Collapse; unified `24px` height footer with `6px` (`gap_1p5`) spacing (Copy, Edit, Fork, timestamp).
- **Process Details Group**:
  - `ShimmerText` pulsing primary color while `is_turn_live`; static muted label when settled.
  - Collapsed by default once settled unless "Expand thinking by default" is enabled.
  - **Thinking Steps**: Rendered via rich Markdown `TextView`.
  - **Tool Calls**: Expandable cards; `Bash` hides copy in collapsed header and shows copy next to command inside `160px` scrollable body.
  - **Compaction Card**: Expandable Markdown summary + collapsible `FileContext` section listing `Modified files` and `Read files` with file icons.
  - **Errors**: Mid-turn errors render in-place inside `ProcessDetailsGroup` with copy button; standalone bottom errors include dismiss (`×`).
- **Modified Files Summary Card**: Shown **ONLY** when `!is_turn_live && !modified_files.is_empty()` (never mid-turn between individual `write`/`edit` tool calls).
- **Turn Footer**: Shown **ONLY** when `!is_turn_live && has_answer_text` (`28px` Copy, Turn Usage DB popover, Duration, Timestamp).

### 5.3 Conversation Navigator (`ConversationNavigator`)
- Positioned vertically centered on the left gutter (`max_rail_h = 260px`), hidden automatically if center column `< chat_width + 88px`.
- Shows recent 8 turns by default; scrolling up past those 8 turns expands to all turns with adaptive tick spacing; scrolling back to the bottom (`is_at_tail`) automatically collapses back to 8 turns.
- Clicking top tick scrolls to `item_ix: 0`; clicking bottom tick calls `scroll_to_end()`; hovering shows preview card (`Question`, `Answer`, `Turn X / Total`).

### 5.4 Composer (`composer.rs`)
- Centered container matching `chat_content_width`.
- Persistent per-session drafts; instant model/thinking extraction from `.jsonl` on switch.
- Outbox queue rows with red `TrashIcon` delete button; `rpiv-todos` bar with persistent dismissal.
- `+` file mention picker, Model selector, Linked Thinking selector, Tool preset selector, Send (`ArrowUp` primary circle) / Stop (`Square` danger circle).

---

## 6. Right Workbench Panels (`src/panels/`)

- **`FileViewer` & `ArtifactPanel`**:
  - Header controls strictly `h(px(24.))`: Segmented `Diff` / `Source` `TabBar` (`h(px(24.))`), text `"Mention"` button (`h(px(24.))`), external `"Open"` button (`h(px(24.))`).
  - `Diff` renders strictly in Unified mode (`header_visible(false)`, single-column flush gutter matching `Editor`).
- **`FileExplorer`**: Virtualized tree with context menu (Open, Reveal, Copy Path, Mention).
- **`TerminalPanel`**: Embedded `gpui_xterm` PTY terminal per project cwd.

---

## 7. Full-Screen Pages (`src/pages/`)

### 7.1 Tasks Page (`tasks_page.rs`)
- Grouped live snapshots: Waiting, Running (with `Spinner`), Recent (up to 5).

### 7.2 Scheduled Tasks Page (`scheduled_page.rs`)
- **List View**: Task cards with status badge, next/last run, Run Now / Pause / Edit / Delete (`TrashIcon`).
- **Detail View**: Run history list with load-more pagination and delete confirmation modal (`TrashIcon`).
- **Editor View** (Strictly mirrors Settings `GroupBox::outline()` + `260px` right-aligned controls; **NO redundant "New Task" / "Edit Task" heading**):
  1. **`GroupBox` 1 · 基础信息 (Basic Info)**: `任务名称` (`Input`, `h(32)`), `任务内容` (vertical full-width 6-row `Textarea`).
  2. **`GroupBox` 2 · 执行位置与项目 (Execution Target & Project)**: `执行位置` (2 options ≤ 3 → Segmented `TabBar`: `每次新建对话` / `复用对话`, `h(32)`), `所属项目` (> 3 → `Dropdown`, `h(32)`).
  3. **`GroupBox` 3 · 运行时间与调度 (Schedule & Timing)**: `重复` (> 3 → `Dropdown`: Daily/Weekdays/Weekly/Once/Custom, `h(32)`), `开始日期` (`DatePicker`, `h(32)`), `开始时间` (`TimeField`, `h(32)`), `时区` (`Input`/`Dropdown`, `h(32)`).
  4. **`GroupBox` 4 · 模型与思考强度 (Model & Thinking)**: `使用模型` (`Dropdown`, `h(32)`), `思考强度` (dynamically linked to selected model's `thinking_levels`, Segmented `TabBar` if ≤ 3 choices else `Dropdown`, `h(32)`).
  5. **`GroupBox` 5 · 可用工具 (Available Tools)**: `可用工具` (3 options ≤ 3 → Segmented `TabBar`: `全部工具` / `默认工具` / `仅聊天`, `h(32)`).
  - Footer: `Cancel` (`outline().h(32).w(100)`) + `Save/Create` (`primary().h(32).w(124)`).

### 7.3 Artifacts Page (`artifacts_page.rs`)
- Search & Browse library (NO unread tracking or unread dots):
  - Top Filter Bar: Search `Input` (title, identifier, path, content) + Type Segmented `TabBar` (`All`, `HTML`, `React`, `SVG`, `Markdown`, `Code`, `h(px(32.))`).
  - Scrollable Card List: Type icon, Title, Kind tag, Identifier & Version, `"Preview"` outline button (opens right drawer & activates session without redundant reload).
  - Pagination Footer (`8` items/page): `"Page X of Y"`, Prev / Next buttons.

---

## 8. Settings Modal & Configuration (`src/settings/`)

- **Container**: Centered modal (`Presence` 160ms fade/slide), `Settings` left-nav shell (`General`, `Models`, `MCP`, `Plugins`, `Skills`, `Subagents`), `onBusyChange` dismiss guard during updates.
- **General**: Dark Mode Segmented `TabBar` (`w_full().h(px(32.))`, Filling Space `flex_1()`), Language dropdown, 3 continuous sliders (Chat Font Size `9–22px`, UI Font Size `5–21px`, Chat Width `756–1512px`) with live readout & reset buttons, Send shortcut, Thinking default collapse, Notifications, Network Proxy, Pi Core URL.
- **Models**: Provider cards with inline switch, pencil edit, red `TrashIcon` delete; Model Discovery queries Base URL directly and opens `380px` `DiscoveredModelsPicker` checkbox modal.
- **Plugins / Skills / Subagents / MCP**: `"插件管理"` header with `Add` (`+`) and `Update All` (`RefreshCw` spinner + per-row rotating `LoaderCircle`), `npm`/`git`/`local` source tags, info (`i`) detail modals without footer cancel button, unified switches & red `TrashIcon` delete buttons.
