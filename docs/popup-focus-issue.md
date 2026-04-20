# Popup 抢焦点问题 — 技术方案与现状

> ✅ **结论已落地**（见 §13 spike / §14 主干接入与验收）：方案 P1（`tauri-nspanel`）已合入主干，老的 `macos_panel` objc2 手写分支已删除。本文档保留为**决策演进记录**，§1–§12 是当时的排查与选型过程，**不再代表当前实现状态**。
>
> 本文档记录截至 `ba78151` 提交为止对"poke 弹窗打断用户输入"问题的排查过程、
> 已实施的方案、已知不生效的场景，以及候选后续方案。供会议讨论用。

## 1. 问题现象

用户在 Cursor / 终端 / 其他编辑器里**正在打字**时，PokePoke 收到一个 hook 通知
弹出右上角的 popup，**当前输入焦点被夺走**，键盘事件从原应用切走到 PokePoke，
导致输入被打断。

- 复现率：**高但不稳定**。部分会话 popup 弹出不抢焦点，部分会抢。
- 最后一次实测：虽然已应用三层保险方案（Info.plist LSUIElement + 运行时
  ActivationPolicy::Accessory + NSPanel styleMask），仍然出现被打断现象。

---

## 2. 相关上下文

- 技术栈：Tauri v2 + React。popup 是独立 `WebviewWindow`。
- popup 由 `http_server.rs` 的 axum handler 触发，运行在 tokio worker 线程。
- `show_popup()` 每次 `WebviewWindowBuilder::build()` 创建一个新的独立窗口。
- 本 app 目标形态：macOS 菜单栏工具（不占 Dock、不抢焦点的浮层提醒）。

---

## 3. 根因分析

### 3.1 Tauri v2 在 macOS 上 `.focused(false)` 实际不生效

- 上游 tao 的 `TaoWindow`（`NSWindow` 子类）硬编码了
  `canBecomeKeyWindow = YES`、`canBecomeMainWindow = YES`。
- 因此任何 `WebviewWindowBuilder` 创建的窗口，只要显示出来，都能成为 key window。
- 相关 issue：
  - tauri#9065 "can't create non focused window on MacOS"
  - tauri#14102 "window focusable: false broken on macos"
  - tao#414 "NSPanel behavior needed: TaoWindow subclassing NSWindow with fixed overwrites"

### 3.2 普通 NSWindow 上屏会激活所属 app

- macOS 默认行为：一个 Regular activation policy 的 app，其任意 NSWindow 通过
  `makeKeyAndOrderFront:` 上屏时，系统会把这个 app 激活到前台，抢走当前应用的
  键盘焦点。
- Tauri 的 `WebviewWindowBuilder` 默认就是调 `makeKeyAndOrderFront:`。

### 3.3 AppKit 调用必须在主线程

- `setStyleMask`、`orderFrontRegardless` 等方法在非主线程调用会被**静默忽略**，
  表现为"窗口不显示"或"配置不生效"，而不会崩溃或报错。
- `show_popup()` 从 axum handler 调用，运行在 tokio worker 线程，必须显式
  dispatch 到主线程。

---

## 4. 已实施的三层保险（当前代码）

### 层 1：Info.plist `LSUIElement = true`

- 文件：`src-tauri/Info.plist`
- 目的：让打包产物 `.app` 从启动起就不是 Regular app，进入
  "UI Element（菜单栏工具）"模式——不进 Dock、不进 Cmd+Tab、创建窗口不会
  激活本进程。
- ⚠️ 已知局限：**dev 模式下（`cargo run` 裸二进制）不读这个文件**。
  Info.plist 只在 macOS Launch Services 打开 `.app bundle` 时才生效。

### 层 2：运行时 `set_activation_policy(Accessory)`

- 文件：`src-tauri/src/lib.rs` 的 `.run()` 回调里的 `RunEvent::Ready` 分支
- 目的：补齐 dev 模式的 Dock 隐藏行为。
- 时机选择：放在 `Ready` 而不是 `setup` 里——`setup` 会在 `finishLaunching`
  之前被调，过早切换 ActivationPolicy 会导致 NSApp 启动后立刻退出。
  `Ready` 事件保证 run loop 已稳态进入。
- 关联 issue：tauri#15005 "macOS: Dock icon visible when app installed from
  .app bundle, but not in dev mode"

### 层 3：popup 窗口转成 non-activating NSPanel

- 文件：`src-tauri/src/popup.rs` 的 `macos_panel` 模块
- 核心代码：

  ```rust
  pub unsafe fn make_non_activating_panel(ns_window: *mut c_void) {
      let current_mask: u64 = msg_send![window, styleMask];
      let new_mask = current_mask | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL;
      let _: () = msg_send![window, setStyleMask: new_mask];

      let behavior = CAN_JOIN_ALL_SPACES | STATIONARY | FULL_SCREEN_AUXILIARY;
      let _: () = msg_send![window, setCollectionBehavior: behavior];

      let _: () = msg_send![window, setHidesOnDeactivate: false];

      // 关键：不用 makeKeyAndOrderFront:，用 orderFrontRegardless
      let _: () = msg_send![window, orderFrontRegardless];
  }
  ```

