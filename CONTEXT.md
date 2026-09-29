# Glyphshift 领域语言

Glyphshift 是面向桌面软件的运行时界面翻译工具。领域模型把用户资产、运行期望、目标实际状态与
技术能力分开；任何一个概念都不能代替另一个概念。

## 用户资产

**工作流软件设置（Workflow Application Settings）**：
工作流里的程序名称、文件绑定和兼容方式选择。用户直接在工作流第一步选择程序，不再维护独立软件资料库。内部稳定绑定负责精确路径授权及运行所有权，不拥有字典或字体策略。
_Avoid_: 独立软件管理页、运行实例、翻译任务

**最近软件（Recent Applications）**：
最近选过的程序快捷记录，最多保留 20 项。清空只影响选择列表，不删除工作流引用的程序绑定或停止运行。
_Avoid_: 必须维护的软件资产、正在运行的进程列表

**词典（Dictionary）**：
可独立编辑和复用的翻译资产，包含便携元数据以及原文到可选译文的条目。词典不拥有运行技术、字体或目标位置。
_Avoid_: Hook 配置、Adapter 配置、字体方案

**词典元数据（Dictionary Metadata）**：
词典自身的身份、版本、语言、作者、许可证、主页与标签。下载来源和安装状态不属于词典元数据。
_Avoid_: Catalog 状态、运行配置

**待翻译词条（Pending Dictionary Entry）**：
词典中原文非空、译文为空的条目。它可以参与编辑和 AI 规划，但不会成为运行时替换规则。
_Avoid_: 保持原文规则、空字符串替换

**翻译词条（Translation Entry）**：
同一词典内由唯一非空原文和非空译文组成的映射。
_Avoid_: Replacement Rule、Keep Rule

**工作流（Workflow）**：
一组可保存、可启停的持续运行期望，只包含一个软件目标，并拥有收集与翻译操作。
_Avoid_: 一次性任务、软件配置

**工作流目标（Workflow Target）**：
工作流中针对一个软件的组合根，拥有 Adapter Plan、有序词典集合和可选字体策略。
_Avoid_: 软件行开关、全局翻译配置

**字体策略（Font Policy）**：
工作流目标选择保留原字体、统一默认字体或优先字典设置，包含候选字体、字号比例与覆盖范围。字典可携带字体和字号偏好，两者分别继承。字体策略不是独立资料库资产。
_Avoid_: Font Profile、软件字体设置

**字体覆盖范围（Font Coverage）**：
字体策略决定何时使用候选字体的语义选择：仅作用于词典命中，或作用于所选 Adapter 捕获的全部文字。
_Avoid_: 页面位置、主界面区域

## 工作流收集

**写入字典（Write Dictionary）**：工作流所选字典中的一本，用来接收新原文和 AI 译文。不选择则不收集。它是普通字典，不存在临时字典资产。

**附加字典（Attached Dictionary）**：工作流内除写入字典以外的所选字典。按原文存在性排除追加，与译文是否为空无关；同时参与有序运行时翻译。

**观察记录（Observation Record）**：工作流内部保存的来源、次数、时间和忽略状态；显式归属于工作流，不提供独立任务入口。不同写入字典使用不同记录。

**收集暂停（Collection Pause）**：只暂停新原文收集，保留工作流翻译。停止运行关闭整个工作流。

## Runtime

**Runtime Bundle**：
Glyphshift 在本机用于连接目标和执行界面能力的受信发布集合。它提供可验证的能力目录，不包含用户工作流。
当前发布将公共 Controller、Target Runtime 与适配器一起打包；它不等同于单个适配器插件包。
_Avoid_: 工作流包、词典包

**适配器（Capability Adapter，简称 Adapter）**：
在明确界面技术路径上提供文字观察、文字替换或字体替换能力的执行模块。
按引擎、框架或稳定 API 能力定义，软件和游戏是验证样本。Adapter 是选择和激活能力的单位，
不与源码仓库、安装包或 DLL 强制一一对应。
_Avoid_: Technology、Hook Type、GDI 作为单一 Adapter 身份、插件包

