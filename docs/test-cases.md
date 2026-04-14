# Poke Poke 测试用例文档

> 本文档为 Poke Poke 项目的完整测试用例集，涵盖单元测试、集成测试和端到端手动测试场景。
> 与 `test-plan.md`（代码层面实施规划）互补，本文档侧重**功能验证和行为描述**。

---

## 项目概览

Poke Poke 是一个基于 Tauri + React + TypeScript 的 macOS 桌面通知中心，用于接收 AI 编程助手（Claude Code / Codex CLI / Cursor）的 Hook 事件，并以系统托盘 + 弹窗的方式展示任务状态变化。

**核心模块：**

| 模块 | 文件 | 职责 |
|------|------|------|
| 任务存储 | `notifications.rs` | 任务数据模型、状态机、持久化 |
| HTTP 服务 | `http_server.rs` | 接收外部 Hook 通知的 REST API |
| Hook 二进制 | `bin/hook.rs` | 通用 Hook 处理器（stdin → HTTP） |
| 弹窗管理 | `popup.rs` | 弹窗创建、定位、自动消失 |
| 系统托盘 | `tray.rs` | 托盘图标、菜单、面板窗口 |
| 设置 | `settings.rs` | 用户配置持久化 |
| 音效 | `sound.rs` | macOS 系统声音播放 |
| 快捷键 | `shortcut.rs` | 全局键盘快捷键注册 |
| 通知面板 | `NotificationPanel.tsx` | 通知列表 UI |
| 弹窗窗口 | `PopupWindow.tsx` | 单条弹窗通知 UI |
| 设置窗口 | `SettingsWindow.tsx` | 设置界面 UI |
| 国际化 | `i18n/` | 中英文翻译 |
| 图标 | `SourceIcon.tsx` | 像素风怪兽图标 |

---

## 一、任务存储模块 (notifications.rs)

### 1.1 任务插入 (upsert_task)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-001 | 新任务插入 | TaskStore 为空 | 调用 `upsert_task` 传入新 task_id | `is_new == true`，任务列表长度为 1 |
| TC-N-002 | 相同 task_id 再次插入 | 已存在一条 task_id="abc" 的任务 | 用相同 task_id 再次调用 `upsert_task` | `is_new == false`，任务列表仍为 1 条记录（更新而非新增） |
| TC-N-003 | 不同 task_id 插入 | 已存在一条任务 | 用不同 task_id 调用 `upsert_task` | `is_new == true`，任务列表长度为 2 |
| TC-N-004 | 新任务插入顺序 | 已存在任务列表 | 插入新任务 | 新任务插入到列表头部（`tasks[0]`） |

### 1.2 状态转换

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-010 | Running → Success | 任务状态为 Running | upsert 状态为 Success | 状态更新为 Success |
| TC-N-011 | Running → Failed | 任务状态为 Running | upsert 状态为 Failed | 状态更新为 Failed |
| TC-N-012 | Running → Pending | 任务状态为 Running | upsert 状态为 Pending | 状态更新为 Pending |
| TC-N-013 | Pending → Running | 任务状态为 Pending | upsert 状态为 Running | 状态更新为 Running |
| TC-N-014 | Success → Running | 任务状态为 Success | upsert 状态为 Running | 状态更新为 Running |
| TC-N-015 | Failed → Running | 任务状态为 Failed | upsert 状态为 Running | 状态更新为 Running |
| TC-N-016 | Running → Running | 任务状态为 Running | upsert 仍为 Running | 状态保持 Running |
| TC-N-017 | Success → Success | 任务状态为 Success | upsert 仍为 Success | 状态保持 Success |
| TC-N-018 | source 为 None 时不覆盖 | 任务已有 source="claude-code" | upsert 传 source=None | source 保持 "claude-code" |
| TC-N-019 | terminal_tty 为 None 时不覆盖 | 任务已有 tty="/dev/ttys001" | upsert 传 terminal_tty=None | terminal_tty 保持原值 |
| TC-N-020 | prev_status 返回值 | 任务状态为 Running | upsert 更新为 Success | `prev_status == Some(Running)` |
| TC-N-021 | 新任务 prev_status | 无该 task_id | 首次 upsert | `prev_status == None` |

### 1.3 未读计数 (unread_count) [不纳入当前用例]

当前产品设计不包含未读计数逻辑，`unread_count` 相关测试不纳入当前用例范围。

### 1.4 标记已读 [不纳入当前用例]

当前产品设计不包含 `mark_read` / `mark_all_read` 标记已读逻辑，相关测试不纳入当前用例范围。

### 1.5 删除任务

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-050 | 删除存在的任务 | 存在 id="xxx" | 调用 remove_task("xxx") | 返回 true，列表减少一条 |
| TC-N-051 | 删除不存在的任务 | 无 id="yyy" | 调用 remove_task("yyy") | 返回 false，列表不变 |

