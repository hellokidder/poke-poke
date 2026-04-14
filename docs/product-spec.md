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
│              (常驻，显示未读数角标)                      │
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
| **会话状态** | `pending` / `running` / `success`，状态点动画指示 |
| **项目信息** | workspace 路径（截取项目名）|
| **时间** | 最后活动时间（相对时间：刚刚 / 3分钟前 / 1小时前） |

### Session 注册时机

Session 在以下时机被注册（创建/更新）到列表：

| 事件 | 触发源 | 行为 |
|------|--------|------|
| Agent Hook Start（SessionStart） | CC / Codex / Cursor hook | 创建新 Session，状态 → `running` |
| 用户发送消息（UserPromptSubmit） | CC / Codex / Cursor hook | 更新已有 Session，状态 → `running` |
| Agent 停止等待用户输入（Stop/Notification） | CC / Codex hook | 更新状态 → `pending`（需要用户操作） |
| Agent 任务完成（Stop: success） | CC / Codex / Cursor hook | 更新状态 → `success` |

### 状态说明

| 状态 | 含义 | 视觉表现 |
|------|------|----------|
| `pending` | Agent 停下来等用户操作（权限确认、需要输入等） | 黄色闪烁圆点，**这是需要提醒用户的核心状态** |
| `running` | Agent 正在工作中 | 绿色呼吸动画圆点 |
| `success` | 会话已完成 | 灰色实心圆点 |

### 交互行为

- **点击 Session 项** → 跳转到对应的终端/编辑器窗口（focus terminal by tty / open Cursor workspace）
- **右下角** → Settings 齿轮图标，点击打开设置面板（模块 2）
- **左下角** → 全局开关 Switch，控制 Poke Poke 的启用/禁用（禁用后不再接收 hook 事件、不弹出提示）

### Session 生命周期

- **注册** → Agent hook 事件到达时创建
- **移除** → 用户关闭对应 Agent Session（SessionEnd / Stop 终止）时，自动从列表移除
- **兜底清理** → 保留时间到期后自动清理残留 Session（防止异常退出未发送关闭事件的情况）

### 排序规则

按注册时间先后排列（先注册的在上）

---

## 模块 2：设置面板（Settings）

### 入口

会话列表右下角齿轮图标。

### 设置项

| 设置项 | 说明 | 选项 |
|--------|------|------|
| **提示音** | pending 状态触发时播放的系统提示音 | 系统声音列表 / 静音 |
| **会话保留（兜底）** | 异常残留 Session 的最大保留时间（正常关闭会立即移除） | 1小时 / 24小时 / 7天 / 永不清理 |
| **语言** | 界面语言 | 中文 / English |
| **开机自启** | 系统登录时自动启动 Poke Poke | 开 / 关 |
| **全局快捷键** | 打开会话列表的键盘快捷键 | 自定义组合键 |

### 交互

- 所有设置即时保存，无需确认按钮
- ESC 或点击外部区域关闭

---

## 模块 3：提示面板（Notification Popup）

### 触发条件

当 Session 状态变为 `pending`（Agent 需要用户介入）时，自动弹出提示面板。

> 这是产品的核心提示能力：Agent 停下来等你了，Poke Poke 戳你一下。

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
- **智能消失** → 如果用户已经在对应终端窗口操作了（Session 状态回到 `running`），自动关闭该 Popup
- **多个 Popup** → 从屏幕右下角向上堆叠，移除时有滑动动画

### 形象关联

Popup 中的像素形象与模块 1 会话列表中的形象保持一致，用户可以通过形象颜色/形态快速识别是哪个会话在呼叫自己，无需阅读文字。

---

## 接入端支持

| 接入端 | 接入方式 | 支持事件 | 配置位置 |
|--------|----------|----------|----------|
| **Claude Code** | 一键接入（tray 菜单） | SessionStart, UserPromptSubmit, Notification, Stop | `~/.claude/settings.json` |
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
    ├── Notification Store (~/.pokepoke/notifications.json)
    ├── Settings Store (~/.pokepoke/settings.json)
    ├── Tray Icon (菜单栏常驻)
    ├── Panel Window (会话列表)
    ├── Popup Window (提示面板)
    └── Settings Window (设置面板)
```

---

## 与当前代码的差异说明

当前代码实现中存在一些与上述产品定义不一致的地方：

### 1. 状态模型偏差

| 产品定义 | 当前代码 | 差异 |
|----------|----------|------|
| 3 种状态：`pending` / `running` / `success` | 4 种状态：`Pending` / `Running` / `Success` / `Failed` | `Failed` 不在产品设计中，应合并到 `success`（会话结束）或用其他方式处理 |

### 2. Popup 触发逻辑偏差

| 产品定义 | 当前代码 |
|----------|----------|
| `pending` 状态时弹出（Agent 等待用户） | `Success` / `Failed` 终态 **或** `Running → Pending` 时弹出 |

产品核心场景是 **Agent 等你操作时提醒你**，不是任务完成后通知你。当前代码的 Popup 触发偏向"通知"而非"提示"。

### 3. 全局开关缺失

产品要求左下角有一个 Switch 控制整体启用/禁用，当前代码未实现。

### 4. 会话列表排序

产品要求 `pending` 置顶优先展示，当前代码按 `created_at` 倒序排列，未做状态优先级排序。

### 5. 术语统一

当前代码中混用 `Task` / `Notification` / `Session` 等概念，产品层面应统一为 **Session（会话）**。

---

## 产品语言

- 产品名：**Poke Poke**
- 核心概念：Session（会话）
- 核心动作：Poke（戳一下）— Agent 需要你时，Poke 你一下
- Mascot：每个会话有独特的像素形象伴侣