**原生适配器制品（Native Adapter Artifact）**：
适配器可供加载的原生二进制文件，当前 Windows 实现主要为 DLL。一个插件包可以携带多个 DLL；
支持 DLL、辅助程序、脚本或资源不因此自动成为独立 Adapter。DLL 是运行制品，不是插件包格式。
_Avoid_: 一个 DLL 必定对应一个插件包、把所有 DLL 都列为可选适配器

**Controller / Target Runtime**：
Controller 负责目标连接与控制；Target Runtime 在目标进程内协调适配器与运行能力。
它们是公共运行组件，按需要的目标架构提供，不应在每个适配器插件包内重复携带。
_Avoid_: 插件 SDK、某个技术族专属适配器

**Adapter SDK / Native ABI**：
SDK 是开发适配器使用的合同与辅助接口；Native ABI 是原生制品与宿主之间的二进制调用合同。
SDK 版本、ABI 版本、适配器版本和插件包版本分别表达不同的兼容边界，不能互相替代。
编译时依赖 SDK 不表示用户安装插件时需要安装一份完整开发工具链。
_Avoid_: 公共 Runtime、插件包版本等于 ABI 版本

**Platform**：
目标和 Adapter 的操作系统兼容事实，例如 Windows。它不进入词典，也不作为 Adapter 名称前缀。
_Avoid_: 产品分类、词典属性

**Technology**：
用于解释和筛选 Adapter 的界面技术分类，例如 GDI、GDI+ 或 DirectWrite。Technology 不能被直接启用。
_Avoid_: Adapter、执行计划

**Adapter Plan**：
工作流目标或探针选择的 Adapter 集合及执行策略。选择 Technology 只筛选候选，选择 Adapter 才改变计划。
_Avoid_: Technology Filter、自动 Hook 扫描

**文字观测（Text Observation）**：
目标界面出现某段文字的事实。观测不代表已有译文，也不证明替换最终可见。
_Avoid_: Translation Entry、替换成功

**替换决策（Replacement Decision）**：
Runtime 针对一次观测选择保持原样、替换文字、替换字体或同时替换的结果。
_Avoid_: 最终像素证明、写回成功

**运行状态（Runtime State）**：
目标实际确认的能力、应用代次和错误。它是工作流期望的短期执行结果，不是持久配置。
_Avoid_: Workflow、启用开关

**运行诊断（Runtime Diagnostics）**：
目标最近生成的有界替换决策证据，用于解释匹配和恢复问题，不用于证明最终画面。
_Avoid_: 成功审计、像素验证

**Generation**：
目标确认应用的一份完整运行配置代次。
_Avoid_: 词典版本、工作流 revision

## 自动翻译

**Translation Profile（翻译配置）**：
可复用的翻译 Provider 连接与请求策略，包含协议、服务地址、可选模型或部署、可选明文 API Key、
Provider 专属参数、分批、超时、并发、重试和本机过滤规则。具备大模型能力的 Provider 还可包含提示词与
推理强度。旧代码中的 AI Profile 是这一概念的兼容名称。
_Avoid_: App Settings、一次翻译任务

### Translation Task（翻译任务）

全局唯一的后台翻译执行对象，绑定一次候选快照、一个目标词典和启动时的 Profile；拥有批次状态、写回、
取消、终态与 Provider 实报 usage。任务期间目标词典只读，应用与目标软件不被锁定。
_Avoid_: Provider 请求、前端进度弹窗、工作流任务、探针任务

**Translation Provider**：
把统一翻译请求转换为具体服务通信的 Adapter。当前可由大模型协议（OpenAI Responses、Anthropic
Messages、Gemini、Ollama、Codex）或专用机器翻译协议（Microsoft Translator）实现。任务层只依赖统一
请求、结果、批量策略和 usage，不根据 Provider 是否为 AI 决定调度或写回。
_Avoid_: 模型名称、服务地址、Translation Job