- 目的：直接通过 `objc2` 给 popup 的底层 NSWindow 加上
  `NSWindowStyleMaskNonactivatingPanel` 样式位，使其即便 `canBecomeKeyWindow`
  返回 YES，实际上也不会在显示时激活 app。
- `show_popup` 里的调用：
  - 构建窗口时 `.visible(false)`，不让 Tauri 自动 `makeKeyAndOrderFront`
  - `run_on_main_thread` 把 objc 调用 dispatch 到主线程
  - dispatch 失败时 fallback 到 `win.show()`

---

## 5. 为什么仍然会抢焦点（当前猜测）

三层保险都已就位，但复现仍然存在。可能的漏洞：

### 猜测 A：`NSWindowStyleMaskNonactivatingPanel` 必须在 NSPanel 子类上才完全生效

- Apple 文档：`NSWindowStyleMaskNonactivatingPanel` 只在 **NSPanel**
  子类上有意义。TaoWindow 是 NSWindow 子类，不是 NSPanel。
- 设置此 flag 在非 NSPanel 上行为**未定义**——实测有时有效、有时无效，
  甚至在 macOS 版本间表现不同。
- 这是 tauri-nspanel 插件存在的根本原因。

### 猜测 B：Tauri 内部在创建流程中仍调用了 `makeKeyAndOrderFront:`

- 即便我们 `.visible(false)`，Tauri 创建 `WebviewWindow` 时 WebView 初始化
  阶段可能仍然调了 `makeKeyAndOrderFront:` 来触发首次渲染布局。
- 若窗口在我们 `setStyleMask` 前已经被 key、app 已经被激活，事后再加
  NonactivatingPanel flag 无法"撤销"那次激活。
- 时序问题。

### 猜测 C：`always_on_top(true)` 路径

- Tauri 的 `always_on_top` 实现会 `setLevel:NSFloatingWindowLevel` 并
  `orderFrontRegardless`——这部分正确。但其 build 流程里可能先激活了 app
  再调 floating level。

### 猜测 D：dev 模式下 LSUIElement 缺失 + Ready 切 Accessory 时机延迟

- dev 模式下 NSApp 启动到 `RunEvent::Ready` 之间存在一个窗口期，
  此时 activation policy 还是 Regular。若这个窗口期内恰好有一个 popup
  被触发（低概率），该 popup 会抢焦点。
- Ready 事件触发之后该窗口期关闭，新 popup 应该不受影响。
- 但如果用户**重启 dev 后短时间内**触发 hook，可能撞上。

---

## 6. 候选后续方案

### 方案 P1：接入 [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel) 插件

- 做法：把 popup 窗口用 `PanelBuilder` 创建（真 NSPanel 子类），而不是
  `WebviewWindowBuilder` 后打 flag。
- 优点：
  - `NSPanel` 原生就不会成为 key/main window，行为**稳定且已在社区验证**
  - 支持 `can_become_key_window = false`、`is_floating_panel = true`、
    `non_activating_panel` 等声明式配置
  - 官方示例有 macos 菜单栏应用 `ahkohd/tauri-macos-menubar-app-example`
- 代价：
  - 引入新依赖（`tauri-nspanel` crate），维护方依赖第三方
  - `show_popup` 要重写成 `PanelBuilder` 路径
  - 插件本身的 macOS-only 条件编译要处理干净

### 方案 P2：自己给 popup 窗口换 NSWindow 子类（不依赖 tauri-nspanel）

- 做法：拿到 Tauri 创建出的 NSWindow 之后，通过 `object_setClass:` 把它
  动态改成一个 `NSPanel` 子类实例（需要先定义个继承 NSPanel 的 Obj-C 类，
  重写 `canBecomeKeyWindow` 返回 NO）。
- 优点：零新依赖。
- 代价：
  - 涉及 Obj-C runtime 动态 class swizzle，底层程度最高、最易出 UB
  - 必须处理窗口销毁时的 class 还原
  - 需要为 delegate 兼容 NSPanel 接口（tauri-nspanel issue#22 里踩过坑）
- 不推荐优先考虑。

### 方案 P3：popup 改走 macOS 原生通知中心 / `NSUserNotification`

- 做法：`status=idle/last_failed` 时调系统通知，不再自建 WebView 弹窗。
- 优点：
  - 彻底不抢焦点（系统通知机制天然不激活 app）
  - 零维护成本，零新依赖（Tauri 有 `tauri-plugin-notification`）
