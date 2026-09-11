---
name: Glyphshift Desktop
description: A compact Windows translation workbench for workflows, software, dictionaries, and probes.
colors:
  primary-dark: "#6c8cff"
  primary-light: "#315ff4"
  canvas-dark: "#0c0f14"
  frame-dark: "#10141b"
  surface-dark: "#151a22"
  field-dark: "#191f29"
  border-dark: "#29313f"
  text-dark: "#f1f4f8"
  text-muted-dark: "#8f98a8"
  canvas-light: "#eef1f5"
  frame-light: "#e7ebf1"
  surface-light: "#fafbfc"
  field-light: "#ffffff"
  border-light: "#d8dde6"
  text-light: "#171b24"
  text-muted-light: "#6f7888"
  success-dark: "#6ed2a7"
  warning-dark: "#f0bd69"
  danger-dark: "#ff8c91"
  success-light: "#157a54"
  warning-light: "#9a5b00"
  danger-light: "#c43942"
typography:
  page-title:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.02em"
  section-title:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: 1.35
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "normal"
  label:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: 1.4
    letterSpacing: "normal"
  metadata:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "normal"
  caption:
    fontFamily: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
    fontSize: "10px"
    fontWeight: 400
    lineHeight: 1.4
    letterSpacing: "normal"
rounded:
  menu-item: "4px"
  control: "6px"
  modal: "8px"
  surface: "8px"
  section: "10px"
spacing:
  control-gap: "8px"
  page: "16px"
  form-horizontal-gap: "32px"
components:
  button-primary-dark:
    backgroundColor: "#263866"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
  button-primary-light:
    backgroundColor: "{colors.primary-light}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
  field-dark:
    backgroundColor: "{colors.field-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.control}"
  field-light:
    backgroundColor: "{colors.field-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.control}"
  management-surface-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.surface}"
  management-surface-light:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.surface}"
---

# Design System: Glyphshift Desktop

## Overview

**Creative North Star: “Windows Translation Workbench”**

Glyphshift 使用成熟 Windows 管理工具的视觉语法：紧凑、连续、精确，所有界面都服务于“找到目标、
验证文字、维护词典、启用工作流”这条操作链。品牌感来自一致的钴蓝焦点、明确的层级和小尺寸下仍然
可靠的交互，而不是大面积装饰。

这是一个 **Operate** 模式的桌面应用。列表页强调扫描和批量管理；详情页强调上下文、草稿保护和
当前动作；帮助与 README 属于 **Read** 模式，用连续内容解释首次使用和恢复路径。两种模式共享同一
字体系、颜色角色和内容轴。

**Key Characteristics:**

- 48px 单层标题栏同时承载品牌、一级导航、工具入口与窗口控制。
- 高密度表格和表单使用薄边界、连续表面与克制的 4–10px 圆角。
- 钴蓝只表示当前选择、主要动作和显式焦点；成功、警告和危险色具有严格语义。
- 深色与浅色主题使用相同的层级关系，不通过反转或高饱和制造第二套视觉世界。
- 不使用营销式仪表盘、玻璃、渐变、发光、嵌套卡片或无意义的大留白。

## Colors

Glyphshift 的色彩由一个钴蓝主色和一组冷中性色构成。深色主题以近黑画布逐层升亮，浅色主题以冷灰
框架包围纯白或近白内容面；交互 hover/selection 不复用静态层级色。

### Primary

- **Glyphshift Cobalt** (`#6c8cff` dark / `#315ff4` light)：当前导航、主要动作、选中页码、开关和
  键盘焦点。
- **Contained Cobalt** (`#263866` dark)：深色主题主要实心按钮，避免直接使用高亮钴蓝造成刺眼面积。

### Neutral

