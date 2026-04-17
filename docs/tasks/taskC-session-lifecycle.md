# Task C — Session 生命周期语义重构（Agent-as-Session）

## Title

把 PokePoke 面板从"通知事件流"改造成"活着的 AI 实例仪表盘"：session 生命周期不再由时间 TTL 决定，而是严格跟随宿主（终端 / IDE 进程）的存活。

## Background

当前实现把 `SessionStatus` 混用了两类语义：

- **Agent 存活状态**：Running / Pending
- **上一轮结果**：Success / Failure

Success/Failure 被 `is_terminal()` 标记为终态，只能等 24 小时 TTL 下架。调试一天后面板积累 75+ 条已完成/已失败 session，用户找不到真正还活着的 agent；"N 个活跃"副标题和列表长度严重不对应。

产品真正想表达的语义是：**panel 每一行 = 一个活着的 AI 实例**。只要对应的终端 / IDE 进程还在，无论跑了多少轮、成功失败，这一行都应该在。agent 退出或终端关闭，这一行立即消失——没有"24h 保质期"一说。

Stop / StopFailure 类事件是 agent 生命周期中的**阶段性事件**（触发 popup 通知），不是 session 的终态。

## Goal

1. 重新定义 `SessionStatus`，让"agent 存活"和"上一轮结果"不再被混淆
2. 删除 24h TTL 清理线程——session 唯一的清理路径是宿主探活失败
3. 探活线程覆盖所有 session 状态（不再按 `is_terminal()` 过滤）
4. 为无 TTY 的 session（Cursor / 将来可能的 GUI Codex）补进程级探活，否则这类 session 在新模型下永不清理，是严重回归
5. 前端文案、颜色、StatusDot 配置表同步新状态语义
6. 老 `sessions.json` 有 `"success" / "failure"` 值的向下兼容，不让用户数据消失或启动失败

## Non-goals

- 不改 popup 触发时机（Stop / StopFailure / Notification 依然该弹就弹）
- 不改 popup 文案的 i18n 机制（Task B 已实现 `failure_reason` 本地化，此次沿用）
- 不改 `focus_session_terminal` / `open_session_source` 的跳转策略（跳到已死终端时的静默失败是独立 bug，不在此范围）
- 不改 hook binary 的协议字段（`event_type` 升级在 P1-A 批次，独立排期）
- 不做"同一 TTY 多 session 去重"的硬约束——走自然清理路径（见决策 3）
- 不做 panel UI 重设计（排序规则、按 source 分组、批量清理按钮等都延后）
- 不处理 Cursor 打开多个 workspace 的 GUI 精度问题（继续接受进程级探活的粗粒度）

## 决策点与理由

### 决策 1：新的 `SessionStatus` 枚举

四态：

| Variant | 语义 | 何时进入 |
|---|---|---|
| `Running` | agent 正在跑某一轮 | UserPromptSubmit / SessionStart / Cursor beforeSubmitPrompt |
| `Pending` | agent 停下来等用户操作 | CC Notification（权限询问等） |
| `Idle` | 上一轮已结束，agent 空闲等下一轮 | **Stop** hook（原映射到 Success 的位置） |
| `LastFailed` | 上一轮因 API 错误结束，agent 仍活着 | **StopFailure** hook（原映射到 Failure） |

**删除**：`is_terminal()` 方法。新模型下没有"终态"这个概念——这四种都是"活着的 agent 的一种状态"。

**serde 兼容**：使用 `#[serde(alias = "success")]` 和 `#[serde(alias = "failure")]` 让老 `sessions.json` 能无痛读入。

```rust
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Pending,
    #[serde(alias = "success")]
    Idle,
    #[serde(alias = "failure")]
    LastFailed,
}
```

rename 的副作用：新版本写入的 `sessions.json` 用新名字；老版本读新文件会失败——这是单向升级，可接受（PokePoke 不做降级场景）。

### 决策 2：探活线程覆盖所有状态

`lib.rs` 高频探活线程里：

```rust
.filter(|s| matches!(
    s.status,
    SessionStatus::Running | SessionStatus::Pending
))
```

改为不过滤：**所有 session 都进入探活队列**。Idle / LastFailed 的 agent 同样要被探活——只要宿主还在，这一行就保留；宿主消失，立刻 reap。