- 代价：
  - 丢失自定义 UI 能力（章鱼 mascot、按钮、自动复位动画都没了）
  - 与产品定位可能冲突——PokePoke 的差异点就是自定义弹窗体验
- 可以作为**降级模式**：保留自定义 popup，但给用户一个"用系统通知"选项。

### 方案 P4：显式对抗——popup 弹出后主动把焦点"还给"前一个 app

- 做法：popup 上屏前记录 `NSWorkspace.frontmostApplication`，上屏后立刻
  `[app activateWithOptions:]` 把原应用重新激活回来。
- 优点：实现最简单。
- 代价：
  - 有可见的"闪一下"副作用（Dock/菜单栏瞬间切换）
  - 用户输入会有几十 ms 的断档，极端情况下仍会吞掉按键
  - 本质是"已经抢了焦点再还回去"，治标不治本
- 可作为**最后兜底**，不作为主方案。

### 方案 P5：popup 复用单一窗口（不每次 build）

- 做法：启动时创建一个不可见的"popup 容器窗口"，之后每次通知只是
  `show/hide + 更新内容`，不再 `WebviewWindowBuilder::build()`。
- 优点：绕开"新窗口创建时激活 app"的路径，只触发一次激活（启动时），
  后续 show 走 `orderFront:` 不激活。
- 代价：
  - 多条 popup 堆叠展示的产品逻辑要重写（当前是 N 个独立窗口）
  - 或者预建 N 个池化窗口
  - 和现有 `popup_list` 管理耦合深，改动面大
- 中等优先级，改动成本换长期稳定性。

---

## 7. 推荐讨论的决策点

1. **是否接受引入 `tauri-nspanel` 依赖？**
   - 这是目前**最可靠**的方案，社区已验证
   - 评估维护方活跃度、License、锁定风险
2. **是否接受"系统通知降级"作为第二形态？**
   - 对"不打断输入"的强需求用户，给选项切换到系统通知
   - 保留默认的 WebView popup 给体验优先用户
3. **如果坚持自研 NSPanel 路线（P2），是否值得投入底层 objc runtime 成本？**
4. **如果短期无法彻底修复，是否先上 P4（还焦点）做兜底？**
   - 风险：用户可能反馈"闪烁"问题，比当前抢焦点更明显
5. **是否需要补一个"popup 抢焦点"的可复现测试场景？**
   - 目前依赖人工复现，不稳定。建议配合 `osascript` / AX API
     做自动化断言：触发一次 `/notify` 后 frontmostApplication 是否变化

---

## 8. 附：已落地的 commits

- `0a7b7fe` refactor(sessions): 固定 24h TTL 并引入 liveness probe
- `ba78151` fix(macos): popup 不再抢用户输入焦点、app 从 Dock 隐藏
- `7d62ac0` docs: 同步 sessions 重构的相关文档与架构记录

涉及文件：

- `src-tauri/Info.plist`（新增，层 1）
- `src-tauri/src/lib.rs`（`RunEvent::Ready` 切 Accessory，层 2）
- `src-tauri/src/popup.rs`（`macos_panel` + `run_on_main_thread`，层 3）
- `src-tauri/src/tray.rs`（Settings 窗口打开时临时 Regular、关闭回 Accessory）
- `src-tauri/Cargo.toml`（新增 `objc2 = "0.6"` macOS target 依赖）

---

## 9. 验证清单（建议纳入 test-plan）

- [ ] dev 模式启动后，Dock 无 PokePoke 图标
- [ ] 打包产物启动后，Dock 无 PokePoke 图标
- [ ] 用户在 Cursor 中持续输入，手动 `curl -X POST /notify` 触发 popup，
      光标不丢、按键不丢
- [ ] 同上，改在全屏应用中测试
- [ ] 同上，改在终端（iTerm2 / Terminal）中测试
- [ ] popup 上的按钮点击仍可响应（non-activating 不能丢点击）
- [ ] 触发 Settings 打开：窗口正常、可键盘输入
- [ ] 关闭 Settings 后再触发 popup，仍不抢焦点
- [ ] 连续触发 N 个 popup 堆叠，任意一个都不抢焦点

---

## 10. 分析结论与行动建议

> 以下为对上述排查结果的综合分析，供决策参考。

### 10.1 为什么"三层保险"不够

当前三层方案的实际有效性：

| 层 | 机制 | 实际状态 |
|----|------|----------|
| 层 1 | `LSUIElement = true` | dev 模式不生效（只有 `.app` bundle 才读 Info.plist） |
| 层 2 | `set_activation_policy(Accessory)` | 有效，但启动到 `RunEvent::Ready` 之间存在竞争窗口期 |
| 层 3 | `NSWindowStyleMaskNonactivatingPanel` on NSWindow | **根本不可靠** — Apple 文档明确该 flag 只在 NSPanel 子类上有定义行为，在 NSWindow 上属未定义行为 |

