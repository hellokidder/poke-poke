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
pub enum SessionStatus { Pending, Running, Success }
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
    pub terminal_tty:   Option<String>,   // e.g. /dev/ttys003，用于终端聚焦检测
    pub workspace_path: Option<String>,
}

pub struct UpsertResult {
    pub session:     Session,
    pub is_new:      bool,
    pub prev_status: Option<SessionStatus>,
}
```

### TypeScript 侧（`src/types.ts`）

```typescript
interface Session {
    id:             string;
    task_id:        string;
    title:          string;
    message:        string;
    source:         string | null;
    priority:       "normal" | "high";
    status:         "pending" | "running" | "success";
    created_at:     string;   // ISO 8601
    updated_at:     string;
    terminal_tty:   string | null;
    workspace_path: string | null;
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
   && prev 是 Pending 或 Success（terminal）
场景：用户回到终端操作了（permission 已确认 / 开始新对话），主动消除提醒
```

### 弹出新 Popup

```
情形 A — 进入终态：
  新状态 == Success
  && prev 存在且 prev != Success     ← 防止 success→success 重复弹

情形 B — 进入 pending：
  新状态 == Pending
  && prev 是 Running 或 None         ← 防止 pending→pending 重复弹

抑制条件：terminal_tty 非空
       && is_terminal_session_focused(tty) 返回 true
       → 用户已在对应终端，不打扰
```

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
- 活跃判定：`status === "running" || status === "pending"`
- 非活跃（`success`）的 session 整体 `opacity: 0.45`，视觉弱化
- 点击 session 项：`invoke("open_session_source", { id })` 跳转终端/编辑器

### 状态圆点（StatusDot）

列表每一项右侧显示一个彩色圆点，表示当前 session 状态：

| 状态 | 颜色 | 动画 | 含义 |
|---|---|---|---|
| `running` | 绿色 `#4ade80` | 呼吸脉冲（scale + opacity 循环，2s） | Agent 正在工作中 |
| `pending` | 黄色 `#facc15` | 呼吸脉冲（同上） | Agent 等待用户操作（权限确认、需要输入等） |
| `success` | 半透明白 `rgba(255,255,255,0.25)` | 无动画，静态实心圆 | 会话已完成 |

`running` 和 `pending` 共用同一套 CSS 动画（`dotPulse`）；`success` 圆点无动画，配合整体 0.45 透明度共同实现弱化效果。

### 删除按钮

- **显示条件**：`!isActive(session)`，即仅 `success` 状态的 session 才渲染删除按钮
- **触发方式**：鼠标悬停（hover）到该 session 行时按钮才从 `opacity: 0` 变为可见，点击后变红色高亮
- **行为**：`e.stopPropagation()` 阻止冒泡（避免触发行点击跳转），然后调用 `invoke("remove_session", { id })` 从后端删除并落盘，后端随即 emit `sessions-updated` 触发列表刷新
- **`running` / `pending` 状态下没有删除按钮**：活跃会话不允许手动删除，只能等其自然进入 `success` 或被后台探活巡检回收

---

## 7. 持久化与后台维护

**持久化**：每次 upsert / remove 后同步写入 `~/.pokepoke/sessions.json`（pretty JSON）。

**后台双线程架构**（P0 重构后）：

**高频探活线程（每 5s）**：
- 遍历所有 `Running` / `Pending` 状态的 session，检查宿主是否存活
- 终端类 session：`stat(terminal_tty)` 检查 TTY 设备文件是否存在
- 无 TTY 的 session（Cursor 等）：P0 阶段视为存活，P1 补进程级探活
- grace period：连续 2 次探活失败才执行 remove（`miss_count` 存于线程内存 `HashMap`，不落盘）
- remove 完整序列：`close_popup → remove_session → emit`（批量 emit，避免逐次重绘）

**低频 TTL 清理线程（每 1 小时）**：
- 仅清理 `Success` 状态且 `updated_at` 超过 24 小时的 session
- `Running` / `Pending` 状态永不被 TTL 清理
- TTL 固定为 24h，不可由用户配置

两个线程清理后都 emit `sessions-updated`。

---

## 8. 状态映射（Hook 事件 → SessionStatus）

| HTTP 请求 `status` 字段 | 映射为 |
|---|---|
| `"running"` | `Running` |
| `"success"` | `Success` |
| `"failed"` | `Success`（合并，无 failed 态） |
| 其他 / 缺失 | `Pending` |

---

## 9. 关键文件索引

| 文件 | 职责 |
|---|---|
| `src-tauri/src/http_server.rs` | Axum 服务、`/notify` 入口、Popup 决策 |
| `src-tauri/src/sessions.rs` | Session 数据结构、`SessionStore`、upsert / remove / cleanup_expired |
| `src-tauri/src/commands.rs` | Tauri IPC commands（get_sessions、remove_session、open_session_source 等） |
| `src-tauri/src/popup.rs` | Popup 窗口管理、堆叠位置、动画、终端聚焦检测 |
| `src-tauri/src/lib.rs` | 应用初始化、高频探活线程、低频 TTL 清理线程、`remove_session_with_cleanup()` |
| `src/panel/SessionPanel.tsx` | 列表渲染、事件监听、全量拉取 |
| `src/types.ts` | 前端 Session 类型定义 |

---

## 10. 已知约束与现状

- **前端全量拉取**：每次 `sessions-updated` 都拉完整列表，无增量 patch。当前 session 数量小，无明显问题。
- **新建插 index 0**：`insert(0, ...)` 使最新 session 在列表头，前端再按 `created_at` 升序排——两者方向相反，前端排序是最终顺序的唯一依据。
- **`source` / `tty` / `workspace_path` 仅在后续事件携带非 `None` 值时覆盖**：首次写入后如果后续事件对应字段为 `None` 则保留原值；若字段非 `None`，则覆盖。
- **无 `failed` 状态**：`failed` 在入口处统一映射为 `success`，列表不区分完成方式。
- **全局开关未实现**：产品规格要求的左下角 Switch（禁用后不接收 hook、不弹窗）尚未开发。
- **Cursor 跳转粒度**：目前 `open_session_source` 对 Cursor 只能打开 workspace，无法定位到具体会话窗口。
- **Cursor 探活粒度**：P0 阶段无 TTY 的 session（含 Cursor）视为存活，不会被巡检清理。P1 计划补进程级探活（`pgrep Cursor`），但多 workspace 场景下精度有限。
- **Cursor popup 抑制**：P0 阶段无 TTY 的 session `already_focused` 永远为 false。P1 计划补 frontmost app 级粗粒度抑制。

---

> **讨论记录已收敛**：三方（CC / Codex / OpenCode）多轮讨论的完整过程已归档，最终执行方案见 `docs/session-lifecycle-refactor.md`。P0 已实现并通过评审。
