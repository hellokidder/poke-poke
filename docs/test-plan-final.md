# Poke Poke 测试方案（定稿）

> 基于 Claude Code / Codex / Composer 三方评审共识。原始讨论记录见 `docs/test-plan.md`。

---

## 一、基础设施搭建

### Rust 后端

- **框架**：Rust 内置 `#[cfg(test)]` + `#[test]`，无需额外测试框架
- **新增依赖**（`src-tauri/Cargo.toml`）：
  ```toml
  [dev-dependencies]
  tempfile = "3"
  ```
- **测试位置**：各源文件末尾 `#[cfg(test)] mod tests` 块
- **平台约束**：涉及 macOS 系统路径的测试加 `#[cfg(target_os = "macos")]`
- **运行**：`cd src-tauri && cargo test`

### TypeScript 前端

- **框架**：Vitest（与 Vite 原生集成）
- **新增依赖**：
  ```bash
  pnpm add -D vitest jsdom @testing-library/react @testing-library/jest-dom
  ```
- **新增配置**：`vitest.config.ts`（jsdom 环境）
- **全局 Mock**：`src/test/setup.ts` mock `@tauri-apps/api/core`（invoke）和 `@tauri-apps/api/event`（listen）
- **新增脚本**（`package.json`）：`"test": "vitest run"`, `"test:watch": "vitest"`
- **运行**：`pnpm test`

---

## 二、实施阶段

| Phase | 内容 | 说明 |
|-------|------|------|
| **1** | `sessions.rs` 状态机 + `http_server.rs` 弹窗决策纯函数 | 最高 ROI，核心业务逻辑 |
| **2** | `hook.rs`（辅助函数 + payload 构建）+ `settings.rs` | 第二核心链路 |
| **3** | 前端纯函数测试（SourceIcon / Settings / Panel / i18n） | 零摩擦即可跑通 |
| **4** | `commands.rs` 择要副作用测试 + 前端组件级测试 | 需 Tauri harness + mock 基建 |
| **5（可选）** | 手工冒烟清单 | 不可自动化的产品级验证 |

---

## 三、Phase 1 — 核心业务逻辑

### 3.1 sessions.rs — 状态机（Task C 后四态 + 探活）

文件：`src-tauri/src/sessions.rs`
测试模式：`tempfile::tempdir()` 创建隔离 SessionStore

#### upsert_session() 状态转换

| # | 场景 | 预期 |
|---|------|------|
| 1 | 新 session 插入 | `is_new == true`，列表新增一条记录 |
| 2 | 同 `task_id` 再次 upsert | `is_new == false`, 只有一条记录 |
| 3 | Running → Idle | 状态更新为 `Idle` |
| 3b | Running → LastFailed（含 failure_reason） | 状态更新为 `LastFailed`，`failure_reason` 被保存 |
| 4 | Running → Pending | 状态更新为 `Pending` |
| 5 | Pending → Running | 状态更新为 `Running` |
| 6 | Idle → Running | 状态更新为 `Running`（新一轮） |
| 6b | LastFailed → Running | 状态更新为 `Running`，旧 `failure_reason` 被清除 |
| 7 | Running → Running | 状态保持 `Running` |
| 8 | `source: None` 更新 | 不覆盖已有 source |
| 9 | `terminal_tty: None` 更新 | 不覆盖已有 tty |
| 10 | `prev_status` 返回值 | 等于更新前的状态 |
| 10b | 非 LastFailed 收到 `failure_reason` | 字段被忽略，不写入 |
| 10c | 老数据 serde alias | `"success"` → `Idle`，`"failure"` → `LastFailed` |

#### ~~cleanup_expired()~~（Task C 已移除）

TTL 已删除；相关测试不再适用。Session 回收改由 `lib.rs` 的启动 reap + 高频探活驱动。

#### lib.rs 探活（建议用集成测试或手工冒烟覆盖）

| # | 场景 | 预期 |
|---|------|------|
| 11 | `is_session_alive` — source=claude-code、TTY 上 claude 进程在 | true |
| 12 | `is_session_alive` — source=codex、TTY 上无 codex 进程 | false |
| 13 | `is_session_alive` — source=cursor、Cursor app 运行中 | true |
| 14 | `is_session_alive` — source 未知 / CLI 缺 TTY | false（宁可误清） |
| 15 | 启动 reap | sessions.json 中宿主已死的 session 启动后立即消失 |
| 16 | Grace period 2 次 miss | 单次 miss 保留，连续 2 次 miss 触发 remove |

> 注：进程探活（pgrep）不是纯函数，集成测试或手工冒烟更合适；纯单测只测 `is_session_alive` 的分支路由逻辑。

#### 持久化

