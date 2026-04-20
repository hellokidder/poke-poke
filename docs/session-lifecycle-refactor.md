# Session 生命周期重构方案

> 基于 CC / Codex / OpenCode 三方讨论汇总的最终执行方案。讨论已收敛并归档，结论反映在本文档和 `session-list-architecture.md` 正文中。

> ⚠️ **Task C 覆盖通知**（2026-04-17）：本文档的 P0 和 P1-B B3 已被 `docs/tasks/taskC-session-lifecycle.md` **整体重构**。核心变化：
> - `SessionStatus` 重命名为 `Running` / `Pending` / `Idle` / `LastFailed`，**不再有终态概念**——Idle/LastFailed 只是"agent 活着、上一轮已结束"。
> - 删除 24h TTL 兜底清理（`cleanup_expired` / `is_terminal`），session 唯一回收路径是宿主探活失败。
> - 探活线程覆盖所有 session 状态（不再按状态过滤）。
> - 探活策略严格按 source 分层：CLI agent 用 `pgrep -t`，Cursor 用 `pgrep -x Cursor`，识别不出 source 的直接判死（"宁可误清也不留僵尸"）。
> - 启动时做一次性全量探活 reap，清理老数据里宿主已死的 session。
>
> 下文中凡是标注 ✅ 已完成、但涉及"Success 终态" / "TTL" / "cleanup_expired" / "is_terminal"的实现细节，都应以 Task C 为准。保留原文作为决策演进记录。

---

## 一、当前问题

### 1.1 生命周期语义错误

当前 `reap_stale_sessions()` 把无主 session 标为 `Success` + `"Session lost"` 并等待 TTL 清理，而产品规格要求"宿主消失 → 立即从列表移除"。现有路径：

```
宿主关闭
  → 5 分钟后巡检发现 TTY 消失
  → 标为 Success（伪终态）
  → 最多再等 24 小时 TTL 清理
```

目标路径：

```
宿主关闭
  → 10 秒内巡检发现
  → 直接 remove（含关联 popup）
```

### 1.2 巡检频率与职责混杂

后台线程每 5 分钟跑一次，将两个完全不同职责混在一起：

- 宿主存活探测（即将改为 lib.rs 内联探活循环）：应该高频
- `cleanup_expired()`：TTL 兜底清理，本就低频

TTY 文件存在性检查是纯 `stat()` syscall，成本极低，没有理由和 TTL 清理共享同一频率。

### 1.3 Cursor session 是系统盲区

`terminal_tty` 对 Cursor（GUI IDE）永远为 null，导致三处失效：

| 机制 | 对终端 Agent | 对 Cursor |
|---|---|---|
| reap（TTY stat） | 有效 | 永远跳过，session 永远不被 reap |
| popup 抑制 | 有效 | `already_focused` 永远为 false，正在 IDE 里操作也会弹 popup |
| 焦点检测（AppleScript） | iTerm2 / Terminal.app | 无任何支持 |

### 1.4 非 iTerm2 终端的 popup 抑制失效

`is_terminal_session_focused()` 硬编码了 iTerm2 和 Terminal.app 的 AppleScript 接口。Warp、Ghostty 等终端的用户在操作时仍会被弹 popup。

### 1.5 Pending session 被 reap 遗漏

`reap_stale_sessions()` 只巡检 `Running` 状态。用户关掉终端后处于 `Pending` 的 session（如 Claude Code 等权限确认时用户直接关窗口）会永远卡在 Pending，只靠 24h TTL 兜底。这是一个明确的 bug。

### 1.6 remove 后 popup 孤儿

当前 `remove_session`（Tauri command）会先 close 关联 popup，但后台巡检线程直接操作 store，**绕过了这一步**。一旦 reap 改为直接 remove，用户屏幕上会出现引用已删 session 的幽灵 popup，点击跳转静默失败。

---

## 二、核心决策

### 决策 1：生命周期事件语义

