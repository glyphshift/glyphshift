# Glyphshift 领域语言

Glyphshift 是通用运行时界面替换工具。当前交付面向 Windows，领域模型将 Platform 作为独立
兼容事实；核心只组合已声明、已验证的运行能力，不包含具体软件品牌或专属分支。

## 产品

**工作流（Workflow）**：一组可保存、可启停的运行期望。一个工作流包含一个或多个软件目标，
每个目标引用有序词典集合。避免称为“任务”或“软件配置”。

**软件（Software）**：用户登记的目标应用身份，包括名称、说明和程序绑定。软件不拥有本次
运行要使用的词典或字体策略。

**词典（Dictionary）**：可独立编辑、安装、发布和复用的语言资产，承载便携元数据与文字规则。
词典不拥有字体、Platform、Technology、Adapter 或 Hook。

**字典元数据（Dictionary Metadata）**：词典自身的身份、发布版本、语言、作者、许可、主页与
标签。下载来源、内容摘要、签名、安装状态和运行配置不属于字典元数据。

**字体方案（Font Profile）**：可独立命名和复用的有序字体候选。字体方案不拥有词典或 Adapter；
工作流目标决定其语义适用范围。

**替换规则（Replacement Rule）**：按位置、可选语境与原文匹配，决定文字保持或替换的语言规则。

**工作流目标（Workflow Target）**：工作流中针对一个软件的组合根，分别包含 Adapter Plan、
有序词典绑定和字体方案绑定。所需能力从组合结果推导，不由软件行开关配置。

**字体方案绑定（Font Profile Binding）**：工作流目标对字体方案及其全部或指定 Location
适用范围的引用。_Avoid_: Dictionary default font、逐词条字体。

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

**替换决策（Replacement Decision）**：对一次观测产生 Pass、Text-only、Font-only 或
Text+Font 的结果。所有失败路径必须 fail-open。

**Generation**：一份完整 Runtime Publication 的版本。只有目标 Runtime 的 ACK 才能推进
实际应用代次。

**运行状态（Runtime State）**：目标实际确认的能力、Generation 和错误，是工作流期望的短期
执行结果，不是持久配置。

## Translation

**位置（Location）**：软件 Extension 声明的稳定用户界面区域标识。

**翻译语境（Translation Context）**：区分同一原文在不同位置或状态下含义的信息。

**Translation Snapshot**：由有序词典集合编译出的不可变文字规则。

**Font Policy**：由字体方案绑定编译并与 Translation Snapshot 正交发布的不可变字体决策策略。

**Artifact Descriptor**：在线 Catalog 在 Dictionary payload 外保存的 media type、大小、摘要、
下载地址、发布者身份和签名来源。它不是 Dictionary Metadata。

## 不变量

- Core、Desktop 和 GUI 不按软件品牌、Adapter ID 或可执行文件名分支。
- 新增只使用既有能力的软件，只增加 Extension/配置，不修改通用 Module。
- 工作流期望、软件身份、词典内容、字体方案、Adapter Catalog 和 Runtime 实际状态必须分开。
- Dictionary metadata 与 entry 都不能携带字体、Platform、Technology、Adapter、Hook 或执行配置。
- Presentation 不能改变 Registry 使用的 Platform、Architecture、ABI、Feature 或执行入口事实。
- 当前未发布结构直接使用 Dictionary `/2`、Font Profile `/1`、Workflow `/2` 与 Target Runtime
  Deployment `/2`；不保留旧结构的兼容读取、迁移或双写。
- 普通界面不显示进程标识、Controller token、DLL 路径或内部 Adapter ID。
- 帮助页从 Runtime Bundle 的公开 Catalog 展示 Adapter 名称、版本、Platform、Technology、Feature
  与配置要求；标题栏不显示桌面服务连接徽标。
- 设置不保存在线翻译器或服务地址。当前 Dictionary 是本地版本化资产；未来下载来源与安装状态
  进入独立 Catalog/Artifact seam，不进入 Dictionary metadata。
- 本机路径、实机样本、截图与日志只存在于被忽略的 `target/local-test/`。