- **Graphite Canvas** (`#0c0f14`) 与 **Cold Gray Canvas** (`#eef1f5`)：应用主画布。
- **Frame** (`#10141b` dark / `#e7ebf1` light)：标题栏与框架层。
- **Surface** (`#151a22` dark / `#fafbfc` light)：表格、设置分区、Modal 与内容表面。
- **Field** (`#191f29` dark / `#ffffff` light)：输入、选择器和可编辑区域。
- **Border** (`#29313f` dark / `#d8dde6` light)：表面、行、分区与控件边界。
- **Primary Text** (`#f1f4f8` dark / `#171b24` light)：标题、对象身份和关键值。
- **Muted Text** (`#8f98a8` dark / `#6f7888` light)：说明、元数据和次级上下文。

### Status

- **Success** (`#6ed2a7` dark / `#157a54` light)：只表示 Runtime 已确认或操作完成。
- **Warning** (`#f0bd69` dark / `#9a5b00` light)：可恢复问题、权限提示和需要用户继续处理的状态。
- **Danger** (`#ff8c91` dark / `#c43942` light)：删除、确定失败和不可接受的输入。

**The Semantic Color Rule.** 状态不能只靠颜色表达；标签、图标或可读文案必须同时说明含义。

**The Accent Budget Rule.** 钴蓝只用于当前选择和主要动作。静态说明、普通图标和容器边界保持中性。

## Typography

**Display and Body Font:** Segoe UI Variable，回退到 Noto Sans SC、Microsoft YaHei 与 system-ui。

字体系服务 Windows 桌面密度：对象身份明确，说明紧凑但不挤压，数字与计数保持稳定。普通界面不为
“技术感”使用等宽字体；仅键帽、程序化值或真实代码需要 monospace。

### Hierarchy

- **Page Title**（600，20px，`-0.02em`）：页面和独立详情唯一 `h1`。
- **Section Title**（600，13px，`-0.01em`）：设置分区、帮助章节和表单小节。
- **Body**（400，12px）：主要说明、对象内容和普通阅读文本。
- **Label**（600，11px）：表头、字段标签、按钮和紧凑功能文字。
- **Metadata**（400，11px）：状态解释、路径辅助信息、时间和次级事实。
- **Caption**（400，10px）：非关键计数与徽标；不能承载错误恢复、对象上下文或必读说明。

**The Functional Copy Floor.** 可操作文案不得小于 11px；错误和恢复说明优先使用 11–12px 并允许换行。

## Layout

应用占满桌面窗口，`body` 不滚动。每个页面自己管理唯一主滚动区；表格正文、详情画布或工具页内容
分别承担滚动，避免嵌套页面滚动。

- **Title bar:** 固定 48px，高度内整合品牌、四个主导航、主题、帮助、设置和窗口控制。
- **Primary page axis:** 一级页面统一使用 16px 页面 inset、“图标页头 + 满宽内容”；Settings 与 Help
  不建立独立居中内容轴或额外页面级水平 padding。详情页使用“全宽详情头 + 主画布”。
- **Form grid:** 184px 标签轨 + 弹性控件轨，水平间距 32px；容器不足 620px 时折为单列。
- **Management tables:** 工具栏、表头、正文与 56px 分页脚形成连续表面。横向表格固定身份列与操作列。
- **Workflow editor:** 左侧单层分区栏切换基础、软件、词典和字体四个任务面；右侧只显示当前任务。
- **Reference viewports:** 960×640 与 1440×900 保持相同信息结构；紧凑宽度收缩标签或换行，不隐藏
  核心运行操作。

列表空态占据表头与分页之间的正文区域，不额外制造卡片。复杂详情允许一个明确分区栏；简单详情只
使用居中的配置面，不创建伪侧栏填充空白。

## Elevation & Depth

Glyphshift 以色阶和 1px 边界表达绝大多数深度，默认不依赖阴影。画布、框架、内容面、内嵌表面和
字段按照固定语义逐层变化；同一层级不重复套壳。

