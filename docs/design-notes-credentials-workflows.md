# PiCrust 新增左下角三大功能设计文档：笔记、凭证、工作流

> 目标读者：执行开发模型（Flash）与后续维护者。  
> 本文档涵盖 **入口与 UI 规范、gpui_kit 控件选型、数据结构与存储路径、后端/Agent 注入与执行机制、以及分步实现清单**。

---

## 一、总体入口与 UI 规范

### 1. 左下角入口设计（`src/workspace/sidebar.rs`）
当前 `render_sidebar` 返回一个固定宽度、带右边框（`border_color(cx.theme().sidebar_border)`）的 `v_flex()`，结构为：
1. 顶部 `new-session-btn` 下拉按钮；
2. 中间 `.flex_1().min_h_0().overflow_y_scroll()` 的 `scroll_body`。

**改造方案**：在 `.child(scroll_body)` **之后**追加一个固定在左下角底部的工具栏区域（位于 `status_bar` 上方，与左侧边栏宽度一致）：
- 容器样式：`h_flex().w_full().px_2().py_1p5().gap_1().border_t_1().border_color(cx.theme().sidebar_border).items_center()`
- 三个紧凑入口按钮（均采用 `Button::new(...).ghost().small().flex_1().h(px(28.))`）：
  1. **笔记（Notes）**：图标 `IconName::FileText`（或 `BookOpen`），文案 `tr(locale, "sidebar.notes")`（"笔记" / "Notes"），点击打开 **笔记弹窗**；
  2. **凭证（Credentials）**：图标 `IconName::KeyRound` / `Icon::new(crate::icons::ShieldIcon)`，文案 `tr(locale, "sidebar.credentials")`（"凭证" / "Credentials"），点击打开 **凭证弹窗**；
  3. **工作流（Workflows）**：图标 `IconName::Workflow` / `IconName::Play`，文案 `tr(locale, "sidebar.workflows")`（"工作流" / "Workflows"），点击打开/切换到右侧边栏 **`RightPanelTab::Workflows`**（若已打开且当前即为 `Workflows` 则折叠关闭，体验与 Files/GitGraph 一致）。

### 2. 通用视觉与控件规范（对齐现有 `gpui_kit` 与 `add_dialog.rs`）
- **颜色 Token**：统一使用 `cx.theme().background`、`cx.theme().secondary`、`cx.theme().border`、`cx.theme().muted_foreground`、`cx.theme().foreground`、`cx.theme().primary`、`cx.theme().danger`，**禁止硬编码 RGB 色值**，确保浅色/深色主题自动适配。
- **弹窗规范**（用于「笔记」「凭证」）：
  - 遮罩：全屏半透明 `div().bg(cx.theme().background.opacity(0.65))`（参考 `src/settings/add_dialog.rs`）；
  - 主容器：`v_flex().w(px(760.)).h(px(520.)).rounded_lg().border_1().border_color(cx.theme().border).bg(cx.theme().background).shadow_lg()`；
  - 控件统一采用 `gpui_kit::component::*`：
    - 标签切换：`TabBar` + `Tab`（或分段 `Button` 组）；
    - 单行输入：`Input::new(&state).small()`；
    - 多行正文/指令：`Textarea::new(&state)`；
    - Markdown 渲染预览：`TextView::markdown(...)`（参考 `src/chat/message_row.rs`）；
    - 复选框：`Checkbox::new(...)`；
    - 按钮：主动作 `.primary().small()`，次要 `.outline().small()`，图标/辅助 `.ghost().small()`，危险操作 `.danger().ghost().small()`；
    - 轻提示：`window.push_notification(Notification::success/error(...), cx)`。
- **国际化**：所有可见文案必须写入 `src/i18n.rs`（`en_text` 与 `zh_text` 同步补齐）。

---

## 二、功能 1：笔记（Notes）

### 1. 存储规范（跨平台）
- **工作区级笔记**：`<active_cwd>/.picrust/notes/*.md`
  - 仅在当前为项目工作区（`active_workspace_kind == WorkspaceKind::Project` 且非 `is_chat_workspace_cwd(&active_cwd)`）时可用；
  - 首次在项目内创建 `.picrust/notes/` 时，可选检查并向 `.picrust/.gitignore`（或提示用户）保持干净。
- **全局级笔记**：统一放在跨平台 Pi 配置根目录下（与 `~/.pi/agent` 一致）：
  - macOS / Linux：`~/.pi/agent/picrust/notes/*.md`
  - Windows：`%USERPROFILE%\.pi\agent\picrust\notes\*.md`（即复用 `crate::client::local_store::pi_agent_root().join("picrust/notes")`）。