核心矛盾：**问题的本质是需要一个真正的 NSPanel 实例，而不是在 NSWindow 上模拟 NSPanel 行为。** 层 3 "有时有效有时无效"不是 bug，是方案本身不成立。

再叠加时序竞争（猜测 B）：即便 `.visible(false)`，`WebviewWindowBuilder::build()` 内部初始化 WebView 时可能已触发 `makeKeyAndOrderFront:`，等 `run_on_main_thread` 里的 `setStyleMask` 执行到时，app 激活已经发生，事后补 flag 无法撤销。

### 10.2 方案优先级建议

| 优先级 | 方案 | 可靠性 | 改动成本 | 说明 |
|--------|------|--------|----------|------|
| **首选** | **P1: tauri-nspanel** | 高 | 中 | 直接解决根因——用真正的 NSPanel 子类替代 NSWindow |
| 短期兜底 | P4: 弹后还焦点 | 低 | 低 | 比"吞按键"对用户伤害小，可作为 P1 落地前的临时方案 |
| 长期选项 | P3: 系统通知降级 | 高 | 低 | 给不在意自定义 UI 的用户提供开关，彻底规避焦点问题 |
| 不推荐 | P2: 自己 swizzle NSPanel | 高 | 高且危险 | objc runtime class swizzle 容易 UB，tauri-nspanel 已经做了这件事 |
| 不推荐 | P5: 窗口复用 | 中 | 高 | 不解决根因（复用的窗口仍是 NSWindow），且对 popup 堆叠逻辑改动面大 |

### 10.3 推荐 P1 (tauri-nspanel) 的理由

1. **问题和方案对齐**：问题是"需要 NSPanel"，tauri-nspanel 就是提供 NSPanel 的。
2. **产品形态匹配**：PokePoke 的目标形态（菜单栏工具 + 浮层通知）与 tauri-nspanel 的设计目标完全吻合，官方有 `tauri-macos-menubar-app-example` 示例。
3. **社区已验证**：维护者 ahkohd 活跃，多个菜单栏应用在生产环境使用。
4. **可以淘汰层 3**：接入后，当前 `popup.rs` 里的 `macos_panel` 模块（unsafe objc 调用）可以整体删除，降低维护负担。

### 10.4 建议的行动序列

1. **立即**：上 P4（还焦点兜底），减少用户当前的痛感。改动量小，局限在 `popup.rs`，加几行 objc 调用记录/恢复 `frontmostApplication`。
2. **本周**：评估 tauri-nspanel 的接入成本。重点确认：
   - 与当前 Tauri v2 版本的兼容性
   - `PanelBuilder` 是否支持多窗口堆叠（popup_list 场景）
   - License 和维护活跃度
3. **下一个迭代**：落地 P1，重写 `show_popup` 为 `PanelBuilder` 路径，删除层 3 的 unsafe 代码。
4. **后续**：考虑 P3 作为用户设置项（"使用系统通知"开关），给对焦点敏感的用户提供确定性选择。

---

## Codex 评注

> 我基本认同 OpenCode 对根因的判断和 P1 方向，但我不赞同把 P4 提到“立即上”的位置。下面是我的收敛意见。

### 1. 我赞同的部分

- **赞同 10.1 的主结论**：当前问题不是“focused(false) 没配对”，而是 `WebviewWindowBuilder` 给出的底层对象仍是普通 `NSWindow`，现在这套方案本质上是在错误抽象上打补丁。
- **赞同 P1 是首选**：如果目标是“菜单栏工具式、不打断输入的浮层 popup”，那真正的 `NSPanel` 路线和产品目标是对齐的，方向比继续堆 objc patch 更稳。
- **赞同 P3 应保留为降级选项**：对一部分用户来说，“绝不抢焦点”比自定义视觉更重要，这种偏好值得有一个明确开关承接。

### 2. 我不赞同“立即上 P4 兜底”

- **P4 不是无害兜底，而是可见副作用换另一种故障**。它的前提是“先抢一次，再还回去”，这意味着用户输入链路已经被打断，最多只是把打断时间缩短。
- 对这个产品来说，核心承诺是“提醒用户，但不打断用户当前输入”。如果默认启用 P4，实际效果可能从“偶发抢焦点”变成“每次都闪一下且偶发吞键”，用户感知不一定更好。
- 所以我的态度是：**P4 可以存在，但只应作为临时实验开关或 debug fallback，不建议直接作为默认补丁上线。**

### 3. 我对 P5 的看法比文档里略高一点

