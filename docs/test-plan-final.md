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
| **1** | `notifications.rs` 状态机 + `http_server.rs` 弹窗决策纯函数 | 最高 ROI，核心业务逻辑 |
| **2** | `hook.rs`（辅助函数 + payload 构建）+ `settings.rs` | 第二核心链路 |
| **3** | 前端纯函数测试（SourceIcon / Settings / Panel / i18n） | 零摩擦即可跑通 |
| **4** | `commands.rs` 择要副作用测试 + 前端组件级测试 | 需 Tauri harness + mock 基建 |
| **5（可选）** | 手工冒烟清单 | 不可自动化的产品级验证 |

---

## 三、Phase 1 — 核心业务逻辑

### 3.1 notifications.rs — 状态机（26 个测试）

文件：`src-tauri/src/notifications.rs`
测试模式：`tempfile::tempdir()` 创建隔离 TaskStore

#### upsert_task() 状态转换（12 个）

| # | 场景 | 预期 |
|---|------|------|
| 1 | 新任务插入 | `is_new == true`, `read == false` |
| 2 | 同 `task_id` 再次 upsert | `is_new == false`, 只有一条记录 |
| 3 | Running → Success | `read = false`（终态转换标记未读） |
| 4 | Running → Failed | `read = false` |
| 5 | Running → Pending | `read = false`（权限等待，需要关注） |
| 6 | Pending → Running | `read = true`（用户恢复，自动已读） |
| 7 | Success → Running | `read = true` |
| 8 | Failed → Running | `read = true` |
| 9 | Running → Running | `read` 不变（无匹配分支） |
| 10 | `source: None` 更新 | 不覆盖已有 source |
| 11 | `terminal_tty: None` 更新 | 不覆盖已有 tty |
| 12 | `prev_status` 返回值 | 等于更新前的状态 |

#### unread_count()（3 个）

| # | 场景 | 预期 |
|---|------|------|
| 13 | 终态 + Pending 未读 | 计入 |
| 14 | 已读任务 | 不计入 |
| 15 | Running 状态 | 永远不计入（即使 `read == false`） |

#### mark_read / mark_all_read（2 个）

| # | 场景 | 预期 |
|---|------|------|
| 16 | `mark_read` 存在 / 不存在 | 返回 true / false |
| 17 | `mark_all_read` | 只标记终态和 Pending（不标记 Running） |

#### cleanup_expired()（3 个）

| # | 场景 | 预期 |
|---|------|------|
| 18 | 过期终态任务 | 被删除 |
| 19 | Running / Pending（即使过期） | 保留 |
| 20 | 未过期终态 | 保留 |

#### reap_stale_sessions()（3 个）

| # | 场景 | 预期 |
|---|------|------|
| 21 | tty 路径不存在 | → Failed + "Session lost" |
| 22 | 非 Running 任务 | 跳过 |
| 23 | tty 为 None | 跳过 |

#### 持久化（3 个）

| # | 场景 | 预期 |
|---|------|------|
| 24 | save → load 往返 | 数据一致 |
| 25 | 文件不存在 | 空列表，不 panic |
| 26 | 文件内容损坏 | 空列表，不 panic |

### 3.2 http_server.rs — 弹窗决策纯函数（8 个测试）

文件：`src-tauri/src/http_server.rs`

**前置重构**：从 `handle_notify` 中提取两个纯函数：

```rust
pub fn should_close_popup(is_new: bool, status: &TaskStatus, prev: Option<&TaskStatus>) -> bool
pub fn should_show_popup(status: &TaskStatus, prev: Option<&TaskStatus>, is_new: bool) -> bool
```

#### should_close_popup（4 个）

| # | 场景 | 预期 |
|---|------|------|
| 27 | 新任务（`is_new == true`） | → false（新任务无旧弹窗可关） |
| 28 | 已有任务，Running，prev = Success | → true（终态恢复运行，关旧弹窗） |
| 29 | 已有任务，Running，prev = Pending | → true（用户批准权限，关弹窗） |
| 30 | 已有任务，Running，prev = Running | → false（无状态变化） |

#### should_show_popup（4 个）

| # | 场景 | 预期 |
|---|------|------|
| 31 | status = Success，prev = Running | → true（终态转换弹通知） |
| 32 | status = Pending，prev = Running | → true（权限等待弹通知） |
| 33 | status = Running，prev = Pending | → false（恢复运行不弹） |
| 34 | 新任务，status = Pending | → true（新建即待批准） |

---

## 四、Phase 2 — Hook 链路 + 设置

### 4.1 bin/hook.rs — 事件解析 + payload 构建（19 个测试）

文件：`src-tauri/src/bin/hook.rs`

#### detect_source()（5 个）

| # | 输入 JSON | 预期 |
|---|-----------|------|
| 35 | 含 `workspace_roots` | → Cursor |
| 36 | 含 `workspaceRoots`（驼峰） | → Cursor |
| 37 | 含 `turn_id` | → Codex |
| 38 | 无特殊字段 | → ClaudeCode |
| 39 | 同时含 `workspace_roots` + `turn_id` | → Cursor（优先） |

