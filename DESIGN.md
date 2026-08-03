---
name: Glyphshift Desktop
description: A compact workflow, software, dictionary, and probe manager for runtime translation.
colors:
  accent: "Glyphshift cobalt (#315ff4 light / #6c8cff dark)"
  surface: "#151a22 dark / #fafbfc light"
  surface-subtle: "#10141b dark / #e7ebf1 light"
  canvas: "#0c0f14 dark / #eef1f5 light"
  border: "#29313f dark / #d8dde6 light"
typography:
  family: "Segoe UI Variable, Noto Sans SC, Microsoft YaHei, system-ui, sans-serif"
  body: "11–12px"
  metadata: "9–10px"
  page-title: "20px"
geometry:
  integrated-title-navigation: "48px"
  control-radius: "5–8px"
  table-row: "52–54px"
  page-padding: "16px"
---

# Glyphshift 设计系统

## 产品方向

Glyphshift 是高密度 Windows 桌面管理工具。主要任务分为工作流、软件、词典和探针：工作流表达
持续运行期望并拥有目标字体策略，软件只管理身份与程序绑定，词典是可复用语言资产。
启用工作流就是持续期望，没有第二个“开始翻译”动作。

界面采用成熟数据管理器语法：单层标题/导航栏、图标标题与说明、搜索和批量工具栏、满宽表格、
分页与行级操作。不使用首页卡片、软件主从详情、营销式仪表盘或大面积强调色。

## 信息架构

- **工作流：** 默认页。展示名称、说明、软件和词典摘要、期望/实际状态、启用、编辑、复制和
  删除；支持搜索、筛选、显示列、多选、分页与批量操作。
- **软件：** 管理名称、用途说明和完整程序路径；新增前显示基础接入预检，支持两段式全局快捷键
  捕获前台程序，但不展示或配置文字/字体能力。
- **词典：** 同一页面以紧凑模式切换本地 Library 与在线 Catalog，不新增一级导航。本地表展示
  名称、说明、语言、版本、规则数及 verified/modified/unmanaged 来源状态；低频 metadata 进入
  词典设置 Modal；词条表本身支持直接编辑，末行常驻空白新增入口。Catalog 表展示本地化
  presentation、语言、版本、发布者与标签，并使用后端
  cursor 分页；现有 metadata tags 同时作为可点击的精确筛选条件，不另建重复的分类模型。本地与
  Catalog 当前模式使用明确的主色选中态。标准 Dictionary `/2` JSON 通过本地页头导入、逐行导出，
  不增加发布中心或向导。
  详情页主表只展示原文和译文，不出现字体、Hook、Adapter 或内部匹配键。
- **探针：** 管理可恢复的观察任务。每个任务绑定一个软件、一个词典和一组 Adapter；详情用一张
  联合表展示原文、译文、状态与技术证据，译文编辑直接修改绑定词典。
- **Adapter：** 在 Workflow Target 中按 Platform/Technology 分组展示并多选，名称只显示具体
  Adapter；普通界面不展示内部 ID、DLL、hash 或签名。
- **帮助：** 由标题栏图标进入，使用紧凑表格展示当前 Runtime Bundle 的 Adapter 名称、说明、
  Platform、Technology、Capability、配置要求与版本；不展示内部 ID 或 DLL target。
- **设置：** 只在标题栏提供图标入口，不占主导航文字位置。设置页不提供在线翻译器或服务地址，
  只展示真实生效的界面语言与主题偏好；语言和主题均立即应用并持久化。

完整软件或词典集合不用普通下拉框承载。Workflow 独立编辑页使用可搜索、多选的管理列表，并为
每个目标维护 Adapter Plan、有序词典集合和至多一个内联 Font Policy；字体策略直接选择本机字体
候选优先级，并用紧凑的“应用范围”选择器切换“仅词典命中”或“Hook 捕获的全部文字”。字体
目录显示缓存数量，重新扫描系统字体只能由相邻的显式刷新动作触发。
Workflow 的四个配置分区使用窄左侧单层导航，右侧只显示当前分区内容；左栏只负责导航和问题
提示，不承载软件列表或第二套业务表单。当前项使用低强度 selection 色，不再叠加另一块内容表面。
详情头横向占满模块，保留返回、标题、未保存状态和主要保存动作；其下直接进入应用主背景画布。