主要 contained 按钮是例外：它使用轻微内描边、顶部高光和有垂直偏移的柔和阴影，表达可按压性。
hover 阴影从 `0 2px 5px` 增加到 `0 3px 8px`，active 收回到 `0 1px 2px`。Modal 依靠 overlay 和
内容层级建立焦点，不叠加装饰性 glow。

**The Flat-by-Default Rule.** 先用语义色阶和边界解决层级；只有需要表达按压或受保护焦点时才使用阴影。

## Shapes

Glyphshift 使用小半径、近矩形的 Windows 工具形态：

- 菜单项 4px。
- 输入、选择器、紧凑按钮与键帽 5–6px。
- 表格、工作表面和 Modal 8px。
- 设置分区等独立配置容器 10px。
- 胶囊只用于小型状态徽标、开关或滚动条 thumb，不作为普通按钮和卡片轮廓。

所有大表面使用薄边界；不在同一容器同时堆叠重边框和宽阴影。表格行依靠分隔线与 hover 色变化，
不把每一行改造成独立圆角卡片。

## Components

### Navigation

标题栏是唯一一级导航。工作流、软件、词典和探针使用文字 + Tabler 图标；当前项使用底部钴蓝线和
高亮文字。翻译任务、帮助与设置只作为标题栏工具入口出现，不占用主导航文字位置；活动翻译任务在
入口旁显示完成批次，不把进度扩展成新的主导航标签。

### Page headers

所有一级页面使用同一满宽页头：40px 图标、20px 标题和右侧主要动作；只有需要补充非显然范围或后果
时才显示单行说明。
独立详情使用 64px 或 80px 高的横向标题带，保留返回、对象标题、未保存状态和直接动作。对象身份与
操作不得被重复包进第二张卡。

### Management tables

表格工具栏统一承载搜索、筛选、显示列和页面动作；选中行后在同一位置出现批量工具栏。身份列、状态
列和操作列使用固定语义，行级高频操作直接显示，低频动作进入命名菜单。分页完整提供首尾页、前后页、
页码和每页数量。

### Forms and settings

表单使用共享的字段行、分区和 Modal。Settings 使用通用、软件、字体、语言、文字处理五个局部导航；通用页按外观、AI 翻译、快捷键、应用与权限等主题分区。
AI 翻译在同一分区管理连接与模型。立即生效的设置不增加
保存按钮；草稿型编辑必须显式保存并保护未保存状态。Profile 的推理强度只占一个选择器，默认明确
标记“关闭”；供应商不支持的虚假档位不出现在选项中。API Key 随 Profile 明文保存在
本机配置文件中；字段默认遮蔽并使用可访问的显示/隐藏按钮，旁边明确说明本机明文风险。编辑时载入
当前 Key，删除 Profile 同时删除 Key，不依赖系统凭据库或隐藏的外部清理。

### Probe and dictionary surfaces

探针详情直接显示暂停/继续、释放连接和 AI 操作；设置、导出及临时资产管理进入次级菜单。观测与词典
译文在同一联合表中展示，但视觉上仍区分证据和内容。词典编辑使用原文/译文双列与常驻新增行，不为
每条规则打开 Modal。
探针发起后台 AI 翻译后只显示一条整体状态和“查看任务”；批次、并发、Token 与停止操作不在探针重复。
新建探针使用临时词典时，源语言与目标语言是同权重、常显、必填的并列字段；紧凑宽度折为单列，不用
虚构的自动识别文案替代源语言输入。

### Translation tasks

翻译任务使用与工作流、软件相同的满宽管理页头和任务图标，并按使用频率分为“当前任务 / 统计 / 任务
列表”三个横向 Tab；Tab 内容直接占满剩余工作区，不重复显示分区标题或解释。默认 Tab 只突出当前目标、
整体进度、耗时、写入和停止；批次是可折叠诊断信息，重试或失败时自动展开。统计按 Profile 筛选并比较
模型表现，连接协议默认隐藏、仅作为诊断显示列；统计和任务记录都复用管理表格的搜索、筛选、显示列和
完整分页。历史批次通过单条展开披露低频细节。Token 使用等宽数字但不使用仪表盘大数字；状态同时使用
文字与语义色。目标词典锁定必须在
词典详情中显示可恢复说明和“查看任务”，不能只依赖禁用控件。标题栏入口使用任务清单语义图标，不
使用语言图标。