- **文件名规范**：用户输入的标题自动做安全文件名清洗（保留中英文、数字、`-`、`_`，去除 `/ \ : * ? " < > |`，自动补全 `.md` 后缀）。

### 2. UI 结构与控件选择（单区 `预览 | 编辑` 切换）
```text
+----------------------------------------------------------+
|  笔记         [ 全局笔记 | 工作区笔记 ]             [×]  |
+--------------------+-------------------------------------+
| [+ 新建]           |  架构备忘.md          [ 预览 | 编辑 ]|
| > 架构备忘.md      |-------------------------------------|
|   部署步骤.md      |                                     |
|   SSH笔记.md       |   (预览态: TextView::markdown 渲染)  |
|                    |   (编辑态: Textarea + 保存状态提示)  |
|                    |                                     |
+--------------------+-------------------------------------+
| 路径: .picrust/notes/架构备忘.md  [删除] [保存] [插入引用] |
+----------------------------------------------------------+
```
- **顶部栏**：标题 + `TabBar`（`全局笔记` / `工作区笔记`，若当前无项目工作区，工作区 Tab 置灰禁用或显示提示）+ 关闭按钮 `IconName::Close`。
- **左侧列表**（宽 `px(200.)`，`overflow_y_scroll()`）：
  - 顶部 `+ 新建` 按钮（弹出内联输入框或直接创建 `untitled-N.md` 并切到编辑态）；
  - 列表项展示文件名（去掉 `.md` 后缀）与修改时间，选中态使用 `cx.theme().accent`。
- **右侧单内容区**（不左右分栏）：
  - 右上角提供分段切换按钮 `预览 | 编辑`：
    - **预览态**（默认）：使用 `TextView` 渲染 Markdown；
    - **编辑态**：使用 `Textarea` 编辑原文，点击底部 **「保存」**（或切回预览时自动保存）原子写入磁盘，并切回预览态。
- **底部操作栏**：
  - 左侧：显示当前笔记文件相对/绝对路径（截断 + tooltip）；
  - 右侧：**「删除」**（带二次确认或删除通知）、**「保存」**（仅编辑态高亮显示）、**「插入引用」**（主按钮 `.primary()`）。

### 3. 插入对话交互
- 点击 **「插入引用」**：
  - 工作区笔记插入 `@.picrust/notes/<file>.md`；全局笔记插入 `@<abs_path>`；
  - 写入 `AppState.pending_composer_mention`（或直接调用 Composer 插入草稿文本 helper），关闭弹窗并聚焦输入框，完全复用现有 `@` 文件引用链路。

---

## 三、功能 2：凭证（Credentials）

### 1. 存储规范（单行独立文件 + 元数据索引）
**为什么不使用单个 JSON 存 K-V？**  
若存成单个 JSON，Agent 一旦调用 `read` 读取该 JSON，所有凭证的明文都会一次性暴露进模型上下文与会话日志。拆分为「索引 + 独立单行文件」后，Agent 用到哪条就只读那一个文件，最小化暴露范围；同时 Pi 原生 `read` 工具跨平台（macOS/Linux/Windows 通用，不依赖 `cat`）。

- **目录**：`pi_agent_root().join("picrust/credentials/")`
- **索引文件**：`index.json`（仅保存元数据，**严禁保存明文值**）：
  ```json
  {
    "items": [
      {
        "name": "STEAMDECK_PASS",
        "description": "Steam Deck (192.168.0.39) SSH 密码",
        "autoDiscover": true,
        "updatedAt": 1794800000000
      }
    ]
  }
  ```
- **单行凭据文件**：`secrets/<NAME>.txt`
  - 仅允许合法环境变量风格名称：`^[A-Za-z_][A-Za-z0-9_]*$`；
  - 写入时 trim 首尾换行并确保**文件内容只有单行**（无尾部换行或仅单行文本），Unix 下强制设置文件权限 `0o600`（`std::os::unix::fs::PermissionsExt`），Windows 下正常写入并归入用户配置目录。

### 2. UI 结构与控件选择（管理 + 选择一体化弹窗）
```text
+---------------------------------------------------------------+
| 凭证管理                                                  [×] |
+--------------------+------------------------------------------+
| [+ 新建凭证]       | 名称: [ Input (STEAMDECK_PASS)         ] |
| > STEAMDECK_PASS   | 说明: [ Input (Steam Deck SSH 密码)    ] |
|   GH_TOKEN         | 值:   [ Input (masked=true) ] [显示/隐藏] |
|   PROD_DB_URL      | 路径: ~/.pi/agent/picrust/credentials/...|
|                    | [Checkbox] 允许 Agent 自动发现           |
+--------------------+------------------------------------------+
|                              [删除] [保存] [插入本次对话]      |
+---------------------------------------------------------------+
```
- 列表项只显示 `name` 与 `description`，绝不展示明文；
- 右侧值输入框默认开启 `masked(true)`（`InputState` 密码掩码），右侧提供「眼睛」图标切换明文显示；
- 底部操作：**「删除」**（同时删除 `index.json` 条目与 `secrets/<NAME>.txt`）、**「保存」**（原子写入 `secrets/<NAME>.txt` 与 `index.json`）、**「插入本次对话」**（主按钮）。

