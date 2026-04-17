# Changelog

本文件记录 Poke Poke 的**行为契约变更**，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## 写入原则

只有以下改动需要在这里追加记录（其余改动靠 git log 即可）：

- Hook 协议 / 安装 / 卸载行为变化
- Session 状态机（状态枚举、迁移规则、TTL、reap 行为）
- Popup 触发 / 关闭 / 抑制规则
- 外部配置文件写入规则（`~/.claude/settings.json`、`~/.codex/config.toml`、`<workspace>/.cursor/hooks.json`）
- HTTP 接口（`/notify` 入参 / 响应 / 端口策略）
- 设置项 schema 变化（新增、删除、默认值调整）

文案格式：一行一条，中文描述行为变化，必要时在括号里关联 brief 或 PR。

## 类别

- `Added`：新增行为
- `Changed`：已有行为的变更
- `Deprecated`：即将废弃
- `Removed`：已移除
- `Fixed`：修复（带行为修正）
- `Security`：安全相关

---

## [Unreleased]

### Added

- （预留）

### Changed

- （预留）

### Fixed

- （预留）

---

## 历史

尚未发布正式版本。当前仓库处于行为契约梳理阶段，参见 `docs/product-spec.md` 与 `docs/reliability-todo.md`。
