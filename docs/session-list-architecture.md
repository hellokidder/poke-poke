# Session 列表架构文档

> 记录当前实现的数据结构、流转逻辑与关键决策，供后续优化讨论使用。

---

## 1. 整体架构

```
外部 Agent (Claude Code / Codex / Cursor)
    │  hook event → stdin JSON
    ▼
poke-hook binary (src-tauri/src/bin/hook.rs)
    │  HTTP POST 127.0.0.1:9876/notify
    ▼
Axum HTTP Server (src-tauri/src/http_server.rs)
    │  handle_notify()
    ▼
SessionStore.upsert_session()  (src-tauri/src/sessions.rs)
    │  写入 ~/.pokepoke/sessions.json
    ▼
app.emit("sessions-updated", ())   ← Tauri 事件，空载荷
    ▼
React SessionPanel (src/panel/SessionPanel.tsx)
    │  listen("sessions-updated") → invoke("get_sessions") → 全量拉取
    ▼
重新渲染 Session 列表
```

并行地，`upsert_session` 返回后还会进行 Popup 决策：

```
UpsertResult { session, is_new, prev_status }
    ├─ should_close_popup? → popup::close_popup()
    └─ should_popup?       → popup::show_popup() + 播提示音
```

---

## 2. 核心数据结构

### Rust 侧（`sessions.rs`）

```rust
// Task C 起：四状态，都表示"agent 活着"的不同阶段，无终态语义
pub enum SessionStatus { Pending, Running, Idle, LastFailed }
pub enum Priority      { Normal, High }

pub struct Session {
    pub id:             String,           // 内部 UUID，前端跳转/Popup 引用
    pub task_id:        String,           // 业务主键，hook 传入，upsert 依据
    pub title:          String,
    pub message:        String,
    pub source:         Option<String>,   // "claude-code" / "cursor" / "codex"
    pub priority:       Priority,
    pub status:         SessionStatus,
    pub created_at:     DateTime<Utc>,
    pub updated_at:     DateTime<Utc>,
    pub terminal_tty:   Option<String>,   // e.g. /dev/ttys003，用于 pgrep 探活与聚焦检测
    pub workspace_path: Option<String>,
    pub failure_reason: Option<String>,   // 仅 LastFailed 状态下携带，CC StopFailure 的 reason code
}

pub struct UpsertResult {
    pub session:     Session,
    pub is_new:      bool,
    pub prev_status: Option<SessionStatus>,
}
```

> **serde 命名**：`#[serde(rename_all = "snake_case")]`，确保 `LastFailed` 正确序列化为 `"last_failed"`。兼容老数据：`#[serde(alias = "success")] Idle`、`#[serde(alias = "failure")] LastFailed`。

### TypeScript 侧（`src/types.ts`）

```typescript
interface Session {
    id:              string;
    task_id:         string;
    title:           string;
    message:         string;
    source:          string | null;
    priority:        "normal" | "high";
    status:          "pending" | "running" | "idle" | "last_failed";
    created_at:      string;   // ISO 8601
    updated_at:      string;
    terminal_tty:    string | null;
    workspace_path:  string | null;
    failure_reason:  string | null;   // last_failed 时承载错误类型，用于前端 i18n
}
```

---

## 3. upsert_session() 详解

`task_id` 是业务主键，每次 hook 事件以此查找现有 session：

**找到（更新路径）**：
- 保存旧状态为 `prev_status`
- 全量覆盖 `title`、`message`、`priority`、`status`、`updated_at`
- `source`、`terminal_tty`、`workspace_path` 仅在非 `None` 时更新（保留首次注册值）
- 落盘 → 返回 `UpsertResult { is_new: false, prev_status: Some(...) }`

**找不到（新建路径）**：
- 生成新 UUID 作为 `id`
- `insert(0, ...)` 插入列表头部
- 落盘 → 返回 `UpsertResult { is_new: true, prev_status: None }`

---

## 4. UpsertResult 的三个字段用途

| 字段 | 用途 |
|---|---|
| `session` | 操作后的最新快照，用于 Popup 决策和 HTTP 响应体 |
| `is_new` | 决定返回 201 / 200；跳过新建 session 的 popup 关闭判断 |
| `prev_status` | 状态转换方向判断，防止重复弹窗，决定开/关 Popup |

---

## 5. Popup 决策逻辑

### 关闭已有 Popup

```
条件：!is_new
   && 新状态 == Running
   && prev ∈ { Pending, Idle, LastFailed }
场景：用户开始新一轮对话（stage-ending → Running），上一轮遗留的 popup 已达成提醒目的，顺手收掉
```

