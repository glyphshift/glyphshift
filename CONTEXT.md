# Glyphshift 领域语言

Glyphshift 是通用运行时界面替换工具。当前交付面向 Windows，领域模型将 Platform 作为独立
兼容事实；核心只组合已声明、已验证的运行能力，不包含具体软件品牌或专属分支。

## 产品

**工作流（Workflow）**：一组可保存、可启停的运行期望。一个工作流包含一个或多个软件目标，
每个目标引用有序词典集合。避免称为“任务”或“软件配置”。

**软件（Software）**：用户登记的目标应用身份，包括名称、说明和程序绑定。软件不拥有本次
运行要使用的词典或字体策略。

**词典（Dictionary）**：可独立编辑、安装、发布和复用的纯翻译资产，承载便携元数据以及唯一的
`source → translation` 映射。词典不拥有位置、语境、字体、Platform、Technology、Adapter、Hook
或保护原文策略。

**字典元数据（Dictionary Metadata）**：词典自身的身份、发布版本、语言、作者、许可、主页与
标签。下载来源、内容摘要、签名、安装状态和运行配置不属于字典元数据。

**字典发布（Dictionary Release）**：以 Dictionary ID 与 release version 唯一标识的一次不可变
词典内容发布。_Avoid_: 本地 revision、Catalog 条目、安装状态。

**字典目录（Dictionary Catalog）**：供用户发现和解析 Dictionary Release 的远端产品索引，
拥有查询展示与 Artifact Descriptor，但不拥有已安装词典或 Runtime 状态。_Avoid_: Dictionary Library。

**制品展示（Artifact Presentation）**：Catalog 按 locale 提供的名称、摘要、说明与标签，只影响
发现和展示；缺少目标 locale 时回退到发布声明的默认展示。_Avoid_: UI Locale、Dictionary Metadata。

**字典安装（Dictionary Installation）**：本机 Dictionary 资产与一个已验证 Dictionary Release
之间的来源关联；本地内容改变后关联仍保留，但状态变为 modified。_Avoid_: 下载任务、工作流启用。

**发布者身份（Publisher Identity）**：信任验证确认的发布主体，而不是 payload 或 Catalog 自报的
普通文字字段。_Avoid_: author、vendor display name。

**字体策略（Font Policy）**：工作流目标拥有的可选渲染配置，包含有序字体候选与字体覆盖范围。
它不是独立资产，也不进入 Dictionary、Software 或 Probe Run。_Avoid_: Font Profile、字体方案绑定。

**字体覆盖范围（Font Coverage）**：字体策略决定何时替换字体的语义选择。`dictionary_matches`
只作用于最终 Translation Snapshot 命中的原文；`all_observations` 作用于所选且支持字体替换的
Adapter 捕获的全部文字。_Avoid_: 全部位置、指定位置、main-ui。

**翻译词条（Translation Entry）**：由非空原文和非空译文构成的唯一映射。同一词典内原文唯一；
空译文和“保持原文”不属于翻译词条。_Avoid_: Replacement Rule、Keep Rule。

**保护策略（Protection Policy）**：显式阻止某段原文被后续翻译命中的独立策略。它不伪装成空
译文，也不进入 Dictionary；需要真实产品用例后再建立持久合同。_Avoid_: Keep Translation。

**工作流目标（Workflow Target）**：工作流中针对一个软件的组合根，分别包含 Adapter Plan、
有序词典绑定和可选字体策略。所需能力从组合结果推导，不由软件行开关配置。

**区域绑定（Region Binding）**：工作流目标把一个 Dictionary 与可被 Runtime 真实识别的区域
选择器组合起来的运行配置。当前 Adapter 没有稳定区域信号，因此首版只有 `all`，不得用猜测的
Location 冒充区域。_Avoid_: Dictionary Location、逐词条 Region。

## Runtime

**Runtime Bundle**：本地授权的 Controller、目标 Runtime 与 Capability Adapter 发布集合。
Bundle 提供可验证的机器描述和本地化 Catalog，不包含产品工作流。

**Capability Adapter**：在明确 seam 上实现观察或写回能力的具体 Adapter，例如
`ExtTextOutW` 或 `GdipDrawString`。第一方和第三方 Adapter 使用同一 Descriptor、Registry 与
协商模型。_Avoid_: Hook Type、GDI Adapter 作为唯一身份。

**Platform**：目标和 Adapter 的操作系统兼容事实，例如 Windows 或 macOS。Platform 不进入
Dictionary，也不作为 Adapter 显示名称前缀。

**Technology**：Adapter 的受控技术分类，例如 GDI、GDI+、Core Text。Technology 只用于分组和
说明，不能被激活，也不代替具体 Adapter。