### 3. 两种注入与自动发现机制
1. **按需手动插入（默认）**：
   - 点击 **「插入本次对话」**，向输入框追加一段标准引用文本（**不含明文值**）：
     ```text
     [凭证 STEAMDECK_PASS（Steam Deck SSH 密码）：单行文件位于 `<abs_path>/secrets/STEAMDECK_PASS.txt`，需要时请用 read 工具读取或通过命令读取，切勿在回复中输出明文]
     ```
2. **勾选「允许 Agent 自动发现」**：
   - 仅当存在 `autoDiscover == true` 的凭证时，在 `AppState::dispatch_single_prompt`（发送给 sidecar/native 引擎之前，或在系统附加提示中）**只附加索引清单（名称 + 说明 + 文件路径，无明文）**；
   - 未勾选 `autoDiscover` 的凭证绝不出现在自动清单中，仅在用户手动插入时才对本次对话可见。

---

## 四、功能 3：工作流（Workflows）

### 1. 存储规范（复用 Pi 原生 `promptTemplates`）
Pi Sidecar 原生支持从以下两个目录加载 `promptTemplates`（`*.md`，在对话中输入 `/<name> [args]` 触发）：
- **全局工作流**：`~/.pi/agent/prompts/<name>.md`（即 `pi_agent_root().join("prompts")`）
- **项目工作流**：`<active_cwd>/.pi/prompts/<name>.md`

**统一 Markdown 格式**（同时兼容 Pi 原生展开 + PiCrust 步骤卡片解析）：
```markdown
---
description: 发布前检查、测试并打包应用
---
## Step 1: 运行单元测试
执行 `cargo test`，确认所有测试用例全部通过；若失败立即停止并报告原因。

## Step 2: 构建发布包
运行 `./bundle-macos.sh`，验证输出产物存在。

## Step 3: 提交并推送 ($1)
检查 `git status`，按参数 `$1` 提交并推送。
```
- 解析规则：
  - 读取 YAML frontmatter 中的 `description`；
  - 按 `## Step <N>: <Title>`（兼容 `## 步骤 <N>: <Title>` 或有序列表）解析出步骤数组 `Vec<WorkflowStep { title, instruction }>`；
  - 扫描正文中是否包含 `$1` / `$2` / `$ARGUMENTS`（决定是否在 UI 显示参数输入框）。

### 2. UI 设计：右侧边栏 `RightPanelTab::Workflows`（`src/panels/`）
在 `src/app_state.rs` 的 `RightPanelTab` 枚举中新增 `Workflows` 变体，在 `src/panels/mod.rs` 的 tab 栏与内容区挂载 `WorkflowsPanel`（新建 `src/panels/workflows_panel.rs`）。

#### 浏览 / 运行态
```text
+----------------------------------------+
| 工作流  [全局 | 项目]                  |
| [▾ Select: release-check      ] [编辑] |
| [+ 新建]                        [运行] |
+----------------------------------------+
| 描述：发布前检查、测试并打包应用       |
|                                        |
|  ┌──────────────────────────────────┐  |
|  │ [1] 运行单元测试                 │  |
|  │     执行 cargo test...           │  |
|  └──────────────────────────────────┘  |
|                   ↓                    |
|  ┌──────────────────────────────────┐  |
|  │ [2] 构建发布包                   │  |
|  │     运行 ./bundle-macos.sh...    │  |
|  └──────────────────────────────────┘  |
|                   ↓                    |
|  ┌──────────────────────────────────┐  |
|  │ [3] 提交并推送 ($1)              │  |
|  │     检查 git status...           │  |
|  └──────────────────────────────────┘  |
+----------------------------------------+
| 参数 (可选): [ Input                 ] |
+----------------------------------------+
```