**Provider Protocol**：
Translation Profile 选择的供应商通信合同，例如 OpenAI Responses、Anthropic Messages、Gemini、
Ollama 或 Microsoft Translator。
_Avoid_: 模型名称、服务地址

**Translation Plan**：
针对一个词典草稿或探针联合视图生成的短期候选集合，说明哪些空白项将翻译、哪些内容被跳过。
_Avoid_: Dictionary、任务结果

**Translation Batch Policy**：
Provider 根据 Profile 声明的单批限制，可同时包含条目数和源字符数。超出上限的候选由 Translation Job
自动继续处理；不同 Provider 可以有不同限制。
_Avoid_: 全局 App Settings、Token 限额

**Translation Job**：
使用一个 Translation Profile 执行 Translation Plan 的可查询、可取消运行。它保留已完成结果，并只把仍为空白且未变化的译文写回。
_Avoid_: Workflow、持续 Runtime 翻译

**翻译运行记录（Translation Run Record）**：
一次 Translation Job 完成或取消后留下的持久、脱敏诊断事实，包含规模、批次、耗时、请求尝试和供应商 usage，但不复制原文或译文。
_Avoid_: 词典历史、请求日志、聊天记录

## 目录与发布

### 适配器插件

以下为已确定的领域与命名约定。`.gsp` 本地打包、校验、不可变安装与版本选择已提供开发者试点；
Registry 审核发布证明与 Rust 指定版本验签安装已实现；生产部署、作者个人签名与插件管理界面尚未接入。
具体清单与当前限制见[插件包合同](docs/plugins.md)和[在线安装合同](docs/registry-client.md)。

**适配器插件包（Adapter Plugin Package，简称插件包）**：
适配器能力的发布、下载、安装和升级单位。一个包可提供一个或多个 Adapter，携带对应的 DLL、
必要支持文件、清单和许可声明。共同维护、共同升级的技术族能力可归为一包，不按软件品牌拆包。
用户侧称“适配器插件”；技术文档中用“插件包”强调交付边界，用“适配器”强调执行能力。
_Avoid_: 单个 DLL、源码仓库、词典、Translation Provider

**GSP（Glyphshift Plugin，`.gsp`）**：
适配器插件包的文件格式与扩展名约定。内部采用标准 ZIP 容器和版本化清单，不自创压缩算法。
扩展名用于识别和安装入口，不是信任依据；安装必须校验清单、制品和发布来源。
词典保持独立的资产与交付格式，不因同属在线资源而成为可执行插件。
_Avoid_: DLL 的另一种扩展名、通用资源压缩包、改后缀即可加载

**包身份（Package ID）与适配器身份（Adapter ID）**：
Package ID 唯一标识持续发布的插件包，例如 `glyphshift-adapter-qt`；Adapter ID 标识包提供的
具体运行能力。包名、展示名、文件名、仓库名与 Adapter ID 不混用。既有 Adapter ID 不因拆仓或改包名而改变。
_Avoid_: 用 DLL 文件名推导永久身份、用展示名称匹配工作流

**插件发布（Plugin Release）与平台变体（Package Variant）**：
插件发布是一个 Package ID 与发布版本对应的不可变交付；平台变体是同一发布面向操作系统和目标架构的制品组合。
一个离线包可包含多个变体，在线分发可按需交付；x64 桌面 App 仍可能需要 x86 目标适配器。
包版本管理整体交付，包内 Adapter 版本描述具体能力，二者不要求数值一致。
_Avoid_: 源码提交号、本地工作流 revision、只按桌面壳架构选包

**插件清单（Plugin Manifest）**：
描述包身份、版本、提供的 Adapter、平台变体、ABI/Runtime 兼容要求、依赖、文件摘要与许可入口的机器可读合同。
摘要确认内容完整性，发布者验证确认来源；包内自报作者或摘要不能单独建立信任。
_Avoid_: 展示文案、文件名约定、自报可信