> ⚠️ Task C 后此表已过期：`Success` → `Idle`；TTL 清理整条路径已移除。当前实际语义见 `docs/session-list-architecture.md` §7-§8。

| 事件 | 语义 | 对 session 的操作 |
|---|---|---|
| `Stop` hook | 这一轮任务完成/中止 | → `Success`，保留在列表供用户确认 |
| `sessionEnd` hook（P1） | 上游明确宣告会话结束 | → 直接 remove |
| reap（巡检发现宿主消失） | 上游没给信号但宿主已消亡 | → 直接 remove |
| TTL 清理 | 24h 超时兜底 | → 直接 remove |

`Stop` 和 `sessionEnd` 是不同语义：Stop 后用户可能开启下一轮对话，session 应保留展示；sessionEnd 才是会话彻底结束。当前 Claude Code 没有单独的 `sessionEnd` 事件，`Stop` 既承担"任务完成"也承担"会话结束"，这个映射应在 hook binary 中按 source 处理，不泄漏到 session store。

### 决策 2：巡检架构拆分

> ⚠️ Task C 后已简化：低频清理线程整条删除，高频探活覆盖**所有状态**（不再按 status 过滤）。当前实际架构见 `docs/session-list-architecture.md` §7"Task C 后的后台架构"。

```
高频巡检线程（每 5s）：
  lib.rs 内联探活循环（get_all() 过滤 + is_alive() + miss_counts）
  覆盖 Running + Pending 状态
  结果：直接 remove（含 close popup），见 Task 3 / Task 4

低频清理线程（每小时）：
  仅运行 cleanup_expired(24h)
  覆盖 Success 状态的 TTL 超时 session
```

### 决策 3：grace period 实现方式

连续 2 次探活失败才执行 remove，**miss_count 存于巡检线程内存 `HashMap`，不进入 `Session` 持久化模型**。

理由：
- 运行时巡检状态不属于业务状态，不应落盘
- 应用重启后计数重置是合理行为
- 避免 `sessions.json` 成为"业务数据 + 线程临时计数器"的混合体

```
巡检线程维护：HashMap<session_id, miss_count>
探活成功 → 删除条目（清零）
探活失败 → miss_count += 1
miss_count >= 2 → remove session + close popup + delete 条目
```

### 决策 4：模块边界

`sessions.rs` 不反向依赖 `popup` 模块。remove 的完整动作序列（close popup → remove from store → emit）由巡检线程在 `lib.rs` 或其调度层统一完成。

```
lib.rs（巡检线程，Task 3 探活循环内，当 miss_count >= 2 时触发）：
  // miss_counts 在线程内维护，达到阈值后执行以下序列：
  // 1. lock 已在查询 get_all() 时释放，这里直接执行写操作
  popup::close_popup(&app, &session_id, &popup_list);  // 现有接口，popup.rs:97
  store.lock().unwrap().remove_session(&session_id);
  // emit 收集到循环末尾批量发一次（避免每次 remove 都触发前端重绘）
```

`sessions.rs` 中没有任何探活相关的函数调用，lib.rs 直接使用 `store.get_all()` 过滤出活跃 session 并内联判断存活。

### 决策 5：Cursor 能力边界（当前阶段）

当前阶段 Cursor 支持的精确能力边界（写入文档约束，不宣传不存在的精确能力）：

| 能力 | 实现方式 | 粒度 |
|---|---|---|
| 跳转 | `cursor` CLI 打开 workspace | workspace 级 |
| 探活 | 检查 Cursor 进程是否存在 | 进程级（多 workspace 时有精度损失） |
| popup 抑制 | frontmost app == Cursor 即抑制 | app 级（粗粒度，可接受） |

已知局限：用户打开多个 Cursor workspace 时，其中一个关闭但进程仍在，当前探活无法检测。等 Cursor 提供更好的 hook API 时再解决。

### 决策 6：TTL 用户配置移除

"会话保留时长"从用户设置面板移除，系统写死 24h。用户几乎不会关心一个已完成的 session 在列表里多待了几小时，此设置项是内部实现泄漏，减少配置面板噪音。