### 弹出新 Popup

```
情形 A — stage-ending 转换：
  新状态 ∈ { Idle, LastFailed }
  && (prev 不存在 或 prev != 新状态)   ← 防止同状态重复 upsert 重复弹

情形 B — 进入 pending：
  新状态 == Pending
  && (prev 不存在 或 prev == Running)   ← 防止 pending→pending 重复弹

抑制条件：terminal_tty 非空
       && is_terminal_session_focused(tty) 返回 true
       → 用户已在对应终端，不打扰
```

> Task C 后不再有"进入终态"的概念。`idle` 和 `last_failed` 仍是"一轮工作结束"的提醒点，与用户体感一致。

---

## 6. 前端监听与渲染

```typescript
// SessionPanel.tsx
useEffect(() => {
    loadSessions();  // 初始全量拉取
    const unlisten = listen("sessions-updated", () => loadSessions());
    return () => unlisten.then(fn => fn());
}, []);

// 每次事件触发：全量拉取，不做 diff
const loadSessions = () =>
    invoke<Session[]>("get_sessions").then(setSessions);
```

**渲染规则**：
- 排序：按 `created_at` 升序（先注册的在上）
- 活跃判定：`status === "running" || status === "pending"`，仅表示"正在干活"
- 副标题"N 个活跃"按活跃判定计数，`idle` / `last_failed` 不计入（它们是"在线但空闲")
- 点击 session 项：`invoke("open_session_source", { id })` 跳转终端/编辑器

### 状态圆点（StatusDot）

列表每一项右侧显示一个彩色圆点，表示当前 session 状态：

| 状态 | 颜色 | 动画 | 含义 |
|---|---|---|---|
| `running` | 绿色 `#4ade80` | 呼吸脉冲（scale + opacity 循环，2s） | Agent 正在工作中 |
| `pending` | 黄色 `#facc15` | 呼吸脉冲（同上） | Agent 等待用户操作（权限确认、需要输入等） |
| `idle` | 淡蓝 `#60a5fa` | 无动画，静态实心圆 | 一轮结束，agent 空闲等下一轮（在线但静止） |
| `last_failed` | 红色 `#f87171` | 无动画，静态实心圆 | 上一轮因 API 错误结束，agent 仍活着 |

`running` 和 `pending` 共用同一套 CSS 动画（`dotPulse`）；`idle` / `last_failed` 圆点无动画。`idle` 选用淡蓝而非旧版灰白，避免被误读成"已归档"——Task C 起 session 本身代表活着的 agent，不需要整体弱化。

### 删除按钮（遗留行为）

- **显示条件**：`!isActive(session)`，即 `idle` / `last_failed` 状态的 session 才渲染删除按钮
- **触发方式**：鼠标悬停（hover）到该 session 行时按钮才从 `opacity: 0` 变为可见，点击后变红色高亮
- **行为**：`e.stopPropagation()` 阻止冒泡（避免触发行点击跳转），然后调用 `invoke("remove_session", { id })` 从后端删除并落盘，后端随即 emit `sessions-updated` 触发列表刷新
- **`running` / `pending` 状态下没有删除按钮**：活跃会话不允许手动删除
- Task C 后这个按钮的语义变轻——agent 活着时用户很少有理由手动删 session，主要用途是强制去掉一条探活尚未 reap 的残留（例如用户知道 agent 已经死、但 pgrep 暂时还判活）

---

## 7. 持久化与后台维护

**持久化**：每次 upsert / remove 后同步写入 `~/.pokepoke/sessions.json`（pretty JSON）。

**Task C 后的后台架构（单线程 + 启动 reap）**：

**启动时全量 reap（`lib.rs` setup 阶段同步执行一次）**：
- 遍历持久化加载出来的所有 session，跑一次 `is_session_alive`，立即移除宿主已死的 session
- 目的：消化开发/调试期遗留在 `sessions.json` 里的僵尸 session（例如升级前 24h TTL 遗留的老数据）
- 不发 popup 事件，仅执行 remove + 最终 emit 一次 `sessions-updated`

**高频探活线程（每 5s）**：
- 遍历**所有**状态的 session（不再按 status 过滤），跑 `is_session_alive`
- grace period：连续 2 次 miss 才执行 remove（`miss_count` 存于线程内存 `HashMap`，不落盘）
- remove 完整序列：`close_popup → remove_session → emit`（批量 emit，避免逐次重绘）