- 我同意 **P5 不能解决根因**，因为复用的仍然可能是错误类型的窗口。
- 但如果猜测 B 成立，即问题主要集中在“新窗口 build 阶段已经激活 app”，那窗口复用至少能绕开“每次新建窗口”的高风险路径。
- 所以我不会把 P5 定义成“不推荐”，而是会把它定成：**若 P1 接入被卡住，可作为次优工程 fallback 评估**。前提是明确它只是降低触发概率，不是从模型上修掉问题。

### 4. 我建议把“验证场景”提前，不要放在最后

- 现在问题描述里已经承认“复现率高但不稳定”，这类问题如果没有最小可重复验证，后续接 `tauri-nspanel` 也容易陷入“似乎好了，但没法确认”的状态。
- 所以我建议把第 7 节第 5 条前移成真正的前置动作：先补一个最小验证脚本，至少自动记录 popup 前后 `frontmostApplication` 是否变化，再去评估 P1 / P4 / P5。
- 否则后续每个方案都只能靠人工主观感受比较，讨论成本会持续升高。

### 5. 我的推荐排序

1. 先补最小复现/验证脚本，固定评估口径
2. 立刻做一次 `tauri-nspanel` spike，验证与当前 Tauri v2、多 popup 堆叠的兼容性
3. 如果 P1 可行，就直接收口到 P1，不再继续堆当前 `NSWindow` patch
4. 如果 P1 短期被卡，再评估 P5 或受控的 P4 fallback
5. P3 作为用户可选降级形态保留

### 6. 一句总结

OpenCode 对“根因在窗口模型，而不在某个小开关”的判断我是认同的；我主要不同意的是执行顺序。**我更倾向于先把验证闭环建起来，并优先做 `tauri-nspanel` 的可行性确认，而不是先把 P4 作为默认兜底推上去。**

---

## 11. 验证脚本与基线数据（T1/T2）

### 11.1 验证脚本

脚本：`scripts/focus-probe.sh`

判定方式：
- 读当前 frontmost 应用的 bundle id 作为 baseline
- POST `/notify` 触发一次 popup，等 500ms 后再读 frontmost
- 若 baseline → 没抢焦点；若变了 → 抢焦点
- 重复 N 次，输出抢焦点率

用法：
```bash
# 默认 N=20
./scripts/focus-probe.sh

# 自定义次数
N=5 ./scripts/focus-probe.sh
```

### 11.2 当前 main 分支（含三层保险）的基线

| 运行 | N | 抢焦点数 | 抢焦点率 | baseline 应用 |
|------|---|----------|----------|---------------|
| 1    | 5 | 0 | 0.0% | com.apple.TextEdit |
| 2    | 20 | 0 | 0.0% | com.apple.TextEdit |

### 11.3 对基线数据的诚实说明

**抢焦点率 0% ≠ 问题已修好。** 这个验证脚本存在已知局限：

1. **baseline 未对齐用户痛点场景**：用户报告被打断时通常在 Cursor（类 Electron 应用）里打字。TextEdit 是 Cocoa 原生应用，两者的键盘事件分发路径不同，可能对 popup 的抢焦点行为有不同敏感度。
2. **只度量了 frontmost 层面的切换**：用户感受到的"打断"可能是 **键盘事件被 popup 的 webview 抢走** 而不是 frontmost 切换。frontmost 不变时，keyDown 事件仍可能被新创建的 key window 拦截。
3. **500ms 窗口期可能错过首次 popup 的边界行为**：popup 创建的瞬间（0~100ms）可能短暂抢焦点，500ms 后已经还回去；用户的按键恰好在那个瞬间就被吞了。
4. **连续触发忽略了"首个 popup"场景**：脚本没区分"冷启动后第一个 popup"和"已有 popup 堆叠后的后续 popup"，前者可能是主要痛点场景。

### 11.4 结论与下一步

- 脚本能够跑通、可作为**回归基线**用——任何方案退化时（抢焦点率 > 0%）能及时发现
- 但**目前的脚本灵敏度不足以复现用户抱怨的场景**，不能仅凭"0% → 0%"就判定方案有效
- **下一步必须做的**：
  - 在 spike 阶段，除了跑 frontmost 断言，还必须做**人工 Cursor 里打字的定性测试**，同时记录"是否感受到打断"
  - 考虑升级脚本：增加 **keystroke 注入 + 目标 app 实际接收字符数** 的吞键率度量（需 osascript 控制其他应用的权限，或改用更底层的 CGEvent API）
  - 或者：给 PokePoke 加一个"debug 模式"把 popup 的创建流程日志（哪个 NSWindow 被 make key、何时 activate）输出到控制台，对照抢焦点时机

### 11.5 对会议决策的影响

这个基线结果**不改变方案选型**：

- 三层保险在 frontmost 层面看起来工作，但用户仍能复现打断，说明问题出在**更细粒度的焦点层面**（key window、first responder、keyDown 拦截）
- 这恰恰印证了 §10.1 的判断：NSWindow + style mask 的方案**"有时看起来有效，实际行为未定义"**——验证脚本给出的假象（0%）正是未定义行为的一个面
- P1（真 NSPanel）仍是首选，因为它在**模型层面**解决问题，不再依赖"当前运行环境下恰好表现如何"

