# Settings 功能 TODO

## 第一梯队：核心设置

- [x] **提示音设置** — 选择系统音效 / 静音；`settings.alert_sound` 默认 `system:Glass`，设置页含下拉选择与试听
- [x] ~~**弹窗自动消失时间**~~ — 已撤销该设计：popup 不自动超时，仅靠"用户聚焦对应终端"或"session 进入新一轮"关闭。`Settings` 不含 `popup_timeout` 字段
- [x] **语言切换** — `settings.locale` zh / en，UI 实时切换（`src/i18n/strings.ts`）
- [x] **开机自启** — `settings.auto_start` + `@tauri-apps/plugin-autostart`

## 第二梯队：个性化体验

- [ ] **弹窗位置** — 四角可选（当前硬编码右下角）
- [ ] **终端偏好** — 选择默认终端：iTerm2 / Terminal / Warp / Ghostty（当前 iTerm2 优先）
- [ ] **Monster 配色方案** — 允许手动指定某个 session 的颜色（当前自动 hash 生成）

## 第三梯队：高级设置

- [x] ~~**数据保留**~~ — Task C 起 TTL 彻底移除；session 生命周期由宿主探活驱动，用户无可配置项。存储文件 `sessions.json`
- [ ] **Hook 事件筛选** — 选择性注册哪些 hook（当前 5 个事件全部注册）
- [ ] **动画速度** — 开关动画 / 调速（当前各动画 1.2s~1.4s）
- [x] ~~**TTY 轮询间隔**~~ — 已固定 5 秒（P0 重构决定），不可由用户配置
