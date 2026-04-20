# Hook Events Reference

CC / Codex CLI / Cursor 完整 hook 事件列表 + 兼容性对比，用于 Poke Poke 接入开发。

## Claude Code Hook Events

| 事件 | 触发时机 | 阻塞 | 说明 |
|------|---------|------|------|
| SessionStart | 会话启动 | 否 | 拿到 session_id, cwd |
| UserPromptSubmit | 用户提交 prompt | 可拦截 | 用户按回车时 |
| PreToolUse | 调用工具前 | 可拦截 | 工具名+参数，可阻止执行 |
| PostToolUse | 调用工具后 | 否 | 工具执行结果 |
| Notification | CC 发通知 | 否 | 如等待用户输入、权限确认 |
| Stop | 每轮对话结束 | 否 | stop_reason |
| **StopFailure** | **API 错误导致对话结束** | **否(fire-and-forget)** | **matcher: `rate_limit` / `server_error` / `authentication_failed` / `billing_error` / `invalid_request` / `max_output_tokens` / `unknown`。output 和 exit code 被忽略** |
| SubagentStop | 子代理完成 | 否 | Agent 工具派出的子代理 |

## Codex CLI Hook Events

**状态：** hooks 功能在 `codex_hooks` feature flag 后面（under development），需手动启用：
```
codex --enable codex_hooks
# 或 ~/.codex/config.toml → [features] codex_hooks = true
```

| 事件 | 触发时机 | 阻塞 | 说明 |
|------|---------|------|------|
| SessionStart | 会话启动 | 否 | 额外字段 `source`: startup/resume/clear |
| UserPromptSubmit | 用户提交 prompt | 可拦截 | 额外字段 `turn_id` |
| PreToolUse | 调用工具前 | 可拦截 | 额外字段 `turn_id` |
| PostToolUse | 调用工具后 | 否 | 额外字段 `turn_id` |
| Stop | 每轮对话结束 | 否 | 额外字段 `turn_id`, `stop_hook_active` |

**仅此 5 个事件，无 Notification / SubagentStop 等 CC 独有事件。**

### Codex Legacy Notify（独立于 hooks 的通知机制）

| 事件 | 说明 |
|------|------|
| agent-turn-complete | Agent 轮次结束 |
| approval-requested | 等待用户批准 |
| user-input-requested | 等待用户输入 |

环境变量传递：`thread-id`, `turn-id`, `cwd`, `client`, `input-messages`, `last-assistant-message`

### CC vs Codex 兼容性

**stdin JSON 共有字段：** `session_id`, `cwd`, `hook_event_name`, `permission_mode`, `transcript_path`, `tool_name`, `tool_input`, `tool_use_id`

**Codex 独有字段：** `turn_id`, `stop_hook_active`(Stop), `model`

> ⚠️ `source: startup/resume/clear/compact`（SessionStart 上的字段）**两边都有**，不是 codex 独有，详见 Anthropic Hooks reference。早期 `detect_source` 曾把它当作 codex 信号导致每个 cc 新会话被误判，已修复。

**CC 额外字段：** `agent_id`, `agent_type`（子 agent 感知）

**输出协议完全一致：**
- Exit 0 → 解析 stdout JSON；Exit 2 → 阻塞错误；其他 → 非阻塞
- JSON 输出：`continue`, `stopReason`, `suppressOutput`, `systemMessage`, `hookSpecificOutput`, `decision`, `reason`

**配置格式不同：**
- CC: `~/.claude/settings.json` → `hooks.{EventName}[].hooks[].command`
- Codex: `~/.codex/config.toml` + 外部 `hooks.json`
- Handler 类型：CC 有 command/http/prompt/agent，Codex 有 command/prompt/agent（无 http）

## Cursor Hook Events

