# Scheduled Tasks UI Specification (`pi-gpui`)

> **Design Principle**: Align visually and structurally with the **Settings** dialog (`src/settings/general_page.rs`, `src/settings/mod.rs`), reusing `GroupBox` (`GroupBoxVariant::Outline`), horizontal two-column `SettingItem`-style rows, uniform right-aligned control width (`260px`), and strict mutual-exclusion control selection rules.

---

## 1. Layout & Visual Tokens (Matching Settings)

| Token / Rule | Value / Component | Notes |
| --- | --- | --- |
| **Page Max Width** | `max_w(px(760.))`, centered (`mx_auto`) or `p_6()` padded scroll container | Prevents controls from stretching across ultra-wide monitors |
| **Group Container** | `GroupBox::new().outline().title(...)` (`GroupBoxVariant::Outline`) | Identical card border, background, corner radius, and title header as `SettingGroup` in Settings |
| **Section Spacing** | `gap_5()` (`20px`) between `GroupBox` cards | Matches `SettingPage` vertical rhythm |
| **Row Spacing** | `gap_4()` (`16px`) inside each `GroupBox` | Matches `SettingGroup` item spacing |
| **Row Layout** | Horizontal (`h_flex().justify_between().items_center().gap_4()`) for single-line controls;<br>Vertical (`v_flex().gap_2()`) for multi-line controls (`Textarea`) | Left column: Label (`text_sm`, `font_medium`) + optional description (`text_xs`, `muted_foreground`);<br>Right column: Fixed-width control (`w(px(260.))`) |
| **Unified Control Width** | `SETTINGS_DROPDOWN_W = px(260.)` | Every right-side input, dropdown, and segmented tab bar shares exact `260px` width |
| **Unified Control Height** | `px(32.)` (`h_8`) | Dropdown buttons (`outline().h(px(32.))`), inputs, and segmented `TabBar`s (`h(px(32.))`) MUST all be `32px` tall |

---

## 2. Control Selection Rule (Strict Mutual Exclusion)

1. **≤ 3 Mutually Exclusive Options → Segmented Group Tag (`TabBar::segmented()`)**
   - Container: `div().w(px(260.)).child(TabBar::new(...).segmented().w_full().h(px(32.)))`
   - Each option: `Tab::new().flex_1().label(...).selected(...)` (Filling Space enabled, equal widths)
2. **> 3 Mutually Exclusive Options → Dropdown (`Button::outline().dropdown_caret(true).dropdown_menu(...)`)**
   - Width: `.w(px(260.))`, Height: `.h(px(32.))`
   - Anchor: `Anchor::TopRight` or `Anchor::BottomLeft` with `PopupMenuItem::new(...).checked(...)`

---

## 3. Exact 5-Group Information Architecture

The Scheduled Task Editor contains **ONLY** the following 5 `GroupBox` sections in top-to-bottom order. No extra unrelated fields (no description field, no duration timeout field, no write-acknowledge checkbox).

### Group 1: 基础信息 (Basic Information)
*`GroupBox::new().outline().title("基础信息")`*

1. **任务名称 (Task Name)** — Horizontal Row (`h_flex`)
   - Left: Label `"任务名称"` + optional hint `"用于在列表与侧边栏中识别任务"`
   - Right (`w(px(260.))`): `Input::new(&self.name_input).h(px(32.))`
2. **任务内容 (Task Prompt)** — Vertical Row (`v_flex`)
   - Top: Label `"任务内容"` + hint `"触发时发送给 Agent 的完整提示词"`
   - Bottom (`w_full()`): `Textarea::new(&self.prompt_input)` (6 rows, full card width)

---

### Group 2: 执行位置与项目 (Execution Target & Project)
*`GroupBox::new().outline().title("执行位置与项目")`*

1. **执行位置 (Execution Target)** — Horizontal Row (`h_flex`)
   - Options (2 choices ≤ 3 → **Segmented Group Tag**):
     - `"每次新建对话"` (`"new"`)
     - `"复用对话"` (`"reuse"`)
   - Right (`w(px(260.))`): `TabBar::new("sched-exec-target-tabs").segmented().w_full().h(px(32.))` with two `.flex_1()` `Tab`s.
2. **所属项目 (Project)** — Horizontal Row (`h_flex`)
   - Options (N workspace projects > 3 → **Dropdown**):
     - Enumerates all known projects (`project_name` + `cwd`) from `AppState`.
   - Right (`w(px(260.))`): `Button::new("sched-project-select").outline().h(px(32.)).w_full().dropdown_caret(true).dropdown_menu(...)` showing current project name and checkmarked menu items.

