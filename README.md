# Poke Poke

Poke Poke 是一个基于 Tauri + React + TypeScript 的本地通知工具，用来承接 Claude Code / Codex / Cursor 等 agent 的 hook 事件并展示任务状态。

## AI Agent 必读

进入本仓库做任何非 trivial 改动前，请按顺序读完：

1. [`AGENTS.md`](./AGENTS.md)：仓库级约束、模块边界、Non-goals、完成定义
2. [`docs/product-spec.md`](./docs/product-spec.md)：产品意图与三个模块的行为契约
3. [`docs/reliability-todo.md`](./docs/reliability-todo.md)：外部集成漂移事故防线 P0 清单
4. [`docs/hook-events.md`](./docs/hook-events.md)：CC / Codex / Cursor hook 事件差异
5. [`docs/harness-engineering-strategy.md`](./docs/harness-engineering-strategy.md)：Agent 协作原则（背景阅读）

非 trivial 任务必须先写 task brief，模板见 [`docs/templates/agent-task-brief.md`](./docs/templates/agent-task-brief.md)，归档到 [`docs/briefs/`](./docs/briefs/README.md)。

行为契约变化（hook 协议 / 状态机 / popup 规则 / 外部配置写入）必须同步 [`CHANGELOG.md`](./CHANGELOG.md)。

## 目录结构速览

- `src/`：React 前端（`panel/` 会话列表、`popup/` 提示弹窗、`settings/` 设置、`clawd/` 吉祥物等）
- `src-tauri/`：Rust 后端与 Tauri 能力
  - `src/bin/hook.rs`：独立二进制 `poke-hook`，Cursor / CC / Codex 都靠它转发事件
  - `src/http_server.rs`：`/notify` 入口与状态流转
  - `src/sessions.rs`：session 状态机与持久化
- `docs/`：产品规格、可靠性约束、hook 事件、测试计划、task brief 归档
- `scripts/`：手工验证脚本（如 `focus-probe.sh`）
- `.cursor/`：仓库级 Cursor 规则与 hooks

## 本地开发

```bash
pnpm install

pnpm typecheck          # 仅类型检查
pnpm build              # 前端构建（tsc + vite）
pnpm dev                # 仅前端热更新（无托盘 / 无 HTTP）
pnpm tauri dev          # 完整桌面调试（有托盘 / 有 HTTP）

pnpm install:hook       # 重新构建 poke-hook 并部署到 ~/.local/bin/poke-hook
                        # 改 src-tauri/src/bin/hook.rs 后必须执行

cd src-tauri && cargo check
cd src-tauri && cargo test
```

## Cursor Hooks

`.cursor/hooks.json` 注册了 `sessionStart` / `beforeSubmitPrompt` / `stop` / `sessionEnd` 四个事件，全部指向 `~/.local/bin/poke-hook`（timeout 8s）。由 `poke-hook` 统一向 `127.0.0.1:9876`（fallback `9877`）发 `POST /notify`。

前置条件：

- 工作区已信任、Cursor Hooks 功能已开启
- Poke Poke 通过 `pnpm tauri dev` 或安装版运行（`pnpm dev` 无托盘、无 HTTP）
- `~/.local/bin/poke-hook` 存在且为最新构建（用 `pnpm install:hook` 部署）

排查：Cursor 底部打开 **Hooks** 通道看 `[pokepoke-hook]` 日志；连不上 `/notify` 通常是应用未起或端口被占。
