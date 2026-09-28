# 适配器插件：本地 GSP 试点

第一阶段已提供 `.gsp` 包合同、开发者 CLI、本地不可变安装与桌面启动加载。尚未提供在线 Registry、
发布者签名验证、插件管理界面、双击关联、自动下载、热切换、物理卸载或独立 SDK/适配器仓库。
术语以 [CONTEXT.md](../CONTEXT.md#适配器插件) 为准。当前包不是面向普通用户的线上发行流程。

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
在线按架构下载、发布者信任与撤回、GUI 安装提示以及独立 SDK/CI 属于后续阶段。

## 验证

```powershell
. ./scripts/cargo-target.ps1
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot (Get-Location).Path
cargo build -p glyphshift-adapter-raylib-native
cargo test -p glyphshift-plugin-package
cargo test -p glyphshift-desktop-runtime --lib --test plugin_bundle_contract
```

合同覆盖多适配器/多架构、路径与链接拒绝、摘要/ABI/架构错误、外部摘要认可、不可变版本、并发写入、
失败回退与真实 DLL 描述检查。合成的 Controller/Runtime 仅用于加载组合测试，不证明真实 Raylib 应用兼容性；
引擎兼容矩阵仍按现有适配器验证计划推进。
