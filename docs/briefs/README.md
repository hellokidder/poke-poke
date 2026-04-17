# Task Briefs

本目录归档所有非 trivial 任务的 task brief，作为历史写集和意图痕迹。

## 命名规则

```
YYYYMMDD-<slug>.md
```

- `YYYYMMDD`：任务启动日期
- `<slug>`：短横线连接的英文短名，建议与主改动模块关联
  - 示例：`20260417-hook-reliability-p0.md`
  - 示例：`20260418-popup-focus-suppression.md`

## 工作流

1. 开工前：复制 `docs/templates/agent-task-brief.md` 到本目录，填完 `Title / Background / Goal / Non-goals / Owned Files / Read-only Files / Forbidden Files / Acceptance Criteria / Risks / Verification`
2. 执行中：brief 只改 `Notes` 或澄清条目，不回头扩大 `Goal`
3. 完成后：回填 `Changed Files`，必要时在 `CHANGELOG.md` 追加行为契约变更记录

## 与 AGENTS.md 的关系

- `AGENTS.md §3` 规定非 trivial 任务必须先写 brief
- 本目录是 brief 的**唯一归档位**，不要散落在 `docs/` 根目录
- 历史 brief 不删除，作为后续 agent 查询"这一区域之前是否被改过、为什么改"的依据

## 什么算 trivial 可以不写

以下情况可以跳过 brief：

- 单行文案 / 注释 / typo 修正
- 本地临时验证脚本（不提交的那类）
- 依赖版本小升（不影响行为）

其余涉及行为、协议、状态机、UI 结构、外部集成的任何改动，都应当先出 brief。