**Adapter Plan**：工作流目标选择的有序 Adapter 集合及执行策略。首版执行策略是 parallel；
Technology 多选只筛选 Catalog，Adapter 多选才改变执行计划。

**文字观测（Text Observation）**：从宿主绘制或协议事件解码出的文字出现事实。观察本身不
代表替换成功。

**捕获会话（Capture Session）**：对一个已授权目标进程和一组 Adapter 的有界观察运行，只收集
文字事实，不承诺可写回或可翻译。_Avoid_: Translation Session、自动 Hook 扫描。

**探针任务（Probe Run）**：可恢复的持续观察任务，绑定一个软件、一个词典和一组 Adapter。
它只保存运行状态与观测事实，不拥有翻译内容。_Avoid_: Probe Workspace、探针字典。

**观测索引（Observation Index）**：探针任务的本地证据集合，按原文关联 Adapter、出现次数、
时间与忽略状态；它不进入 Dictionary，也不是用户需要单独管理的目录。_Avoid_: Capture Catalog、
Technical Dictionary、Dictionary Draft。

**替换决策（Replacement Decision）**：对一次观测产生 Pass、Text-only、Font-only 或
Text+Font 的结果。所有失败路径必须 fail-open。

**Generation**：一份完整 Runtime Publication 的版本。只有目标 Runtime 的 ACK 才能推进
实际应用代次。

**运行状态（Runtime State）**：目标实际确认的能力、Generation 和错误，是工作流期望的短期
执行结果，不是持久配置。

## Translation

**Translation Snapshot**：由有序词典集合编译出的不可变文字规则。

**Compiled Font Policy**：由工作流目标的字体策略编译并与 Translation Snapshot 一起发布的不可变
字体决策。它在词典命中模式下复用 Translation Snapshot 的最终匹配集合。

**Artifact Descriptor**：在线 Catalog 在 Dictionary payload 外保存的 media type、大小、摘要、
下载地址、发布者身份和签名来源。它不是 Dictionary Metadata。

## Application

**App Settings**：应用级设备偏好聚合。`AppSettings/1` 仅包含界面语言偏好与主题偏好；首次运行
默认深色，语言默认跟随系统。Dictionary、Font、Adapter、Software 与 Workflow 都不能进入它。

**UI Locale**：Glyphshift 自身菜单、按钮、提示与错误的展示语言。它与目标软件语言、Dictionary
内容语言及动态 Artifact presentation locale 相互独立。

**Command Error**：桌面后端跨 IPC 返回的稳定语义失败合同。`CommandError/1` 由 code、类型化
参数与可选 diagnostic id 构成，不携带 Vue I18n key 或本地化句子。

## 不变量

- Core、Desktop 和 GUI 不按软件品牌、Adapter ID 或可执行文件名分支。
- 新增只使用既有能力的软件，只增加 Extension/配置，不修改通用 Module。
- 工作流期望、软件身份、词典内容、Adapter Catalog 和 Runtime 实际状态必须分开；字体策略只属于
  Workflow Target。
- Dictionary metadata 与 entry 都不能携带 Location、Context、字体、Platform、Technology、Adapter、
  Hook 或执行配置；entry 只能是非空 `source + translation`。
- 只有 Adapter 或专用 Detector 实际提供稳定区域信号时，Region Binding 才能增加 `all` 以外的
  选择器；UI 标签、窗口猜测和硬编码路由不构成区域事实。
- 一个 Probe Run 必须绑定且只绑定一个 Dictionary；探针发现的原文只有在存在非空译文后才成为
  Translation Entry，技术来源与忽略状态始终只属于 Observation Index。
- Probe Run 与 Dictionary 不做转换或双向同步；探针编辑直接修改其绑定 Dictionary，已有
  Dictionary 可以直接继续探针工作。
- Presentation 不能改变 Registry 使用的 Platform、Architecture、ABI、Feature 或执行入口事实。
- 当前未发布结构直接使用 Dictionary `/2`、Workflow `/3` 与 Target Runtime
  Deployment `/2`；不保留旧结构的兼容读取、迁移或双写。
- 普通界面不显示进程标识、Controller token、DLL 路径或内部 Adapter ID。
- 帮助页从 Runtime Bundle 的公开 Catalog 展示 Adapter 名称、版本、Platform、Technology、Feature
  与配置要求；标题栏不显示桌面服务连接徽标。
- 设置不保存在线翻译器或服务地址。当前 Dictionary 是本地版本化资产；未来下载来源与安装状态
  进入独立 Catalog/Artifact seam，不进入 Dictionary metadata。
- 顶部主题切换与设置页操作共同写入唯一 App Settings；页面和组件不各自维护主题副本。
- 本机路径、实机样本、截图与日志只存在于被忽略的 `target/local-test/`。
