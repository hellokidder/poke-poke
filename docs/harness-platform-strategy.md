# Poke Poke 的 Harness 平台改造策略

## 1. 结论先行

这个项目适合引入 Harness，但**不适合一上来做“全家桶式改造”**。

原因很直接：

- 当前仓库是 **单仓 Tauri 桌面应用**，不是典型的多服务云原生系统
- 真正复杂的链路不在普通前端构建，而在 **Rust + Tauri + macOS 打包 + 本地 hook 集成**
- 当前仓库还没有成型的自动化测试和 CI 基线，直接上高级能力会把平台复杂度加在工程短板上

因此建议采用下面的原则：

1. **先用 Harness CI 建立可重复的验证和打包流水线**
2. **把 macOS 构建单独建成自管能力，不和普通验证流水线混在一起**
3. **把审批、制品、密钥、供应链安全放在第二阶段接入**
4. **Test Intelligence 先不作为核心收益点，先把测试资产补出来**

---

## 2. 当前项目现状

从仓库内容看，Poke Poke 当前具备以下特征：

- 前端是 `Vite + React + TypeScript`，入口脚本只包含 `dev/build/preview/tauri`，没有测试脚本，见 [package.json](/Users/edy/Documents/Kidder/poke-poke/package.json)
- 桌面端是 `Tauri 2 + Rust`，同时产出主应用和 `poke-hook` 二进制，见 [src-tauri/Cargo.toml](/Users/edy/Documents/Kidder/poke-poke/src-tauri/Cargo.toml)
- 当前打包目标是 macOS `dmg` 和 `app`，并开启了 `macOSPrivateApi`，见 [src-tauri/tauri.conf.json](/Users/edy/Documents/Kidder/poke-poke/src-tauri/tauri.conf.json)
- 应用运行时依赖用户家目录中的外部状态，例如 `~/.pokepoke/`、`~/.local/bin/poke-hook`、`~/.claude/settings.json`、`~/.codex/config.toml`，这意味着发布成功不等于功能健康，见 [src-tauri/src/lib.rs](/Users/edy/Documents/Kidder/poke-poke/src-tauri/src/lib.rs) 和 [src-tauri/src/bin/hook.rs](/Users/edy/Documents/Kidder/poke-poke/src-tauri/src/bin/hook.rs)
- 产品和可靠性文档已经明确暴露了“配置漂移”“一键修复”“安装幂等”“升级回归”等需求，见 [docs/product-spec.md](/Users/edy/Documents/Kidder/poke-poke/docs/product-spec.md) 和 [docs/reliability-todo.md](/Users/edy/Documents/Kidder/poke-poke/docs/reliability-todo.md)

这意味着 Harness 改造目标不应该只是“把 `pnpm build` 搬到平台上”，而应该是：

- 建立 **验证可信度**
- 建立 **macOS 制品可信打包**
- 建立 **发布前后对本地集成健康度的治理**

---

## 3. Harness 能力与本项目的匹配方式

### 3.1 适合立即采用的能力

#### A. Harness CI

这是本项目最应该先上的模块，用来承接：

- PR 验证
- 分支构建
- macOS 打包
- release 候选产物生成
- 回归任务编排

对 Poke Poke 来说，Harness CI 的价值不是替代现有脚本，而是把“本地能跑”升级为“平台可重复执行、可追踪、可审批”的工程基线。

#### B. Git Experience

建议把 Harness pipeline、template、input set 都存回 Git，而不是只存在 Harness UI。

这样做的收益：

- 平台配置进入代码审查流程
- 流水线演进与产品版本同步
- 回滚更简单
- 适合当前这个单仓项目，减少“平台里改了一版，仓库里没人知道”的配置漂移

建议目录：

```text
.harness/
  pipelines/
    pr-verify.yaml
    macos-package.yaml
    release.yaml
  templates/
    steps/
    stages/
  input-sets/
```

#### C. Secrets Management

后续只要做签名、notarization、GitHub Release/S3 上传，就一定要把这些敏感信息放进 Harness Secret Manager 或外部密钥系统，而不是散在 runner 机器上。

最典型的敏感项：

- Apple Developer 证书
- `APPLE_ID` / app-specific password / API key
- GitHub token
- 可能的对象存储凭据

#### D. Manual Approval

发布 `dmg` 之前建议加入人工审批，而不是每次 tag 后自动外发。

这个项目是桌面工具，且会改写用户本地集成配置；一旦发出有问题的安装包，影响的是用户本机环境，不是单纯线上流量回滚。因此 release 应保留人工闸门。

### 3.2 适合第二阶段采用的能力

#### A. Artifact Registry

适合用来集中管理构建依赖或制品元数据，但**不建议作为当前阶段唯一的桌面发布出口**。

对这个项目更现实的做法是：