### 1.6 过期清理 (cleanup_expired)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-060 | 过期终态被删除 | Success 任务 updated_at 为 25 小时前 | 调用 cleanup_expired(24) | 该任务被删除 |
| TC-N-061 | Running 不被删除 | Running 任务 updated_at 为 100 小时前 | 调用 cleanup_expired(24) | 任务保留（Running 永不清除） |
| TC-N-062 | Pending 不被删除 | Pending 任务 updated_at 为 100 小时前 | 调用 cleanup_expired(24) | 任务保留 |
| TC-N-063 | 未过期终态保留 | Success 任务 updated_at 为 1 小时前 | 调用 cleanup_expired(24) | 任务保留 |

### 1.7 僵死会话收割 (reap_stale_sessions)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-070 | TTY 不存在 → Failed | Running 任务 tty="/dev/ttys999"（不存在的路径） | 调用 reap_stale_sessions | 状态变为 Failed，message="Session lost" |
| TC-N-071 | 非 Running 跳过 | Success 任务 tty 路径不存在 | 调用 reap_stale_sessions | 不受影响 |
| TC-N-072 | tty 为 None 跳过 | Running 任务无 tty | 调用 reap_stale_sessions | 不受影响 |
| TC-N-073 | tty 为空字符串跳过 | Running 任务 tty="" | 调用 reap_stale_sessions | 不受影响 |

### 1.8 持久化

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-N-080 | save → load 往返 | 插入多条任务 | save 后用相同路径 load | 数据完全一致 |
| TC-N-081 | 文件不存在 | file_path 指向不存在的文件 | 调用 load | 返回空列表，不 panic |
| TC-N-082 | 文件内容损坏 | 写入非法 JSON 到文件 | 调用 load | 返回空列表，不 panic |

---

## 二、HTTP 服务模块 (http_server.rs)

### 2.1 POST /notify

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-H-001 | 创建新任务 | 服务运行中 | POST /notify 传入新 task_id | 响应 201 Created，返回任务 JSON |
| TC-H-002 | 更新已有任务 | 已存在 task_id="test-1" | POST /notify 用相同 task_id | 响应 200 OK，任务数据已更新 |
| TC-H-003 | priority 解析 | - | POST priority="high" | 任务 priority 为 High |
| TC-H-004 | priority 默认值 | - | POST 不传 priority | 任务 priority 为 Normal |
| TC-H-005 | status 解析 — running | - | POST status="running" | 任务状态为 Running |
| TC-H-006 | status 解析 — success | - | POST status="success" | 任务状态为 Success |
| TC-H-007 | status 解析 — failed | - | POST status="failed" | 任务状态为 Failed |
| TC-H-008 | status 默认值 | - | POST 不传 status | 任务状态为 Pending |
| TC-H-009 | 必填字段缺失 | - | POST 缺少 title 字段 | 响应 4xx 错误 |

### 2.2 弹窗触发逻辑

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-H-020 | Running → Success 弹窗 | 已有 Running 任务 | POST status="success" | 弹窗弹出 + 播放提示音 |
| TC-H-021 | Running → Failed 弹窗 | 已有 Running 任务 | POST status="failed" | 弹窗弹出 |
| TC-H-022 | Running → Pending 弹窗 | 已有 Running 任务 | POST status="pending" | 弹窗弹出（需要用户审批） |
| TC-H-023 | Pending → Running 关闭弹窗 | 有 Pending 弹窗 | POST status="running" | 已有弹窗关闭（用户恢复操作） |
| TC-H-024 | Success → Running 关闭弹窗 | 有 Success 弹窗 | POST status="running" | 已有弹窗关闭 |
| TC-H-025 | Running → Running 无弹窗 | 已有 Running 任务 | POST 仍为 running | 不弹窗（状态未变化） |
| TC-H-026 | 新任务直接 Success | 无该 task_id | POST 新任务 status="success" | 弹窗弹出（直接创建为终态） |
| TC-H-027 | 终端已聚焦时不弹窗 | 用户正在查看该终端会话 | POST status="success" | 跳过弹窗（用户已在关注） |

### 2.3 GET /notifications

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-H-030 | 获取全部任务 | 存在 3 条任务 | GET /notifications | 返回包含 3 条任务的 JSON 数组 |
| TC-H-031 | 空列表 | 无任务 | GET /notifications | 返回空数组 [] |

### 2.4 POST /notifications/{id}/read [不纳入当前用例]

当前产品设计不包含标记已读接口，`POST /notifications/{id}/read` 相关测试不纳入当前用例范围。

### 2.5 端口回退

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-H-050 | 主端口可用 | 9876 端口空闲 | 启动 HTTP 服务 | 监听 127.0.0.1:9876 |
| TC-H-051 | 主端口占用回退 | 9876 端口被占 | 启动 HTTP 服务 | 自动绑定 127.0.0.1:9877 |
| TC-H-052 | 两个端口都占用 | 9876 + 9877 都被占 | 启动 HTTP 服务 | 输出错误日志，服务不启动（不 panic） |

---

## 三、Hook 二进制模块 (bin/hook.rs)

### 3.1 来源检测 (detect_source)

