# Glyphshift 领域语言

Glyphshift 是面向桌面软件的运行时界面翻译工具。领域模型把用户资产、运行期望、目标实际状态与
技术能力分开；任何一个概念都不能代替另一个概念。

## 用户资产

**软件（Software）**：
用户登记的目标应用身份，包含名称、说明和程序绑定。软件不拥有词典、Adapter 选择或字体策略。
_Avoid_: 运行实例、软件配置、翻译任务

**软件资料库（Software Library）**：
可供工作流和探针复用的软件集合。它不是当前正在运行的进程列表。
_Avoid_: 运行目标全集、进程列表

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
一组可保存、可启停的持续运行期望，由一个或多个工作流目标组成。
_Avoid_: 一次性任务、软件配置

**工作流目标（Workflow Target）**：
工作流中针对一个软件的组合根，拥有 Adapter Plan、有序词典集合和可选字体策略。
_Avoid_: 软件行开关、全局翻译配置

**字体策略（Font Policy）**：
工作流目标拥有的可选字体替换选择，包含有序字体候选和覆盖范围。它不是独立资料库资产。
_Avoid_: Font Profile、词典字体、软件字体设置

**字体覆盖范围（Font Coverage）**：
字体策略决定何时使用候选字体的语义选择：仅作用于词典命中，或作用于所选 Adapter 捕获的全部文字。
_Avoid_: 页面位置、主界面区域

## 探针

**探针任务（Probe Run）**：
可暂停、继续和恢复的界面文字观察任务，绑定一个软件、一个词典和一组 Adapter。它保存任务状态与观测证据，不拥有翻译内容。
_Avoid_: Probe Workspace、探针词典

**探针目标来源（Probe Target Source）**：
创建探针时选择目标身份的方式：复用软件资料库，或从当前运行的软件解析目标。
_Avoid_: 软件必须预先登记

**探针词典来源（Probe Dictionary Source）**：
创建探针时选择词典的方式：复用现有词典，或创建由本次探针拥有的临时词典。
_Avoid_: 独立探针草稿

**探针词典绑定（Probe Dictionary Binding）**：
探针任务对唯一词典的可修改引用。联合视图读取和编辑这份词典，但切换绑定不会复制或删除词典内容。
_Avoid_: 自动双向同步、观测即词条

**观测索引（Observation Index）**：
探针按原文保存的本地证据集合，包含来源 Adapter、出现次数、时间与忽略状态。它不属于词典。
_Avoid_: Capture Catalog、技术词典、词典草稿

**临时探针资产（Temporary Probe Asset）**：
一次探针创建过程中产生、由该探针明确拥有的软件或词典。用户可以保留它们，也可以在结束探针时安全清理。
_Avoid_: 根据名称或 ID 前缀猜测临时性

## Runtime

**Runtime Bundle**：
Glyphshift 在本机用于连接目标和执行界面能力的受信发布集合。它提供可验证的能力目录，不包含用户工作流。
_Avoid_: 工作流包、词典包

**Capability Adapter**：
在明确界面技术路径上提供文字观察、文字替换或字体替换能力的执行模块。
_Avoid_: Technology、Hook Type、GDI 作为单一 Adapter 身份

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

## AI 翻译

**AI Profile**：
可复用的 AI 翻译连接与请求策略，包含供应商协议、服务地址、模型、可选明文 API Key、推理强度、分批、
超时、并发、重试和本机过滤规则。
_Avoid_: App Settings、一次翻译任务

### Translation Task（翻译任务）

全局唯一的后台 AI 执行对象，绑定一次候选快照、一个目标词典和启动时的 Profile；拥有批次状态、写回、
取消、终态与实报 Token。任务期间目标词典只读，应用与目标软件不被锁定。
_Avoid_: Provider 请求、前端进度弹窗、工作流任务、探针任务

**Provider Protocol**：
AI Profile 选择的供应商通信合同，例如 OpenAI Responses、Anthropic Messages、Gemini 或 Ollama。
_Avoid_: 模型名称、服务地址

**Translation Plan**：
针对一个词典草稿或探针联合视图生成的短期候选集合，说明哪些空白项将翻译、哪些内容被跳过。
_Avoid_: Dictionary、任务结果

**Translation Batch Policy**：
AI Profile 拥有的单批条目上限。超出上限的候选由 Translation Job 自动继续处理。
_Avoid_: 全局 App Settings、Token 限额

**Translation Job**：
使用一个 AI Profile 执行 Translation Plan 的可查询、可取消运行。它保留已完成结果，并只把仍为空白且未变化的译文写回。
_Avoid_: Workflow、持续 Runtime 翻译

**翻译运行记录（Translation Run Record）**：
一次 Translation Job 完成或取消后留下的持久、脱敏诊断事实，包含规模、批次、耗时、请求尝试和供应商 usage，但不复制原文或译文。
_Avoid_: 词典历史、请求日志、聊天记录

## 目录与发布

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
当前用户本机保存软件、词典、工作流、探针、AI 配置与 App Settings 的品牌目录。读取采用字段、记录、
文件三级容错；bundle identifier 与隔离测试根不属于产品工作区身份。
_Avoid_: bundle identifier 目录、测试证据目录、整份配置一次性拒绝

**UI Locale**：
Glyphshift 自身菜单、按钮和提示使用的语言，与目标软件语言、词典语言和目录展示语言相互独立。
_Avoid_: Source Locale、Target Locale、Artifact Presentation Locale