- 先把 `dmg/app` 作为 pipeline artifact 保存
- 再发布到更贴近桌面分发的出口，例如 GitHub Releases 或对象存储
- 等发布链路稳定后，再评估是否把部分制品治理统一收到 Harness Artifact Registry

#### B. STO / SCS

这两个模块对长期是有价值的，但不是第一阶段主战场。

适合后置的原因：

- 现在连基础测试流水线都还没成型
- 这个仓库当前的主要风险在发布正确性和本地集成健康度，而不是“已经成熟到只剩安全左移”
- 更合理的顺序是先把 CI、打包、签名、审批打稳，再补安全扫描、SBOM、SLSA

### 3.3 不应高估的能力

#### Test Intelligence

对 Poke Poke 这样的 `TypeScript + Rust + Tauri` 项目，Test Intelligence 不是第一波最直接的收益来源。

所以建议：

- **先补测试**
- **再接普通 Test/Run steps**
- **最后再评估 TI 是否值得接入**

不要把“TI 能提速”当成当前 Harness 改造的核心 ROI。

---

## 4. 目标态架构

建议把 Harness 里的流水线拆成 3 条，而不是一条超长总线。

### 4.1 Pipeline A: `pr-verify`

用途：每个 PR 的快速质量门禁。

建议内容：

1. checkout
2. Node 依赖恢复
3. Rust 依赖恢复
4. `pnpm build`
5. `cargo test` 或最小可行 Rust 测试集
6. `pnpm test` 或 Vitest（补齐后）
7. 产出测试报告和失败日志

目标：

- 让 PR 至少有统一的编译与基础测试门禁
- 不要求产出正式安装包

### 4.2 Pipeline B: `macos-package`

用途：面向主干分支或 release branch 的 macOS 构建。

建议内容：

1. checkout
2. 恢复 `pnpm` / Cargo 缓存
3. 安装 Node/Rust/Tauri 依赖
4. 执行 `pnpm build`
5. 执行 `tauri build`
6. 收集 `src-tauri/target/release/bundle` 下产物
7. 生成构建元数据
8. 上传 pipeline artifact

这个 pipeline 必须单独跑在 **macOS 自管 VM 基础设施** 上，不要和普通 Linux 验证机混用。

### 4.3 Pipeline C: `release`

用途：对外发布。

建议内容：

1. 读取 tag / version input
2. 调用 `macos-package`
3. 执行签名 / notarization
4. 人工审批
5. 上传外部分发渠道
6. 记录 release note、构建元数据、校验和

目标：

- 让“可下载给用户”的动作与“能构建出包”的动作分离
- 把签名和发布权限收紧到少数人

---

## 5. 针对 Poke Poke 的阶段化落地方案

### Phase 0: 先补工程基线

这一步不做，Harness 改造会变成“平台包裹混乱脚本”。

建议先在仓库里补齐：

- `pnpm test`
- `cargo test`
- 一个统一的本地 CI 脚本，例如 `scripts/ci/verify.sh`
- 一个统一的 macOS 打包脚本，例如 `scripts/ci/package-macos.sh`
- 版本注入策略，明确 `package.json` / `tauri.conf.json` / Cargo version 谁是 source of truth

同时建议把文档里已经写出来的测试计划真正落到代码：

- [docs/test-plan-final.md](/Users/edy/Documents/Kidder/poke-poke/docs/test-plan-final.md)

验收标准：

- 开发机上一条命令能稳定完成 verify
- 开发机上一条命令能稳定完成 package

### Phase 1: 引入 Harness PR 验证

目标是最低风险切入。

做法：

- 先创建 `pr-verify` pipeline
- 先不做发布，不接签名，不接审批
- 只做编译、测试、报告上传
- 将 pipeline YAML 放进 `.harness/`

推荐执行环境：

- 如果 Rust/Tauri 全量编译对 Linux 不稳定，就直接使用 macOS runner
- 如果可以分层，就把前端和纯逻辑测试放 Linux，把桌面打包留给 macOS

### Phase 2: 引入 macOS 构建基础设施

这是本项目 Harness 改造的技术核心。

建议：

- 将 macOS 打包独立为 `macos-package`
- 使用 Harness 的 **self-managed macOS VM build infrastructure**
- 不建议把正式桌面包构建依赖在普通 Linux/K8s build infra 上

理由：

- 当前产物就是 `dmg` 和 `app`
- 开启了 `macOSPrivateApi`
- 后续签名和 notarization 也天然在 macOS 环境里更顺

此外建议把 macOS runner 当成“稀缺资源”使用：

- PR 默认不打正式包
- 只有主干、release branch、tag 才触发 `macos-package`

### Phase 3: 接入发布治理

这一步引入：

- Secrets
- Manual Approval
- 外部发布

建议规则：

- 非 tag 构建只产出内部 artifact
- `v*` tag 才进入 release pipeline
- notarization 成功后进入审批
- 审批通过后才上传外部分发渠道