| 编号 | 用例名称 | 输入 JSON | 预期结果 |
|------|---------|----------|---------|
| TC-K-001 | Cursor — snake_case | `{"workspace_roots": ["/path"]}` | Source::Cursor |
| TC-K-002 | Cursor — camelCase | `{"workspaceRoots": ["/path"]}` | Source::Cursor |
| TC-K-003 | Codex | `{"turn_id": "xxx"}` | Source::Codex |
| TC-K-004 | Claude Code (默认) | `{"session_id": "yyy"}` | Source::ClaudeCode |
| TC-K-005 | Cursor 优先级 | `{"workspace_roots": [], "turn_id": "xxx"}` | Source::Cursor（workspace_roots 优先判断） |

### 3.2 事件名称标准化 (normalize_event)

| 编号 | 用例名称 | 输入 | 预期输出 |
|------|---------|------|---------|
| TC-K-010 | sessionStart | `"sessionStart"` | `"SessionStart"` |
| TC-K-011 | beforeSubmitPrompt | `"beforeSubmitPrompt"` | `"UserPromptSubmit"` |
| TC-K-012 | stop | `"stop"` | `"Stop"` |
| TC-K-013 | sessionEnd | `"sessionEnd"` | `"SessionEnd"` |
| TC-K-014 | 已为 PascalCase | `"Notification"` | `"Notification"`（透传） |
| TC-K-015 | 未知事件 | `"unknownEvent"` | `"unknownEvent"`（透传） |

### 3.3 辅助函数

| 编号 | 用例名称 | 操作 | 预期结果 |
|------|---------|------|---------|
| TC-K-020 | pick_str 多候选 | `pick_str(data, ["a", "b"])` 其中 a 有值 | 返回 a 的值 |
| TC-K-021 | pick_str 跳过空串 | 第一个 key 值为 ""，第二个有值 | 返回第二个 key 的值 |
| TC-K-022 | pick_str 全缺失 | 所有 key 都不存在 | 返回 None |
| TC-K-023 | contains_poke_hook 匹配 | hook command 包含 "poke-hook" | 返回 true |
| TC-K-024 | contains_poke_hook 不匹配 | hook command 为其他值 | 返回 false |
| TC-K-025 | flag_path 拼接 | `flag_path("cc-abc", "registered")` | `/tmp/pokepoke-cc-abc.registered` |
| TC-K-026 | PrintOnDrop 保证输出 | hook_mode 正常退出 | stdout 一定输出 `{}` |
| TC-K-027 | PrintOnDrop 提前返回 | hook_mode 因无效 JSON 提前 return | stdout 仍输出 `{}` |

### 3.4 事件处理流程

| 编号 | 用例名称 | 前置条件 | 输入事件 | 预期行为 |
|------|---------|---------|---------|---------|
| TC-K-030 | CC SessionStart | Poke Poke 运行中 | CC 发送 SessionStart | POST /notify status=running，title 含 "Claude Code:" |
| TC-K-031 | CC UserPromptSubmit (首次) | 无 lock 文件 | CC 发送 UserPromptSubmit | 创建 lock 文件，POST status=running，payload 含 tty |
| TC-K-032 | CC UserPromptSubmit (重复) | 已有 lock 文件 | CC 再次发送 UserPromptSubmit | 仍 POST status=running，但不重新获取 tty |
| TC-K-033 | CC Notification | CC 运行中 | CC 发送 Notification | POST status=pending，message 为通知内容 |
| TC-K-034 | CC Stop | CC 运行中 | CC 发送 Stop | 清除 lock 文件 + pending 文件，POST status=success |
| TC-K-035 | Cursor stop (completed) | Cursor 运行中 | Cursor 发送 stop, status=completed | POST status=success |
| TC-K-036 | Cursor stop (aborted) | Cursor 运行中 | Cursor 发送 stop, status=aborted | POST status=failed |
| TC-K-037 | Cursor sessionEnd | Cursor 会话中 | Cursor 发送 sessionEnd | POST status=success，message="Session ended" |
| TC-K-038 | Codex Stop | Codex 运行中 | Codex 发送 Stop | POST status=success |
| TC-K-039 | 未知事件 | - | 发送未知事件名 | 静默忽略，不报错 |

### 3.5 CLI 安装/卸载/检查

#### Claude Code

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-K-050 | CC 安装 | 无 ~/.claude/settings.json | 执行 `poke-hook --install` | 创建 settings.json，包含 4 个事件的 hooks 配置 |
| TC-K-051 | CC 重复安装 | 已安装过 | 再次执行 --install | 先清除旧的 poke-hook 条目，再写入新的（幂等） |
| TC-K-052 | CC 安装保留原有 hooks | settings.json 已有其他 hooks | 执行 --install | 其他 hooks 不受影响 |
| TC-K-053 | CC 卸载 | 已安装 | 执行 `poke-hook --uninstall` | 移除所有 poke-hook 条目，清理空事件和空 hooks 对象 |
| TC-K-054 | CC 卸载 — 无配置文件 | 无 settings.json | 执行 --uninstall | 输出 "nothing to do"，不报错 |
| TC-K-055 | CC 卸载清理遗留事件 | 存在 PreToolUse/PostToolUse 遗留配置 | 执行 --uninstall | 同时清理遗留事件 |
| TC-K-056 | CC 检查 — 已连接 | 二进制已安装，hooks 已配置 | 执行 --check | 返回 `{"connected": true}` |
| TC-K-057 | CC 检查 — 未连接 | 二进制不存在 | 执行 --check | 返回 `{"connected": false}` |