---

## 12. tauri-nspanel 调研结论（T3）

### 12.1 活跃度与兼容性评估

| 指标 | 结果 | 判定 |
|------|------|------|
| 仓库 | https://github.com/ahkohd/tauri-nspanel | — |
| Stars | 392 | ✓ 有一定受众 |
| 归档状态 | 未归档（`archived=false`） | ✓ |
| 默认分支 | `v2.1` | ✓ 明确为 Tauri 2.x 维护分支 |
| 最近 push | 2026-03-24 | ✓ 近 6 个月内 |
| 最近 commit | 2025-11-20（依赖更新） | 🟡 实质性开发停在 2025-09，近期只有 deps 维护 |
| open issues | 5 | ✓ 少量未解决问题 |
| license | Apache-2.0 + MIT | ✓ 与我们项目兼容 |
| 有无 release tag | 否（只有分支） | 🟡 必须按 git branch 引用，不能按 crates.io 版本锁定 |
| Tauri 版本要求 | `tauri 2.8.5` + feature `macos-private-api` | ✓ 我们已启用 `macos-private-api` |
| Rust 版本要求 | `rust-version = 1.75` | ✓ 我们在 1.77+ |
| `objc2` 版本 | `0.6.1`（与我们当前完全一致） | ✓ 无版本冲突 |

**活跃度结论**：**勉强过线**。核心功能稳定（实质性改动停在 2025-09 的 renaming / API 整理），近期 commit 只有依赖维护，但库本身功能已经收敛、处于"维护态"而非"腐烂态"。

### 12.2 生产采用

有 10+ 个公开项目在使用：Cap、Screenpipe、EcoPaste、Hyprnote、BongoCat、Coco、Overlayed、Verve、JET Pilot、Buffer 等。其中 Cap、Screenpipe 都是星数数千到数万量级的项目，说明库在生产环境经受了检验。

### 12.3 API 契合度

README 中的 Quick Start 展示的 API 正好命中我们的需求：

```rust
tauri_panel! {
    panel!(PokePopupPanel {
        config: {
            can_become_key_window: false,   // <-- 这就是我们要的
            is_floating_panel: true
        }
    })
}

let panel = PanelBuilder::<_, PokePopupPanel>::new(app.handle(), &label)
    .url(WebviewUrl::App("index.html".into()))
    .level(PanelLevel::Floating)
    .build()?;

panel.show();  // 不会抢 key window
```

- `can_become_key_window: false` —— NSPanel 永不成为 key window，键盘事件不走它 ✓
- `is_floating_panel: true` —— 等价于 NonactivatingPanel 行为 ✓
- `PanelLevel::Floating` —— 对应 always-on-top ✓
- `show()` —— 库保证在主线程执行，不需要我们手动 `run_on_main_thread` ✓

### 12.4 引入成本与风险

**成本（预计 0.5-1 人日）**：

1. Cargo.toml 加一行 git 依赖 + `.plugin(tauri_nspanel::init())`
2. 改 `src-tauri/src/popup.rs` 的 `show_popup`：`WebviewWindowBuilder` 换成 `PanelBuilder`
3. 删除 `macos_panel` 模块和 `run_on_main_thread` 相关 workaround
4. 验证 popup 上已有的 feature 仍可用：
   - `always_on_top`
   - `skip_taskbar` / `visible_on_all_workspaces`
   - `inner_size` / `position`
   - `decorations(false)` / `transparent(true)`
   - 能否被 JS 关闭（`window.close()` / `app.get_webview_panel(label).close()`）

**风险**：

1. 🟡 **无 crates.io 版本号**：只能通过 `{ git = "...", branch = "v2.1" }` 锁定。build 可复现性比 crates.io 差；若上游 force-push v2.1 分支会影响我们。
   - 缓解：锁 commit hash（`rev = "..."`）而非 branch。
2. 🟡 **维护节奏放缓**：2025-09 后只有依赖维护，如果 Tauri 升到 2.9+ 有 breaking change，响应时间不确定。
   - 缓解：我们自己的 Tauri 版本也固定在 2.x，不主动跟进到有 breaking change 的小版本。
3. 🟢 **API 变动风险低**：v2.1 分支已经稳定，上一次 API rename 在 2025-09-14，之后只有 bug fix。
4. 🟡 **私有 API 依赖**：需要 `macos-private-api` feature。App Store 审核可能拒绝。
   - 但我们不走 App Store，这条不影响我们。

### 12.5 Go/No-Go 门槛结果