| # | 场景 | 预期 |
|---|------|------|
| 17 | save → load 往返 | 数据一致 |
| 18 | 文件不存在 | 空列表，不 panic |
| 19 | 文件内容损坏 | 空列表，不 panic |

### 3.2 http_server.rs — 弹窗决策纯函数

文件：`src-tauri/src/http_server.rs`

**前置重构**：从 `handle_notify` 中提取两个纯函数：

```rust
pub fn should_close_popup(is_new: bool, status: &SessionStatus, prev: Option<&SessionStatus>) -> bool
pub fn should_show_popup(status: &SessionStatus, prev: Option<&SessionStatus>, is_new: bool) -> bool
```

#### should_close_popup

| # | 场景 | 预期 |
|---|------|------|
| 20 | 新 session（`is_new == true`） | → false（新 session 无旧弹窗可关） |
| 21 | 已有 session，Running，prev = Idle | → true（新一轮开始，关旧 idle 弹窗） |
| 21b | 已有 session，Running，prev = LastFailed | → true（新一轮开始，关旧 last_failed 弹窗） |
| 22 | 已有 session，Running，prev = Pending | → true（用户批准权限，关弹窗） |
| 23 | 已有 session，Running，prev = Running | → false（无状态变化） |

#### should_show_popup

| # | 场景 | 预期 |
|---|------|------|
| 24 | status = Idle，prev = Running | → true（stage-ending 弹通知） |
| 24b | status = LastFailed，prev = Running | → true（stage-ending 弹通知） |
| 24c | status = Idle，prev = Idle | → false（同状态 upsert 不重复弹） |
| 24d | status = LastFailed，prev = LastFailed | → false |
| 25 | status = Pending，prev = Running | → true（权限等待弹通知） |
| 26 | status = Running，prev = Pending | → false（恢复运行不弹） |
| 27 | 新 session，status = Pending | → true（新建即待批准） |
| 27b | 新 session，status = Idle / LastFailed | → true（新建视为 stage-ending） |

---

## 四、Phase 2 — Hook 链路 + 设置

### 4.1 bin/hook.rs — 事件解析 + payload 构建（19 个测试）

文件：`src-tauri/src/bin/hook.rs`

#### detect_source()（5 个）

| # | 输入 JSON | 预期 |
|---|-----------|------|
| 25 | 含 `workspace_roots` | → Cursor |
| 26 | 含 `workspaceRoots`（驼峰） | → Cursor |
| 27 | 含 `turn_id` | → Codex |
| 28 | 无特殊字段 | → ClaudeCode |
| 29 | 同时含 `workspace_roots` + `turn_id` | → Cursor（优先） |

#### normalize_event()（4 个）

| # | 输入 | 预期 |
|---|------|------|
| 30 | `sessionStart` | → `SessionStart` |
| 31 | `beforeSubmitPrompt` | → `UserPromptSubmit` |
| 32 | `stop` | → `Stop` |
| 33 | `Notification`（已 PascalCase） | → 透传 |

#### 辅助函数（6 个）

| # | 函数 | 场景 | 预期 |
|---|------|------|------|
| 34 | `pick_str` | 多个候选 | 选第一个非空值 |
| 35 | `pick_str` | 有空字符串 | 跳过空串 |
| 36 | `pick_str` | 全缺失 | → None |
| 37 | `contains_poke_hook` | 命令含 poke-hook | → true |
| 38 | `contains_poke_hook` | 命令不含 | → false |
| 39 | `flag_path` | 正常输入 | 正确拼接 `/tmp/pokepoke-{id}.{flag}` |

#### build_payload() 主链路 smoke test（4 个）

**前置重构**：提取 `fn build_payload(event: &str, json: &Value, source: Source) -> Option<NotifyPayload>`

| # | 场景 | 预期 |
|---|------|------|
| 40 | Cursor `stop` + `hookStatus:"completed"` | status = idle |
| 40b | CC `StopFailure` + reason | status = last_failed + `failure_reason` 透传 |
| 41 | CC `Notification` | source = `claude-code`，保留原始 message |
| 42 | Codex `Stop` | task_id 前缀为 `codex-`，status = idle |
| 43 | task_id 生成 | source prefix + session_id 拼接正确 |

### 4.2 settings.rs — 设置存储（4 个测试）

文件：`src-tauri/src/settings.rs`

| # | 场景 | 预期 |
|---|------|------|
| 44 | 默认值 | `alert_sound == "system:Glass"`, `locale == "zh"`, `auto_start == false`, `panel_shortcut == None`（**不再有** `popup_timeout` / `session_retention_hours`） |
| 45 | update + save → load | 往返一致 |
| 46 | 部分 JSON（缺失字段） | 缺失字段用默认值补齐 |
| 47 | 损坏文件 | 加载默认值，不 panic |

---

## 五、Phase 3 — 前端纯函数测试

### 5.1 SourceIcon 纯函数（4 个测试）