#### Codex CLI

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-K-060 | Codex 安装 | 无 ~/.codex/ | 执行 --install-codex | 创建 hooks.json + 更新 config.toml（含 codex_hooks=true） |
| TC-K-061 | Codex 卸载 | 已安装 | 执行 --uninstall-codex | hooks.json 中移除 poke-hook 条目，config.toml 移除 hooks 键 |
| TC-K-062 | Codex 检查 | 已安装 | 执行 --check-codex | 返回 `{"connected": true, "feature_enabled": true}` |

#### Cursor

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-K-070 | Cursor 安装 | 项目目录 /path/project | 执行 --install-cursor /path/project | 在 .cursor/hooks.json 写入 4 个事件配置 |
| TC-K-071 | Cursor 安装无路径 | - | 执行 --install-cursor（无参数） | 输出错误提示 |
| TC-K-072 | Cursor 安装清理遗留脚本 | .cursor/hooks/ 下有旧 .py/.sh 文件 | 执行 --install-cursor | 清理遗留文件 |
| TC-K-073 | Cursor 卸载 | 已安装 | 执行 --uninstall-cursor /path | 移除 poke-hook 条目，空文件则删除 |
| TC-K-074 | Cursor 检查 | 已安装 | 执行 --check-cursor /path | 返回 `{"connected": true}` |

---

## 四、弹窗管理模块 (popup.rs)

### 4.1 弹窗创建与定位

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-P-001 | 首个弹窗位置 | 无现有弹窗 | 触发弹窗 | 弹窗位于屏幕右上角，x = 屏幕宽 - 360 - 12，y = 12 + 30 |
| TC-P-002 | 第二个弹窗位置 | 已有 1 个弹窗 | 触发第二个弹窗 | y = 12 + 30 + 150 + 8（向下堆叠） |
| TC-P-003 | 弹窗属性 | - | 创建弹窗 | 无边框、始终置顶、不获取焦点、不可调整大小、透明背景 |
| TC-P-004 | 弹窗 label 格式 | task.id = "abc-123" | 创建弹窗 | 窗口 label 为 "popup-abc-123" |

### 4.2 弹窗自动消失

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-P-010 | 超时自动消失 | popup_timeout = 10 | 等待 10 秒 | 弹窗自动关闭 |
| TC-P-011 | 永不自动消失 | popup_timeout = 0 | 等待任意时长 | 弹窗不会自动关闭 |
| TC-P-012 | 超时前手动关闭 | popup_timeout = 30 | 5 秒后手动关闭 | 超时线程检测到弹窗已不存在，不重复关闭 |
| TC-P-013 | 聚焦终端自动消失 | 弹窗有关联 tty | 用户切换到该终端窗口 | 弹窗在 1.5 秒内自动关闭 |
| TC-P-014 | 无 tty 不启动聚焦检测 | 任务无 terminal_tty | 触发弹窗 | 不启动终端聚焦检测线程 |

### 4.3 弹窗关闭与动画

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-P-020 | 关闭后重新排列 | 有 3 个弹窗，关闭第 1 个 | 关闭中间弹窗 | 后续弹窗向上滑动补位，动画约 230ms（14 帧 × 16ms） |
| TC-P-021 | 关闭最后一个弹窗 | 有 1 个弹窗 | 关闭它 | 正常关闭，popup_list 变空 |
| TC-P-022 | 关闭不存在的弹窗 | popup_list 中无该 id | 调用 close_popup | 无报错，无副作用 |

---

## 五、设置模块 (settings.rs)

### 5.1 默认值

| 编号 | 用例名称 | 操作 | 预期结果 |
|------|---------|------|---------|
| TC-S-001 | 默认提示音 | 创建 Settings::default() | `alert_sound == "system:Glass"` |
| TC-S-002 | 默认语言 | 创建 Settings::default() | `locale == "zh"` |
| TC-S-003 | 默认会话保留 | 创建 Settings::default() | `session_retention_hours == 24` |
| TC-S-004 | 默认弹窗超时 | 创建 Settings::default() | `popup_timeout == 0`（永不超时） |
| TC-S-005 | 默认自动启动 | 创建 Settings::default() | `auto_start == false` |
| TC-S-006 | 默认快捷键 | 创建 Settings::default() | `panel_shortcut == None` |