### 决策 7：协议升级推至 P1

`event_type` 区分和 `external_session_id` 字段涉及 hook binary 和三端（CC/Codex/Cursor）hook 配置的同步变更，不是纯后端改动，需要单独排期协调。P0 只做对外无感知的纯后端改动。

---

## 三、P0 实现清单 ✅ 已全部完成

> 纯后端改动，不涉及 hook 协议变更，不影响三端接入配置。
> **状态**：6 个 Task 已全部实现并通过代码评审（2026-04-17）。

### Task 1：巡检线程拆分 + 提频 ✅（**Task C 已二次简化**：低频线程整条删除，仅保留高频探活）

**文件**：`src-tauri/src/lib.rs`

- 拆分现有单一 5 分钟线程为两个独立线程
- 高频线程：每 5s 运行探活循环（逻辑完全在 lib.rs 内，直接调 `store.get_all()` + 内联 `is_alive()`，不调 sessions.rs 的探活函数）
- 低频线程：每小时触发 `cleanup_expired(24)`（参数单位为**小时**，见 `sessions.rs:165`）
  - 从 5 分钟改为 1 小时的原因：高频巡检已接管所有活跃 session 的清理职责；低频线程只负责 TTL 过期的 `success` session，1 小时精度完全足够，降低无效唤醒

### Task 2：删除 `reap_stale_sessions()` ✅

**文件**：`src-tauri/src/sessions.rs`

- **直接删除** `reap_stale_sessions()` 函数——探活主逻辑已整体移至 lib.rs（Task 3），sessions.rs 不再承担任何探活职责
- sessions.rs 只保留纯存储操作：`get_all()`、`upsert_session()`、`remove_session()`、`cleanup_expired()`
- 同步删除 lib.rs 中对 `reap_stale_sessions()` 的调用点（统一替换为 Task 3 的探活循环）

### Task 3：grace period 计数器 ✅

**文件**：`src-tauri/src/lib.rs`（巡检线程内）

```rust
// 巡检线程局部状态
let mut miss_counts: HashMap<String, u32> = HashMap::new();

loop {
    let store = /* ... */;
    // 用现有 get_all() 在 lib.rs 内过滤，不在 sessions.rs 新增 helper
    let sessions: Vec<Session> = store.lock().unwrap()
        .get_all()
        .iter()
        .filter(|s| matches!(s.status, SessionStatus::Running | SessionStatus::Pending))
        .cloned()
        .collect();

    for session in &sessions {
        // P0 阶段 is_alive() 只做 TTY stat；无 TTY 的 session（Cursor 等）视为存活，
        // 等 P1-B 补进程级探活，避免 P0 与 P1-B 产生依赖
        let alive = match session.terminal_tty.as_deref() {
            Some(tty) if !tty.is_empty() => std::path::Path::new(tty).exists(),
            _ => true,
        };
        if alive {
            miss_counts.remove(&session.id);
        } else {
            let count = miss_counts.entry(session.id.clone()).or_insert(0);
            *count += 1;
            if *count >= 2 {
                // 触发 remove
                miss_counts.remove(&session.id);
                // → Task 4 的 remove 序列
            }
        }
    }

    // 清理已不在活跃列表的计数条目（session 已被其他途径删除）
    miss_counts.retain(|id, _| sessions.iter().any(|s| &s.id == id));

    thread::sleep(Duration::from_secs(5));
}
```

### Task 4：remove 完整序列（含 popup 关联关闭） ✅

**文件**：`src-tauri/src/lib.rs`（巡检线程内）

```rust
fn remove_session_with_cleanup(
    app: &AppHandle,
    store: &Arc<Mutex<SessionStore>>,
    popup_list: &PopupList,
    session_id: &str,
) {
    // 直接使用现有接口 popup::close_popup(app, id, popup_list)，无需新增包装
    popup::close_popup(app, session_id, popup_list);
    store.lock().unwrap().remove_session(session_id);
    app.emit("sessions-updated", ()).ok();
}
```

