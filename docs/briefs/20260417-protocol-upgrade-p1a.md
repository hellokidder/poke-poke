# Agent Task Brief

## Title

落地 P1-A 协议升级：显式 `event_type`、`session_end` 删除、`external_session_id`。

## Background

当前 `/notify` 仍然只靠 `status` 隐式表达事件类型，`Cursor sessionEnd` 仍被映射成 `idle`，而 `Session` 结构里也还没有 `external_session_id`。这会让协议语义不完整，也阻碍后续更精确的会话级诊断与跳转能力。

## Goal

只完成 P1-A 协议升级批次：

1. hook binary 对 `/notify` 发送显式 `event_type`
2. 后端兼容解析 `event_type`，并让 `session_end` 直接 remove session + popup
3. `Session` / 前端类型补 `external_session_id`
4. 保持对老 `status` 协议的向下兼容

## Non-goals

- 不做 `Cursor frontmost` popup 抑制
- 不做 Warp / Ghostty 焦点检测扩展
- 不做 reliability P0 自检/修复链路
- 不做 `commands.rs` 的跳转精度提升
- 不顺手调整 panel / popup 的视觉结构

## Owned Files

- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src/types.ts`
- `docs/hook-events.md`
- `docs/session-lifecycle-refactor.md`
- `docs/briefs/20260417-protocol-upgrade-p1a.md`

## Read-only Files

- `docs/product-spec.md`
- `docs/reliability-todo.md`
- `docs/test-plan-final.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/popup.rs`
- `src-tauri/src/tray.rs`

## Forbidden Files

- `.cursor/hooks.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Info.plist`
- `package.json`
- `src-tauri/Cargo.toml`

## Acceptance Criteria

- [x] `hook.rs` 对运行中/等待/结束/会话结束发送显式 `event_type`
- [x] `http_server.rs` 在 `event_type=session_end` 时直接 remove session 并关闭 popup
- [x] 缺失 `event_type` 时仍按现有 `status` 协议正常工作
- [x] `Session` 与前端类型包含 `external_session_id`，旧数据可兼容加载
- [x] 自动化验证通过

## Risks

- `session_end` 从 upsert 改 remove，若 task_id 映射不一致会出现删不掉的情况
- `event_type` 与 `status` 同时存在时需要明确优先级，避免新老协议交错导致状态漂移
- `external_session_id` 需要保持“纯存储字段”，不能不小心参与既有行为分支

## Verification

- [x] `npm run test`
- [x] `npm run build`
- [x] `cd src-tauri && cargo test`

## Notes

- 新协议要优先兼容“老 hook -> 新 app”，即 `event_type` 缺失时继续走 `status`
- 对于 `session_end`，新 hook 仍带 `status: "idle"` 作为旧版本 app 的退化兼容
- 构建通过，但本机 Node `22.5.1` 低于 Vite 7 推荐的 `22.12+`，构建时会打印告警

---

## Changed Files（完成时回填）

- `docs/briefs/20260417-protocol-upgrade-p1a.md`
- `src-tauri/src/bin/hook.rs`
- `src-tauri/src/http_server.rs`
- `src-tauri/src/sessions.rs`
- `src/types.ts`
- `docs/hook-events.md`
- `docs/session-lifecycle-refactor.md`