### 5.2 持久化

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-S-010 | 保存并加载 | 修改设置后保存 | 用同路径加载 | 数据一致 |
| TC-S-011 | 部分 JSON 兼容 | 文件中只有 `{"locale":"en"}` | 加载 | locale="en"，其余用默认值填充 |
| TC-S-012 | 文件损坏 | 文件内容为非法 JSON | 加载 | 使用全部默认值，不 panic |
| TC-S-013 | 文件不存在 | 路径不存在 | 加载 | 使用全部默认值 |
| TC-S-014 | 目录自动创建 | 父目录不存在 | 保存 | 自动创建目录链 |

---

## 六、音效模块 (sound.rs)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-SD-001 | 播放系统声音 | settings.alert_sound = "system:Glass" | 触发通知 | 调用 `afplay /System/Library/Sounds/Glass.aiff` |
| TC-SD-002 | 静音模式 | settings.alert_sound = "mute" | 触发通知 | 不播放任何声音 |
| TC-SD-003 | 列举系统声音 | macOS 系统正常 | 调用 list_system_sounds | 返回系统声音列表（如 Glass, Ping, Basso 等） |
| TC-SD-004 | 预览声音 | - | 调用 play_sound_by_name("Ping") | 播放 Ping.aiff |

---

## 七、前端 — 通知面板 (NotificationPanel.tsx)

### 7.1 工具函数

| 编号 | 用例名称 | 输入 | 预期输出 |
|------|---------|------|---------|
| TC-FP-001 | projectName 提取冒号后 | title = "Claude Code: my-project" | `"my-project"` |
| TC-FP-002 | projectName 无冒号 | title = "plain-title" | `"plain-title"` |
| TC-FP-003 | sourceLabel — claude-code | source = "claude-code" | `"Claude Code"` |
| TC-FP-004 | sourceLabel — cursor | source = "cursor" | `"Cursor"` |
| TC-FP-005 | sourceLabel — codex | source = "codex" | `"Codex"` |
| TC-FP-006 | sourceLabel — null | source = null | `""` |
| TC-FP-007 | workspacePath 缩写 | `/Users/edy/projects/foo` | `"~/projects/foo"` |
| TC-FP-008 | workspacePath 无路径 | workspace_path = null | `""` |
| TC-FP-009 | isActive — running | status = "running" | `true` |
| TC-FP-010 | isActive — pending | status = "pending" | `true` |
| TC-FP-011 | isActive — success | status = "success" | `false` |
| TC-FP-012 | isActive — failed | status = "failed" | `false` |

### 7.2 面板 UI 交互

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-FP-020 | 空列表提示 | 无任务 | 打开面板 | 显示空态提示文字 |
| TC-FP-021 | 任务排序 | 有多条任务 | 打开面板 | 按注册时间先后排列（先注册的在顶部） |
| TC-FP-022 | 活跃任务计数 | 1 个 Running + 1 个 Pending + 1 个 Success | 打开面板 | 标题显示 "2 个活跃会话" |
| TC-FP-023 | 点击任务跳转 | 存在任务 | 点击任务行 | 调用 open_task_source，聚焦对应终端 |
| TC-FP-024 | 删除非活跃任务 | 存在 Success 任务 | 点击删除按钮 | 任务被移除 |
| TC-FP-025 | 活跃任务无删除按钮 | 存在 Running 任务 | 查看面板 | 无删除按钮 |
| TC-FP-026 | 设置按钮 | - | 点击底部齿轮图标 | 打开设置窗口 |
| TC-FP-027 | 实时更新 | 面板已打开 | 新通知到达 | 面板自动刷新显示新任务 |

---

## 八、前端 — 弹窗窗口 (PopupWindow.tsx)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-FW-001 | 弹窗展示内容 | 收到 Success 通知 | 观察弹窗 | 显示来源徽章、标题、消息、时间、"点击跳转" 提示 |
| TC-FW-002 | 点击弹窗跳转 | 弹窗显示中 | 点击弹窗 | 聚焦对应终端，弹窗关闭 |
| TC-FW-003 | 弹窗滑入动画 | 新弹窗触发 | 观察弹窗 | 从右侧滑入的过渡动画 |
| TC-FW-004 | 弹窗数据加载 | 弹窗窗口创建 | 从 label 提取 task id | 正确加载对应任务数据 |

---

## 九、前端 — 设置窗口 (SettingsWindow.tsx)

### 9.1 快捷键解析函数

| 编号 | 用例名称 | 输入 | 预期输出 |
|------|---------|------|---------|
| TC-FS-001 | codeToKey 字母键 | code="KeyA" | `"A"` |
| TC-FS-002 | codeToKey 数字键 | code="Digit5" | `"5"` |
| TC-FS-003 | codeToKey 功能键 | code="F12" | `"F12"` |
| TC-FS-004 | codeToKey 方向键 | code="ArrowUp" | `"Up"` |
| TC-FS-005 | codeToKey 特殊键 | code="Space" | `"Space"` |
| TC-FS-006 | codeToKey 小键盘 | code="Numpad3" | `"Num3"` |
| TC-FS-007 | eventToShortcut Cmd+K | metaKey=true, code="KeyK" | `"CmdOrCtrl+K"` |
| TC-FS-008 | eventToShortcut Cmd+Shift+P | metaKey=true, shiftKey=true, code="KeyP" | `"CmdOrCtrl+Shift+P"` |
| TC-FS-009 | eventToShortcut 仅修饰键 | 只按 Shift（无主键） | `null` |
| TC-FS-010 | eventToShortcut 无修饰键 | 只按 A（无修饰键） | `null`（需要至少一个修饰键） |
| TC-FS-011 | formatShortcut 显示 | `"CmdOrCtrl+Shift+K"` | `"⌘ ⇧ K"` |
| TC-FS-012 | formatShortcut Alt | `"CmdOrCtrl+Alt+Space"` | `"⌘ ⌥ Space"` |