**注意**：`popup::close_popup(app, id, popup_list)` 已存在（`popup.rs:97`），签名完全匹配，无需新增接口。

### Task 5：TTL 硬编码 + 移除用户配置项 ✅（**Task C 后**：TTL 整条机制已删除，本 Task 退化为历史记录）

**文件**：`src-tauri/src/lib.rs`、`src-tauri/src/settings.rs`、前端设置面板

- `cleanup_expired()` 调用时固定传入 `24`（小时），不再从 settings 读取
- `Settings` 结构体移除 `session_retention_hours` 字段（`settings.rs:25`）
- `settings.json` 中已有的 `session_retention_hours` 字段：直接删除 Rust 字段定义即可，serde 默认忽略未知 JSON key，无需迁移脚本

**前端改动范围（明确到文件和行）：**

`src/settings/SettingsWindow.tsx`：
- 删除 `session_retention_hours` 字段（`Settings` interface，行 12）
- 删除 `RETENTION_OPTIONS` 常量（行 83–88）
- 删除 retention radio group JSX（行 247–262）

`src/i18n/strings.ts`：
- 删除以下 5 个 i18n key（已确认仅在 `SettingsWindow.tsx` 里使用，可安全删除）：
  - `settings.retention`、`settings.retention_desc`
  - `settings.hours`、`settings.days`、`settings.forever`

### Task 6：修正架构文档 §10 ✅

**文件**：`docs/session-list-architecture.md`

将以下错误描述：
> `source` / `tty` / `workspace_path` 只写一次：首次非 None 值后不再被覆盖

修正为：
> `source` / `tty` / `workspace_path` 仅在后续事件携带非 None 值时覆盖，首次写入后如果后续事件对应字段为 None 则保留原值

---

## 四、P1 实现清单

> 分为"协议升级"和"Cursor/终端扩展"两个独立批次，可并行推进。

### P1-A：协议升级批次 ✅ 已实现

**背景**：涉及 hook binary 改动和三端配置升级，需要协调上线节奏，单独出 brief。

> 2026-04-18 更新：P1-A 已落地。当前 `/notify` 显式区分 `event_type`，老 hook 缺字段时继续按 `status` 降级；`Cursor sessionEnd` 走 `session_end` 直删；`Session` 已新增 `external_session_id` 纯存储字段。

#### A1：`/notify` 增加 `event_type` 字段

当前 `/notify` 用 `status` 字段隐式表达事件类型，需要补显式 `event_type`：

| event_type | 语义 | 后端处理 |
|---|---|---|
| `running` | Agent 开始工作 | upsert → Running |
| `pending` | Agent 等待用户操作 | upsert → Pending |
| `stop` | 本轮任务完成/中止 | upsert → `Idle` / `LastFailed`（取决于 `status`） |
| `session_end` | 会话彻底结束 | remove（含 popup） |

**文件**：`src-tauri/src/bin/hook.rs`、`src-tauri/src/http_server.rs`

兼容策略：`event_type` 缺失时按现有 `status` 字段降级处理，保持向下兼容。

#### A2：数据结构补 `external_session_id` 字段

```rust
pub struct Session {
    // ...现有字段...
    pub external_session_id: Option<String>,  // 上游 Agent 的原始会话 ID
}
```

用途：为后续 Cursor 精确跳转、跨会话诊断、去重预留接口。当前不参与任何逻辑分支，纯存储字段。

各端映射关系（由 hook binary 填充）：

| Source | external_session_id 来源 |
|---|---|
| Claude Code | hook 事件的 `session_id` 字段 |
| Codex | hook 事件的对话标识 |
| Cursor | `sessionStart` 事件的会话 ID |

### P1-B：Cursor / 终端扩展批次

#### B1：Cursor popup 抑制

**文件**：`src-tauri/src/popup.rs`（`is_terminal_session_focused()` 或其调用点）

在 popup 抑制决策里增加 Cursor 判断：