---

### Group 3: 运行时间与调度 (Schedule & Timing)
*`GroupBox::new().outline().title("运行时间与调度")`*

1. **重复 (Repeat)** — Horizontal Row (`h_flex`)
   - Options (5 choices > 3 → **Dropdown**):
     - `"单次"` (`Once`)
     - `"每天"` (`Daily`)
     - `"工作日"` (`Weekdays`)
     - `"每周"` (`Weekly`)
     - `"自定义 Cron"` (`Custom`)
   - Right (`w(px(260.))`): `Button::new("sched-repeat-select").outline().h(px(32.)).w_full().dropdown_caret(true).dropdown_menu(...)`
   - *(Conditional sub-row when `"自定义 Cron"` is selected)*: Horizontal Row `"Cron 表达式"` with `w(px(260.))` `Input::new(&self.cron_input)`.
   - *(Conditional sub-row when `"每周"` is selected)*: Horizontal Row `"星期"` with `w(px(260.))` Dropdown (`周日`..`周六`).
2. **开始日期 (Start Date)** — Horizontal Row (`h_flex`)
   - Right (`w(px(260.))`): `Input::new(&self.start_date_input)` (`YYYY-MM-DD`, height `32px`)
3. **开始时间 (Start Time)** — Horizontal Row (`h_flex`)
   - Right (`w(px(260.))`): `Input::new(&self.time_input)` (`HH:MM`, height `32px`)
4. **时区 (Timezone)** — Horizontal Row (`h_flex`)
   - Options (> 3 common IANA timezones + System Local → **Dropdown** or editable `Input` with timezone dropdown preset)
   - Right (`w(px(260.))`): `Button::outline().h(px(32.)).w_full().dropdown_caret(true)` (or `Input` `32px` height)

---

### Group 4: 模型与思考强度 (Model & Thinking)
*`GroupBox::new().outline().title("模型与思考强度")`*

1. **使用模型 (Model)** — Horizontal Row (`h_flex`)
   - Options (Default + all enabled/configured models > 3 → **Dropdown**):
   - Right (`w(px(260.))`): `Button::new("sched-model-select").outline().h(px(32.)).w_full().dropdown_caret(true).dropdown_menu(...)`
2. **思考强度 (Thinking Level)** — Horizontal Row (`h_flex`)
   - Options (`默认`、`关闭`、`极小`、`低`、`中`、`高`、`超高` > 3 → **Dropdown**):
   - Right (`w(px(260.))`): `Button::new("sched-thinking-select").outline().h(px(32.)).w_full().dropdown_caret(true).dropdown_menu(...)`

---

### Group 5: 可用工具 (Available Tools)
*`GroupBox::new().outline().title("可用工具")`*

1. **可用工具预设 (Tool Preset)** — Horizontal Row (`h_flex`)
   - Options (3 choices ≤ 3 → **Segmented Group Tag**):
     - `"全部工具"` (`"full"`)
     - `"默认工具"` (`"default"`)
     - `"仅聊天"` (`"none"`)
   - Right (`w(px(260.))`): `TabBar::new("sched-tools-tabs").segmented().w_full().h(px(32.))` with three `.flex_1()` `Tab`s.

---

## 4. Header & Footer Actions

- **Top Header Bar**:
  - Back button (`Button::ghost().small().icon(IconName::ArrowLeft)`), Page Title (`"新建定时任务"` / `"编辑定时任务"`).
- **Bottom Footer Bar** (Right-aligned inside `max_w(px(760.))`):
  - `Button::new("sched-cancel-btn").outline().h(px(32.)).w(px(100.)).label("取消")`
  - `Button::new("sched-save-btn").primary().h(px(32.)).w(px(124.)).label("保存")` (disabled when `name`, `prompt`, or `cwd` is empty or `busy`)

---

## 5. Reference Implementation Snippet (Settings Style Pattern)

```rust
fn sched_row(
    title: impl Into<SharedString>,
    desc: Option<impl Into<SharedString>>,
    control: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .gap_4()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(div().text_sm().font_medium().text_color(cx.theme().foreground).child(title.into()))
                .when_some(desc.map(Into::into), |this, d| {
                    this.child(div().text_xs().text_color(cx.theme().muted_foreground).child(d))
                }),
        )
        .child(div().w(px(260.)).flex_shrink_0().child(control))
}
```