Software 只有一组资料，不创建单条目的伪侧栏。详情表单按“标签 / 控件”左右对齐，紧凑宽度下
保持足够的控件宽度。Settings 占满页面剩余空间，内部使用单列设置列表与受限阅读宽度；语言和
主题继续立即生效，不增加确认动作。Settings 作为标题栏进入的工具型子页，复用详情标题带但不
显示无意义的返回按钮；背景、底边界、高度和标题基线与 Workflow、Software 详情保持一致。

Settings 外观、Workflow 基础配置与 Software 编辑属于同一类轻量配置表面，共享分组标题、说明与
字段行语法：配置面板在工作表面内以 980px 最大宽度水平居中，并用一层完整边界承载分组；字段
使用 184px 标签轨与弹性控件轨。枚举选择器使用 compact 宽度并贴齐控件轨右侧，名称、描述和
路径使用 fill 宽度；字段容器不足 620px 时按自身宽度折为上下布局，不依赖整个窗口的断点。各页
仍保留真实的导航差异：Workflow 有四个任务分区，Software 没有伪侧栏，Settings 没有保存动作。

## 布局与密度

- 标题、品牌、主导航、主题切换、帮助、设置与窗口控件位于同一条 48px 顶栏；不显示常驻桌面
  服务连接状态。主题按钮显示当前可执行动作，并具有随语言变化的可访问名称。
- 页面外边距 16px；页面标题 20px；普通表格文本 11px，元数据 9–10px。
- 搜索、筛选、显示列和批量动作共用表格工具栏；选中行时批量动作清楚出现。
- 表格自己滚动，页面和 `body` 不滚动；960×640 与 1440×900 使用同一结构。
- 管理表的中间内容区使用连续表面色；空态铺满表头与底部分页之间的空间，最后一行保留底边界。
- 列表页使用“页头 + 满宽数据表面”；详情页使用“全宽详情头 + 应用主背景画布”，不再用一张
  满高空 Card 包裹所有详情内容。复杂详情可在画布上加入带右侧分隔线的分区栏，简单详情只保留
  水平居中的配置面板；空白属于页面画布，不用额外表面或伪导航填补。
- 程序路径只出现在软件资料管理，不进入工作流、词典或 Runtime 状态。

## 交互合同

- 工作流启用状态表示持久期望；Runtime actual state 与错误单独展示。
- 工作流实际状态直接命名“软件未启动”“权限不匹配”“组件加载失败”等原因，不显示笼统的
  “需要处理”。错误状态可点击展开逐软件的完整解释与恢复动作；软件未启动使用可恢复警告色，
  确定的激活失败使用错误色。
- Workflow 创建/编辑、Software 编辑、Dictionary 详情和 Probe Run 详情使用同一种独立详情页：
  左侧返回与标题，右侧保存或运行期动作，主体占满模块内容区。简单创建和低频设置仍可使用 Nuxt UI
  Modal；Modal 和 Select 浮层必须高于 sticky 表头。
- 独立详情页按 Esc 返回上一级；打开 Modal、下拉菜单或选择器时由最内层浮层先处理 Esc。Workflow、
  Software 和 Dictionary 存在未保存修改时，返回、顶部导航与关闭窗口均必须先请求确认。
- Dictionary 低频 metadata 使用单列设置 Modal；应用设置只更新同一份编辑草稿，并立即显示
  “未保存”。词条不再使用编辑 Modal，现有行直接编辑，末行在每次成功添加后自动恢复为空白行。
- Dictionary 设置、行内修改、新增和删除共享一份草稿与校验；保存成功才更新基线。返回列表、
  顶部导航和关闭窗口都必须在有未保存更改时请求确认。
- Dictionary entry 只有非空原文与非空译文，同一 Dictionary 内原文唯一；Location、Context、
  keep 和逐词条字体都不属于 Dictionary。未来区域限制由 Workflow 的 Region Binding 组合。
- 删除资产必须确认；删除软件只删除 Glyphshift 记录，不删除原程序。仍被 Workflow 或 Probe Run
  引用的软件必须保留，并在软件表内显示具体引用数量和解除方法；失败项保持选中以便处理后重试。
- Workflow、Software、Dictionary 和 Probe Run 本地管理表保留明确的行级操作，同时允许双击行内
  非交互区域进入编辑或详情；复选框、开关、链接和操作按钮不得触发双击捷径。在线目录条目不提供
  编辑双击。