### Phase 4: 接入供应链与安全能力

等基础 CI 稳定后，再加：

- STO：代码和依赖扫描
- SBOM：生成并保存依赖清单
- SLSA provenance：为 release 产物补可追溯性

这一步更适合做“发布质量增强”，不是“先决条件”。

---

## 6. Harness 里的具体实施建议

### 6.1 Build Infra 选型

建议两层：

- **默认验证层**：Harness Cloud 或普通自管执行环境，用于轻量 verify
- **macOS 专用层**：自管 macOS VM runner，用于 Tauri 打包和签名

不要把所有任务都堆到 macOS runner 上，否则成本高、排队长、反馈慢。

### 6.2 缓存策略

对这个项目，缓存优先级应是：

1. `pnpm` store
2. Cargo registry
3. Cargo git
4. Tauri/Rust 构建中间产物（谨慎）

建议：

- 先用 Harness 的 Cache Intelligence 或显式 cache step 处理依赖缓存
- 不要一开始就缓存整个 `target/`
- 先确保缓存 key 与 `pnpm-lock.yaml`、`Cargo.lock` 绑定

### 6.3 制品策略

建议分三层：

1. **pipeline artifact**
   - 构建日志
   - 测试报告
   - `dmg/app`
2. **release artifact**
   - 对外分发安装包
   - checksum
   - release notes
3. **supply-chain metadata**
   - SBOM
   - provenance

### 6.4 密钥与权限

至少把下面这些角色拆开：

- 维护 pipeline 的工程角色
- 可审批 release 的产品/负责人角色
- 可管理 signing secrets 的少数管理员角色

不要让打包 runner 机器长期保存明文签名资料。

### 6.5 质量门禁

建议在 Harness 里明确 4 道门：

1. 编译通过
2. 单测通过
3. 集成安装/卸载 smoke 通过
4. release 人工审批通过

其中第 3 条对 Poke Poke 特别重要，因为项目真正高风险的不是 UI，而是对本地 agent hooks 的改写。

---

## 7. 这项目最该补的 Harness 专属验证场景

普通应用只测 build/test 不够，Poke Poke 还需要把“集成状态”纳入流水线。

建议额外加入以下 smoke job：

### 7.1 `poke-hook` 安装幂等验证

执行顺序：

1. `--install`
2. `--check`
3. 再次 `--install`
4. 再次 `--check`
5. `--uninstall`
6. `--check`

验证点：

- 不重复注入
- 不误删其他 hooks
- 卸载后状态正确

### 7.2 Claude/Codex/Cursor 配置漂移回归

针对文档里提到的事故，做最小自动化回归：

- 配置文件为空
- 配置文件已有其他 hooks
- `poke-hook` 二进制缺失
- 配置被部分覆盖

### 7.3 打包后基本健康检查

至少校验：

- app bundle 存在
- dmg 存在
- `poke-hook` 二进制已产出
- 版本号一致

---

## 8. 建议的实施顺序

### 第 1 周

- 补脚本：`test`、`verify`、`package-macos`
- 清理当前构建入口
- 统一版本来源

### 第 2 周

- 上 Harness Git Experience
- 建 `pr-verify`
- 接入缓存

### 第 3 周

- 建 macOS runner
- 建 `macos-package`
- 跑通主干构建

### 第 4 周

- 接 secrets
- 接审批
- 建 `release`

### 第 5 周以后

- 补 STO / SBOM / SLSA
- 评估是否引入 Artifact Registry 作为统一制品治理层

---

## 9. 不建议的做法

- 不要在没有测试基线时直接上 Test Intelligence
- 不要把正式桌面打包建立在普通 Linux 容器流水线上
- 不要让 tag 自动直发给用户，至少保留审批闸门
- 不要把签名密钥散放在自管 runner 本机目录
- 不要把 Harness 改造理解成“把现有命令换个平台执行”

---

## 10. 最终建议

对 Poke Poke，Harness 的最佳切入点不是“高级智能能力”，而是这四件事：

1. **用 Git Experience 管住流水线配置**
2. **用 CI 建立 PR 验证和 macOS 打包**
3. **用 Secrets + Approval 管住桌面发布**
4. **在基础稳定后再接 STO / SBOM / SLSA**

一句话概括：

> 这个项目的 Harness 改造，核心不是“更快构建”，而是“把本地集成型桌面工具的验证、打包、发布和回归治理起来”。
> 先治理发布正确性，再追求平台高级特性。

---

## 参考资料

- Harness Git Experience overview
- Harness Git Experience quickstart
- Harness CI build infrastructure / VM build infrastructure
- Set up a macOS VM build infrastructure with Anka Registry
- Harness CI Cache Intelligence
- Harness Test Intelligence overview
- Harness approvals tutorial / manual approval stages
- Harness secrets management overview
- Harness Artifact Registry docs
- Harness STO / SCS / SBOM / SLSA docs