### 9.2 设置 UI 交互

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-FS-020 | 切换提示音 | 当前为 Glass | 从下拉菜单选择 Ping | 声音设置更新，显示 "已保存" 提示 |
| TC-FS-021 | 预览提示音 | 已选择 Glass | 点击"试听"按钮 | 播放 Glass 声音 |
| TC-FS-022 | 静音时隐藏预览 | 选择静音 | 查看界面 | 预览按钮隐藏 |
| TC-FS-023 | 切换弹窗超时 | 当前为"永不" | 选择"10 秒" | popup_timeout 更新为 10 |
| TC-FS-024 | 切换会话保留 | 当前为 24 小时 | 选择"7 天" | session_retention_hours 更新为 168 |
| TC-FS-025 | 切换语言 | 当前为中文 | 选择 English | 界面立即切换为英文 |
| TC-FS-026 | 开关自动启动 | 当前关闭 | 打开开关 | 调用系统 autostart enable，保存设置 |
| TC-FS-027 | 录制快捷键 | 无快捷键 | 点击输入框 → 按 Cmd+Shift+P | 显示 "⌘ ⇧ P"，保存设置 |
| TC-FS-028 | ESC 取消录制 | 正在录制 | 按 ESC | 退出录制模式，不保存 |
| TC-FS-029 | 清除快捷键 | 已设置快捷键 | 点击清除按钮 | panel_shortcut 设为 null |
| TC-FS-030 | ESC 关闭窗口 | 未在录制 | 按 ESC | 设置窗口关闭 |
| TC-FS-031 | 保存指示器 | - | 修改任意设置 | 底部显示 "已保存" 提示，1.2 秒后消失 |

---

## 十、前端 — 国际化 (i18n)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-I-001 | 中英文 key 一致 | - | 对比 strings.zh 和 strings.en 的 key 集合 | 完全一致 |
| TC-I-002 | 模板变量替换 | t("time.minutes_ago", {n: 5}) | 渲染 | zh: "5 分钟前"，en: "5m ago" |
| TC-I-003 | 不存在的 key | t("nonexistent.key") | 渲染 | 返回 key 本身作为 fallback |
| TC-I-004 | 语言切换实时生效 | 面板已打开，切换为英文 | 查看面板 | 所有文字切换为英文 |

---

## 十一、前端 — 像素图标 (SourceIcon.tsx)

| 编号 | 用例名称 | 输入 | 预期结果 |
|------|---------|------|---------|
| TC-IC-001 | Claude Code 颜色 | source="claude-code" | 橙色系 `hsl(25, 80%, 55%)` |
| TC-IC-002 | Cursor 颜色 | source="cursor" | 青色系 `hsl(175, 70%, 45%)` |
| TC-IC-003 | Codex 颜色 | source="codex" | 绿色系 `hsl(145, 70%, 45%)` |
| TC-IC-004 | 未知来源哈希颜色 | source=null, colorSeed="test-123" | 由哈希算法确定的 HSL 颜色 |
| TC-IC-005 | 哈希颜色确定性 | 两次传入相同 seed | 颜色一致 |
| TC-IC-006 | 表情 — pending | status="pending" | 平静圆眼 |
| TC-IC-007 | 表情 — running | status="running" | 眯眼 |
| TC-IC-008 | 表情 — success | status="success" | 开心 ^^ 眼 |
| TC-IC-009 | 表情 — failed | status="failed" | X 形眼 |
| TC-IC-010 | 动画 — running | status="running" | CSS sway 左右摇摆动画 |
| TC-IC-011 | 动画 — success | status="success" | CSS bounce 弹跳动画 |
| TC-IC-012 | 动画 — failed | status="failed" | CSS shake 抖动动画 |

---

## 十二、系统托盘 (tray.rs)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-T-001 | 左键点击开面板 | 面板未打开 | 左键点击托盘图标 | 打开面板窗口，定位在托盘图标附近 |
| TC-T-002 | 左键再点关面板 | 面板已打开 | 再次左键点击 | 面板关闭 |
| TC-T-003 | 面板失焦自动关闭 | 面板已打开 | 点击桌面其他区域 | 面板隐藏 |
| TC-T-004 | 右键菜单 | - | 右键点击托盘图标 | 显示上下文菜单（连接 CC / 连接 Codex / Cursor / 退出） |
| TC-T-006 | 连接 CC 菜单项 | CC 未连接 | 点击"连接 Claude Code" | 执行安装流程，成功后显示勾选 |
| TC-T-007 | 退出应用 | - | 点击"退出" | 应用完全退出 |
| TC-T-008 | 关闭窗口不退出 | 设置窗口打开 | 关闭设置窗口 | 应用继续在托盘运行 |