race 保护条件相应调整：移除"state 必须是 Running/Pending"的二次检查，只保留"TTY 再次 stat 失败"的检查。

### 决策 3：同 TTY 多 session 走自然路径，不做硬去重

**场景**：用户在 `/dev/ttys017` 里先跑 CC（session A，cc-12345678），退出 CC 回到 shell，再开一次新 CC（session B，cc-87654321）。

**问题**：按 `task_id = cc-{session_id[:8]}` 的生成规则，这是两条 session。TTY 探活只能检查 TTY 本身存活，不能检查"这个 TTY 上还有没有 CC 进程"。

**决策**：接受这种场景下**同一 TTY 短时间内出现两条 session**。自然清理路径是进程级探活（见决策 4）——A 的 CC 进程不在了会被 reap，留下 B。

**理由**：强做"同 TTY 只保留一条"的去重会引入时序竞态（新 session 还没完全建立时旧 session 先被覆盖？覆盖谁的字段？），代价大于收益。只要进程级探活做到位，旧 session 会在几秒到十几秒内被自然 reap。

### 决策 4：探活策略严格按 source 分层，无兜底

**核心原则**：**TTY 存活 ≠ session 存活**。旧代码用 TTY stat 判活是错的——用户在 ttys017 里关了 CC 回 shell，TTY 还在但 agent 已死，旧逻辑会把这条 session 留一辈子。Task C 彻底废弃 TTY stat 作为判活依据。

**宁可误清也不留僵尸**：source 字段是判活的硬前提。识别不出 source 的 session 直接视为死亡，第一次探活循环就 reap。

新的 `is_alive(session) -> bool`：

| Source | 探活策略 |
|---|---|
| `claude-code` | `pgrep -t $tty_short claude`（TTY 必须存在且 TTY 上有 claude 进程） |
| `codex` | `pgrep -t $tty_short codex` |
| `cursor` | `pgrep -x Cursor`（不依赖 TTY，看整个 Cursor app 进程） |
| 其他/未识别 / source 为空 | **直接判死**，下个探活循环 reap |
| 有 source 但缺 TTY（CLI agent 丢 tty 信息） | **判死**——没法判活的 session 不留 |

**为什么这么狠**：新产品语义下 panel = 活着的 agent 仪表盘，留一条无法判活的 session 就是在骗用户。误清的代价是"用户看到 session 消失后重新发起请求会重建一条新 session"，成本低；留僵尸的代价是"panel 又回到 75 条的调试噩梦"，成本高。

**hook.rs 同步改**：所有 hook 分支必须发 `source` 字段（目前已经是这样，只需补一条 assert 或日志，让后端发现空 source 时记个 warn）。

**风险**：
- `pgrep` 在权限异常时返回空 → 2 次 miss 后误清。miss_count = 2 的 grace period（10 秒）是唯一缓冲，已足够。
- `claude` / `codex` 进程名和实际二进制名不一致（例如 `npx` 启动的 claude，进程名可能是 `node`）→ 这种场景先不处理，观察真实使用中是否复现，出问题再加 `pgrep -f` fallback。

**测试矩阵**（见 Acceptance Criteria 第 3 组）：source × "关 agent / 关终端 / 保持活着"三种操作。

### 决策 5：cleanup_expired 线程直接删除

- `sessions.rs::cleanup_expired()` 函数删除
- `lib.rs` 的低频线程删除
- `settings.rs` 里相关字段在 P0 阶段已经移除，无残留

### 决策 6：前端状态视觉

| 新状态 | StatusDot 颜色 | 动画 | 文案 zh / en |
|---|---|---|---|
| Running | `#4ade80` 绿 | 呼吸 | 运行中 / Running |
| Pending | `#facc15` 黄 | 呼吸 | 等待中 / Pending |
| **Idle** | `#60a5fa` 淡蓝（改原灰白） | 无 | 空闲中 / Idle |
| **LastFailed** | `#f87171` 红 | 无 | 上一轮出错 / Last turn failed |

理由：Idle 用淡蓝而非原灰白——灰白暗示"已失效/归档"，和新语义"agent 活着"冲突；淡蓝传达"静止但在线"。LastFailed 保留红色但文案调整，让用户理解 agent 没死。

