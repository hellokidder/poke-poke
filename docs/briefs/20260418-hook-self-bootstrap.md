# Agent Task Brief

## Title

修复 Claude Code 一键修复不可用：让主 app 自举为 `poke-hook`。

## Background

当前 reliability P0 已有检测、告警和修复入口，但当 `~/.local/bin/poke-hook` 缺失时，面板会显示“当前无法修复”。根因是修复逻辑仍依赖另一个现成的 `poke-hook` 二进制来源，而当前 app 本身没有充当 hook CLI 的能力。

## Goal

让当前主 app 可执行文件在被当作 `poke-hook` 调起时执行 hook CLI 逻辑，从而为 repair/install 提供稳定的自举来源。

## Non-goals

- 不引入新的 sidecar 打包链路
- 不重构 `hook.rs` 安装逻辑本身
- 不修改 Cursor / Codex 行为目标
- 不改 reliability UI 文案结构

## Owned Files

- `src-tauri/src/lib.rs`
- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/tray.rs`
- `docs/briefs/20260418-hook-self-bootstrap.md`

## Read-only Files

- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `docs/reliability-todo.md`
- `src/panel/SessionPanel.tsx`

## Forbidden Files

- `.cursor/hooks.json`
- `src-tauri/Info.plist`
- `package.json`

## Acceptance Criteria

- [x] 当前 app 可执行文件在被命名为 `poke-hook` 或收到 hook CLI 参数时，会执行 hook CLI 而不是启动 GUI
- [x] `repair_cc_integration` 在 `~/.local/bin/poke-hook` 缺失时仍可用
- [x] 托盘 “Repair Claude Code Integration” 在该场景下不再错误禁用
- [x] 自动化验证通过

## Risks

- macOS 从 Finder 启动 app 时可能带额外参数，hook 识别逻辑不能误判
- `src/bin/hook.rs` 既作为独立 bin 又作为共享模块使用时，要避免引入不兼容的 crate 根依赖

## Verification

- [x] `npm run test`
- [x] `cd src-tauri && cargo test`

## Notes

- 优先采用“当前 app 自举”而不是新 sidecar，减少打包链路变动

---

## Changed Files（完成时回填）

- `docs/briefs/20260418-hook-self-bootstrap.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/tray.rs`