---

## 十三、全局快捷键 (shortcut.rs)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-SC-001 | 注册快捷键 | settings.panel_shortcut = "CmdOrCtrl+Shift+P" | 应用启动 | 全局快捷键已注册 |
| TC-SC-002 | 触发快捷键 | 快捷键已注册 | 在任意应用中按 Cmd+Shift+P | 打开/关闭设置窗口 |
| TC-SC-003 | 无快捷键 | settings.panel_shortcut = null | 应用启动 | 不注册任何快捷键 |
| TC-SC-004 | 修改快捷键 | 旧快捷键 Cmd+Shift+P | 在设置中改为 Cmd+Shift+K | 旧快捷键失效，新快捷键生效 |

---

## 十四、终端聚焦检测

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-TF-001 | iTerm2 当前会话匹配 | iTerm2 前台，当前 tab 的 tty 匹配 | 调用 is_terminal_session_focused | 返回 true |
| TC-TF-002 | iTerm2 其他 tab | iTerm2 前台，但目标 tty 在另一个 tab | 调用 is_terminal_session_focused | 返回 false（窄检测只看当前 tab） |
| TC-TF-003 | Terminal.app 匹配 | Terminal.app 前台，选中 tab 的 tty 匹配 | 调用 is_terminal_session_focused | 返回 true |
| TC-TF-004 | 非终端应用前台 | Chrome 在前台 | 调用 is_terminal_session_focused | 返回 false |
| TC-TF-005 | focus_terminal — iTerm2 优先 | iTerm2 和 Terminal.app 都运行中 | 调用 focus_terminal | 优先尝试 iTerm2 |
| TC-TF-006 | focus_terminal — 回退 Cursor | Cursor 任务，无 tty | 调用 focus_task_terminal | 使用 `open -a Cursor` 打开工作区 |
| TC-TF-007 | focus_terminal — 回退 Finder | 无终端可聚焦 | 调用 focus_task_terminal | 在 Finder 中打开工作区路径 |

---

## 十五、CLI 工具 (cli/pokepoke)

| 编号 | 用例名称 | 前置条件 | 操作步骤 | 预期结果 |
|------|---------|---------|---------|---------|
| TC-CLI-001 | 发送通知 | Poke Poke 运行中 | `pokepoke send "测试" "测试消息"` | 面板中出现新通知 |
| TC-CLI-002 | 开始任务 | Poke Poke 运行中 | `pokepoke start "项目" "开始工作"` | 创建 Running 状态任务 |
| TC-CLI-003 | 完成任务 | 存在 Running 任务 | `pokepoke done <task_id>` | 任务变为 Success，弹窗弹出 |
| TC-CLI-004 | 失败任务 | 存在 Running 任务 | `pokepoke fail <task_id>` | 任务变为 Failed，弹窗弹出 |
| TC-CLI-005 | 列出通知 | 有通知 | `pokepoke list` | 输出通知 JSON 列表 |

---

## 十六、端到端场景测试

### 16.1 Claude Code 完整生命周期

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-001 | CC 正常完成 | 1. 启动 Poke Poke<br>2. 在终端启动 CC 会话（触发 SessionStart）<br>3. 输入提示词（触发 UserPromptSubmit）<br>4. CC 完成（触发 Stop） | 面板显示 Running → Success，弹窗在完成时弹出 |
| TC-E2E-002 | CC 需要权限 | 1. CC 运行中<br>2. CC 请求权限（触发 Notification） | 状态变为 Pending，弹窗弹出提醒用户审批 |
| TC-E2E-003 | CC 权限通过后恢复 | 1. 状态为 Pending<br>2. 用户审批后 CC 继续（触发 UserPromptSubmit） | 弹窗关闭，状态变回 Running |

### 16.2 Cursor 完整生命周期

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-010 | Cursor 正常完成 | 1. Cursor 开始（sessionStart）<br>2. 提交提示（beforeSubmitPrompt）<br>3. Agent 完成（stop, status=completed） | 面板显示 Success，弹窗弹出 |
| TC-E2E-011 | Cursor 中止 | 1. Cursor 运行中<br>2. 用户中止（stop, status=aborted） | 面板显示 Failed，弹窗弹出 |
| TC-E2E-012 | Cursor 会话结束 | 1. Cursor 运行中<br>2. 触发 sessionEnd | 状态变为 Success |

### 16.3 Codex CLI 完整生命周期

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-020 | Codex 正常完成 | 1. Codex 开始（SessionStart）<br>2. 提交（UserPromptSubmit）<br>3. 完成（Stop） | 面板显示 Running → Success |