#### 编排 / 编辑态（点击 `+ 新建` 或 `编辑` 进入）
```text
+----------------------------------------+
| 编辑工作流             [ 步骤 | 源码 ] |
| 名称: [ Input (release-check)        ] |
| 描述: [ Input (发布前检查...)        ] |
+----------------------------------------+
| 步骤 1: [ Input (标题)               ] |
| 指令:   [ Textarea (步骤执行指令)    ] |
|         [↑ 上移] [↓ 下移] [删除]       |
|----------------------------------------|
| [+ 添加步骤]                           |
+----------------------------------------+
|                        [ 取消 ] [ 保存 ]|
+----------------------------------------+
```
- **「步骤 | 源码」双模式**：步骤模式可增删/上移/下移步骤，源码模式直接用 `Textarea` 编辑 `.md` 全文；
- **点击「保存」**：将标题/描述/步骤序列化为标准 Markdown 原子写入 `~/.pi/agent/prompts/<name>.md`（或项目 `.pi/prompts/<name>.md`），通知 sidecar 刷新（若连接），刷新下拉框并切回运行态。

### 3. 保证 Agent 严格按工作流顺序执行的机制
1. **确定性执行指令包裹（Execution Wrapper）**：
   点击「运行」时，构造如下指令并调用 `AppState::dispatch_single_prompt` 发送（若不带额外隐藏逻辑，也可将用户勾选的命令按 `/name args` 或展开后的严格步骤提示发送）：
   - 先用实际填写的参数替换 `$ARGUMENTS` / `$1` / `$2`...；
   - 外层包裹强制约束协议：
     1) **第一步先调用 `todo` 工具**：把 Step 1 ~ Step N 全部写入任务清单；
     2) **严格顺序执行**：必须完成并验证 `Step i` 后，更新 `todo` 状态，再开始 `Step i+1`，**严禁跳步、合并未完成步骤或跳过失败步骤**；
     3) **失败即停**：任意一步失败时立刻停止后续步骤并明确汇报卡在哪一步及失败原因。
2. **进度联动**：因为第一步强制写入 `todo`，右侧工作流步骤卡片与原有 Todo 进度面板可实时同步展示当前执行到第几步。

---

## 五、后端与模块拆分建议（Rust 模块职责）

1. **存储与解析层（`src/client/local_store.rs` 或新模块 `src/client/workspace_extras.rs`）**：
   - `NotesStore`：`list_notes(scope, cwd)`、`read_note(path)`、`save_note(path, content)`、`delete_note(path)`；
   - `CredentialsStore`：`list_credentials()`、`save_credential(name, desc, secret, auto_discover)`、`delete_credential(name)`、`auto_discover_prompt_block()`；
   - `WorkflowsStore`：`list_workflows(scope, cwd)`、`parse_workflow_md(content)`、`serialize_workflow_md(def)`、`save_workflow(...)`、`delete_workflow(...)`、`build_workflow_execution_prompt(def, args)`。
2. **应用状态（`src/app_state.rs`）**：
   - 弹窗控制状态：`notes_modal_open: bool`、`credentials_modal_open: bool`（与现有 `agy_link_modal_open` 互斥关闭）；
   - 右侧面板：`RightPanelTab::Workflows`；
   - 发送前钩子：在 `dispatch_single_prompt`（仅限非隐式继续的正常用户消息）检查是否有 `autoDiscover` 凭证并注入轻量索引提示（或在发送给模型的 prompt 中追加）。
3. **UI 视图层**：
   - `src/workspace/sidebar.rs`：左下角三个入口按钮；
   - `src/settings/notes_modal.rs`（或 `src/panels/notes_modal.rs`）：笔记弹窗；
   - `src/settings/credentials_modal.rs`（或 `src/panels/credentials_modal.rs`）：凭证弹窗；
   - `src/panels/workflows_panel.rs`：右侧边栏工作流面板；
   - `src/panels/mod.rs`：注册 `RightPanelTab::Workflows` 标签与渲染；
   - `src/i18n.rs`：补齐中英文翻译键值。

---

## 六、建议实施顺序（便于 Flash 模型分步交付与验证）

1. **阶段 1（存储与单元测试）**：实现 `NotesStore`、`CredentialsStore`、`WorkflowsStore` 及单测（文件名清洗、单行凭证权限 `0o600`、Workflow Markdown 解析与序列化往返测试）。
2. **阶段 2（左下角入口 + 笔记弹窗）**：接通 `sidebar` 左下角按钮与笔记弹窗（全局/工作区切换 + `预览 | 编辑` + 插入 `@` 引用到输入框）。
3. **阶段 3（凭证弹窗 + 自动发现注入）**：实现凭证增删改查、值掩码、手动插入引用、以及勾选 `autoDiscover` 后的清单注入。
4. **阶段 4（右侧工作流面板 + 严格顺序执行）**：挂载 `RightPanelTab::Workflows`，完成步骤卡片浏览、编辑编排保存、参数填充与 `todo` 联动执行。
5. **收尾验证**：`cargo check --message-format=short`、`cargo test -- --nocapture`、`./bundle-macos.sh`。
