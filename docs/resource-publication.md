# Registry 资源发布检查

`glyphshift-resource-inspector` 是给 Registry 使用的静态检查工具，接受 stdin 原始字节，输出
`glyphshift.resource-inspection/1` JSON：kind、packageId、version、mediaType、sha256、size 和具名兼容性。
没有文件修改、格式迁移、DLL 加载或安装动作；检查通过不等于发布者可信或实际运行兼容。

```powershell
. ./scripts/cargo-target.ps1
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot (Get-Location).Path
cargo build -p glyphshift-resource-inspector
```

命令形式为 `glyphshift-resource-inspector dictionary` 或 `glyphshift-resource-inspector adapter`，
调用方通过二进制 stdin 传入制品，避免 PowerShell 文本管道改写 UTF-8 或 ZIP 字节。
字典上限 16 MiB，GSP 上限 64 MiB，失败退出非零且只输出稳定检查失败标识。

## 字典

- 发布使用 `DictionaryPackage::decode_publication_json`，拒绝未知和重复字段、错误类型、被丢弃的坏词条、重复原文、不合法正则及非 SemVer 发行版本。
- `schema`、`revision`、`metadata.releaseVersion` 与 `entries` 等身份/版本字段必须显式存在。
- 合法待译词条仍保留；展示作者不授予 Registry 发布权。
- 本地导入继续使用既有容错方法，发布入口不改变本地修复体验。
- 字典媒体类型统一为 `application/vnd.glyphshift.dictionary+json;version=3`，由 package crate 定义。
  旧 version=2 分发描述符/安装元数据不能再作为 schema 3 的已验证来源；需重新生成并重新验证，不能修改已有签名声明。
  历史 schema 2 文件必须先显式迁移、审阅再发布，检查器不会静默升级。

## 适配器

复用 GSP 权威解析器检查归档、清单、路径、大小、内容摘要与 PE 架构；报告保留原清单的版本、变体和兼容性。
归档 UIA 能力拒绝进入发布目录；不构建或执行归档实现。
真正的 DLL 导出校验仍由 App 在完成来源授权后执行，Registry 静态检查不替代它。

Registry 独立实现见 [glyphshift/registry](https://github.com/glyphshift/registry)。当前只完成本地可信操作员流程、
持久化审核、只读目录与下载；Identity、Asset、发布声明签名和生产服务接入尚未完成。
