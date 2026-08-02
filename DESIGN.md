---
name: Glyphshift Desktop
description: A compact workflow, software, and dictionary manager for Windows runtime translation.
colors:
  accent: "Nuxt UI emerald"
  surface: "oklch(23% 0.013 270)"
  surface-subtle: "oklch(21.5% 0.012 270)"
  canvas: "oklch(17% 0.012 270)"
  border: "oklch(29.5% 0.012 270)"
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

Glyphshift 是高密度 Windows 桌面管理工具。主要任务分为工作流、软件和词典：工作流表达持续
运行期望，软件只管理身份与程序绑定，词典是可复用的文字和字体规则资产。启用工作流就是持续
期望，没有第二个“开始翻译”动作。

界面采用成熟数据管理器语法：单层标题/导航栏、图标标题与说明、搜索和批量工具栏、满宽表格、
分页与行级操作。不使用首页卡片、软件主从详情、营销式仪表盘或大面积强调色。

## 信息架构

- **工作流：** 默认页。展示名称、说明、软件和词典摘要、期望/实际状态、启用、编辑、复制和
  删除；支持搜索、筛选、显示列、多选、分页与批量操作。
- **软件：** 只管理名称、用途说明和完整程序路径；不展示或配置文字/字体能力。
- **词典：** 独立资产表展示名称、说明、适用 Hook、语言、规则数与修订。编辑器按原文、译文、
  文字处理、字体处理、字体排列规则，并支持搜索、筛选、显示列、多选和批量删除。
- **设置：** 只在标题栏提供图标入口，不占主导航文字位置。

完整软件或词典集合不用普通下拉框承载。Workflow 编辑弹窗使用可搜索、多选的管理列表，并为
每个目标维护有序词典集合。

## 布局与密度

- 标题、品牌、主导航、服务状态、设置与窗口控件位于同一条 48px 顶栏。
- 页面外边距 16px；页面标题 20px；普通表格文本 11px，元数据 9–10px。
- 搜索、筛选、显示列和批量动作共用表格工具栏；选中行时批量动作清楚出现。
- 表格自己滚动，页面和 `body` 不滚动；960×640 与 1440×900 使用同一结构。
- 程序路径只出现在软件资料管理，不进入工作流、词典或 Runtime 状态。

## 交互合同

- 工作流启用状态表示持久期望；Runtime actual state 与错误单独展示。
- 创建和编辑资产复用同一 Nuxt UI Modal；Modal 和 Select 浮层必须高于 sticky 表头。
- 删除资产必须确认；删除软件只删除 Glyphshift 记录，不删除原程序。
- 适用 Hook 只在创建词典时选择，编辑页作为不可变范围展示，规则行不重复配置。
- 原文和译文输入使用轻量边界标明编辑区；下拉、分页和滚动条复用统一组件样式。
- 图标按钮必须有可访问名称；状态不能只依赖颜色。

## 视觉语言

- 固定使用近黑 `oklch` 表面、薄灰边界和 Nuxt UI emerald 强调色。
- primary solid 按钮使用低亮度 contained surface，不使用刺目的高饱和绿色。
- `--accent` 只用于当前导航、启用状态、选中页码和主要动作。
- `--success` 只表示目标 Runtime 已确认；`--warning` 表示可恢复问题；`--danger` 只用于删除
  与明确失败。
- 不使用渐变、玻璃、发光、大卡片、大面积圆角或营销式留白。

## 实现规则

- Vue 使用 Nuxt UI 组件与 Tailwind CSS；重复的页头、表格框架、表单 Modal 和确认 Modal 必须
  通过共享 Module 复用。
- `main.css` 只保留 Tailwind/Nuxt UI 引入、语义 token 和根级浏览器规则。
- 图标统一来自 Tabler；普通界面不得显示 Driver、Profile、Domain ID、进程标识或 DLL。
- 本地截图、真实软件样本、进程信息和测试词典只能写入 `target/local-test/`，不得提交。

## 禁止项

- 不恢复单软件“选择实例 → 开始翻译”任务链路。
- 不恢复 per-Software 文字/字体开关、固定软件品牌布局或旧数据迁移入口。
- 不把 Adapter 已加载、进程已发现或 Observe 成功表述为翻译已经生效。