#### normalize_event()（4 个）

| # | 输入 | 预期 |
|---|------|------|
| 40 | `sessionStart` | → `SessionStart` |
| 41 | `beforeSubmitPrompt` | → `UserPromptSubmit` |
| 42 | `stop` | → `Stop` |
| 43 | `Notification`（已 PascalCase） | → 透传 |

#### 辅助函数（6 个）

| # | 函数 | 场景 | 预期 |
|---|------|------|------|
| 44 | `pick_str` | 多个候选 | 选第一个非空值 |
| 45 | `pick_str` | 有空字符串 | 跳过空串 |
| 46 | `pick_str` | 全缺失 | → None |
| 47 | `contains_poke_hook` | 命令含 poke-hook | → true |
| 48 | `contains_poke_hook` | 命令不含 | → false |
| 49 | `flag_path` | 正常输入 | 正确拼接 `/tmp/pokepoke-{id}.{flag}` |

#### build_payload() 主链路 smoke test（4 个）

**前置重构**：提取 `fn build_payload(event: &str, json: &Value, source: Source) -> Option<NotifyPayload>`

| # | 场景 | 预期 |
|---|------|------|
| 50 | Cursor `stop` + `hookStatus:"success"` | status = success |
| 51 | CC `Notification` | source = `claude-code`，保留原始 message |
| 52 | Codex `Stop` | task_id 前缀为 `codex-` |
| 53 | task_id 生成 | source prefix + session_id 拼接正确 |

### 4.2 settings.rs — 设置存储（4 个测试）

文件：`src-tauri/src/settings.rs`

| # | 场景 | 预期 |
|---|------|------|
| 54 | 默认值 | `alert_sound == "system:Glass"`, `locale == "zh"`, `session_retention_hours == 24`, `popup_timeout == 0` |
| 55 | update + save → load | 往返一致 |
| 56 | 部分 JSON（缺失字段） | 缺失字段用默认值补齐 |
| 57 | 损坏文件 | 加载默认值，不 panic |

---

## 五、Phase 3 — 前端纯函数测试

### 5.1 SourceIcon 纯函数（5 个测试）

文件：`src/icons/SourceIcon.test.ts`
前置：从 `SourceIcon.tsx` 导出 `hashColor` 和 `getExpression`

| # | 场景 | 预期 |
|---|------|------|
| 58 | `hashColor` 确定性 | 同输入同输出（固定样例快照） |
| 59 | `hashColor` 输出格式 | 匹配 `hsl(H, 65%, 60%)` |
| 60 | `getExpression` 四种状态 | 各返回 16 个像素元组 |
| 61 | `getExpression("pending")` | 使用白色 `#FFFFFF` 画眼睛 |
| 62 | `getExpression("failed")` | 使用 X 形对角线眼睛 |

### 5.2 SettingsWindow 键盘解析（8 个测试）

文件：`src/settings/SettingsWindow.test.ts`
前置：从 `SettingsWindow.tsx` 导出 `codeToKey`, `eventToShortcut`, `formatShortcut`

| # | 场景 | 预期 |
|---|------|------|
| 63 | `codeToKey({code:"KeyA"})` | → `"A"` |
| 64 | `codeToKey({code:"Digit5"})` | → `"5"` |
| 65 | `codeToKey({code:"F12"})` | → `"F12"` |
| 66 | `codeToKey({code:"ArrowUp"})` | → `"Up"` |
| 67 | `eventToShortcut({metaKey:true, code:"KeyK"})` | → `"CmdOrCtrl+K"` |
| 68 | `eventToShortcut({metaKey:true, shiftKey:true, code:"KeyP"})` | → `"CmdOrCtrl+Shift+P"` |
| 69 | `eventToShortcut` 纯修饰键 | → `null` |
| 70 | `formatShortcut("CmdOrCtrl+Shift+K")` | → `"⌘ ⇧ K"` |

### 5.3 NotificationPanel 工具函数（7 个测试）

文件：`src/panel/NotificationPanel.test.ts`
前置：从 `NotificationPanel.tsx` 导出 `projectName`, `sourceLabel`, `workspacePath`, `isActive`

| # | 场景 | 预期 |
|---|------|------|
| 71 | `projectName({title:"CC: my-proj"})` | → `"my-proj"` |
| 72 | `projectName({title:"plain"})` | → `"plain"` |
| 73 | `sourceLabel("claude-code")` | → `"Claude Code"` |
| 74 | `sourceLabel(null)` | → `""` |
| 75 | `workspacePath` 缩写 `/Users/xxx/` | → `~/` |
| 76 | `isActive({status:"running"})` | → `true` |
| 77 | `isActive({status:"success"})` | → `false` |

### 5.4 i18n 翻译（3 个测试）

文件：`src/i18n/i18n.test.ts`

| # | 场景 | 预期 |
|---|------|------|
| 78 | zh 和 en 的 key 集合 | 完全一致 |
| 79 | 模板变量替换 `{n}` | 正确插值 |
| 80 | 不存在的 key | 返回 key 本身作为 fallback |