**`is_session_alive(session)` 分层策略**：

| source | 探活方式 | 判死条件 |
|---|---|---|
| `claude-code` | `pgrep -t <ttyname> claude` | 对应 TTY 上找不到 claude 进程 |
| `codex` | `pgrep -t <ttyname> codex` | 对应 TTY 上找不到 codex 进程 |
| `cursor` | `pgrep -x Cursor` | 整个 Cursor app 不在前后台 |
| 其他 / CLI agent 缺 TTY | 直接判死 | 宁可误清也不留僵尸（Task C 决策 4）|

> 24h TTL 清理线程已彻底删除。session 回收路径只剩"探活 miss 2 次"这一条。

---

## 8. 状态映射（Hook 事件 → SessionStatus）

| HTTP 请求 `status` 字段 | 映射为 | 说明 |
|---|---|---|
| `"running"` | `Running` | agent 接到新一轮 |
| `"idle"` | `Idle` | 一轮正常结束（CC Stop / Codex Stop / Cursor stop） |
| `"success"` | `Idle` | 老版 hook 兼容（Task C 前的写法） |
| `"last_failed"` | `LastFailed` | CC StopFailure，附带 `failure_reason` |
| `"failure"` / `"failed"` | `LastFailed` | 老版 hook 兼容 |
| 其他 / 缺失 | `Pending` | CC Notification 等待操作 |

---

## 9. 关键文件索引

| 文件 | 职责 |
|---|---|
| `src-tauri/src/http_server.rs` | Axum 服务、`/notify` 入口、status 映射、Popup 决策 |
| `src-tauri/src/sessions.rs` | Session 数据结构、`SessionStatus` 枚举、`SessionStore`、upsert / remove |
| `src-tauri/src/commands.rs` | Tauri IPC commands（get_sessions、remove_session、open_session_source 等） |
| `src-tauri/src/popup.rs` | Popup 窗口管理、堆叠位置、动画、终端聚焦检测 |
| `src-tauri/src/lib.rs` | 应用初始化、启动 reap、高频探活线程、`is_session_alive()` 分层探活、`remove_session_with_cleanup()` |
| `src-tauri/src/bin/hook.rs` | poke-hook 二进制；Stop → `"idle"`、StopFailure → `"last_failed"` |
| `src/panel/SessionPanel.tsx` | 列表渲染、事件监听、全量拉取、StatusDot 配色 |
| `src/popup/PopupWindow.tsx` | popup 卡片；按 status 与 failure_reason 渲染文案 |
| `src/types.ts` | 前端 Session 类型定义 |
| `src/i18n/strings.ts` | `status.idle` / `status.last_failed` / `failure.*` 文案 |

---

## 10. 已知约束与现状

- **前端全量拉取**：每次 `sessions-updated` 都拉完整列表，无增量 patch。当前 session 数量小，无明显问题。
- **新建插 index 0**：`insert(0, ...)` 使最新 session 在列表头，前端再按 `created_at` 升序排——两者方向相反，前端排序是最终顺序的唯一依据。
- **`source` / `tty` / `workspace_path` 仅在后续事件携带非 `None` 值时覆盖**：首次写入后如果后续事件对应字段为 `None` 则保留原值；若字段非 `None`，则覆盖。
- **`failure_reason` 只在 `last_failed` 状态下有意义**：`upsert_session` 写入前会校验，其它状态上收到的 `failure_reason` 会被忽略。
- **全局开关未实现**：产品规格要求的左下角 Switch（禁用后不接收 hook、不弹窗）尚未开发。
- **Cursor 跳转粒度**：目前 `open_session_source` 对 Cursor 只能打开 workspace，无法定位到具体会话窗口。
- **Cursor 探活粒度**：`pgrep -x Cursor` 只能判断 Cursor app 是否整体存活，无法区分多 workspace。用户关了某个 workspace 但整个 Cursor app 还在时，对应 session 会被保留，直到用户退出整个 Cursor。
- **Cursor popup 抑制**：无 TTY 的 session `already_focused` 永远为 false，Cursor session 的 popup 不受终端聚焦检测影响。后续可选补 frontmost app 级粗粒度抑制。
- **识别不出 `source` 的老 session**：启动 reap 时会被判死直接清掉（Task C 决策 4）。

---

> **讨论记录已收敛**：三方（CC / Codex / OpenCode）多轮讨论的完整过程已归档，最终执行方案见 `docs/session-lifecycle-refactor.md`。P0 已实现并通过评审。
