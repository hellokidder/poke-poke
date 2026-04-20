# Agent Task Brief

## Title

为 Poke Poke 落地第一批测试基线：Rust 核心逻辑单测 + 前端纯函数 Vitest。

## Background

仓库已有 `docs/test-plan-final.md`，但当前代码库还没有真正落地测试基础设施与第一批高 ROI 测试。现在最容易回归的区域是 session 状态机、popup 判定逻辑、设置存储，以及前端围绕状态名和 i18n 的纯函数逻辑。

## Goal

只落地第一批测试基线：

1. Rust：为 `sessions.rs`、`settings.rs`、`http_server.rs` 增加可运行的测试
2. 前端：接入 Vitest，并为 `SourceIcon`、`SettingsWindow`、`SessionPanel`、i18n 增加纯函数测试
3. 仅做测试所需的最小提纯与导出，不顺手扩到组件级测试、hook 链路测试或产品行为修改

## Non-goals

- 不做 `hook.rs` 的 payload/build/install/check 测试
- 不做 `commands.rs`、`tray.rs`、`popup.rs` 的副作用测试
- 不做前端组件渲染测试、Tauri harness、端到端测试
- 不修改 hook 协议、session 生命周期契约、popup 交互策略
- 不顺手重构 UI 结构、命名、样式或目录

## Owned Files

本次允许写入以下文件：

- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src-tauri/src/settings.rs`
- `src/icons/SourceIcon.tsx`
- `src/panel/SessionPanel.tsx`
- `src/settings/SettingsWindow.tsx`
- `src/i18n/*`
- `package.json`
- `src-tauri/Cargo.toml`
- 新增文件：
  - `vitest.config.ts`
  - `src/test/setup.ts`
  - `src/icons/SourceIcon.test.ts`
  - `src/settings/SettingsWindow.test.ts`
  - `src/panel/SessionPanel.test.ts`
  - `src/i18n/i18n.test.ts`

## Read-only Files

- `docs/product-spec.md`
- `docs/reliability-todo.md`
- `docs/hook-events.md`
- `docs/test-plan-final.md`
- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/tray.rs`
- `src-tauri/src/popup.rs`
- `src-tauri/src/commands.rs`

## Forbidden Files

- `.cursor/hooks.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Info.plist`
- `src-tauri/src/lib.rs`

## Acceptance Criteria

- [x] `sessions.rs`、`settings.rs`、`http_server.rs` 各自具备覆盖核心行为的单测
- [x] 前端已接入 Vitest，并新增 `SourceIcon`、`SettingsWindow`、`SessionPanel`、i18n 的纯函数测试
- [x] 为测试落地所做的实现改动仅限最小导出/提纯，不改变既有行为契约
- [x] `cargo test`、前端测试、前端构建通过

## Risks

- `http_server.rs` 目前 popup 判定内联在 handler 中，提纯时若改坏条件分支会引入行为偏差
- 前端纯函数原先未导出，补导出时需要避免影响现有默认导出和组件使用
- Vitest 接入需要 mock Tauri API，但本次应把 mock 范围收敛到最小

## Verification

- [x] `npm run test`
- [x] `npm run build`
- [x] `cd src-tauri && cargo test`

## Notes

- 若实现过程中发现测试需要进一步改动 `hook.rs` 或 `popup.rs`，先停下收敛范围，不直接扩写集
- 完成后回填 Changed Files，便于后续继续推进第二批测试
- 本机 `pnpm` / `npm` 默认 registry 指向 `https://registry.npmmirror.com/`，其中 `pnpm add` 失败；本次通过一次性 `npm --registry=https://registry.npmjs.org/` 安装前端测试依赖，未改全局配置
- `vite build` 成功，但当前 Node 版本 `22.5.1` 低于 Vite 7 建议的 `22.12+`，构建时会打印告警

---

## Changed Files（完成时回填）

- `docs/briefs/20260417-test-baseline-phase1.md`
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src-tauri/src/settings.rs`
- `src/i18n/context.tsx`
- `src/icons/SourceIcon.tsx`
- `src/panel/SessionPanel.tsx`
- `src/settings/SettingsWindow.tsx`
- `vitest.config.ts`
- `src/test/setup.ts`
- `src/i18n/i18n.test.ts`
- `src/icons/SourceIcon.test.ts`
- `src/panel/SessionPanel.test.ts`
- `src/settings/SettingsWindow.test.ts`
