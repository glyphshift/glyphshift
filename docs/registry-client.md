# Registry 指定版本安装

`glyphshift-registry-client` 提供共享 Rust 验签/下载/安装逻辑，`glyphshift-registry` 是显式操作的 CLI。
当前完成在线字典发现/精确详情、指定资源 ID、版本和发布者的安装入口；桌面 GUI 的在线搜索、安装按钮、账户登录和网页唤起尚未接入。
现有桌面目录端口继续保持离线占位；共享库和 CLI 已消费独立的全站字典发现接口。

## 信任与流程

1. 调用方固定 Registry HTTPS origin，并从独立可信渠道提供 `glyphshift.release-trust/1` 公钥配置。
   不从网页安装参数、资源内容或下载服务器自动发现 issuer、公钥或任意下载 URL。
2. 获取精确版本的 `/proof`，核对 Ed25519/JWS、固定 issuer、key ID、撤销状态、资源类型/ID/版本和期望发布者。
3. 从同一固定 origin 的精确 `/download` 路由获取原始字节，不跟随重定向、不使用下载地址中的 Cookie/令牌。
4. 复核大小和摘要，并调用与 Registry admission 相同的 `glyphshift-resource-inspector` Rust 库。
   整份检查结果必须与已签名声明一致，包含字典语言/条目数或 GSP 变体、ABI 和 Runtime 条件。
5. 下载及静态检查结束后重新读取 proof；隔离、撤回、网络错误或证明变化均阻止安装。
6. 字典交给既有 `DictionaryDistribution` / `FileDictionaryInstallStore`；插件交给既有 `PluginStore`。
   不重写落盘与恢复协议，不加载 DLL，不启动安装脚本。

验签使用固定版本 [ed25519-dalek 的严格验证入口](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict)，
HTTP 使用已采用的 reqwest 版本。重复 JSON 字段、未知 protected header、算法替换、内嵌密钥地址和多签名格式均拒绝。
HTTP 每次请求连接上限 10 秒、总上限 60 秒，proof 上限约 2 MiB，字典 16 MiB，GSP 64 MiB；流式读取也受限。
TLS 证书验证始终开启。确有私有 CA 的部署可显式设置 `GLYPHSHIFT_REGISTRY_CA_FILE`，将 PEM 根证书加入系统信任集合，
不提供跳过 TLS 校验或允许生产 HTTP 的开关。

公开已审核资源的读取不需要账户令牌；投稿/审核继续由 Registry 的 Identity 与产品权限检查负责。
公钥配置在 `TrustStore` 创建时固定，CLI 每次操作重新读取文件。长期运行的 GUI 集成必须在后续安装前使用最新受信配置，
目前没有远程撤销推送或自动公钥发现。

## 使用入口

使用 `scripts/cargo-target.ps1` 初始化项目 Cargo 缓存后，构建 `glyphshift-registry-client`。
命令示例中的 origin、公钥文件和 ID 均由调用方明确提供；不包含任何预置生产服务或生产密钥。

```text
glyphshift-registry verify <trust.json> <proof.json> <dictionary|adapter> <package-id> <version> <publisher-userKey> <original-file>
glyphshift-registry install-adapter <https-origin> <trust.json> <package-id> <version> <publisher-userKey> <plugin-store>
glyphshift-registry install-dictionary <https-origin> <trust.json> <catalog-id> <dictionary-id> <version> <publisher-userKey> <absolute-data-root> <reject-existing|replace-verified>
```

离线 `verify` 同时校验签名和原件格式，但不查询当前可用状态，不能据此声称版本仍允许在线安装。
安装命令会执行上述在线检查；`catalog-id` 是调用方固定的本地来源标识，生产产品应把它绑定到唯一 origin/信任配置。
操作实际 App 的字典数据目录前关闭 App；当前 CLI 不拥有 App 进程内的编辑锁，GUI 接入仍需在既有命令串行边界内执行。

- 字典默认应使用 `reject-existing`；升级可使用 `replace-verified`，本地修改过的内容不会被覆盖。本入口没有强制覆盖策略。
- 字典安装记录保存签名、发布者和版本；保留既有崩溃恢复与本地修改状态识别。
- 插件安装结果的 `selected` 为 false，除非该相同摘要此前已经被用户选择；安装不改变当前选择。
  显式启用继续使用 `glyphshift-plugin select` 的 Runtime 预检和目标启动边界。
- 插件存储当前沿用精确摘要批准回执，不新增发布者来源数据库。后续 GUI 来源展示和已安装插件的远程撤销策略仍待定义。
- 请求完成后远端状态仍可能变化，第二次 proof 查询是安装前检查，不是覆盖本地提交期间的服务端事务或永久执行许可。

## 验收

```text
./scripts/test-registry-client.ps1 -RegistryExecutable <approved-registry-local-executable>
```

入口运行定向合同测试、当前两个 crate 的 Clippy 和构建，然后使用真实 Go Registry、临时测试密钥及验证证书的 HTTPS 通道：
字典安装/升级、本地修改冲突、插件幂等安装但不启用、撤销 key、错误 TLS 信任和隔离版本拒绝。
可选 `-AdapterPackage <reviewed-gsp>` 验收已有原生包；默认使用不可执行的合成 PE/GSP。
不从测试包加载 DLL。所有测试输入、临时 TLS/签名密钥、服务端数据库和安装证据位于 `local-test/`，辅助服务退出时清理进程。

CI 将 Registry 测试提供方 checkout 到本机测试目录，版本固定在
[`registry.lock.json`](../crates/product/registry-client/registry.lock.json)。它只用于跨语言验收，不是运行时相邻源码依赖。
Rust 生产库没有依赖 Go 可执行文件；主产品静态检查器的 stdin 协议保持不变。

生产部署、正式信任配置分发、在线检索/详情界面、桌面账户和深链接仍是后续工作。

## 字典发现与详情

```text
glyphshift-registry search-dictionaries <https-origin> <trust.json> <query.json>
glyphshift-registry dictionary-detail <https-origin> <trust.json> <dictionary-id> <version> <publisher-userKey>
```

查询文件为 JSON，例如 `{"text":"menu","sourceLocale":"en-US","targetLocale":"zh-CN","tag":"menus","size":50}`。
可选 `cursor` 使用上一页的 `nextCursor`；改变筛选或页大小时清空游标。空页仍可能有下一页，直到 `nextCursor` 为 null。

目录展示每个字典最新可用稳定版本，详情可访问仍有效的历史/预发行版本。展示文本和标签是原件的有界预览，不表示自动翻译或软件兼容。
客户端限制响应体、条目数、字段、版本和重复 ID，拒绝循环游标；只查询固定 origin，不接收远端分页 URL。
目录及详情经过 HTTPS 获取，**不是客户端已验签的安装结果**。从条目创建 `ReleaseRequest` 可固定用户选择的发布者及版本，安装再执行完整证明/原件检查。

桌面后续接入应在后台执行网络请求，安装提交仍使用既有编辑锁；不能把现有 GUI 的强制覆盖按钮直接连接到拒绝强制覆盖的 Registry 安装入口。