文件：`src/icons/SourceIcon.test.ts`
前置：从 `SourceIcon.tsx` 导出 `hashColor` 和 `getExpression`

| # | 场景 | 预期 |
|---|------|------|
| 48 | `hashColor` 确定性 | 同输入同输出（固定样例快照） |
| 49 | `hashColor` 输出格式 | 匹配 `hsl(H, 65%, 60%)` |
| 50 | `getExpression` 四种状态 | `running` / `pending` / `idle` / `last_failed` 各返回 16 个像素元组 |
| 51 | `getExpression("pending")` | 使用白色 `#FFFFFF` 画眼睛 |
| 51b | `getExpression("idle")` 与 `last_failed` | 分别复用旧 `success` / `failure` 表情不变 |

### 5.2 SettingsWindow 键盘解析（8 个测试）

文件：`src/settings/SettingsWindow.test.ts`
前置：从 `SettingsWindow.tsx` 导出 `codeToKey`, `eventToShortcut`, `formatShortcut`

| # | 场景 | 预期 |
|---|------|------|
| 52 | `codeToKey({code:"KeyA"})` | → `"A"` |
| 53 | `codeToKey({code:"Digit5"})` | → `"5"` |
| 54 | `codeToKey({code:"F12"})` | → `"F12"` |
| 55 | `codeToKey({code:"ArrowUp"})` | → `"Up"` |
| 56 | `eventToShortcut({metaKey:true, code:"KeyK"})` | → `"CmdOrCtrl+K"` |
| 57 | `eventToShortcut({metaKey:true, shiftKey:true, code:"KeyP"})` | → `"CmdOrCtrl+Shift+P"` |
| 58 | `eventToShortcut` 纯修饰键 | → `null` |
| 59 | `formatShortcut("CmdOrCtrl+Shift+K")` | → `"⌘ ⇧ K"` |

### 5.3 SessionPanel 工具函数（7 个测试）

文件：`src/panel/SessionPanel.test.ts`
前置：从 `SessionPanel.tsx` 导出 `projectName`, `sourceLabel`, `workspacePath`, `isActive`

| # | 场景 | 预期 |
|---|------|------|
| 60 | `projectName({title:"CC: my-proj"})` | → `"my-proj"` |
| 61 | `projectName({title:"plain"})` | → `"plain"` |
| 62 | `sourceLabel("claude-code")` | → `"Claude Code"` |
| 63 | `sourceLabel(null)` | → `""` |
| 64 | `workspacePath` 缩写 `/Users/xxx/` | → `~/` |
| 65 | `isActive({status:"running"})` | → `true` |
| 65b | `isActive({status:"pending"})` | → `true` |
| 66 | `isActive({status:"idle"})` | → `false` |
| 66b | `isActive({status:"last_failed"})` | → `false` |

### 5.4 i18n 翻译（3 个测试）

文件：`src/i18n/i18n.test.ts`

| # | 场景 | 预期 |
|---|------|------|
| 67 | zh 和 en 的 key 集合 | 完全一致 |
| 68 | 模板变量替换 `{n}` | 正确插值 |
| 69 | 不存在的 key | 返回 key 本身作为 fallback |

---

## 六、Phase 4 — 需要 Mock 基建的测试

### 6.1 commands.rs 择要副作用测试

依赖 Tauri test harness（`tauri::test::mock_builder`）或抽 `CommandDeps` trait 注入。

**重点覆盖：**
- `save_settings` → 验证依次触发 `apply_shortcut` + emit `settings-updated`

**不覆盖：**
- 纯转发命令（`get_sessions`, `get_settings` 等）
- AppleScript 调用（`focus_task_terminal`, `focus_iterm2` 等）

### 6.2 前端组件级测试

依赖 `src/test/setup.ts` mock 基础设施完善后实施。

**高回归点优先：**
- `SessionPanel` 首屏加载 + 监听 `sessions-updated` 刷新
- `App.tsx` 监听 `settings-updated` 后刷新 locale
- `PopupWindow` 点击后先 focus 再 close

### 6.3 sound.rs 路径解析（3 个测试）

**前置重构**：提取 `fn resolve_sound_path(sound: &str) -> Option<String>` 纯函数
**平台约束**：`#[cfg(target_os = "macos")]`

| # | 输入 | 预期 |
|---|------|------|
| 70 | `"system:Glass"` | → `Some("/System/Library/Sounds/Glass.aiff")` |
| 71 | `"mute"` | → `None` |
| 72 | 未知格式 | → `Some(".../Glass.aiff")`（fallback） |

### 6.4 popup.rs 位置计算（2 个测试）

| # | 场景 | 预期 |
|---|------|------|
| 73 | `target_y(0)` | `== 12.0 + 30.0` |
| 74 | `target_y(1)` | `== 12.0 + 30.0 + 150.0 + 8.0` |

