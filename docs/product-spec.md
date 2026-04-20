# Poke Poke 产品说明文档

## 产品定位

**Agent 对话强提示工具** — 常驻菜单栏的桌面伴侣，帮助用户感知和管理所有正在进行的 AI Agent 会话，在关键时刻通过视觉/声音提示拉回用户注意力。

核心价值：开发者同时开 3-5 个 Agent 会话是常态，Poke Poke 让你不会漏掉任何一个需要你操作的 Agent。

---

## 产品架构

产品由 **3 个模块** 构成：

```
┌─────────────────────────────────────────────────────┐
│                    菜单栏图标                          │
│                   (常驻)                              │
│                      │                               │
│          点击展开 ┌───┴───┐                            │
│                  │       │                            │
│          ┌──────┴──┐  ┌──┴──────┐                    │
│          │ 模块 1   │  │ 模块 3   │                    │
│          │ 会话列表  │  │ 提示面板  │                    │
│          │ (Panel)  │  │ (Popup)  │                   │
│          └────┬─────┘  └─────────┘                   │
│               │                                      │
│          setting 入口                                 │
│               │                                      │
│          ┌────┴─────┐                                │
│          │ 模块 2    │                                │
│          │ 设置面板   │                                │
│          │(Settings) │                                │
│          └──────────┘                                │
└─────────────────────────────────────────────────────┘
```

---

## 模块 1：会话列表（Session Panel）

### 入口

点击菜单栏 Poke Poke 图标，弹出下拉面板。

### 功能描述

展示当前所有已接入的 Agent Session 列表，是用户的**主控台**。

### 列表项结构

每一个 Session 项包含：

| 字段 | 说明 |
|------|------|
| **像素形象** | 每个 Session 对应一个独特的像素风格 mascot 形象（基于 session_id hash 生成颜色/形态变体） |
| **接入端标识** | 来源：`Claude Code` / `Cursor` / `Codex`，配合对应 icon |
| **会话状态** | `running` / `pending` / `idle` / `last_failed`，状态点动画指示 |
| **项目信息** | workspace 路径（截取项目名）|
| **时间** | 最后活动时间（相对时间：刚刚 / 3分钟前 / 1小时前） |

### Session 注册时机

Session 在以下时机被注册（创建/更新）到列表：

| 事件 | 触发源 | 行为 |
|------|--------|------|
| Agent Hook Start（SessionStart） | CC / Codex / Cursor hook | 创建新 Session，状态 → `running` |
| 用户发送消息（UserPromptSubmit） | CC / Codex / Cursor hook | 更新已有 Session，状态 → `running` |
| Agent 停下等用户输入（Notification） | CC hook | 更新状态 → `pending`（需要用户操作） |
| Agent 一轮任务完成（Stop） | CC / Codex / Cursor hook | 更新状态 → `idle`（agent 仍活着，等下一轮） |
| Agent 一轮任务因 API 错误结束（StopFailure） | CC hook | 更新状态 → `last_failed`（agent 仍活着） |

### 状态说明

**核心语义**：Session 代表**一个活着的 Agent 实例**，不是单轮通知。四种状态都描述 agent 活着时的不同阶段，没有"终态"概念——session 的消失由宿主（终端/IDE 进程）存活判定，不由状态决定。

| 状态 | 含义 | 视觉表现 |
|------|------|----------|
| `running` | Agent 正在处理某一轮 | 绿色呼吸动画圆点 |
| `pending` | Agent 停下来等用户操作（权限确认、需要输入等） | 黄色闪烁圆点，**需要提醒用户的核心状态** |
| `idle` | 一轮正常结束，agent 空闲等下一轮 | 淡蓝 `#60a5fa` 静态圆点，传达"在线但空闲" |
| `last_failed` | 上一轮因 API 错误结束，agent 仍活着 | 红色静态圆点；popup 按 `failure_reason` 本地化展示错误类型 |

### 交互行为

- **点击 Session 项** → 跳转到对应的终端/编辑器窗口（focus terminal by tty / open Cursor workspace）
- **右下角** → Settings 齿轮图标，点击打开设置面板（模块 2）
- ~~**左下角** → 全局开关 Switch，控制 Poke Poke 的启用/禁用（禁用后不再接收 hook 事件、不弹出提示）~~ — **未实现**（产品规划项，详见下文 §"与当前代码的差异说明 §2 全局开关缺失" 和 §"后续 TODO 第 6 条"）

