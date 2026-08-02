# Glyphshift 领域语言

Glyphshift 是面向 Windows 桌面软件的通用运行时界面替换工具。核心只组合已声明、已验证的
运行能力，不包含具体软件品牌、可执行文件名或专属分支。

## 产品

**工作流（Workflow）**：一组可保存、可启停的运行期望。一个工作流包含一个或多个软件目标，
每个目标引用有序词典集合。避免称为“任务”或“软件配置”。

**软件（Software）**：用户登记的目标应用身份，包括名称、说明和程序绑定。软件不拥有本次
运行要使用的词典或字体策略。

**词典（Dictionary）**：可独立命名、编辑和复用的替换资产，承载文字规则、默认字体行为和
逐词条字体覆盖。词典可以被多个工作流引用，不从属于某个软件。

**替换规则（Replacement Rule）**：按位置、可选语境与原文匹配的规则，同时决定文字保持或
替换，以及字体跟随、保持或替换。

**工作流目标（Workflow Target）**：工作流中针对一个软件的运行期望，包含有序词典绑定。
所需能力从词典编译结果推导，不由软件行开关配置。

## Runtime

**Runtime Bundle**：本地授权的 Controller、目标 Runtime 与 Capability Adapter 发布集合。
Bundle 提供可用能力和用户级 Hook 标签，不包含产品工作流。

**Capability Adapter**：在明确 seam 上实现一种观察或写回能力的 Adapter，例如 GDI 文字替换
或 GDI+ 字体替换。第一方和第三方 Adapter 使用同一 Descriptor、Registry 与协商模型。

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

**适用 Hook（Hook Scope）**：整份词典可选的用户级写回范围。选择 GDI、GDI+ 等标签后，
文字、默认字体和逐词条字体统一受该范围限制；未指定时适用于全部兼容 Adapter。

**Translation Snapshot**：由有序词典集合编译出的不可变文字规则。

**Font Policy**：与 Translation Snapshot 正交发布的默认字体和逐词条字体策略。

## 不变量

- Core、Desktop 和 GUI 不按软件品牌、Adapter ID 或可执行文件名分支。
- 新增只使用既有能力的软件，只增加 Extension/配置，不修改通用 Module。
- 工作流期望、软件身份、词典内容和 Runtime 实际状态必须分开存储和展示。
- 普通界面不显示进程标识、Controller token、DLL 路径或内部 Adapter ID。
- 本机路径、实机样本、截图与日志只存在于被忽略的 `target/local-test/`。
