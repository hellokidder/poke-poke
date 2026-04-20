# Agent Task Brief

## Title

为 Poke Poke 落地第二批 Rust 测试：`hook.rs`、`sound.rs`、`popup.rs`。

## Background

第一批测试已经覆盖了 `sessions.rs`、`settings.rs`、`http_server.rs`。下一批高价值但仍可控的测试集中在 hook 事件解析与 payload 构建、音效路径解析、popup 位置计算。这些模块有明确的纯逻辑边界，适合用最小提纯补上回归保护。

## Goal

只落地第二批 Rust 测试基线：

1. 为 `src-tauri/src/bin/hook.rs` 增加 source/event/helper/payload 构建测试
2. 为 `src-tauri/src/sound.rs` 提取纯路径解析并补测试
3. 为 `src-tauri/src/popup.rs` 的位置计算补最小单测

## Non-goals

- 不做 `hook.rs` 的 install/check/uninstall 集成测试
- 不做 `commands.rs`、`tray.rs`、`lib.rs` 的测试
- 不做前端测试、组件测试、端到端测试
- 不修改 hook 协议、不升级 `event_type`
- 不顺手修改 popup 焦点策略、窗口行为或托盘文案

## Owned Files

- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/sound.rs`
- `src-tauri/src/popup.rs`
- `docs/briefs/20260417-test-baseline-phase2-rust.md`

## Read-only Files

- `docs/product-spec.md`
- `docs/reliability-todo.md`
- `docs/hook-events.md`
- `docs/test-plan-final.md`
- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/tray.rs`

## Forbidden Files

- `.cursor/hooks.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Info.plist`
- `package.json`
- `vitest.config.ts`
- `src/test/setup.ts`

## Acceptance Criteria

- [x] `hook.rs` 已覆盖 `detect_source`、`normalize_event`、`pick_str`、`contains_poke_hook`、`flag_path` 以及 payload 构建主链路
- [x] `sound.rs` 已提取路径解析纯函数并覆盖核心分支
- [x] `popup.rs` 已覆盖 popup 纵向位置计算
- [x] 所做改动仅为测试所需的最小提纯，不改变现有行为契约
- [x] `cd src-tauri && cargo test` 通过

## Risks

- `hook.rs` 若把现有 handler 逻辑提纯过度，容易引入行为偏差
- `popup.rs` 依赖 Tauri window API，测试必须严格收敛到纯数学函数
- `sound.rs` 的测试需要避免真正触发系统音效播放

## Verification

- [x] `cd src-tauri && cargo test`

## Notes

- 若实现中发现需要跨到 `lib.rs` 或 `commands.rs` 才能测试，则先停下收敛范围
- `hook.rs` 优先覆盖纯 helper 与 payload 构建，不做 I/O 侧安装链路验证
- `hook.rs` 本次只把 task_id / cwd / payload 构建提纯为可测试 helper，未触碰 install/check/uninstall 行为

---

## Changed Files（完成时回填）

- `docs/briefs/20260417-test-baseline-phase2-rust.md`
- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/sound.rs`
- `src-tauri/src/popup.rs`
