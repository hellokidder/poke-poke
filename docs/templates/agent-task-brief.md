# Agent Task Brief

> 复制本模板到 `docs/briefs/YYYYMMDD-<slug>.md`，开工前填完前 7 段，完成时回填 "Changed Files"。

## Title

一句话描述本次任务。

## Background

为什么要做这件事。贴近当前问题，不写大而空背景。

## Goal

本次只解决什么。

## Non-goals

本次明确不解决什么。参考 `AGENTS.md §9b` 当前阶段 Non-goals。

## Owned Files

本次**可以写入**的文件。从下列模块菜单中勾选并具体到文件；若需新增文件请显式声明。

**后端（Rust）**

- [ ] `src-tauri/src/bin/hook.rs`（hook 协议 / 安装 / 检查 / 卸载）
- [ ] `src-tauri/src/http_server.rs`（`/notify` 入口、状态流转、弹窗判定）
- [ ] `src-tauri/src/sessions.rs`（session 状态机与持久化）
- [ ] `src-tauri/src/commands.rs`（Tauri commands 与系统集成）
- [ ] `src-tauri/src/popup.rs`（popup 窗口管理）
- [ ] `src-tauri/src/tray.rs`（托盘菜单）
- [ ] `src-tauri/src/settings.rs`（设置存储）
- [ ] `src-tauri/src/sound.rs` / `shortcut.rs` / `lib.rs`（按需）

**前端（React）**

- [ ] `src/panel/*`（会话面板 UI）
- [ ] `src/popup/*`（提示弹窗 UI）
- [ ] `src/settings/*`（设置 UI）
- [ ] `src/icons/*` / `src/i18n/*` / `src/types.ts`（按需）

**其他**

- [ ] `.cursor/hooks.json` / `.cursor/rules/*`
- [ ] `docs/*`、`scripts/*`、`package.json`、`src-tauri/Cargo.toml`
- [ ] 新增文件：`path/to/file`

## Read-only Files

仅读、不可改动的上下文文件（通常是规格 / 可靠性约束 / 其他 agent 正在改的区域）。

- `docs/product-spec.md`
- `docs/reliability-todo.md`
- `docs/hook-events.md`

## Forbidden Files

本次绝对不得触碰的文件（例如另一个 agent 正在改的模块、上线前冻结的文件）。

- path/to/file

## Acceptance Criteria

- [ ] 行为结果 1
- [ ] 行为结果 2
- [ ] 最小验证完成

## Risks

- 风险 1
- 风险 2

## Verification

根据影响范围选择，不要全选。

- [ ] `pnpm typecheck`
- [ ] `pnpm build`
- [ ] `cd src-tauri && cargo check`
- [ ] `cd src-tauri && cargo test`
- [ ] `pnpm install:hook`（修改 `hook.rs` 时必做）
- [ ] 手工 smoke（例如 `scripts/focus-probe.sh`、hook install/check/uninstall）

## Notes

需要同步的文档、外部约束、人工确认点。行为契约发生变化时记得同步 `CHANGELOG.md`。

---

## Changed Files（完成时回填）

列出本次实际改动的文件清单，用于交接和多 agent 写集核对。

- path/to/file