### Session 生命周期

- **注册** → Agent hook 事件到达时创建
- **状态流转** → `running` / `pending` / `idle` / `last_failed` 之间根据 hook 事件自由切换，任何一种都不是终态
- **移除**（唯一路径）→ 后台探活线程每 5 秒按 `source` 分层检查宿主是否存活，连续 2 次探活失败（grace period ≈ 10 秒）即从列表中移除并关闭关联 popup
  - `claude-code` / `codex`：`pgrep -t <tty> <agent_name>` 对应 TTY 上是否有 agent 进程
  - `cursor`：`pgrep -x Cursor` 整个 Cursor app 进程是否存在（workspace 级别无法精确探活）
  - 识别不出 `source` 或 CLI agent 缺 TTY：直接判死（宁可误清也不留僵尸）
- **启动清理** → 应用启动时跑一次性全量探活，立即 reap 宿主已死的老 session（用于消化调试期残留）

> ⚠️ 不再有 24 小时 TTL 兜底清理。产品语义上 `idle` / `last_failed` 都是"agent 还活着"，不能以时间为由下架。

### 排序规则

按注册时间先后排列（先注册的在上）

---

## 模块 2：设置面板（Settings）

### 入口

会话列表右下角齿轮图标。

### 设置项

| 设置项 | 说明 | 选项 |
|--------|------|------|
| **提示音** | `pending` / `idle` / `last_failed` 三种 stage-ending 场景触发 popup 时播放的系统提示音 | 系统声音列表 / 静音 |
| ~~**会话保留（兜底）**~~ | ~~24h TTL 兜底清理~~ | ~~Task C 已彻底移除；session 回收改由宿主探活驱动~~ |
| **语言** | 界面语言 | 中文 / English |
| **开机自启** | 系统登录时自动启动 Poke Poke | 开 / 关 |
| **全局快捷键** | 打开设置面板的键盘快捷键 | 自定义组合键 |

### 交互

- 所有设置即时保存，无需确认按钮
- ESC 或点击外部区域关闭

---

## 模块 3：提示面板（Notification Popup）

### 触发条件

当 Session 发生"阶段结束"事件时，自动弹出提示面板：

| 状态变化 | 场景 |
|----------|------|
| `running → pending` 或新建 session 直接进入 `pending` | Agent 停下来等用户操作（权限确认、需要输入等） |
| 进入 `idle` 且 prev ≠ `idle` | Agent 一轮任务正常完成 |
| 进入 `last_failed` 且 prev ≠ `last_failed` | Agent 一轮任务因 API 错误结束 |

> 核心提示能力：无论是 Agent 等你操作、任务跑完了，还是撞到了 API 限流/错误，Poke Poke 都会戳你一下。
>
> **抑制条件**：如果用户当前已聚焦在该 Session 对应的终端窗口，则不弹出（避免打扰正在操作的用户）。
>
> **popup 自动关闭**：当 session 从 `pending` / `idle` / `last_failed` 切回 `running` 时（即用户发起了新一轮对话），关联 popup 自动关闭。

### 面板内容

| 元素 | 说明 |
|------|------|
| **像素形象** | 与会话列表中对应 Session 的 mascot 形象一致（用户通过形象快速识别是哪个会话） |
| **提示标题** | 例："Claude Code 需要你的确认" / "任务已完成" |
| **提示消息** | Hook 事件携带的具体信息 |
| **来源标识** | Claude Code / Cursor / Codex icon |
| **时间** | 触发时间 |

### 交互行为

- **点击 Popup** → 自动跳转到对应 Session 的终端/编辑器窗口（与会话列表点击行为一致），Popup 关闭
- **常驻不消失** → Popup 不会自动超时消失，持续驻留屏幕直到用户主动点击或关闭 Poke Poke
- **智能消失** → 如果用户发起了新一轮对话（Session 状态从 `pending` / `idle` / `last_failed` 切回 `running`），自动关闭该 Popup
- **多个 Popup** → 从屏幕右上角向下堆叠（菜单栏下方），移除时有滑动动画

### 形象关联

Popup 中的像素形象与模块 1 会话列表中的形象保持一致，用户可以通过形象颜色/形态快速识别是哪个会话在呼叫自己，无需阅读文字。

---

## 接入端支持