```rust
// http_server.rs handle_notify() 中
let already_focused = if session.source.as_deref() == Some("cursor") {
    is_cursor_frontmost()  // 新增
} else {
    session.terminal_tty.as_deref()
        .is_some_and(|tty| !tty.is_empty() && popup::is_terminal_session_focused(tty))
};
```

```rust
// popup.rs 新增
fn is_cursor_frontmost() -> bool {
    // AppleScript: 检查 frontmost app 是否为 "Cursor"
    run_applescript(r#"
        tell application "System Events"
            set frontApp to name of first application process whose frontmost is true
        end tell
        return frontApp is "Cursor"
    "#).unwrap_or(false)
}
```

#### B2：粗粒度终端焦点检测扩展（Warp / Ghostty 等）

**文件**：`src-tauri/src/popup.rs`

在 `is_terminal_session_focused()` 里，对非 iTerm2 / Terminal.app 的终端补充粗粒度兜底：

```rust
fn is_terminal_session_focused(tty: &str) -> bool {
    // 现有：精确匹配 iTerm2 / Terminal.app（保留）
    if let Some(result) = check_iterm2_focused(tty) { return result; }
    if let Some(result) = check_terminal_app_focused(tty) { return result; }

    // 新增：粗粒度兜底——frontmost 是已知终端 app 就抑制
    // 误抑制优于误打扰
    is_known_terminal_frontmost()
}

fn is_known_terminal_frontmost() -> bool {
    const KNOWN_TERMINALS: &[&str] = &["Warp", "Ghostty", "Alacritty", "kitty", "WezTerm"];
    let frontmost = get_frontmost_app_name().unwrap_or_default();
    KNOWN_TERMINALS.contains(&frontmost.as_str())
}
```

#### B3：Cursor 进程级探活 ✅（已由 Task C 合并实现）

**文件**：`src-tauri/src/lib.rs`（巡检线程的 `is_session_alive()` 判断）

**状态**：Task C 的决策 4 已把 pgrep 分层探活（含 Cursor）整体落地，本小节仅作历史参考。实际代码以 `is_session_alive` / `probe_cursor_alive` 为准。