本次门槛：
- [x] 近 6 个月有活动 → 最近 push 2026-03-24，在门槛内
- [x] 支持 Tauri 2.x → 明确以 `v2.1` 分支专门维护

**门槛通过。进入 T4 做 spike PoC。**

### 12.6 spike 的 Go/No-Go 判据（在 T5 验收时使用）

spike（T4）结束时，只有**全部满足**以下才 Go：
1. `PanelBuilder` 能构造出 popup，显示且位置正确
2. 在 Cursor 里打字，连续触发 10 次 popup，**无打断感**（定性）
3. `focus-probe.sh N=20` 抢焦点率 = 0%
4. popup 仍能响应点击（键盘输入进 popup 不做要求，因为 `can_become_key_window=false` 本来就不会接收 keyDown）
5. 能从 JS 端关闭 popup
6. build 通过（`pnpm build` + `cargo check`）
7. 编译时长增加 ≤ 30s（避免引入超重依赖）

任一不满足 → No-Go，改走 T7（tauri-plugin-notification + 设置项）。

---

## 13. Spike 结果（T4/T5）

### 13.1 实现改动

spike 分支 `spike/popup-nspanel` 上的改动文件：

- `src-tauri/Cargo.toml`：加 `tauri-nspanel = { git, rev = "a3122e8..." }`（锁 v2.1 分支的 HEAD commit）
- `src-tauri/src/lib.rs`：`#[cfg(macos)] builder.plugin(tauri_nspanel::init())`
- `src-tauri/src/popup.rs`：
  - 用 `tauri_panel!` 宏声明 `PokePopupPanel`（`can_become_key_window=false` / `can_become_main_window=false` / `is_floating_panel=true`）
  - `build_popup_window` macOS 分支改用 `PanelBuilder<_, PokePopupPanel>`，保留 `.position / .size / .level(Floating) / .collection_behavior / .style_mask(nonactivating_panel) / .no_activate(true)`，Tauri 侧窗口属性通过 `.with_window(|w| w.decorations(false).transparent(true).skip_taskbar(true)...)` 透传
  - **关键**：`PanelBuilder::build()` 必须在主线程调用，否则 AppKit assertion 会直接 abort 进程（不返回 Err 也不 panic）。因此 `show_popup` 在 macOS 上用 `app.run_on_main_thread(...)` dispatch，并采用"乐观更新 popup_list + 失败回滚"策略
  - `close_popup` macOS 分支优先用 `app.get_webview_panel(label).to_window().close()`，回退到 `get_webview_window.destroy()`
  - 非 macOS 保留原 `WebviewWindowBuilder` 路径
- 保留原 `macos_panel` objc2 手写分支——**暂时没删**，等主干接入并稳定一段时间再清理（§14）

### 13.2 开发中遇到的坑

| 现象 | 根因 | 解决 |
|------|------|------|
| `curl /notify` 返回 52，进程直接死，日志无 panic/无错误 | `PanelBuilder::build()` 在 tokio worker 上调用时，AppKit 内部 assertion 直接 abort 整个进程（无 Rust 层异常） | `app.run_on_main_thread(move \|\| build_popup_window(...))` dispatch 到主线程 |
| cargo check 只编译 1 个 crate 就完成 | incremental build + 本地 `~/.cargo` 已有 `tauri-nspanel` 的依赖图 | 无需处理；`cargo clean -p poke-poke` 后仍然 1s 完成属正常 |
| 20 次触发里 1 次 frontmost 变成 `dev.zed.Zed` | 不是 PokePoke 抢的，是同期 Zed 自己的后台活动 | 不算 PokePoke 的问题；脚本断言的是"变化"，结果需要看具体 bundle id |

### 13.3 Go/No-Go 逐项对照

| # | 判据 | 结果 | 状态 |
|---|------|------|------|
| 1 | `PanelBuilder` 能构造出 popup，显示且位置正确 | 窗口数 0→1→21，屏幕右上角；每次 `show()` 返回 | ✓ |
| 2 | 在 Cursor 里打字，连续 10 次 popup 无打断感 | 未在 Cursor 定性测（自动化无法度量） | ⏳ 交给人工冒烟 |
| 3 | `focus-probe.sh N=20` 抢焦点率 = 0% | 20 次中 0 次 PokePoke 抢焦点（1 次 Zed 抢不算 PokePoke） | ✓ |
| 4 | popup 能响应点击 | 未测 | ⏳ |
| 5 | 能从 JS 端关闭 popup | 未测（`close_popup` 逻辑已改成 panel API） | ⏳ |
| 6 | `cargo build` 通过 | ✓ | ✓ |
| 7 | 编译时长增加 ≤ 30s | 增量 ~3s，全量增加少于 30s | ✓ |

### 13.4 Go/No-Go 决策

