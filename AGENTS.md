# AGENTS

本文件用于约束在本仓库中工作的 AI Agent。目标不是“让 Agent 更自由”，而是让它**更难跑偏**。

## 1. 项目目标

Poke Poke 是一个基于 Tauri + React + TypeScript 的本地通知工具，用来承接 Claude Code / Codex / Cursor 等 agent 的 hook 事件并展示任务状态。

当前阶段的最高优先级不是做大产品叙事，而是：

1. 保证 hook 接入稳定
2. 保证 session 状态与 popup 行为正确
3. 保证外部配置不会被错误安装/卸载逻辑破坏
4. 建立最小测试和回归基线

## 2. Source Of Truth

遇到冲突时，按这个顺序判断：

1. 当前 task brief
2. `docs/product-spec.md`
3. `docs/reliability-todo.md`
4. `docs/hook-events.md`
5. `docs/test-plan-final.md`
6. 当前代码实现

不要只根据现有代码推断产品意图。

## 3. 任务开始前必须做的事

对于任何非 trivial 任务，先补一个 task brief。

推荐使用：

- `docs/templates/agent-task-brief.md`

task brief 至少要写清楚：

- 本次目标
- 非目标
- 影响文件
- 禁止改动的文件
- 验收标准

没有 task brief，不要直接开始跨模块改动。

## 4. 模块边界

默认按以下边界工作：

- `src-tauri/src/bin/hook.rs`
  - hook 输入协议、安装、检查、卸载
- `src-tauri/src/http_server.rs`
  - `/notify` 请求入口、状态流转、弹窗判定
- `src-tauri/src/sessions.rs`
  - session/task 持久化与状态模型（旧名 `notifications.rs`，已重命名）
- `src-tauri/src/commands.rs`
  - Tauri commands 与系统集成
- `src/panel/*`
  - panel UI
- `src/popup/*`
  - popup UI
- `src/settings/*`
  - settings UI

规则：

- UI 任务不要顺手改 hook 协议
- hook 任务不要顺手改 panel/popup 视觉结构
- 测试任务不要顺手重构实现
- 数据模型变更必须同步检查受影响 UI 和测试

## 5. 反跑偏规则

每次任务都要显式说明：

- `owned_files`
- `read_only_files`
- `forbidden_files`

同时遵守：

- 不要把“顺手优化”混进当前任务
- 不要跨 3 个以上模块做无明确必要的改动
- 不要在一次任务里同时做“行为修改 + 大重构”
- 不要改动与目标无关的命名、样式、目录结构

## 6. 高风险改动

下面这些改动需要先出 brief，再进入实现：

- 改 `hook.rs` 的 install/check/uninstall 逻辑
- 改外部配置文件写入规则
- 改 session 状态机
- 改 popup 触发/关闭规则
- 改产品规格或可靠性约束文档

## 7. 验收门禁

至少执行与任务相关的最小验证。

常用门禁：

- `pnpm build`
- `cd src-tauri && cargo test`
- `cd src-tauri && cargo check`

如果任务影响 hook 集成，还应补：

- install/check/uninstall smoke 验证

如果任务影响行为契约，还应对照：

- `docs/product-spec.md`
- `docs/reliability-todo.md`

## 8. 多 Agent 协作

多 agent 并行时：

- 每个 agent 必须有独立写集
- 结果必须列出 changed files
- 如果发现别人正在改同一区域，优先停下来收敛范围
- 不要覆盖、回退、重做其他 agent 已完成的改动，除非任务明确要求

## 9. 当前阶段建议的任务优先级

1. `reliability-todo` 中的 P0
2. 最小测试基线
3. 状态机与 popup 行为契约
4. 其余 UI/体验优化

## 9b. 当前阶段 Non-goals

下面这些事项在当前阶段明确**不做**。如果一个任务正在顺手扩到这些方向，应当停下来收敛范围。

- 不做 Codex hook 正式接入（仍在 feature flag，详见 `docs/hook-events.md`）
- 不做 panel / popup 的视觉重设计，仅限行为级修改
- 不做多 agent 聚合、云端同步、跨设备状态
- 不扩产品叙事章节（不新增 `docs/*-strategy.md` 类长文）
- 不为"更优雅"而做跨 3 个以上模块的重构
- 不顺手调整命名、目录结构、i18n key 布局（除非任务明确要求）

发现以上项目确有必要推进时，必须先出独立 task brief 并更新本小节，而不是在无关任务里附带完成。

## 10. 完成定义

一次任务完成，不等于“代码改完了”，而是同时满足：

- 目标范围内的问题被解决
- 没有明显越界改动
- 最小验证已运行
- 如有行为变化，相关文档已同步
- changed files 清单明确