**插件安装（Plugin Installation）**：
本机对已验证插件发布及所需变体的安装记录。安装不等于工作流已选择或激活其中的 Adapter。
内置与外置来源必须解析为明确版本，不能因来源不同而重复激活同一 Adapter ID；停止适配器不保证 DLL
立即卸载，升级或卸载可能需要目标软件退出。
_Avoid_: 下载完成、工作流开关、停用即热卸载

**在线 Registry 与本地 Adapter Registry**：
在线 Registry 拥有可发现的插件/词典发布与制品分发合同；本地 Adapter Registry 是运行时能力注册与解析组件。
Hub 负责社区展示，源码仓库负责开发协作；它们都不等同于本地插件安装状态。
_Avoid_: 同名 registry crate 已代表线上发布服务、建仓即完成插件化

### 词典与共享发布术语

**Registry 发布证明（Registry Attestation）**：
由受信 Registry 签署的不可变声明，绑定发布者 Public User Key、资源身份、版本、原件摘要和静态检查结果。
它不等同于作者个人私钥签名，也不证明插件安全或授予目标进程执行权限。
_Avoid_: 文件摘要即发布者签名、验签通过即自动启用

**指定版本安装（Exact-version Installation）**：
明确资源类型、ID、版本与期望发布者后，从固定 Registry 验证当前可用状态、签名及实际原件，并交给本地安装存储。
它不拥有全站搜索、账户登录或工作流启用；离线验签不能代替在线可用状态检查。
_Avoid_: 任意下载 URL 安装、安装成功即当前目标已加载

**词典发布（Dictionary Release）**：
以词典身份和发布版本唯一标识的一次不可变内容发布。
_Avoid_: 本地 revision、安装状态

**词典目录（Dictionary Catalog）**：
供用户发现词典发布的远端索引。它不拥有本地词典和 Runtime 状态。
_Avoid_: Software Library、已安装词典列表

**词典安装（Dictionary Installation）**：
本地词典与一个已验证词典发布之间的来源关联。本地修改不会抹掉来源，但会改变安装状态。
_Avoid_: 下载任务、工作流启用

**Artifact Descriptor**：
目录在词典内容之外保存的下载、大小、摘要、媒体类型和签名描述。
_Avoid_: Dictionary Metadata

**Artifact Presentation**：
目录按展示语言提供的名称、摘要、说明和标签，只影响发现界面。
_Avoid_: UI Locale、Dictionary Metadata

**Publisher Identity**：
信任验证确认的发布主体，不是词典内容自报的作者或厂商文字。
_Avoid_: Author、Vendor display name

## 应用

**App Settings**：
当前设备上的 Glyphshift 偏好，包括界面、快捷键、启动、关闭和权限。用户资产不属于 App Settings。
_Avoid_: AI Profile、Workflow、Software、Dictionary

**工作区数据根（Workspace Data Root）**：
当前用户本机保存软件、词典、工作流、探针、翻译配置与 App Settings 的品牌目录。读取采用字段、记录、
文件三级容错；bundle identifier 与隔离测试根不属于产品工作区身份。
_Avoid_: bundle identifier 目录、测试证据目录、整份配置一次性拒绝

**UI Locale**：
Glyphshift 自身菜单、按钮和提示使用的语言，与目标软件语言、词典语言和目录展示语言相互独立。
_Avoid_: Source Locale、Target Locale、Artifact Presentation Locale

### Dictionary Discovery（字典发现）

Registry 对已审核、活动且证明有效的字典提供公开搜索与精确版本详情。默认每个字典展示最新可用稳定版本；名称/说明/标签为有界展示投影，不是客户端验签或安装许可。共享 Rust/CLI 与桌面 GUI 已消费该合同；桌面使用显式受信配置，在后台搜索和准备安装。安装继续固定用户选择的发布者与版本，重新校验 proof 和原件。

### Prepared Dictionary（待提交字典）

后台完成 Registry 证明/原件验证后的短期内存结果，不是可保存或转发的安装许可。桌面在编辑锁内重新检查当前公钥、配置与本地修改后提交；超过 30 秒须重新准备。桌面在线目录现已按显式受信配置接入，账户和网页唤起仍待实现。