---

## 七、Phase 5（可选） — 手工冒烟清单

发版前必须人工验证一遍的检查项，不可自动化：

- [ ] 托盘图标显示正常，右键菜单展示连接状态（✓ / Connect）
- [ ] 一键安装 CC hook → 托盘菜单状态刷新
- [ ] 一键卸载 CC hook → 状态回退
- [ ] 一键安装 / 卸载 Codex hook
- [ ] Popup 弹出位置正确（右上角向下堆叠）
- [ ] Popup 点击 → 聚焦对应终端（iTerm2 / Terminal.app）
- [ ] Popup 用户切到对应终端后自动消失
- [ ] 设置页修改音效 → 预览播放 → 重启后持久化
- [ ] 设置页修改语言 → 全局 UI 切换
- [ ] 快捷键录入 → 全局生效 → 切换设置窗口可见性
- [ ] 开机自启动开关 → 重启 macOS 验证
- [ ] 通知面板按注册时间先后排列（先注册的在上）
- [ ] Idle / LastFailed 会话显示删除按钮，点击删除生效
- [ ] 启动 reap：预置一条 `source` 未知或宿主已死的 session 到 `sessions.json`，启动后立即消失
- [ ] CLI agent 探活：关闭终端后，5~10s 内对应 session 从面板消失
- [ ] Cursor 探活：退出整个 Cursor app 后，5~10s 内对应 session 从面板消失
- [ ] StopFailure i18n：CC 命中 API 错误时，popup 文案走 `failure.<reason>` 翻译
- [ ] port 9876 被占用时 fallback 到 9877

---

## 八、明确不测的部分

| 模块 | 原因 |
|------|------|
| `tray.rs` | 纯 Tauri 窗口 / 菜单管理，无可提取纯逻辑 |
| `shortcut.rs` | 3 行胶水代码 |
| `popup.rs` 窗口管理 | `show_popup` / `close_popup` 依赖 AppHandle（仅测 `target_y` 数学） |
| AppleScript 函数 | 平台相关，非确定性，需要终端运行状态 |
| `hook.rs` I/O | `post_notify` / `hook_mode` / flag 文件 — 留给 Phase 5 冒烟 |
| `commands.rs` 纯转发 | `get_sessions` 等直接调 store 方法，store 测完即覆盖 |
| React 组件渲染（Phase 3 阶段） | Phase 3 只测纯函数；组件级测试推到 Phase 4 |

---

## 九、文件变更清单

### 修改

| 文件 | 变更 |
|------|------|
| `src-tauri/Cargo.toml` | 添加 `[dev-dependencies]` |
| `src-tauri/src/sessions.rs` | 添加 `#[cfg(test)] mod tests` |
| `src-tauri/src/http_server.rs` | 提取 `should_close_popup` / `should_show_popup` + tests |
| `src-tauri/src/settings.rs` | 添加 `#[cfg(test)] mod tests` |
| `src-tauri/src/sound.rs` | 提取 `resolve_sound_path` + tests（macOS only） |
| `src-tauri/src/popup.rs` | 添加 `#[cfg(test)] mod tests`（仅 `target_y`） |
| `src-tauri/src/bin/hook.rs` | 提取 `build_payload` + 添加 tests |
| `src/icons/SourceIcon.tsx` | 导出 `hashColor`, `getExpression` |
| `src/settings/SettingsWindow.tsx` | 导出 `codeToKey`, `eventToShortcut`, `formatShortcut` |
| `src/panel/SessionPanel.tsx` | 导出 `projectName`, `sourceLabel`, `workspacePath`, `isActive` |
| `package.json` | 添加 `test` / `test:watch` 脚本 |

### 新增

| 文件 | 用途 |
|------|------|
| `vitest.config.ts` | Vitest 配置 |
| `src/test/setup.ts` | Tauri API 全局 mock |
| `src/icons/SourceIcon.test.ts` | 像素图标纯函数测试 |
| `src/settings/SettingsWindow.test.ts` | 键盘快捷键解析测试 |
| `src/panel/SessionPanel.test.ts` | 面板工具函数测试 |
| `src/i18n/i18n.test.ts` | 国际化翻译测试 |

---

## 十、验证命令

```bash
# Rust 全量测试
cd src-tauri && cargo test

# 前端全量测试
pnpm test

# 一键全量
cd src-tauri && cargo test && cd .. && pnpm test
```

---

## 附：测试统计

| Phase | Rust | TypeScript | 合计 |
|-------|------|------------|------|
| 1 | 24 | — | 24 |
| 2 | 23 | — | 23 |
| 3 | — | 22 | 22 |
| 4 | 5+ | 3+ | 8+ |
| **合计** | **52+** | **25+** | **77+** |