---

## 六、Phase 4 — 需要 Mock 基建的测试

### 6.1 commands.rs 择要副作用测试

依赖 Tauri test harness（`tauri::test::mock_builder`）或抽 `CommandDeps` trait 注入。

**重点覆盖：**
- `save_settings` → 验证依次触发 `apply_shortcut` + emit `settings-updated`
- `close_popup_window` → 验证 `mark_read` + `close_popup` + `update_tray_icon` 编排顺序

**不覆盖：**
- 纯转发命令（`get_notifications`, `get_settings` 等）
- AppleScript 调用（`focus_task_terminal`, `focus_iterm2` 等）

### 6.2 前端组件级测试

依赖 `src/test/setup.ts` mock 基础设施完善后实施。

**高回归点优先：**
- `NotificationPanel` 首屏加载 + 监听 `notifications-updated` 刷新
- `App.tsx` 监听 `settings-updated` 后刷新 locale
- `PopupWindow` 点击后先 focus 再 close

### 6.3 sound.rs 路径解析（3 个测试）

**前置重构**：提取 `fn resolve_sound_path(sound: &str) -> Option<String>` 纯函数
**平台约束**：`#[cfg(target_os = "macos")]`

| # | 输入 | 预期 |
|---|------|------|
| 81 | `"system:Glass"` | → `Some("/System/Library/Sounds/Glass.aiff")` |
| 82 | `"mute"` | → `None` |
| 83 | 未知格式 | → `Some(".../Glass.aiff")`（fallback） |

### 6.4 popup.rs 位置计算（2 个测试）

| # | 场景 | 预期 |
|---|------|------|
| 84 | `target_y(0)` | `== 12.0 + 30.0` |
| 85 | `target_y(1)` | `== 12.0 + 30.0 + 150.0 + 8.0` |

---

## 七、Phase 5（可选） — 手工冒烟清单

发版前必须人工验证一遍的检查项，不可自动化：

- [ ] 托盘图标显示正常，右键菜单展示连接状态（✓ / Connect）
- [ ] 一键安装 CC hook → 托盘菜单状态刷新
- [ ] 一键卸载 CC hook → 状态回退
- [ ] 一键安装 / 卸载 Codex hook
- [ ] Popup 弹出位置正确（右下角堆叠）
- [ ] Popup 点击 → 聚焦对应终端（iTerm2 / Terminal.app）
- [ ] Popup 用户切到对应终端后自动消失
- [ ] Popup 超时自动消失（设置非 0 时）
- [ ] 设置页修改音效 → 预览播放 → 重启后持久化
- [ ] 设置页修改语言 → 全局 UI 切换
- [ ] 快捷键录入 → 全局生效 → 切换面板可见性
- [ ] 开机自启动开关 → 重启 macOS 验证
- [ ] 通知面板按时间倒序排列
- [ ] 终态任务显示删除按钮，点击删除生效
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
| `commands.rs` 纯转发 | `get_notifications` 等直接调 store 方法，store 测完即覆盖 |
| React 组件渲染（Phase 3 阶段） | Phase 3 只测纯函数；组件级测试推到 Phase 4 |

---

## 九、文件变更清单

### 修改

| 文件 | 变更 |
|------|------|
| `src-tauri/Cargo.toml` | 添加 `[dev-dependencies]` |
| `src-tauri/src/notifications.rs` | 添加 `#[cfg(test)] mod tests` |
| `src-tauri/src/http_server.rs` | 提取 `should_close_popup` / `should_show_popup` + tests |
| `src-tauri/src/settings.rs` | 添加 `#[cfg(test)] mod tests` |
| `src-tauri/src/sound.rs` | 提取 `resolve_sound_path` + tests（macOS only） |
| `src-tauri/src/popup.rs` | 添加 `#[cfg(test)] mod tests`（仅 `target_y`） |
| `src-tauri/src/bin/hook.rs` | 提取 `build_payload` + 添加 tests |
| `src/icons/SourceIcon.tsx` | 导出 `hashColor`, `getExpression` |
| `src/settings/SettingsWindow.tsx` | 导出 `codeToKey`, `eventToShortcut`, `formatShortcut` |
| `src/panel/NotificationPanel.tsx` | 导出 `projectName`, `sourceLabel`, `workspacePath`, `isActive` |
| `package.json` | 添加 `test` / `test:watch` 脚本 |

### 新增

| 文件 | 用途 |
|------|------|
| `vitest.config.ts` | Vitest 配置 |
| `src/test/setup.ts` | Tauri API 全局 mock |
| `src/icons/SourceIcon.test.ts` | 像素图标纯函数测试 |
| `src/settings/SettingsWindow.test.ts` | 键盘快捷键解析测试 |
| `src/panel/NotificationPanel.test.ts` | 面板工具函数测试 |
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
| 1 | 34 | — | 34 |
| 2 | 23 | — | 23 |
| 3 | — | 23 | 23 |
| 4 | 5+ | 3+ | 8+ |
| **合计** | **62+** | **26+** | **85+** |