SourceIcon 的表情继续沿用现有实现，但动画类需要同步：
- `source-icon.css` 里 `.source-icon.success` → `.source-icon.idle`（去掉 bounce 动画——空闲不该持续弹跳）
- `success` case 在 `getExpression()` 里改名为 `idle`（复用笑眼）
- `failure` case 改名为 `last_failed`（复用 X 眼）

`isActive()` 判断保留但含义改变：

```ts
function isActive(s: Session): boolean {
  return s.status === "running" || s.status === "pending";
}
```

Idle / LastFailed 同样有 agent 活着，但"是否正在跑"按 Running/Pending 判断——这样 panel 副标题"N 个活跃"意味着"N 个正在工作的"，与 Idle 的"在线但空闲"区分开。

### 决策 7：老数据迁移

启动时 `SessionStore::load()` 读入老 JSON：

- `status: "success"` → 通过 serde alias 映射到 `Idle`
- `status: "failure"` → 通过 serde alias 映射到 `LastFailed`

**一次性清理**：新模型下 Idle / LastFailed 的 session 必须有存活宿主才应该出现在面板。老数据里那 75 条 session 的宿主大概率已经不在了——启动时跑一次全量探活，发现宿主已死的直接 reap，不必等第一次探活循环。

这个启动清理在 `lib.rs` 的 setup 流程里做，不涉及 sessions.rs 的数据结构。

## Owned Files

- `src-tauri/src/sessions.rs`
  - 重定义 `SessionStatus` 枚举 + `#[serde(alias)]`
  - 删除 `is_terminal()` 方法
  - 删除 `cleanup_expired()` 函数
  - `upsert_session` 里 `failure_reason` 覆盖规则跟着新枚举名（只在 `LastFailed` 时保留）
- `src-tauri/src/lib.rs`
  - 探活线程移除 status 过滤
  - 探活线程移除 "status 必须 Running/Pending" 的二次检查
  - 删除低频 cleanup_expired 线程
  - 新增启动时全量探活 + reap
  - `is_alive()` 辅助函数（决策 4 的分层策略）
- `src-tauri/src/http_server.rs`
  - `/notify` 的 status 字符串解析：`"success"` → `Idle`（新别名）、`"failure"` → `LastFailed`、同时保留老字符串作兼容 alias
  - popup 触发判定：`is_terminal_transition` 的定义改为"进入 Idle/LastFailed 且 prev 不同"
- `src-tauri/src/bin/hook.rs`
  - `handle_stop` 发 `status: "idle"`（原 `success`）
  - `handle_stop_failure` 发 `status: "last_failed"`（原 `failure`）
  - `handle_cursor_stop` / `handle_session_end` 同步改成 `idle`
- `src/types.ts`
  - `SessionStatus` 枚举值改成 `running | pending | idle | last_failed`
- `src/i18n/strings.ts`
  - 删除 `status.success` / `status.failure` key
  - 新增 `status.idle` / `status.last_failed`
  - `stop_failure.*` 保留（由 Task B 引入）
- `src/panel/SessionPanel.tsx`
  - StatusDot 配置表按决策 6 改写
  - `isActive` 函数保持但注释更新
- `src/popup/PopupWindow.tsx`
  - `statusClass` 的 ternary 更新到新状态名
  - `displayMessage` 逻辑里的 `status === "failure"` 改成 `status === "last_failed"`
- `src/popup/popup.css`
  - `.popup-container.success` → `.popup-container.idle`
  - `.popup-container.failure` → `.popup-container.last_failed`
  - 颜色按决策 6 调整
- `src/icons/SourceIcon.tsx`
  - `getExpression` 的 case 名同步
  - `SessionStatus` 类型对齐
- `src/icons/source-icon.css`
  - `.source-icon.success` 相关 class 名同步；删除 success 的 bounce 动画（idle 不该持续弹跳）
- `docs/hook-events.md`
  - Poke 映射表：success → idle、failure → last_failed