### Help and recovery

Help 使用“使用指南 / AI 翻译 / 故障排查 / 技术与兼容 / 关于”五个横向 Tab，默认以六步完整路径教
用户完成一次实际界面翻译，并提供直达页面入口。AI Tab 解释后台任务和 Provider 实报 Token；故障
恢复按用户看到的现象命名；Adapter 版本、配置和官方文档只在技术与兼容 Tab 渐进披露；About 使用
连续资源行显示当前版本、项目仓库和作者主页，不创建营销式卡片。

### Overlays and menus

Modal 用于需要保护焦点的创建、确认和复杂编辑。菜单、Popover、Select 与 Modal 使用统一层级，
Modal 必须高于 sticky 表头和其他临时浮层。关闭后焦点返回触发器；Esc 由最内层浮层优先处理。

### Accessibility and language

核心文案使用 Vue I18n 语义 key，简体中文与 English 结构一致。图标按钮提供可访问名称，表格滚动区、
工具栏与详情标题具有明确 landmark。标题栏之前提供“跳到主要内容”入口，并支持 `Alt+M` 聚焦主内容。
系统减少动态效果时，所有 transition 降到近乎即时。

### Product copy

普通界面默认面向第一次使用运行时翻译工具的 Windows 用户。文案先说明当前结果、直接后果和下一步，
不要求用户理解内部架构。AI Profile、Provider、Runtime Bundle、Credential、revision 和 Adapter 等
实现词分别改用“AI 配置、AI 服务、运行组件、API Key、兼容方式”；只有 API Key、Token、模型名称和
技术与兼容页中的底层名词在影响用户选择时保留。错误文案采用“发生了什么 + 如何恢复”，低频诊断通过
详情渐进披露。description 只有在补充后果、前置条件、作用范围、费用、隐私、不可逆风险或失败恢复时
才存在；不得复述标题、字段名或可见选项。中英文对同一概念只使用一个稳定名称。

## Do's and Don'ts

### Do

- 用一条主要任务链组织页面，让下一步在数秒内可找到。
- 复用共享页头、表格框架、详情头、工作表面、表单分区和确认 Modal。
- 用具体原因描述错误，例如“软件未启动”或“权限不匹配”，并提供对应恢复入口。
- 在紧凑窗口中保留可访问名称、运行期主操作和完整数据语义。
- 用探针证据确认兼容性；把目标实际状态与用户期望分开显示。
- 让中英文、深浅主题和键盘路径获得同等验收。

### Don't

- 不使用首页卡片矩阵、营销式 hero、统计仪表盘、玻璃、渐变、发光或装饰纹理。
- 不把列表行、设置字段或帮助步骤分别包装成嵌套卡片。
- 不用大面积钴蓝、无语义状态色或仅靠颜色表达状态。
- 不把软件页变成功能配置页，也不创建独立字体资产导航。
- 不在普通界面暴露进程标识、内部 Adapter ID、本地模块路径、哈希或签名字节。
- 不把捕获文字、Adapter 已加载或替换决策生成表述成最终翻译已经可见。
- 不让技术目录压过首次使用与故障恢复内容。

设置与帮助使用同一套带图标和底部活动线的局部标签，滚动时固定于内容顶部，背景不透出下方文字。全局文字处理只放“不再收录”。正则规则位于字典元数据表单中，每条显示匹配、替换、开关与排序动作；编辑器及模拟原文、可选模拟译文、测试结果在本条展开。规则确认写入字典草稿，随保存字典持久化；窄容器折为单列。规则生成的运行译文只读，提示到所属字典编辑固定文字。