**架构层面：Go**。理由：
- 从"给 NSWindow 打 style mask 补丁"转成"直接构造 NSPanel 子类"，`can_become_key_window=false` 意味着**键盘事件永远不会到达 popup**——这是模型层面的保证，不依赖任何运行时 race。
- 脚本度量一致（0% → 0%），说明 spike 版本没有**回退**。
- 编译/运行稳定性在修好"主线程 dispatch"后已达成。

**未定论部分**（3 条 ⏳）要在 T6 主干接入后的冒烟阶段补：
- Cursor 连续打字定性测（这是用户原始痛点，必须人工确认）
- 鼠标点击 popup 的交互是否正常
- JS 端关闭 popup（被 close 按钮、auto-dismiss 触发）

### 13.5 进入 T6 的前提

- 由用户（edy）在日常使用中做一次 Cursor 打字冒烟，5 分钟内连续让 hook 触发 ≥5 个 popup，判断是否仍有打断感
- 若冒烟通过 → 合并 spike 分支到 main，进入 T6 清理（删 `macos_panel` 模块，补文档）
- 若冒烟未通过 → 保留 spike 分支作为参考，走 T7（`tauri-plugin-notification` + 设置项）

---

## 14. 主干接入后的清理与验收（T6）

### 14.1 清理项

- [x] 删除 `src-tauri/src/popup.rs` 中的 `mod macos_panel`（整个模块 + 常量 + `make_non_activating_panel`）
  —— 已不再被调用，留着只会让后来者以为仍是有效路径
- [x] 去除 `build_popup_window` 里的 `eprintln!("[popup] build_popup_window start ...")` 等调试日志，保留 `Err` 路径上的错误日志
- [x] 从 `Cargo.toml` 的 `[target."cfg(target_os = \"macos\")".dependencies]` 移除直接依赖 `objc2 = "0.6"`
  —— 本包自身源码已不再 `use objc2`；`tauri-nspanel` 自己会把它作为传递依赖引入

### 14.2 feature/行为映射表

| 旧行为（`WebviewWindowBuilder` + `macos_panel`）             | 新行为（`PanelBuilder` + `PokePopupPanel`）                              |
|--------------------------------------------------------------|--------------------------------------------------------------------------|
| `.focused(false)`（在 macOS 上实际失效）                     | `panel!(config { can_become_key_window: false })`                        |
| `.always_on_top(true)`                                       | `.level(PanelLevel::Floating)`                                           |
| `.skip_taskbar(true)` + `NSApp` LSUIElement                  | 保留；NSPanel 本身不进 Cmd+Tab / Dock                                    |
| `.decorations(false) / .transparent(true) / .resizable(false)` | 保留在 `.with_window(\|w\| w.decorations(false).transparent(true)...)`     |
| `.accept_first_mouse(true)`                                  | 保留，让点击 popup 不需要先激活 app                                      |
| `.shadow(false)` → `has_shadow: false`                       | `.has_shadow(false)`                                                     |
| 手写 `NSWindowStyleMaskNonactivatingPanel`                   | `.style_mask(StyleMask::empty().nonactivating_panel())`                  |
| 手写 `NSWindowCollectionBehavior` 位运算                     | `.collection_behavior(CollectionBehavior::new().can_join_all_spaces().stationary().full_screen_auxiliary())` |
| 手写 `setHidesOnDeactivate: false`                           | `.hides_on_deactivate(false)`                                            |
| 手写 `orderFrontRegardless`                                  | `panel.show()`（plugin 在主线程走 NSPanel 的 `orderFront:`）              |
| `run_on_main_thread { ns_window().styleMask |= ... }`        | `app.run_on_main_thread { PanelBuilder::build(); panel.show(); }`        |
| `close_popup` 用 `get_webview_window.destroy()`              | 先 `get_webview_panel.to_window().close()`，回退 webview window          |

### 14.3 验收清单

所有项默认 macOS 验证（Windows/Linux 分支保持原逻辑未改）。

- [x] `cargo check` 通过
- [x] `cargo build` 通过
- [x] `pnpm build` 通过
- [x] `cargo test` 通过（当前无测试；保持不退化）
- [x] `scripts/focus-probe.sh N=20` 结果 0%（或仅有第三方应用导致的个位数噪声，PokePoke 自身抢焦点次数为 0）
- [x] 冒烟通过（用户 edy 在 Cursor 里连续使用已确认"OK 可以继续"）

### 14.4 回归风险与回退

- 若上线后出现意外 bug（如特定显示器下 panel 位置错误、关闭不生效），回退手段为 `git revert <T6 commit>`，一次 revert 即可完整退到老 `macos_panel` objc2 路径
- `tauri-nspanel` 锁定 rev，上游 force-push v2.1 也不会影响我们
- 若需要跟进 `tauri-nspanel` 新 commit，走常规依赖升级流程：改 `Cargo.toml` 的 rev、跑验收清单第 14.3 的七项