- `docs/session-lifecycle-refactor.md`
  - 决策 1 表格修正：不再用"终态"描述 Stop → Success
  - P0 实现清单里涉及 `cleanup_expired` / is_terminal 的部分标记为"已在 Task C 重构"
  - P1-B B3 Cursor 进程探活标记为"已在 Task C 实现"（或至少合并一部分）
- `docs/session-list-architecture.md`
  - §状态机定义（如存在）同步
- `docs/tasks/taskC-session-lifecycle.md`
  - 本文件自身作为历史记录

## Read-only Files

- `src-tauri/src/commands.rs`（只看 `focus_session_terminal` 等跳转逻辑，不改）
- `src-tauri/src/popup.rs`（popup 触发点的上下文参考；焦点模型 tauri-nspanel 不动）
- `src-tauri/src/settings.rs`
- `src-tauri/src/tray.rs`
- `src-tauri/src/notifications.rs` / `shortcut.rs` / `sound.rs`
- `src/settings/SettingsWindow.tsx`
- `src/popup/popup.css` 里与 `high` 优先级 / popup-glow 形状相关的部分

## Forbidden Files

- `src-tauri/Info.plist` / `tauri.conf.json`（不动 macOS 激活策略）
- `src-tauri/Cargo.toml`（不加新依赖）
- `.cursor/hooks.json` / `.cursor/hooks/*`（不动 Cursor hook 自身协议）
- `scripts/focus-probe.sh`（popup 焦点任务产物，与本任务无关）

## Acceptance Criteria

### 代码/构建门禁

- [ ] `cd src-tauri && cargo check` 通过
- [ ] `cd src-tauri && cargo test` 通过
- [ ] `pnpm build`（tsc + vite build）通过
- [ ] `cargo build --bin poke-hook` 产出新 binary

### 行为验证：状态机

- [ ] 新装：`sessions.json` 不存在，所有事件正确写入新枚举值（`idle` / `last_failed`）
- [ ] 老数据兼容：手工准备一份含 `status: "success"` / `status: "failure"` 的 `~/.pokepoke/sessions.json`，启动后 panel 正常显示，状态分别渲染为 Idle / LastFailed
- [ ] Stop 事件：原 `success` 场景下 panel 小点变淡蓝，文案"空闲中"
- [ ] StopFailure 事件：panel 小点红，文案"上一轮出错"，popup message 用 Task B 的 i18n 展示具体 reason
- [ ] Running → Idle → Running（新一轮开始）：小点从绿变蓝再变绿，之前的 failure_reason 被清空

### 行为验证：生命周期

- [ ] 终端开着，CC 跑一轮 Stop：session 留在 panel 显示 Idle，**不被 TTL 清理**（静置观察 ≥ 5 分钟，记录不消失）
- [ ] 终端开着，CC 退出到 shell：session 在 10~15 秒内（探活 2 次 miss + 一个周期）被 reap，消失
- [ ] 终端关闭（整个窗口）：session 在 10~15 秒内被 reap
- [ ] Cursor 开着，Stop 事件：session 留在 panel 显示 Idle
- [ ] Cursor 退出（整个 app）：session 被 reap
- [ ] Running 中关闭终端：session 立刻被 reap（原有行为不变）
- [ ] Pending 中关闭终端：session 立刻被 reap（原有行为不变）

### 启动时一次性清理

- [ ] 故意在 `sessions.json` 中放入一条 `tty = /dev/ttys999`（不存在）的 Idle session，重启 PokePoke 后 ≤ 15 秒内该 session 从 panel 消失

### 文档同步

- [ ] `docs/hook-events.md` 映射表更新
- [ ] `docs/session-lifecycle-refactor.md` 标注 Task C 覆盖的决策
- [ ] 本 brief 的 Acceptance Criteria 全部勾选

## Risks