```rust
fn is_alive(session: &Session) -> bool {
    match session.source.as_deref() {
        Some("cursor") => is_cursor_process_running(),
        _ => session.terminal_tty.as_deref()
                 .map(|tty| std::path::Path::new(tty).exists())
                 .unwrap_or(false),
    }
}

fn is_cursor_process_running() -> bool {
    // pgrep -x "Cursor" 或等价的进程检查
    std::process::Command::new("pgrep")
        .args(["-x", "Cursor"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

**已知局限**：用户打开多个 Cursor workspace 时，进程存在但某个 workspace 已关闭，此情况无法精确检测。当前阶段接受此精度，记录为已知约束。

#### B4：文档声明 Cursor 能力边界

在 `docs/session-list-architecture.md` §10 末尾补充：

> **Cursor 能力边界（当前实现）**：
> - 跳转粒度：workspace 级（`cursor` CLI），无法定位到具体对话窗口
> - 探活粒度：进程级，多 workspace 场景下精度有限
> - popup 抑制粒度：app 级（frontmost == Cursor 即抑制）
> - 触发宿主元数据分层的条件：出现"一个 Agent 同时运行在终端和 GUI"的真实场景

### P1-C：CC `StopFailure` 事件接入 ✅ 已完成

**背景**：CC 支持 `StopFailure` hook 事件，当一轮对话因 API 错误结束时触发（fire-and-forget，output 和 exit code 被忽略）。

**matcher 值（错误类型）**：`rate_limit`、`authentication_failed`、`billing_error`、`invalid_request`、`server_error`、`max_output_tokens`、`unknown`

**映射方案（已落地）**：

| StopFailure 错误类型 | Poke Poke 状态 | 展示 |
|---|---|---|
| 所有 matcher | `last_failed`（Task C 后的命名，原 `failure`） | popup 红色 + 前端按 `failure_reason` 做 i18n，zh/en 各一套文案 |

**与原方案的差异**：原规划映射到 `pending`，Task A 改为新增 `Failure` 终态；Task C 再把 `Failure` 改名为 `LastFailed` 并剥离"终态"语义——它现在表示"agent 活着但上一轮失败"。理由：StopFailure 是"这一轮任务已终止"信号，pending 的语义是"等待用户交互"，混用会让探活、popup 触发规则变脏。

**数据结构**：`Session` 新增 `failure_reason: Option<String>` 字段（serde_default 向下兼容），仅 `LastFailed` 状态下携带；状态转出 `LastFailed` 时自动清空。

**文件改动**：
- `src-tauri/src/bin/hook.rs`：`CC_HOOK_EVENTS` 加 `"StopFailure"`，新增 `handle_stop_failure()`
- `src-tauri/src/http_server.rs`：`/notify` 请求体支持 `failure_reason` 字段
- `src-tauri/src/sessions.rs`：`Session` 加 `failure_reason`，`upsert_session` 透传
- `src/types.ts` / `src/i18n/strings.ts`：前端类型 + `stop_failure.*` 文案
- `src/popup/PopupWindow.tsx`：根据 `failure_reason` 渲染本地化 message

**已知升级路径要求**：老版本 PokePoke 用户升级后，`~/.claude/settings.json` 不会自动补 `StopFailure` 注册；`poke-hook --check` 会报 `hooks_configured: false`，用户需重新点一次"安装"。选择不在 check 里做自动迁移，因为 check 语义是只读+幂等。

---

## 五、P2 及延后

| 项目 | 触发条件 |
|---|---|
| 全局开关 Switch（产品规格要求） | 优先级排期 |
| 宿主元数据完整分层（`host_kind` / `presence`） | 出现真实多宿主组合场景（如 CC 支持 GUI 模式） |
| 像素 mascot（hash 生成 session 形象） | 体验迭代排期 |

---

## 六、文件改动索引

| 文件 | 改动 | 批次 |
|---|---|---|
| `src-tauri/src/lib.rs` | 拆分巡检线程、grace period 计数器、remove 完整序列 | P0 ✅ |
| `src-tauri/src/sessions.rs` | 删除 `reap_stale_sessions()`（探活逻辑移至 lib.rs） | P0 ✅ |
| `src-tauri/src/settings.rs` | 移除 `session_retention_hours` 字段 | P0 ✅ |
| 前端设置面板 | 移除"会话保留时长"选项 | P0 ✅ |
| `docs/session-list-architecture.md` | 修正 §10 字段覆盖逻辑描述 | P0 ✅ |
| `src-tauri/src/bin/hook.rs` | 增加 `event_type` 字段发送 | P1-A ✅ |
| `src-tauri/src/http_server.rs` | `event_type` 分发、`session_end` → remove | P1-A ✅ |
| `src-tauri/src/sessions.rs` | 新增 `external_session_id` 字段 | P1-A ✅ |
| `src-tauri/src/popup.rs` | Cursor frontmost 抑制、Warp/Ghostty 粗粒度兜底 | P1-B |
| `src-tauri/src/lib.rs` | `is_alive()` 按 source 分发、Cursor 进程探活 | P1-B |
| `docs/session-list-architecture.md` | 声明 Cursor 能力边界 | P1-B |
| `src-tauri/src/bin/hook.rs` | CC hook 注册 `StopFailure` 事件 + `handle_stop_failure` | P1-C ✅ |
| `src-tauri/src/http_server.rs` | `/notify` 支持 `failure_reason` 字段 | P1-C ✅ |
| `src-tauri/src/sessions.rs` | `Session` 加 `failure_reason` 字段 | P1-C ✅ |
| `src/i18n/strings.ts` | `stop_failure.*` 本地化文案 | P1-C ✅ |
| `src/popup/PopupWindow.tsx` | 失败原因本地化渲染 | P1-C ✅ |