- 新增软件必须先通过基础接入检查；浏览程序后自动检查，也可手动重试。检查只确认正在运行、
  目标架构、重复绑定及本地 Runtime/Adapter 就绪等确定事实，并明确提示仍需 Probe 验证真实拦截
  覆盖，不把预检通过表述为“已支持”。
- 软件快速捕获使用 `Ctrl+Shift+F8` 两段式状态：第一次进入等待，第二次读取前台进程并返回已填充
  的新增 Modal；捕获结果不自动保存。等待状态必须可取消，失败后保留等待并给出可恢复原因。
- Dictionary 创建和编辑不出现 Hook、Adapter 或字体字段。
- Catalog 未配置时在 Catalog 模式内显示明确离线空态，本地 Library 继续可用；覆盖 modified 或
  unmanaged Dictionary 必须二次确认。普通界面不展示 digest、signature bytes、key ID 或路径。
- Workflow 的每个 Target 独立组合 Adapter、Dictionary 和 Font Policy；不同 Target 不共享
  隐式选择状态。Font Policy 不成为独立资产，不出现 Location、`main-ui`、全部位置或指定位置。
- 本机字体目录是应用级机器缓存，不属于 Workflow Target 或 AppSettings；普通启动读取缓存，用户
  点击刷新后才重新扫描并替换缓存，刷新失败时保留当前可用目录。
- Probe Run 必须且只绑定一个 Dictionary；观测次数、Adapter 和时间属于内部 Observation Index，
  不进入 Dictionary。暂停不结束任务，重启后可恢复，释放连接后任务和证据仍保留。
- Probe 详情不分“技术目录”和“字典草稿”；联合表支持后端搜索分页、行内翻译、批量忽略/恢复/
  清空及导出。5000 条基准下 Vue 只渲染当前页，不叠加虚拟滚动。
- 原文和译文各占一行并使用轻量边界标明编辑区；下拉、分页和滚动条复用统一组件样式。
- 图标按钮必须有可访问名称；状态不能只依赖颜色。
- 顶部主题按钮在深色时切到浅色、在浅色时切到深色；若原偏好为跟随系统，点击后落为相反的
  固定主题，避免系统设置立即覆盖用户动作。
- 真实 Tauri 桌面组件不可用时显示阻断错误；浏览器测试预览不伪装成连接状态，帮助页仍可打开。

## 视觉语言

- 默认使用近黑石墨表面，并提供完整浅色 token；两种主题共享薄灰边界和钴蓝强调色。首次启动为
  深色，跟随系统只在用户显式选择后生效。
- 应用框架、页面画布、工作表面和 Field/Inset 使用固定语义角色。深色主题逐层轻微变亮；浅色
  使用冷灰页面框架和纯白 Table 内容面；边界、表头与分页保留浅灰层级。交互 hover/selection
  不复用静态层级色。
- 深色 primary solid 按钮使用比导航选中态稍亮的低亮度蓝色 contained surface 和白字，不使用
  刺目的高饱和实心蓝。
- `--accent` 只用于当前导航、启用状态、选中页码和主要动作。
- `--success` 只表示目标 Runtime 已确认；`--warning` 表示可恢复问题；`--danger` 只用于删除
  与明确失败。
- 不使用渐变、玻璃、发光、大卡片、大面积圆角或营销式留白。

## 实现规则

- Vue 使用 Nuxt UI 组件与 Tailwind CSS；重复的列表页头、详情页头、工作表面、配置分组、字段行、
  表格框架、表单 Modal 和确认 Modal 必须通过共享 Module 复用。
- 核心产品文案使用 Vue I18n 语义 key；`zh-CN` 是 master schema，`en-US` 必须保持结构完整。
  软件名、词典内容与 Adapter presentation 属于动态数据，不作为核心 UI 文案翻译。
- `main.css` 只保留 Tailwind/Nuxt UI 引入、语义 token 和根级浏览器规则。
- 图标统一来自 Tabler；普通界面不得显示 Driver、Profile、Domain ID、进程标识或 DLL。
- 本地截图、真实软件样本、进程信息和测试词典只能写入 `target/local-test/`，不得提交。

## 禁止项

- 不恢复单软件“选择实例 → 开始翻译”任务链路。
- 不恢复 per-Software 文字/字体开关、固定软件品牌布局或旧数据迁移入口。
- 不把 Adapter 已加载、进程已发现或 Observe 成功表述为翻译已经生效。