| 接入端 | 接入方式 | 支持事件 | 配置位置 |
|--------|----------|----------|----------|
| **Claude Code** | 一键接入（tray 菜单） | SessionStart, UserPromptSubmit, Notification, Stop, StopFailure | `~/.claude/settings.json` |
| **Codex CLI** | 一键接入（tray 菜单） | SessionStart, UserPromptSubmit, Stop | `~/.codex/config.toml` |
| **Cursor** | 按项目接入 | sessionStart, beforeSubmitPrompt, stop, sessionEnd | `.cursor/hooks.json`（项目级） |

---

## 技术架构简述

```
Agent (CC/Codex/Cursor)
    │ hook event (stdin JSON)
    ▼
poke-hook (Rust binary)
    │ HTTP POST /notify
    ▼
Poke Poke App (Tauri)
    ├── HTTP Server (Axum, port 9876)
    │     └── Upsert Session → Store
    ├── Session Store (~/.pokepoke/sessions.json)
    ├── Settings Store (~/.pokepoke/settings.json)
    ├── Tray Icon (菜单栏常驻)
    ├── Panel Window (会话列表)
    ├── Popup Window (提示面板)
    └── Settings Window (设置面板)
```

---

## 与当前代码的差异说明

当前代码实现中存在一些与上述产品定义不一致的地方：

### ~~1. 状态模型偏差~~ ✅ 已修复（Task C）

状态模型经历两次演进：
- **Task A/B**：在 `success` 之外新增 `failure` 终态，承载 CC `StopFailure` 的 API 错误语义
- **Task C**：把 `success` / `failure` 重命名为 `idle` / `last_failed`，并剥离"终态"语义——两者都只是"agent 活着、上一轮已结束"的状态。session 回收改由宿主探活驱动，24h TTL 彻底移除。

### 2. 全局开关缺失

产品要求左下角有一个 Switch 控制整体启用/禁用，当前代码未实现。

### ~~3. 会话列表排序~~ ✅ 已符合

代码已按 `created_at` 升序排列（先注册的在上），与产品定义一致。

### ~~4. 术语统一~~ ✅ 已完成

代码已统一使用 `Session`/`SessionStatus`/`SessionStore`。文件 `notifications.rs` → `sessions.rs`，`NotificationPanel` → `SessionPanel`，数据文件 `notifications.json` → `sessions.json`（含自动迁移）。外部 API 字段 `task_id` 保持不变（hook 合约）。

### ~~5. 已读 / 未读逻辑偏差~~ ✅ 已移除

`read` 字段、`unread_count`、`mark_read`、`mark_all_read`、托盘未读 tooltip、相关 HTTP 路由和 Tauri 命令均已移除。

## 后续 TODO

1. ~~移除 `failed` 状态，统一合并到 `success`；同步调整 Cursor `stop(aborted)`、stale session 检测与前端展示逻辑。~~ ✅ （已被 Task A/B/C 取代，最终状态机为四态 `running` / `pending` / `idle` / `last_failed`）
2. ~~会话列表保持按注册时间先后排列（先注册的在上）；若实现偏离，需要及时修正代码。~~ ✅
3. ~~提示面板不保留 timeout 相关产品设定，后续移除对应设置项与实现代码。~~ ✅
4. ~~提示音在 `pending` 和 `success` 两种提示场景下都需要播放。~~ ✅ 已确认：sound 在 `should_popup` 为 true 时播放，覆盖 `* → idle` / `* → last_failed` / `running → pending` 三种 stage-ending 场景（Task C 后）。
5. ~~继续确认 Codex 是否存在可用于 `pending` 的 hook / notify 能力；如果有，需要补齐接入。~~ ✅ 已确认：Codex CLI 仅支持 `SessionStart`、`UserPromptSubmit`、`Stop` 三种事件，不具备 `Notification` 事件，无法触发 `pending` 状态。此为 Codex 平台限制，非 Poke Poke 可解决。
6. 增加全局开关：禁用后不弹窗、不播音；该能力当前尚未实现。
7. Cursor 的跳转/聚焦能力需要从 workspace 级提升到会话级。
8. ~~移除已读 / 未读相关实现，包括 `read` 字段、`unread_count`、`mark_read`、`mark_all_read`、相关 HTTP / Tauri 命令，以及托盘未读 tooltip 更新逻辑。~~ ✅

---

## 产品语言

- 产品名：**Poke Poke**
- 核心概念：Session（会话）
- 核心动作：Poke（戳一下）— Agent 需要你时，Poke 你一下
- Mascot：每个会话有独特的像素形象伴侣
