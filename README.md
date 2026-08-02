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
flightdeck/                    可恢复的当前工作与稳定知识
archive/dictionary-sources/    尚待产品化导入的历史词典源，仅作数据保全
```

## 产品模型

- **工作流：** 一组可启停的持续运行期望，可包含多个软件目标和每个目标的有序词典集合。
- **软件：** 用户登记的名称、用途说明和程序绑定，不承载运行功能开关。
- **词典：** 独立、可复用的文字与字体规则资产，可限制整份词典的适用 Hook。

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

本机程序位置、Runtime Bundle、截图、日志和实机结果必须放在 `target/local-test/`，不得提交。

## 架构入口

- [领域语言](CONTEXT.md)
- [产品契约](PRODUCT.md)
- [桌面设计系统](DESIGN.md)
- [实际架构](flightdeck/work/glyphshift/references/architecture.md)
- [当前工作](flightdeck/work/glyphshift/index.md)
