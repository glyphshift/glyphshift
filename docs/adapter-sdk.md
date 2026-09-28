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

## Raylib 独立仓库

`adapter-raylib` 拥有描述符与 Native DLL 实现、跨架构合同、构建/打包入口和 CI 定义。
它通过 `sdk.lock.json` 固定 SDK 来源，展开的 `.sdk/` 是生成依赖缓存，不引用相邻主仓。
源码 SDK 随独立仓库固定在 vendor 中；SDK Release 草稿不作为可匿名下载的依赖地址。第三方 Rust 依赖正常通过
Cargo 锁文件取得，SDK 与构建工具都不进入最终 `.gsp`。

主产品暂保留原路径下的 Raylib **内置兼容快照**，维持现有构建、默认能力与集成测试。
源码权威已经转移到独立仓库；此快照按 `raylib/upstream.json` 的源 commit 与摘要管理，不作为第二份手工实现维护。
在独立包正式发布并具备默认安装政策前，不因拆仓突然删除用户已有的内置能力。

```text
python scripts/sync-raylib-snapshot.py --check
python scripts/sync-raylib-snapshot.py --archive <reviewed-git-archive.zip> --sha256 <approved-sha256> --revision <full-source-commit>
```

导入先验证完整来源摘要和 git archive commit，只复制固定映射的两个源码文件，拒绝覆盖尚未处理的本地快照修改。
导入之后按现有活动包验证入口检查；不更改主产品的共同 SDK/ABI 依赖边界。

独立源码已托管于 [glyphshift/adapter-raylib](https://github.com/glyphshift/adapter-raylib)。
CI 执行双架构构建、合同与打包，成功后保留带源 commit、SDK 锁和 SHA256SUMS 的候选制品。
SDK/插件先使用预发布草稿审核，不自动更新桌面最新版；正式 Release、Registry 发布者验证、在线安装界面仍需后续落地。
