# 适配器插件：基础内置与引擎 GSP

默认 App 构建只内置 DrawText、ExtTextOut、TextOut、GDI+、DirectWrite 五种 Windows 基础文字能力，
同时带 x86/x64 Controller 和 Target Runtime。其余当前在完整 Runtime Bundle 分发的 16 个 Adapter ID
改为 12 个独立包：GTK3、Qt、Raylib、SideFX、Unity、MonoGame、Ren'Py、Web/Chromium、RPGMaker MV、
TyranoScript、VGUI、CatSystem2。Qt 的四个 Adapter ID 放在一个包；Unity 的 Mono/IL2CPP 放在一个包。
架构覆盖、原 Adapter ID 和实验标识保持不变，插件共享 App 的 Runtime，不复制 Controller/Runtime。
分组以 [`adapter-distribution.json`](../scripts/adapter-distribution.json) 为准。

基础版的长期边界是常规 Windows 文字接口与共享运行组件。Qt、Houdini 使用的 SideFX 技术栈、
Unity 及其他游戏引擎均按需安装插件，即使其 Adapter ID 以 `windows.` 开头也不并入基础版。
Unreal Engine（UE）目前没有可分发的适配器实现，未来实现时同样提供独立引擎插件；不按单个游戏拆包。
Public Core 不再提供完整内置引擎集合；跨仓集成验证使用各 Adapter 仓生成的版本化 GSP 制品。

安装包构建在交给 Tauri 打包前运行 `scripts/verify-base-runtime.py`：两个架构各自必须恰好包含
五项基础 Adapter，目录只允许清单引用的 Controller、Target Runtime 和基础 DLL，且摘要全部一致。
未引用的引擎 DLL、GSP、测试程序、插件子目录或 Worker 都会阻止发布构建，防止旧文件混入基础版。
这项分发检查不执行 DLL；原生兼容性仍由 Runtime Bundle 生产加载器验证。

官方扩展 Adapter 的源码由独立的私有 `glyphshift/adapter-*` 仓库拥有，用固定公共 SDK 独立构建 GSP。
Public Core 不保存这些 Adapter 的源码快照、真实引擎夹具或完整 Bundle 构建入口；维护边界见
[SDK 与源码所有权](adapter-sdk.md)。第三方仍可使用公开 SDK/ABI 和 reference adapter 开发自己的 GSP。

开发者构建：

```powershell
# Public Core 与发布安装包都只构建五个基础 Adapter。
./scripts/build-runtime-bundle.ps1 -Profile Release
# 桌面审阅同样只使用 Public Core Runtime。
./scripts/review-app.ps1 -BuildOnly
```

构建产物只保存在忽略的本地证据目录。官方扩展 Adapter 的构建、测试和候选 GSP 由对应私有仓负责；
Public Core Release 不再附带官方扩展 GSP，只发布基础应用、文档、插件 CLI 与校验和。在线市场后续按账户
entitlement 决定官方 GSP 的下载授权。

已有工作流引用外置 Adapter ID 时，升级后须安装并选择对应插件，再重启 App；不会自动下载或执行插件。
安装与选择分开，可使用 App 的「设置 → 插件管理」或下文 CLI。两者共享当前用户数据根中的插件库。
`.gsp` 的 SHA-256 应先与可信发行校验和核对。
同一 Adapter ID 由外置插件接管时不会与旧内置版本叠加，旧工作流无需改名。

