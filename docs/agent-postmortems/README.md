# Agent Postmortems

本目录归档 AI Agent 在本仓库中**跑偏**的事后复盘，用于持续校准规则。

## 什么时候写

下列任一情况发生后，都应当写一份 postmortem：

- Agent 扩题（超出 task brief 中的 `Goal` / `Owned Files`）
- 两个 Agent 写集冲突、互相覆盖
- Agent 改动与 `docs/product-spec.md` 或 `AGENTS.md` 的意图不一致
- Agent 声称已完成，但验证门禁未跑或跑失败
- Agent 误删 / 误改了 `Forbidden Files`
- Agent 把"顺手优化"混进行为修正提交里

不是为了追责，是为了把"人工记忆"沉淀成"规则可检测"。

## 命名规则

```
YYYYMMDD-<slug>.md
```

示例：`20260417-popup-refactor-scope-creep.md`

## 文件结构（三段式，简短即可）

```markdown
# <标题>

## 现象
（Agent 做了什么 / 产生了什么偏差，一到两段）

## 根因
（为什么会发生：规则缺失 / brief 不清 / 上下文冲突 / 验证缺位 / 其他）

## 规则调整
（从本次复盘得出的、对仓库规则/模板/门禁的**具体**改动。
 必须落到某个文件的某一段，否则等于没改。）
- [ ] AGENTS.md §X：新增/修改 ...
- [ ] docs/templates/agent-task-brief.md：补充 ...
- [ ] 其他：...
```

## 与其他约束的关系

- `AGENTS.md §5 反跑偏规则`：定义"应该怎样"
- `docs/briefs/*`：定义"本次想做什么"
- `docs/agent-postmortems/*`（本目录）：记录"实际跑偏了什么"，反馈回前两者

随着 postmortem 增加，`AGENTS.md` 会被逐步精化；这是唯一真正能让规则变聪明的循环。
