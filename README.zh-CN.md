<div align="center">

<img src="assets/icon.png" alt="PiCrust 派壳 图标" width="128" height="128" style="margin-bottom: 12px;" />

# PiCrust (派壳)

**为 Pi 烘焙的一层酥脆、极速、GPU 硬件加速的原生 Rust 外壳，轻咬一口，代码即成。**

*The crispy, ultra-fast, GPU-native Rust crust baked for Pi Agent — bite into a slice of pure coding speed.*

<br/>

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/GUI-GPUI%200.9.1-blue.svg?style=flat-square)](https://github.com/zed-industries/zed)
[![Platform](https://img.shields.io/badge/Platform-macOS%20(Apple%20Silicon%20%2F%20Intel)-black.svg?style=flat-square&logo=apple)](https://apple.com)
[![Bundle Size](https://img.shields.io/badge/包体大小-~26%20MB-brightgreen.svg?style=flat-square)](#性能对比基准)
[![License](https://img.shields.io/badge/开源协议-Apache%202.0-blue.svg?style=flat-square)](#开源协议)

[English](README.md) | [简体中文](README.zh-CN.md)

</div>

---

## 项目介绍

**PiCrust (派壳)** 是专为 AI 辅助编程打造的下一代原生桌面工作台。当传统基于 Web 或 Electron 架构的代码工具仍重度依赖 Chromium 运行时，在启动时即霸占 **1GB 至 2GB 内存** 时，PiCrust 采用 **纯 Rust** 与 Zed 高性能 **GPUI** 框架进行了彻底重构。

它实现了 **高帧率硬件加速流畅渲染**、GPU 文本排版、原生 PTY 虚拟终端集成、完整迁移的 **Git Graph（分支图谱）** 可视化以及全功能 AI Agent 交互工作区——整套独立应用程序打包体积仅 **~26 MB**，在真实编码工作负载下（包含语法树解析缓存、终端回滚缓冲与文件对比差异状态）常态内存占用稳定在 **~300 MB 至 700 MB**，远低于 Electron 方案动辄 1.5 GB 至 3.5 GB+ 的系统开销。

> **为什么叫 PiCrust (派壳)？**  
> 就像烘焙派（Pie）时那层金黄香脆、支撑整道美味的外壳（Crust），**派壳 (PiCrust)** 为强大的 AI 编程智能体提供了一层坚固、轻巧、全硬件加速的原生 Rust 甲壳。它像切开热派一样干脆利索地消除所有延迟，让开发者彻底告别 Web 技术的沉重与臃肿。

---

## 核心优势与竞品对比

| 功能特性 | PiCrust (派壳) | Electron / Chromium 架构编辑器 | 传统 VS Code 插件 |
| :--- | :--- | :--- | :--- |
| **界面框架** | **GPUI 0.9.1 (Metal / GPU 原生渲染)** | Chromium WebView / DOM 树渲染 | 嵌在 Electron 内的 Webview Iframe |
| **活跃工作内存** | **~300 MB – 700 MB** | 1.5 GB – 3.5 GB+ | 受宿主编辑器拖累 (通常 > 1.5 GB) |
| **应用包体积** | **~26 MB (独立 macOS `.app`)** | 250 MB – 450 MB | 需安装庞大宿主环境 |
| **滚动与排版** | **GPU 虚拟化高刷新率渲染** | DOM 重绘抖动 / 事件卡顿 | DOM 回流延迟 / 跨进程拖慢 |
| **本地优先性能** | **0ms 切换会话与分支回溯** | 多秒级页面加载重连 | 跨进程 IPC 通信排队 |
| **Git 分支图** | **内置三次贝塞尔曲线 DAG 图谱** | 需安装 Webview 插件 | 需依赖大型 Webview 扩展 |
| **终端引擎** | **原生 `gpui_xterm` + `portable-pty`** | 基于 Node IPC 的 `xterm.js` | 共享宿主终端环境 |
| **全离线可用性**| **本地 JSONL、模型、工具与调度全离线** | 强依赖远程云端服务 | 离线功能严重受限 |

---

## 核心特性

### 智能 Agent 交互与 Composer 编写器
- **虚拟化会话滚动流**：自研 `ConversationScrollerState` 支持数百轮长会话高频滚动与实时流式输出，顺滑无闪烁。
- **全格式富文本排版**：硬件加速文本渲染，原生支持 LaTeX 数学公式（KaTeX）、Mermaid 图表（流程图、时序图、类图、状态图、甘特图）、带代码复制的代码高亮块，以及 Unified/Split 双模代码对比卡片。
- **智能思考折叠（Thinking Steps）**：支持模型深度推理步骤的动态展开与收起，非末尾的历史轮次自动收敛保洁。
- **强大 Composer**：自适应高度文本域、发送队列管理、单发防抖去重、基于 `nucleo-matcher` + `ignore` 的智能 `@文件` 模糊查找。
- **交互式问卷（Questionnaire）**：原生渲染 `ask_user_question` 问卷，支持单选、多选、自定义输入与等宽对比预览面板。
- **树状分支回溯**：0ms 会话内局部历史跳转（`navigate_tree`）与任意消息点的全量派生分叉（Fork）。

### 内置 Git Graph 分支图谱
- **完备的 vscode-git-graph 原生体验**：底栏分支选择器旁一键呼出，专属右侧面板与全屏独立视图。
- **拓扑 DAG 渲染引擎**：基于 `canvas` + `PathBuilder` 绘制丝滑三次贝塞尔曲线（Cubic Bézier），独立轨道颜色持久化贯穿，相同链路平行线智能合流。
- **多态 Ref 徽标**：`HEAD`、本地分支、远程分支（`origin/*`）与轻量/附注标签彩色独立徽标展示。
- **全画面遮罩 Commit 详情弹窗**：居中悬浮遮罩窗，包含完整 SHA、提交者信息、时间、完整提交日志。
- **变更文件清单与独立代码对比**：固定高度内部滚动的文件清单，展示 `+新增 / -删除` 行数徽标；点击任意文件弹出独立的语法高亮 Diff 窗口（支持 Unified/Split 切换与一键复制）。
- **Git 快捷操作**：支持检出提交/分支、以此提交新建分支、新建标签、一键复制完整 Hash。

### 工作台与开发者工具集
- **原生 PTY 虚拟终端**：基于 `gpui_xterm` 与 `portable-pty`，绕过臃肿的 `node-pty`，启动迅速，吞吐量极大。
- **项目文件树与源码查看器**：集成 Syntect 语法高亮引擎，毫秒级快速预览项目文件。
- **产物库（Artifacts Gallery）**：跨会话聚合展示 HTML、React、SVG、Mermaid 及源码产物，支持离线索引与即时预览。
- **跨时区定时任务调度器（Cron Engine）**：内置 5 字段 Cron 解析器，精准计算跨时区（如 `Asia/Tokyo +9`、`America/New_York -5`）下次运行时刻，配备多窗口抢占式文件租约锁。

### 底层架构与稳定性
- **单线程 Tokio 驱动**：开辟轻量单线程 Tokio 运行时（512 KB 栈）专门接管 `reqwest` HTTP/SSE I/O，GPUI 独占 `smol`，互不抢占调度。
- **自愈式 Sidecar 守护看门狗**：父进程 PID 死亡自毁监测，自动探测与分配空闲端口，回环请求自动豁免，彻底杜绝僵尸进程与 403 错误。
- **双模运行引擎**：既可搭配后台 ESM 伴生进程，也具备纯 Rust 原生 LLM 流式直连与本地沙盒。

---

## 核心依赖与生态联动

PiCrust 是专为 **Pi 编码智能体生态**（`@earendil-works/pi-coding-agent`、`pi` 终端 CLI 与 `omp` 核心）打造的原生桌面客户端与交互工作台。

### Pi 智能体核心依赖 (`pi` / `omp`)
- **共享本地会话池**：自动发现并实时双向同步存储在 `~/.pi/agent/sessions/<encoded-cwd>/` 的所有会话。
- **配置与密钥认证**：无缝读取和管理 `~/.pi/agent/settings.json`、`models.json` 及 `auth.json` 中的供应商模型与凭据。
- **双向实时互通**：在终端通过 `pi` CLI 或 TUI 敲击发送的任何用户命令、模型流式输出或工具结果，会在 **300ms** 内通过文件感知监视器自动同步刷新呈现在 PiCrust 中。

### 实时任务管理功能依赖 (`todo`)
- **扩展依赖**：接入并深度适配 `@juicesharp/rpiv-todo` 任务列表扩展。
- **工具调用拦截解析**：全自动感知与解析智能体执行的 `todo` 工具命令（`create`、`update`、`list`、`get`、`delete`、`clear`）以及响应信封数据。
- **任务状态机渲染**：在界面展示交互式任务清单卡片，实时呈现任务流转阶段（`待处理 pending` → `进行中 in_progress` → `已完成 completed` / `已删除 deleted`），并展示动态处理动画。

### 子代理集群编排依赖 (`subagents`)
- **扩展依赖**：基于 `pi-subagents` 扩展体系构建，支持 `subagent`、`subagent_supervisor`、`contact_supervisor`、`bg_wait` 全套多智能体工具。
- **异步任务协同追踪**：跟踪子代理在后台或隔离 Git Worktree 中的异步派发任务、并行扇出审查（Review Fanout）及督导交互通道。
- **自定义角色配置**：自动扫描并加载用户在 `~/.pi/agent/agents/*.md` 或项目级 `.pi/agents/*.md` 中定义的定制 Subagent Profile 角色配置。

---

## 编译与快速上手

### 环境要求
- macOS 13.0 (Ventura) 或更高版本（原生支持 Apple Silicon M 系列芯片与 Intel 架构）
- [Rust](https://rustup.rs/) `1.80+` (稳定版)
- Node.js `18+`（仅在重新构建伴生服务时需要）

### 本地编译运行

```bash
# 克隆代码仓库
git clone https://github.com/charlescroft/picrust.git
cd picrust/pi-gpui

# 执行单元与集成测试（共 38 项自动化测试）
cargo test --offline

# 编译并运行 Release 优化版
cargo run --release
```

### 多平台独立产物打包

#### macOS 独立应用 (PiCrust.app)
```bash
./bundle-macos.sh
# 产物生成于：dist-macos/PiCrust.app (约 26 MB)
```

#### Windows 可执行程序 (PiCrust.exe)
```powershell
.\bundle-windows.ps1
# 产物生成于：dist-windows/PiCrust.exe 与 dist-windows/PiCrust-windows-x64.zip
```

#### Linux 独立镜像 (PiCrust-x86_64.AppImage)
```bash
./bundle-linux.sh
# 产物生成于：dist-linux/PiCrust-x86_64.AppImage
```

---

## 全局快捷键

| macOS 快捷键 | Windows / Linux 快捷键 | 功能操作 |
| :--- | :--- | :--- |
| `Cmd + N` | `Ctrl + N` | 新建对话 / 会话 |
| `Cmd + F` | `Ctrl + F` | 唤起会话内搜索与全局命令面板 |
| `Cmd + B` | `Ctrl + B` | 展开 / 折叠左侧边栏 |
| `Cmd + J` | `Ctrl + J` | 展开 / 折叠右侧工作台面板 |
| `Cmd + Shift + T` | `Ctrl + Shift + T` | 打开原生虚拟终端 |
| `Cmd + ,` | `Ctrl + ,` | 呼出全局设置面板 |
| `Esc` | `Esc` | 中断当前 Agent 执行 / 关闭当前活动弹窗 |
| `Enter` / `Cmd + Enter` | `Enter` / `Ctrl + Enter` | 发送提示词（可在 设置 › 通用 中切换） |

---

## 配置文件与持久化存储

所有客户端配置均持久化在单一标准的本地 JSON 文件中：
- **macOS / Linux**：`~/.pi/agent/picrust-settings.json`
- **Windows**：`%USERPROFILE%\.pi\agent\picrust-settings.json`

核心配置项包括：
- **外观与主题**：深色 / 浅色 / 跟随系统主题，内置官方配色方案（Twilight、Solarized、Gruvbox 等），独立调节对话字号与 UI 字号。
- **发送快捷键**：支持自定义为 `enter`、`cmd-enter` 或 `ctrl-enter`。
- **Core 核心模式**：`auto`（自动伴生服务寻址）或 `manual`（指定远程核心服务地址）。
- **思考强度联动**：根据所选模型的能力清单动态匹配推理强度。

---

## 整体架构设计

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
│  │  - ArtifactGallery    │       │  - 三次贝塞尔 DAG     │  │
│  │  - FileExplorer       │       │  - Commit 详情模态窗  │  │
│  │  - NativeTerminal     │       │  - 独立 Diff 预览窗   │  │
│  └───────────────────────┘       └───────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   原生底层引擎                        │  │
│  │  - NativeIndex: 快速文件检索与 JSONL 状态扫描         │  │
│  │  - LocalStore: 原子化配置持久化与会话树索引           │  │
│  │  - Scheduler: 跨时区 5 字段 Cron 调度器               │  │
│  │  - 单线程 Tokio 运行时驱动的高性能 SSE 流             │  │
│  └───────────────────────────────────────────────────────┘  │
└──────────────────────────────▲──────────────────────────────┘
                               │ 本地回环 IPC (动态端口)
┌──────────────────────────────▼──────────────────────────────┐
│            轻量伴生服务 (api-sidecar.mjs)                   │
│    - 111 个全量业务接口打包                                 │
│    - 父级退出看门狗自动清理，永不驻留僵尸进程               │
└─────────────────────────────────────────────────────────────┘
```

---

## 参与贡献

欢迎提交 Issue 与 Pull Request！参与贡献时请注意：
1. 所有代码、修改与补丁均须保持在 `pi-gpui/` 目录下，严禁污染外层目录。
2. 提交前确保所有测试全数通过：
   ```bash
   cargo test --offline
   ```
3. 严格遵循 `docs/ui-design-system.md` 中规范的互斥控件原则（选项 `<= 3` 使用 `TabBar`，`> 3` 使用 `Button::dropdown_menu`）、虚拟化列表规范与统一图标策略。

---

## 开源协议

本项目采用 [Apache License 2.0](LICENSE-APACHE) 开源许可。