### 16.4 多会话并发

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-030 | 同时多个会话 | 1. CC 会话运行中<br>2. Cursor 会话也运行中<br>3. 两者先后完成 | 两个弹窗分别弹出并正确堆叠，面板显示两条记录 |
| TC-E2E-031 | 弹窗堆叠和消失 | 3 个弹窗同时显示 | 关闭中间一个 | 下面的弹窗向上滑动补位 |

### 16.5 用户聚焦抑制

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-040 | 用户在查看终端 | 1. CC 运行中<br>2. 用户正在该终端的 tab 上<br>3. CC 完成 | 不弹窗（用户已经在看） |
| TC-E2E-041 | 弹窗后切到终端 | 1. 弹窗已显示<br>2. 用户切到对应终端 | 弹窗自动消失 |

### 16.6 持久化与恢复

| 编号 | 场景 | 步骤 | 预期结果 |
|------|------|------|---------|
| TC-E2E-050 | 应用重启后恢复 | 1. 有多条任务<br>2. 退出重启 Poke Poke | 面板中任务列表恢复（从 ~/.pokepoke/notifications.json 加载） |
| TC-E2E-051 | 过期任务清理 | 1. 有 25 小时前的 Success 任务<br>2. retention=24h<br>3. 等待清理定时器 | 过期任务被自动删除 |
| TC-E2E-052 | 僵死会话检测 | 1. CC 会话 Running<br>2. 关闭终端（TTY 设备文件消失） | 5 分钟内检测到，状态变为 Failed，message="Session lost" |

---

## 十七、异常与边界条件

| 编号 | 用例名称 | 场景 | 预期结果 |
|------|---------|------|---------|
| TC-EX-001 | 无效 JSON stdin | Hook 收到非 JSON 输入 | 静默返回，stdout 输出 `{}`，不 crash |
| TC-EX-002 | HTTP 服务未启动 | Hook 尝试 POST | 两个端口都尝试后静默失败，不 crash |
| TC-EX-003 | 并发 HTTP 请求 | 同时多个 POST /notify | 数据一致性，无 deadlock（Mutex 保护） |
| TC-EX-004 | 数据文件权限 | ~/.pokepoke/ 目录只读 | 保存失败但不 panic（`let _ = fs::write(...)` 忽略错误） |
| TC-EX-005 | 超长 title/message | 传入超长文本 | 正常存储和显示（无长度限制但 UI 应截断/换行） |
| TC-EX-006 | 空 task_id | POST /notify task_id="" | 仍可存储（空字符串作为 key），但不推荐 |
| TC-EX-007 | Hook 二进制丢失 | ~/.local/bin/poke-hook 被删除 | check 命令返回 installed=false，install 命令重新复制 |
| TC-EX-008 | TTY 进程链过深 | get_tty() 遍历 8 层仍找不到 | 返回 None，不影响通知流程 |
| TC-EX-009 | AppleScript 超时 | 终端应用无响应 | osascript 调用返回 false，不阻塞主流程 |
| TC-EX-010 | 弹窗窗口创建失败 | 系统资源不足 | 输出错误日志，不影响其他功能 |

---

## 附录 A：测试用例统计

| 模块 | 数量 | 优先级 |
|------|------|--------|
| 任务存储 (notifications.rs) | 27 | P0 |
| HTTP 服务 (http_server.rs) | 15 | P0 |
| Hook 二进制 (bin/hook.rs) | 39 | P0 |
| 弹窗管理 (popup.rs) | 10 | P1 |
| 设置 (settings.rs) | 14 | P1 |
| 音效 (sound.rs) | 4 | P2 |
| 通知面板 UI | 20 | P1 |
| 弹窗窗口 UI | 4 | P1 |
| 设置窗口 UI | 24 | P1 |
| 国际化 | 4 | P2 |
| 像素图标 | 12 | P2 |
| 系统托盘 | 8 | P1 |
| 全局快捷键 | 4 | P2 |
| 终端聚焦检测 | 7 | P1 |
| CLI 工具 | 6 | P2 |
| 端到端场景 | 11 | P0 |
| 异常与边界 | 10 | P1 |
| **合计** | **219** | - |

## 附录 B：测试环境要求

| 条件 | 说明 |
|------|------|
| 操作系统 | macOS 13.0+ |
| 终端模拟器 | iTerm2 或 Terminal.app |
| AI 编程助手 | Claude Code / Codex CLI / Cursor 至少一个 |
| 端口 | 127.0.0.1:9876 和 9877 需可用 |
| 文件系统 | ~/.pokepoke/ 目录可读写 |
| 权限 | 辅助功能权限（用于 AppleScript 终端控制） |

## 附录 C：与 test-plan.md 的关系

| 文档 | 定位 |
|------|------|
| `test-plan.md` | 代码层面的单元测试实施规划，面向开发者，指导如何编写 `#[test]` 和 vitest 测试 |
| `test-cases.md`（本文档） | 功能层面的测试用例集，涵盖单元/集成/E2E，既可手动验证也可作为自动化测试编写依据 |
