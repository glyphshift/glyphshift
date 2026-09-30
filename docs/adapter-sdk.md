# 独立适配器 SDK 与源码快照

SDK 是编译期源码开发包，用户插件是 `.gsp`。SDK 不含 Controller、Target Runtime、桌面壳或具体 Adapter
实现，也不包含归档 UIA。它与最终用户插件是不同制品。

## SDK 导出

主仓拥有 Domain、Adapter SDK、Native ABI、Native Host 检查、GSP 包合同与开发工具。
`scripts/export-adapter-sdk.py` 从明确 Git commit 的允许列表导出这些源码、C ABI 头文件和已审阅的
`vendor/retour` 补丁；移除 Native Host 对主产品适配器集成测试的开发依赖。不会复制整个主仓或桌面依赖树。

```powershell
. ./scripts/cargo-target.ps1
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot (Get-Location).Path
python scripts/export-adapter-sdk.py --ref <committed-revision> --output <sdk-source.zip>
```

输出应位于本机 `local-test/`。导出使用已提交源码和依赖锁，以离线 Cargo metadata 收敛 SDK 图；
本机需要已取得对应的第三方 crate 索引/缓存。`sdk-release.json` 记录版本、源 commit、Native ABI、GSP schema
与每个文件的 SHA-256。ZIP 文件顺序、时间戳和权限固定；相同 commit、锁文件和工具环境的重复导出字节一致。
输出文件已存在时拒绝覆盖。SDK 来源是具体 commit，导出版本目前沿用工作区版本。
发行身份由 [SDK 版本登记](adapter-sdk-releases.json) 固定：0.1.0 只对应其中登记的源 commit、制品名和摘要，
不得从新 commit 导出同版本后覆盖。修改 SDK 时先分配新版本，并更新消费者锁；相同版本的重复构建只能核对已有字节。
SDK 标签使用 `adapter-sdk-vMAJOR.MINOR.PATCH`，与桌面发布的 `v*` 标签分离。

SDK 内的 `glyphshift-adapter-tool` 提供：

```text
glyphshift-adapter-tool metadata <locally-built.dll> --trusted-sha256 <approved-sha256>
glyphshift-adapter-tool pack <manifest.json> <source-directory> <output.gsp>
glyphshift-adapter-tool verify <package.gsp>
```

工具可独立编译，不依赖桌面 Runtime。`metadata` 需要对应架构的工具与明确摘要认可，因为读取实际导出会执行
DLL 加载初始化；`pack`/`verify` 不执行插件代码。具体包合同见 [plugins.md](plugins.md)。

## 官方 Adapter 独立仓库

Public Core 只拥有 SDK/ABI、Native Host、GSP 生命周期和产品集成能力，不再保存任何具体 Adapter
实现源码。基础版随包的五种 Windows Adapter 也已独立：

- `glyphshift/adapter-win32-text`：ExtTextOutW、TextOutW、DrawTextW/DrawTextExW
- `glyphshift/adapter-gdiplus`：GdipDrawString
- `glyphshift/adapter-directwrite`：DirectWrite TextLayout

这三个仓库公开源码并发布双架构 GSP。主仓通过 `scripts/base-adapters.lock.json` 固定 Release
URL 与 SHA-256，构建 Runtime Bundle 时下载、校验并内置，因此“源码独立开源”和“基础版默认携带”
可以同时成立。

官方扩展能力由其他 `glyphshift/adapter-*` 仓库拥有，包括 Qt、GTK3、SideFX、Unity、Raylib、
MonoGame、Ren'Py、Web/Chromium、RPGMaker MV、TyranoScript、VGUI、CatSystem2，以及独立维护的
KiriKiri、Console、Direct2D、UIA 与 OCR 源码。真实引擎集成夹具也不进入 Public Core。

一个技术栈仍以一个 GSP 包为主要交付单元；Qt 与 Unity 的多个 Adapter ID 不拆成相互依赖的小包。
官方仓固定公共 SDK 版本并独立构建、测试、检查 Native ABI、打包和验证 GSP，不允许通过相邻源码路径
依赖 Public Core。产品构建只消费版本化 GSP/Release/Registry 制品；主仓测试若需要公开基础 Adapter
的描述符类型，也必须固定到明确 Git revision，真实 DLL 仍来自锁定的 GSP。

Public Core 不维护 `adapter-sources.lock.json`、引擎源码兼容快照或源码同步脚本。SDK/ABI 变化通过版本化
公共合同演进；官方 Adapter 仓选择何时升级该合同。第三方 Adapter 开发者使用相同的公开合同，不需要访问
任何官方商业 Adapter 源码。

各官方仓 CI 生成候选制品；Registry 发布、购买 entitlement 与付费下载授权是独立的服务端流程。