- **R1 - 进程级探活跨平台差异**：`pgrep -t` 的 `-t` 参数在 Linux 和 macOS 上的 tty 格式不同（macOS 是 `ttys017`，Linux 可能是 `pts/0`）。本任务只验证 macOS，Linux/Windows 的 fallback 先保留 TTY stat 行为。
- **R2 - 老用户升级**：新版本写入的 `sessions.json` 包含 `idle` / `last_failed` 值，降级回老版本会反序列化失败。有意接受，不做降级兼容。
- **R3 - 启动清理的 race**：启动时 HTTP server 和探活线程是并行起的，如果用户刚启动 PokePoke 就有 hook 事件打进来，可能出现"新 session 还没落盘 → 启动清理把它误删"的理论竞态。实际上 HTTP listen 在 async 任务里，探活线程 sleep 5s 后才跑，窗口极窄。写 brief 时记录，若实测有问题再加 guard。
- **R4 - 状态字符串改名破坏外部 hook**：如果有第三方直接往 `/notify` POST 发 `status: "success"`，新版本仍会正确识别（serde alias）；若发 `status: "failure"` 同样兼容。这是有意保留的兼容层，不做废弃警告。
- **R5 - 面板视觉回归**：Idle 颜色从灰白改成淡蓝是主观决策，用户可能觉得"不如灰色'已完成'的状态感强"。若反馈不佳，回滚颜色的代价小（只改 StatusDot 配置 + popup.css 一处）。

## Verification

### 自动化
```
cd src-tauri && cargo check
cd src-tauri && cargo test
cd /Users/edy/Documents/Kidder/poke-poke && pnpm build
cd src-tauri && cargo build --bin poke-hook
```

### 手工冷烟脚本（建议写成 `scripts/taskc-smoke.sh`，可选）
```bash
# 1. 新装流程：清空 sessions.json，触发各种 hook 事件
rm ~/.pokepoke/sessions.json
# (启动 PokePoke)
echo '{"hook_event_name":"SessionStart","session_id":"abcdef12","cwd":"/tmp"}' | ./target/debug/poke-hook
echo '{"hook_event_name":"Stop","session_id":"abcdef12","cwd":"/tmp"}' | ./target/debug/poke-hook
echo '{"hook_event_name":"StopFailure","session_id":"abcdef12","cwd":"/tmp","reason":"rate_limit"}' | ./target/debug/poke-hook
# 观察 panel：应有一条 idle/last_failed 变化
```

### 老数据兼容测试
```bash
cp ~/.pokepoke/sessions.json /tmp/pp-sessions-backup.json
# 手工编辑 ~/.pokepoke/sessions.json，把部分 status 改成 "success"、"failure"
# 重启 PokePoke
# 预期：无 panic，panel 正常加载，状态显示为 Idle/LastFailed
```

## Notes

### 与历史任务的关系

- **Task A**（新增 Failure 终态）：本任务**不回滚** Task A，但**修正其语义归属**——`Failure` variant 改名为 `LastFailed`，`is_terminal()` 删除。可以理解为 Task A 打下了"红色视觉 + failure_reason 数据结构"的基础，Task C 把它从"终态"修正为"agent 活着时的一种状态"。
- **Task B**（StopFailure 接入）：`handle_stop_failure` 发送的 status 字符串从 `"failure"` 改为 `"last_failed"`，内部映射到 `SessionStatus::LastFailed`。`failure_reason` 字段和 i18n 逻辑完全保留。

### 升级路径告知

老用户升级到含 Task C 的版本后：

- `sessions.json` 无需手工迁移（serde alias 兼容）
- `~/.claude/settings.json` 里的 hook 注册不变（hook binary 协议字段未改）
- 首次启动会触发"历史终态 session 中宿主已不在者立即 reap"——用户会看到 panel 里"历史积累的已完成 session"大面积消失。这是预期行为，不是 bug

### 人工确认点

brief 审阅阶段，请对以下几点明确签字：

1. **状态名**：Idle / LastFailed 是否接受？不接受的话备选：`Completed / Errored`、`Done / Failed`、`Ready / Stuck`
2. **颜色**：Idle 淡蓝（`#60a5fa`）是否接受？
3. **同 TTY 多 session 走自然路径**（决策 3）是否接受？
4. **启动清理**（决策 7）是否接受？反面选项是"不做启动清理，等第一次探活循环（5s 后）自然触发"，体验上基本一致
5. **面板 75 条老数据处理**：签字后实施时会随启动清理自动消化，不再单独写脚本
6. **探活兜底策略**（决策 4）：✅ **已拍板 C 方案**——source 识别不出 / 有 source 但缺 TTY 都直接判死，不留兜底。TTY stat 不再作为 CLI agent 的判活依据。
