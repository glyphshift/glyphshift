# Glyphshift

Glyphshift 是面向 Windows 桌面软件的通用运行时界面翻译工具。它以工作流组合软件和可复用
词典，通过动态 Runtime Bundle 与 Capability Adapter 实现文字替换和字体替换，不修改目标
软件安装文件。

## 仓库结构

```text
apps/glyphshift-desktop/       Tauri 2 + Vue 3 + Nuxt UI 桌面应用
apps/glyphshift-service/       无 GUI 组合根与端到端验证
crates/                        Domain、Extension、Decision、Runtime、Workflow 等深 Module
test-support/                  合成 Adapter、Controller 与 Windows 测试宿主
architecture-tests/            通用边界和禁止依赖检查
scripts/dev-app.ps1            本地 Runtime Bundle 验证与桌面启动
scripts/review-app.ps1         同步构建、校验并启动最新版桌面与 Runtime
scripts/build-runtime-bundle.ps1  Debug/Release Runtime Bundle 共用构建器
scripts/build-desktop-release.ps1 本地 unsigned Windows 安装候选构建器
flightdeck/                    可恢复的当前工作与稳定知识
archive/dictionary-sources/    尚待产品化导入的历史词典源，仅作数据保全
```

## 产品模型

- **工作流：** 一组可启停的持续运行期望；每个软件目标独立组合 Adapter Plan、有序词典和字体绑定。
- **软件：** 用户登记的名称、用途说明和程序绑定，不承载运行功能开关。
- **词典：** 独立、可发布和复用的语言资产，只保存便携元数据与文字规则。
- **字体策略：** 属于单个工作流目标的有序字体候选，并选择仅作用于词典命中或 Hook 全部文字。
- **Adapter Catalog：** 独立描述平台、技术分类、具体拦截方式与能力，不进入词典。

## 开发验证

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test.ps1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm --prefix apps/glyphshift-desktop ci
npm --prefix apps/glyphshift-desktop run build
npm --prefix apps/glyphshift-desktop test
```

启动带真实本地 Runtime Bundle 的开发桌面：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev-app.ps1
```

构建并启动供人工检查的最新版 Release 桌面时，只使用同步评审入口：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/review-app.ps1
```

该入口让桌面壳、Controller、Target Runtime 和 Adapter 使用同一份源码与 Cargo 产物目录，并在
启动前调用生产加载器校验 Bundle。不要把独立 `cargo build` 或 `tauri build` 产生的新桌面壳与旧的
Runtime 目录组合运行。

UI Automation、WriteConsole 两个仅观察 Adapter，以及 UIA/OCR 取词 Worker 当前暂停产品使用。
实现源码和隔离测试保留，但默认 Runtime Bundle、Desktop Release 和产品 Adapter Catalog 均不得注册或
打包这些能力，默认 `scripts/test.ps1` 也不会构建或执行它们；默认 Bundle 只包含 9 个具备
`TextReplace` 能力的 Adapter。暂停 Adapter 的专项合同只能从其自身目录显式运行。

只生成严格分离的 Debug 或 Release Runtime Bundle：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-runtime-bundle.ps1 -Profile Release
```

Release Bundle 只包含 manifest 声明的 Controller、Target Runtime 和正式写回 Adapter，并使用 `/3`
清单；测试宿主只在显式 `-IncludeTestTarget` 时加入。所有输出仍只进入 `local-test/`。

生成包含 Release Runtime Bundle 的本地 unsigned NSIS candidate：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-desktop-release.ps1
```

脚本使用本地临时 Tauri 配置，不修改基础配置，也不安装 candidate。产物、清单和构建中间文件均
只进入 `local-test/`；公开发行仍需要独立的代码签名与发布流程。

本机程序位置、Runtime Bundle、截图、日志和实机结果必须放在 `local-test/`，不得提交。

## 架构入口

- [领域语言](CONTEXT.md)
- [产品契约](PRODUCT.md)
- [桌面设计系统](DESIGN.md)
- [实际架构](flightdeck/work/glyphshift/references/architecture.md)
- [当前工作](flightdeck/work/glyphshift/index.md)
