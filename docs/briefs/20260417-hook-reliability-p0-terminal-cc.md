# Agent Task Brief

## Title

落地 terminal-only 的 Claude Code reliability P0：启动自检、显式告警、一键修复。

## Background

`docs/reliability-todo.md` 记录了 2026-04-13 的配置漂移事故：`~/.local/bin/poke-hook` 或 `~/.claude/settings.json` 失效后，Poke Poke 当前实现只能在用户手动点击菜单时才发现问题。1.0 已决定先锁终端层，因此本轮只聚焦 Claude Code 的终端 hook 链路，不扩到 Cursor GUI。

## Goal

只完成 Claude Code 集成的 P0 事故防线：

1. 启动时主动检查 `poke-hook` 二进制、可执行性和 Claude hooks 配置
2. 在托盘菜单和 Session Panel 中给出明确失效告警
3. 提供一键修复入口，修复动作幂等
4. 不影响 Claude Code 本身已有其他 hooks

## Non-goals

- 不做 Codex / Cursor 的 health 管理统一化
- 不做 P1 的受管元数据、备份、受管标记
- 不做 GUI 会话级跳转或 Cursor 精细能力
- 不做新的设置项或 UI 重设计
- 不做安装/卸载链路的大重构

## Owned Files

- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/tray.rs`
- `src/panel/SessionPanel.tsx`
- `src/panel/panel.css`
- `src/i18n/strings.ts`
- `docs/reliability-todo.md`
- `docs/briefs/20260417-hook-reliability-p0-terminal-cc.md`

## Read-only Files

- `docs/product-spec.md`
- `docs/hook-events.md`
- `docs/test-plan-final.md`
- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src-tauri/src/popup.rs`
- `src-tauri/src/settings.rs`

## Forbidden Files

- `.cursor/hooks.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Info.plist`
- `src-tauri/src/commands.rs` 中与 Cursor GUI 跳转相关的逻辑

## Acceptance Criteria

- [x] App 启动时会主动完成 Claude Code 集成检查
- [x] 托盘菜单在失效时明确显示“需要修复”而不是静默显示“未连接”
- [x] Session Panel 在失效时显示明确告警和修复按钮
- [x] 修复入口执行后会刷新托盘和面板状态
- [x] 修复动作重复执行不会破坏其他 Claude Code hooks
- [x] 自动化验证通过

## Risks

- 直接改 tray 菜单文案可能与当前“连接/断开”切换语义混在一起
- `hook.rs --check` 如果返回信息过少，面板文案可能无法区分“脚本缺失”和“配置漂移”
- 改 SessionPanel 时要保持现有列表/点击行为不被破坏

## Verification

- [x] `npm run test`
- [x] `npm run build`
- [x] `cd src-tauri && cargo test`

## Notes

- 本轮只承诺 Claude Code；Codex / Cursor 若继续显示旧状态，不纳入此次验收
- 若发现没有现成 Storybook 文档可同步，则只记录“仓库内未配置 Storybook 产物”
- 仓库当前未配置 Storybook 产物，本轮未新增 Storybook 文档
- `npm run build` 成功，但本机 Node `22.5.1` 低于 Vite 7 建议的 `22.12+`，构建时会打印告警

---

## Changed Files（完成时回填）

- `docs/briefs/20260417-hook-reliability-p0-terminal-cc.md`
- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/tray.rs`
- `src/panel/SessionPanel.tsx`
- `src/panel/panel.css`
- `src/i18n/strings.ts`
- `docs/reliability-todo.md`