| 事件 | 触发时机 | 阻塞 | 说明 |
|------|---------|------|------|
| sessionStart | 会话启动 | 否(fire-and-forget) | 可返回 env 和 additional_context |
| sessionEnd | 会话结束 | 否(fire-and-forget) | reason: completed/aborted/error/window_close/user_close |
| beforeSubmitPrompt | 用户提交 prompt 前 | 可拦截 | 用户发送消息时 |
| preToolUse / postToolUse / postToolUseFailure | 工具调用前/后 | 可拦截(pre) | 通用工具 hook，支持 matcher 过滤 |
| beforeShellExecution / afterShellExecution | Shell 命令前/后 | 可拦截(before) | 可 allow/deny/ask |
| beforeReadFile | 读取文件前 | 可过滤 | 可过滤/脱敏文件内容 |
| afterFileEdit | 文件编辑后 | 否 | 编辑完成后触发 |
| beforeMCPExecution / afterMCPExecution | MCP 调用前/后 | 可拦截(before) | MCP 工具相关 |
| subagentStart / subagentStop | 子代理启动/完成 | 否 | Task 工具生命周期 |
| afterAgentResponse / afterAgentThought | Agent 回复/思考后 | 否 | 跟踪 Agent 输出 |
| preCompact | 上下文压缩前 | 否 | 观察上下文窗口压缩 |
| stop | 任务结束 | 否(fire-and-forget) | 支持 loop_limit |
| beforeTabFileRead / afterTabFileEdit | Tab 补全读/写 | 可过滤(before) | 仅 Tab(内联补全)模式 |

## Poke Poke 事件映射

Task C 起，Poke 状态名换成 `running` / `pending` / `idle` / `last_failed`；
`success` / `failure` 作为 serde alias 向下兼容老数据，新版本 hook binary 不再发这两个字符串。

P1-A 起，hook binary 对 `/notify` 还会显式发送 `event_type`，用于区分“状态更新”和“会话彻底结束”。

| Poke `event_type` | Poke 结果 | CC 事件 | Codex 事件 | Cursor 事件 |
|-------------------|-----------|---------|-----------|------------|
| `running` | `running`（注册/工作中） | SessionStart, UserPromptSubmit | SessionStart, UserPromptSubmit | sessionStart, beforeSubmitPrompt |
| `pending` | `pending`（等待用户） | Notification | 无对应（需用 legacy_notify: approval-requested） | 无对应（GUI 自带可视化） |
| `stop` | `last_failed`（上一轮 API 错误） | **StopFailure** ✅ 已接入 | 无对应 | 无对应 |
| `stop` | `idle`（一轮正常结束） | Stop | Stop | stop |
| `session_end` | remove（直接删除 session，不入四态） | 无对应 | 无对应 | sessionEnd |

**注意：**
- Task C 重构后，`idle` / `last_failed` **不是** session 的终态——它们表示"agent 活着、上一轮已结束"，session 生命周期改由宿主进程存活决定（见 `docs/session-lifecycle-refactor.md`）。
- `event_type` 缺失时，后端仍按旧 `status` 字段降级解析，兼容老 hook binary。
- `Cursor sessionEnd` 不再 upsert 成 `idle`，而是通过 `event_type = "session_end"` 直接 remove（同时关闭关联 popup）。
- Cursor 没有等待用户交互的 hook，GUI 界面自带权限弹窗。
- Codex 没有 Notification 事件，pending 感知需走 legacy_notify 机制或用 Stop 事件兜底。
- Codex hooks 仍在开发中（feature flag），接入需考虑稳定性风险。

## Poke 接入 Codex 开发要点

> ⚠️ Codex hook 正式接入当前是 **Non-goal**（见 `AGENTS.md §9b`）。现有代码中的 Codex 路径仍在 feature flag 后面，本节仅作未来恢复推进时的设计参考，不应作为当前阶段开工依据。

1. **hook 处理逻辑无需修改** — 5 个核心事件的 JSON 格式与 CC 兼容，serde_json Value 天然忽略额外字段
2. **新增安装逻辑** — 需读写 `~/.codex/config.toml`，格式与 CC 的 settings.json 不同
3. **pending 感知方案** — 需调研 legacy_notify 的接入方式，或暂时只支持 running/idle 两态
4. **source 标识** — task_id 前缀用 `codex-` 区分（类似现有 `cc-` / `cursor-`）