已提供 `.gsp` 包合同、开发者 CLI、本地不可变安装与桌面启动加载。独立 Registry 已实现审核发布证明，
Rust 的[指定版本安装入口](registry-client.md)支持 HTTPS、固定公钥验签和原件复检。
本地插件管理界面已提供；双击关联、自动下载、热切换或物理卸载尚未提供，生产 Registry 尚未部署。
独立 SDK 合同和官方 Adapter 仓边界见 [adapter-sdk.md](adapter-sdk.md)；Release 处于草稿准备阶段。
术语以 [CONTEXT.md](../CONTEXT.md#适配器插件) 为准。当前包不是面向普通用户的线上发行流程。
Registry 的无执行静态验收入口见 [资源发布检查](resource-publication.md)。

## 包合同

`glyphshift.plugin/1` 使用 ZIP 容器，根清单为 `manifest.json`。当前支持 Windows 原生 TargetProcess
适配器和 x86/x86_64 变体；不启动 Worker 或安装脚本。一个包可提供多个 Adapter ID，每个 ID 可有多个架构制品。
支持文件只随包安装，不拥有隐式执行入口；安装目录只能包含清单声明的文件。

| 字段 | 含义 |
| --- | --- |
| `schema` | 固定 `glyphshift.plugin/1` |
| `package_id` | 稳定包身份，例如 `glyphshift-adapter-raylib` |
| `version` | 三个无符号 16 位整数，例如 `[1, 0, 0]`；本版不支持预发布标签 |
| `runtime_bundle_schema` | 当前精确要求 `glyphshift.runtime-bundle/4` |
| `license_file` | 已列入 `files`、角色为 `license` 的非空文件 |
| `variants` | `platform`、`architecture`、`adapters` 数组 |
| `files` | 文件的 `path`、`sha256`、未压缩 `size` 与 `role` |

Adapter 项包含 `file`、`native_metadata`、`name`、`summary`、`technology`、
`process_resident_after_deactivate`。Native Metadata 复用现有 Native Host 合同，包括 Adapter ID、
能力版本、ABI、执行位置、能力位、平台/架构位和文本规则。当前要求 Native ABI `[1, 0]`。
加载时比对 DLL 实际描述；跨架构制品由对应目标加载器在激活时再次比对。

文件角色为 `native_adapter`、`support`、`license`。包内路径使用小写 ASCII 与 `/`，最多四层，
禁止父路径、绝对路径、反斜线、设备名、链接和文件/目录别名。未知清单字段会被拒绝。
上限为 256 个载荷文件、单文件 64 MiB、总载荷 128 MiB、压缩包 64 MiB、清单 1 MiB。

SDK 属于编译期依赖；本版用 Native ABI 与 Bundle 合同约束运行兼容性，不用 SDK 源码版本替代 ABI。
插件不得重复交付公共 Controller/Target Runtime。当前不解析跨插件依赖，需要的支持文件由本包明确声明。
包版本发布后不允许替换内容；重新打包导致不同摘要也必须发布新版本。

## 开发者命令

先按仓库约定初始化 Cargo 缓存，再构建 CLI：

```powershell
. ./scripts/cargo-target.ps1
$pluginTarget = Get-GlyphshiftCargoTargetDirectory -RepoRoot (Get-Location).Path
cargo build -p glyphshift-desktop-runtime --bin glyphshift-plugin
$pluginCli = Join-Path $pluginTarget 'debug/glyphshift-plugin.exe'
```

所有试点包、安装根和测试证据使用本机 `local-test/` 下的目录；不作为仓库必需资源。
以下尖括号参数均需替换为实际位置；导出命令会创建新文件，不覆盖已有输出。

```text
glyphshift-plugin export-bundle <bundle-root> windows.raylib.draw-text-ex glyphshift-adapter-raylib 1.0.0 <license-file> <output.gsp>
glyphshift-plugin inspect <output.gsp>
glyphshift-plugin install <output.gsp> <plugin-store> --approve-sha256 <approved-sha256>
glyphshift-plugin select <plugin-store> <installed-sha256> <bundle-root>
glyphshift-plugin verify <plugin-store> <bundle-root>
glyphshift-plugin list <plugin-store>
glyphshift-plugin deselect <plugin-store> glyphshift-adapter-raylib <bundle-root>
```

`export-bundle` 从已有 v4 Bundle 提取一个 Adapter ID 的所有架构 DLL，校验原摘要，保留 Adapter ID，
不复制 Controller/Runtime。它是自包含 Native DLL 的本地试点工具，不会自动发现二进制依赖。
需要额外支持文件、多个 Adapter ID 或多份许可声明的包，应准备完整源码目录与清单，使用：

```text
glyphshift-plugin pack <manifest.json> <source-directory> <output.gsp>
```

`inspect` 只解析、校验摘要和 PE 架构，不加载 DLL。`install` 必须提供由调用者明确认可的完整包摘要；
它将包保存为候选版本，不改变当前选择。不要自动信任下载站同时提供的未验证文件和摘要。
这是本地开发者对精确字节的认可，**不是发布者签名验证**；不接受包内“作者可信”的自我声明。

`select` 先使用生产 RuntimeBundle 加载器检查完整组合，然后原子更改选择。此步骤会对已认可的同架构 DLL
做实际描述检查，因此可能执行其加载初始化代码。检查失败保留旧选择；选择旧版本即回退。

## 与桌面 App 的组合

桌面启动读取 `<workspace-data-root>/plugins`。未创建插件目录时保持原有 Bundle 行为，不写入空目录。
开发联调可以指定独立数据根，在其 `plugins` 子目录使用以上 CLI；构建启动桌面仍使用仓库的
`scripts/review-app.ps1`，不直接运行单独 Cargo 编译的壳。

- 安装与选择是分开的，未选择的候选包不影响 App。
- 同一 Adapter ID 只能由一个选中插件包拥有；包间冲突不采用“最后安装者获胜”。
- 明确选中的外置 Adapter 替代同 ID 的全部内置变体。未提供的架构不混用内置旧版本。
- 插件声明的架构必须具备对应公共 Controller/Runtime；不能仅凭桌面壳架构推断目标架构。
- 选择变更在下次 App 启动建立新 RuntimeBundle 时生效，旧实例继续持有旧的不可变路径。
- 取消选择恢复可用的内置能力；没有内置实现则移出目录。取消选择不会删除已加载 DLL 或历史版本。
- 安装目录按包摘要保存解压后文件，不另外保存一份 ZIP。版本选择写入同目录临时文件后原子替换；写入者互斥。
- 每次解析重验清单与载荷摘要，拒绝额外文件。损坏的已选插件会阻止该运行环境加载，不静默换用其他版本。

物理卸载和缓存回收留待运行引用跟踪完成后实施；当前历史版本保留，所以升级后的磁盘占用可能增加。
在线按架构下载、发布者信任与撤回仍待后续接入；独立 SDK/CI 和本地 GUI 安装已提供。

## App 内管理本地插件

在「设置 → 插件管理」选择本地 `.gsp`，核对包身份、版本、架构、适配器与摘要后确认安装。
预览只做静态校验；确认绑定预览时的精确包内容，五分钟后失效。安装不会自动启用。

点击「启用」并确认后，App 使用生产加载器检查完整组合，此步骤可能执行包内原生代码；仅启用来源可信的包。
检查失败不保存选择。同一包只选择一个版本，历史版本保留，可通过启用旧版本回退。
「本次已加载」记录本次启动的集合，「下次启动启用／停用」记录已保存的选择；两者不同时提示重启。
先停止工作流，再退出并重新打开 App；当前会话不热加载或卸载 DLL。

列表会保留损坏版本并显示校验错误，已选择的损坏版本可以尝试停用。停用也会验证剩余组合。
刷新可同步 CLI 的更改；过时的停用请求不会误停用其他工具刚选择的新版本。
浏览器预览不能管理本机插件。本阶段不提供物理卸载、双击关联或自动联网安装。

## 验证

```powershell
. ./scripts/cargo-target.ps1
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot (Get-Location).Path
cargo test -p glyphshift-plugin-package
cargo test -p glyphshift-desktop-runtime --lib --test plugin_bundle_contract
```

合同覆盖多适配器/多架构、路径与链接拒绝、摘要/ABI/架构错误、外部摘要认可、不可变版本、并发写入、
失败回退与真实 DLL 描述检查。合成的 Controller/Runtime 仅用于加载组合测试，不证明任何商业 Adapter 的
真实软件兼容性；官方付费 Adapter 的构建与兼容性验证在对应私有仓完成，Public Core 只消费发布后的 GSP。
