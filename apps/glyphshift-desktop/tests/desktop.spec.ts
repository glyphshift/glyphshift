import { expect, test, type Page } from '@playwright/test'

const storageKey = 'glyphshift.workflow-product-model'

function software(index: number) {
  const ready = index % 4 !== 0
  return {
    id: `software-${index}`,
    name: `Vector Studio ${String(index).padStart(3, '0')}`,
    description: `用于合成场景 ${String(index).padStart(3, '0')}`,
    vendor: index % 2 ? 'Northwind Tools' : 'Contoso Creative',
    version: `v${1 + (index % 8)}.${index % 10}`,
    executableName: `VectorStudio${index}.exe`,
    executablePath: `X:\\SyntheticFixtures\\Vector Studio ${index}\\VectorStudio${index}.exe`,
    monogram: `V${index % 10}`,
    lastUsed: index < 5 ? `${index + 1} 分钟前` : null,
    locale: 'zh-CN',
    connected: index === 42,
    translation: { state: ready ? 'ready' : 'unavailable', enabled: false, coverage: ready ? 92 : 0, detail: '合成能力证据', generation: null },
    font: { state: ready ? 'ready' : 'unavailable', enabled: false, coverage: ready ? 84 : 0, detail: '合成能力证据', generation: null },
    observe: { state: 'active', enabled: true, coverage: 100, detail: '合成观察证据', generation: 18 },
    locations: [{ id: 'main-ui', label: '界面文字' }],
  }
}

function productModel(count = 128) {
  const softwareList = Array.from({ length: count }, (_, index) => software(index + 1))
  const dictionaries = Array.from({ length: count }, (_, index) => ({
    id: `dictionary-${index + 1}`,
    name: `界面词典 ${String(index + 1).padStart(3, '0')}`,
    description: `适用于界面汉化场景 ${String(index + 1).padStart(3, '0')}`,
    locale: 'zh-CN',
    hookTypeId: index % 2 ? 'synthetic-gdi' : 'synthetic-gdiplus',
    revision: 1,
    entryCount: 2,
  }))
  const workflows = Array.from({ length: count }, (_, index) => ({
    id: `workflow-${index + 1}`,
    name: `创作工作流 ${String(index + 1).padStart(3, '0')}`,
    description: `创作工具链配置 ${String(index + 1).padStart(3, '0')}`,
    revision: 1,
    softwareIds: [`software-${index + 1}`],
    dictionaryIds: [`dictionary-${index + 1}`],
    targets: [{ softwareId: `software-${index + 1}`, dictionaryIds: [`dictionary-${index + 1}`] }],
  }))
  const dictionaryDetails = Object.fromEntries(dictionaries.map(dictionary => [dictionary.id, {
    id: dictionary.id,
    name: dictionary.name,
    description: dictionary.description,
    locale: dictionary.locale,
    hookTypeId: dictionary.hookTypeId,
    revision: dictionary.revision,
    defaultFont: { kind: 'unchanged' },
    entries: [
      { location: 'main-ui', context: null, source: 'Open', translation: '打开', font: { kind: 'inherit' }, adapterIds: ['synthetic-inline'] },
      { location: 'main-ui', context: null, source: 'Save As…', translation: '另存为…', font: { kind: 'substitute', family: 'Synthetic Sans' }, adapterIds: [] },
    ],
  }]))
  const workflowDetails = Object.fromEntries(workflows.map(workflow => [workflow.id, {
    id: workflow.id,
    name: workflow.name,
    description: workflow.description,
    revision: workflow.revision,
    targets: [{ softwareId: workflow.softwareIds[0], dictionaryIds: workflow.dictionaryIds }],
  }]))
  return {
    selectedSoftwareId: count ? 'software-1' : null,
    software: softwareList,
    dictionaries,
    workflows,
    activations: count >= 42 ? [{ workflowId: 'workflow-42', revision: 1 }] : [],
    workflowRuntimeStatus: Object.fromEntries(workflows.map(workflow => [workflow.id, {
      workflowId: workflow.id,
      targets: workflow.softwareIds.map(softwareId => ({
        softwareId,
        discovered: workflow.id === 'workflow-42',
        active: workflow.id === 'workflow-42',
        translationRequested: true,
        fontRequested: false,
        translationActive: workflow.id === 'workflow-42',
        fontActive: false,
        appliedGeneration: workflow.id === 'workflow-42' ? 18 : null,
      })),
      errors: {},
    }])),
    dictionaryDetails,
    workflowDetails,
    theme: 'dark',
    translationSource: '',
    hookTypes: [
      { id: 'synthetic-gdi', label: 'GDI' },
      { id: 'synthetic-gdiplus', label: 'GDI+' },
    ],
    fontFamilies: ['Synthetic Sans', 'Synthetic Serif', 'Synthetic Mono'],
  }
}

async function contrastRatio(locator: ReturnType<Page['locator']>) {
  return locator.evaluate((element) => {
    const parseColor = (value: string) => {
      const canvas = document.createElement('canvas')
      canvas.width = canvas.height = 1
      const context = canvas.getContext('2d')!
      context.fillStyle = value
      context.fillRect(0, 0, 1, 1)
      return [...context.getImageData(0, 0, 1, 1).data].slice(0, 3)
    }
    const luminance = (value: string) => {
      const channels = parseColor(value).map(channel => {
        const normalized = channel / 255
        return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4
      })
      return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
    }
    const style = getComputedStyle(element)
    const foreground = luminance(style.color)
    const background = luminance(style.backgroundColor)
    return (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05)
  })
}

async function backgroundLuminance(locator: ReturnType<Page['locator']>) {
  return locator.evaluate((element) => {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = 1
    const context = canvas.getContext('2d')!
    context.fillStyle = getComputedStyle(element).backgroundColor
    context.fillRect(0, 0, 1, 1)
    const channels = [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map((channel) => {
        const normalized = channel / 255
        return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4
      })
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
  })
}

async function seedProduct(page: Page, count = 128) {
  await seedValue(page, productModel(count))
}

async function seedValue(page: Page, value: ReturnType<typeof productModel>) {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), {
    key: storageKey,
    value,
  })
}

test('workflow product owns activation while software rows have no feature switches', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')

  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()
  await expect(page.getByText('显示 1–20，共 128 个工作流')).toBeVisible()
  await expect(page.getByRole('switch', { name: '启用 创作工作流 001' })).toHaveAttribute('aria-checked', 'false')
  await page.getByRole('textbox', { name: '搜索工作流' }).fill('创作工作流 042')
  await expect(page.getByRole('switch', { name: '启用 创作工作流 042' })).toHaveAttribute('aria-checked', 'true')
  await expect(page.getByText('运行中', { exact: true })).toBeVisible()

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('heading', { name: '软件', exact: true })).toBeVisible()
  await expect(page.getByRole('switch')).toHaveCount(0)
})

test('empty product starts from workflows and never presents a start task', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: '还没有工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: /开始/ })).toHaveCount(0)
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('heading', { name: '还没有添加软件' })).toBeVisible()
  await expect(page.getByRole('button', { name: '新增软件' }).first()).toBeVisible()
})

test('empty workflow creation stays actionable and explains both prerequisites', async ({ page }) => {
  await page.goto('/')
  const create = page.getByRole('button', { name: '新建工作流' })
  await expect(create).toBeEnabled()
  await create.click()

  const dialog = page.getByRole('dialog', { name: '新建工作流' })
  await expect(dialog).toContainText('先添加软件')
  await expect(dialog).toContainText('先创建词典')
  await expect(dialog.getByRole('button', { name: '去添加软件' })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '去创建词典' })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '创建工作流' })).toBeDisabled()
})

test('workflow table follows the shared filter, column and batch toolbar contract', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await expect(page.getByRole('button', { name: '筛选工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '显示列' })).toBeVisible()
  await page.getByRole('button', { name: '显示列' }).click()
  await page.getByRole('menu').getByRole('menuitemcheckbox', { name: '软件' }).click()
  await expect(page.getByRole('columnheader', { name: '软件' })).toHaveCount(0)
  await page.keyboard.press('Escape')
  await page.getByRole('checkbox', { name: '选择 创作工作流 001' }).check()
  await expect(page.getByRole('button', { name: '批量启用' })).toBeVisible()
  await expect(page.getByRole('button', { name: '批量停用' })).toBeVisible()
  await expect(page.getByRole('button', { name: '批量删除' })).toBeVisible()
  await page.getByRole('button', { name: '批量启用' }).click()
  await expect(page.getByRole('switch', { name: '启用 创作工作流 001' })).toHaveAttribute('aria-checked', 'true')
  await page.getByRole('button', { name: '批量停用' }).click()
  await expect(page.getByRole('switch', { name: '启用 创作工作流 001' })).toHaveAttribute('aria-checked', 'false')
})

test('management descriptions are first-class configurable table columns', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')

  const pages = [
    { navigation: '工作流', description: '创作工具链配置 001' },
    { navigation: '软件', description: '用于合成场景 001' },
    { navigation: '词典', description: '适用于界面汉化场景 001' },
  ]

  for (const item of pages) {
    await page.getByRole('button', { name: item.navigation, exact: true }).click()
    await expect(page.getByRole('columnheader', { name: '描述', exact: true })).toBeVisible()
    await expect(page.getByText(item.description, { exact: true })).toBeVisible()

    await page.getByRole('button', { name: '显示列' }).click()
    const columns = page.getByRole('menu')
    await expect(columns.getByRole('menuitemcheckbox', { name: '描述' })).toHaveAttribute('aria-checked', 'true')
    await columns.getByRole('menuitemcheckbox', { name: '描述' }).click()
    await expect(page.getByRole('columnheader', { name: '描述', exact: true })).toHaveCount(0)
    await page.keyboard.press('Escape')
  }
})

test('shared display-column menu uses trailing checks and restores default columns', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await page.getByRole('button', { name: '显示列' }).click()

  const columns = page.getByRole('menu')
  for (const name of ['描述', '软件', '词典', '实际状态', '启用']) {
    await expect(columns.getByRole('menuitemcheckbox', { name })).toHaveAttribute('aria-checked', 'true')
  }
  await columns.getByRole('menuitemcheckbox', { name: '软件' }).click()
  await expect(page.getByRole('columnheader', { name: '软件' })).toHaveCount(0)

  await columns.getByRole('menuitem', { name: '恢复默认列' }).click()
  await expect(page.getByRole('columnheader', { name: '软件' })).toBeVisible()
})

test('workflow table scales past one hundred rows with search and pagination', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')
  await expect(page.getByRole('button', { name: '首页' })).toBeDisabled()
  await expect(page.getByRole('button', { name: '上一页' })).toBeDisabled()
  await page.getByRole('button', { name: '末页' }).click()
  await expect(page.getByText('显示 121–128，共 128 个工作流')).toBeVisible()
  await expect(page.getByRole('button', { name: '下一页' })).toBeDisabled()
  await expect(page.getByRole('button', { name: '末页' })).toBeDisabled()
  await page.getByRole('button', { name: '首页' }).click()
  await page.getByRole('button', { name: '下一页' }).click()
  await expect(page.getByText('显示 21–40，共 128 个工作流')).toBeVisible()
  await page.getByRole('combobox', { name: '每页数量' }).click()
  await page.getByRole('option', { name: '50' }).click()
  await expect(page.getByText('显示 1–50，共 128 个工作流')).toBeVisible()
  await page.getByRole('textbox', { name: '搜索工作流' }).fill('117')
  await expect(page.getByText('创作工作流 117', { exact: true })).toBeVisible()
  await expect(page.getByText('显示 1–1，共 1 个工作流')).toBeVisible()
})

test('workflow creation selects multiple software and an ordered dictionary stack', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await page.getByRole('button', { name: '新建工作流' }).click()
  await page.getByRole('textbox', { name: '工作流名称' }).fill('双软件创作流')
  await page.getByRole('textbox', { name: '工作流描述' }).fill('同时驱动两款创作软件')
  await page.getByRole('checkbox', { name: '将 Vector Studio 001 添加到工作流' }).check()
  await page.getByRole('checkbox', { name: '将 Vector Studio 002 添加到工作流' }).check()
  await page.getByRole('checkbox', { name: '使用 界面词典 002' }).check()
  await page.getByRole('checkbox', { name: '使用 界面词典 001' }).check()
  await expect(page.getByText('界面词典 002 → 界面词典 001', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '创建工作流' }).click()

  await expect(page.getByText('双软件创作流', { exact: true })).toBeVisible()
  await expect(page.getByText('同时驱动两款创作软件', { exact: true })).toBeVisible()
  await expect(page.getByText('2 个目标', { exact: true })).toBeVisible()
})

test('workflow rows copy and delete definitions through product operations', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await page.getByRole('button', { name: '复制 创作工作流 001' }).click()
  await expect(page.getByText('创作工作流 001 副本', { exact: true })).toBeVisible()
  await expect(page.getByText('显示 1–4，共 4 个工作流')).toBeVisible()
  await page.getByRole('button', { name: '删除 创作工作流 001 副本' }).click()
  await expect(page.getByRole('dialog', { name: '删除工作流' })).toBeVisible()
  await page.getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('创作工作流 001 副本', { exact: true })).toHaveCount(0)
  await expect(page.getByText('显示 1–3，共 3 个工作流')).toBeVisible()
})

test('workflow activation is persistent desired state with separate actual status', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')
  const workflow = page.getByRole('switch', { name: '启用 创作工作流 001' })
  await expect(workflow).toHaveAttribute('aria-checked', 'false')
  await expect(page.getByText('已停用', { exact: true }).first()).toBeVisible()
  await workflow.click()
  await expect(workflow).toHaveAttribute('aria-checked', 'true')
  await expect(page.getByText('等待目标', { exact: true }).first()).toBeVisible()
  await expect.poll(() => page.evaluate((key) => {
    const value = JSON.parse(localStorage.getItem(key) ?? '{}')
    return value.activations?.some((item: { workflowId: string }) => item.workflowId === 'workflow-1')
  }, storageKey)).toBe(true)
  await expect(page.getByRole('button', { name: '刷新状态' })).toBeVisible()
})

test('workflow activation failures stay visible beside the workflow action', async ({ page }) => {
  const snapshot = productModel(1)
  await page.addInitScript((value) => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {
        invoke: async (command: string) => {
          if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 5 }
          if (command === 'desktop_snapshot') return value
          if (command === 'desktop_enable_workflow') throw new Error('无法启用工作流：词典没有可应用规则，请先添加文字替换或字体规则。')
          throw new Error(`unexpected command: ${command}`)
        },
      },
    })
  }, snapshot)
  await page.goto('/')
  await expect(page.getByText('桌面服务已连接', { exact: true })).toBeVisible()

  const workflow = page.getByRole('switch', { name: '启用 创作工作流 001' })
  await workflow.click()

  await expect(workflow).toHaveAttribute('aria-checked', 'false')
  await expect(page.getByRole('alert')).toContainText('词典没有可应用规则')
})

test('workflow editing reuses the creation dialog and can change software and dictionaries', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')
  await page.getByRole('button', { name: '编辑 创作工作流 001' }).click()
  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await expect(dialog).toBeVisible()
  await dialog.getByRole('textbox', { name: '工作流名称' }).fill('默认创作工作流')
  await dialog.getByRole('textbox', { name: '工作流描述' }).fill('AE 与排版词典的默认组合')
  await dialog.getByRole('checkbox', { name: '将 Vector Studio 002 添加到工作流' }).check()
  await dialog.getByRole('checkbox', { name: '使用 界面词典 002' }).check()
  await dialog.getByRole('button', { name: '保存工作流' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByText('默认创作工作流', { exact: true })).toBeVisible()
  await expect(page.getByText('AE 与排版词典的默认组合', { exact: true })).toBeVisible()
  await expect(page.getByText('2 个目标', { exact: true })).toBeVisible()
})

test('management modal stays above the sticky table header', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await page.getByRole('button', { name: '编辑 创作工作流 001' }).click()

  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  const stickyHeader = page.locator('thead th').filter({ hasText: '软件' }).first()
  await expect(dialog).toBeVisible()

  const [dialogBox, headerBox] = await Promise.all([dialog.boundingBox(), stickyHeader.boundingBox()])
  expect(dialogBox).not.toBeNull()
  expect(headerBox).not.toBeNull()
  const overlapLeft = Math.max(dialogBox!.x, headerBox!.x)
  const overlapRight = Math.min(dialogBox!.x + dialogBox!.width, headerBox!.x + headerBox!.width)
  const overlapTop = Math.max(dialogBox!.y, headerBox!.y)
  const overlapBottom = Math.min(dialogBox!.y + dialogBox!.height, headerBox!.y + headerBox!.height)
  expect(overlapRight).toBeGreaterThan(overlapLeft)
  expect(overlapBottom).toBeGreaterThan(overlapTop)

  const dialogOwnsOverlap = await dialog.evaluate((element, point) => {
    const hit = document.elementFromPoint(point.x, point.y)
    return Boolean(hit && element.contains(hit))
  }, {
    x: overlapLeft + (overlapRight - overlapLeft) / 2,
    y: overlapTop + (overlapBottom - overlapTop) / 2,
  })
  expect(dialogOwnsOverlap).toBe(true)
})

test('software table keeps identity editing, search, pagination and batch deletion', async ({ page }) => {
  const value = productModel()
  value.workflows = []
  value.workflowDetails = {}
  value.activations = []
  value.workflowRuntimeStatus = {}
  await seedValue(page, value)
  await page.goto('/')
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByText('显示 1–20，共 128 个软件')).toBeVisible()
  await page.getByRole('button', { name: '下一页' }).click()
  await expect(page.getByText('显示 21–40，共 128 个软件')).toBeVisible()
  await page.getByRole('textbox', { name: '搜索软件' }).fill('117')
  await expect(page.getByText('Vector Studio 117', { exact: true })).toBeVisible()
  await page.getByRole('textbox', { name: '搜索软件' }).fill('001')
  await expect(page.getByRole('checkbox', { name: '选择本页软件' })).toBeVisible()
  await page.getByRole('button', { name: '编辑 Vector Studio 001' }).click()
  await page.getByRole('textbox', { name: '显示名称' }).fill('Vector Studio 2026')
  await page.getByRole('textbox', { name: '软件描述' }).fill('用于年度合成项目')
  await page.getByRole('textbox', { name: '程序路径' }).fill('X:\\SyntheticFixtures\\Vector Studio 2026\\VectorStudio.exe')
  await page.getByRole('button', { name: '保存修改' }).click()
  await page.getByRole('textbox', { name: '搜索软件' }).fill('')
  await expect(page.getByText('Vector Studio 2026', { exact: true })).toBeVisible()
  await expect(page.getByText('用于年度合成项目', { exact: true })).toBeVisible()
  await page.getByRole('checkbox', { name: '选择 Vector Studio 2026' }).check()
  await page.getByRole('button', { name: '批量删除' }).click()
  await page.getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('显示 1–20，共 127 个软件')).toBeVisible()
  await expect(page.getByText('Vector Studio 2026', { exact: true })).toHaveCount(0)
})

test('software creation confirms its name and executable path in one dialog', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.getByRole('button', { name: '新增软件' }).first().click()

  const dialog = page.getByRole('dialog', { name: '新增软件' })
  await dialog.getByRole('textbox', { name: '软件名称' }).fill('合成排版工具')
  await dialog.getByRole('textbox', { name: '软件描述' }).fill('用于合成与排版检查')
  await dialog.getByRole('textbox', { name: '程序路径' }).fill('X:\\SyntheticFixtures\\LayoutTool\\LayoutTool.exe')
  await dialog.getByRole('button', { name: '添加软件' }).click()

  await expect(page.getByText('合成排版工具', { exact: true })).toBeVisible()
  await expect(page.getByText('用于合成与排版检查', { exact: true })).toBeVisible()
  await expect(page.getByText('X:\\SyntheticFixtures\\LayoutTool\\LayoutTool.exe', { exact: true })).toBeVisible()
})

test('dictionary library owns reusable detail and saves rule edits without adapter ids', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await expect(page.getByRole('heading', { name: '词典', exact: true })).toBeVisible()
  await page.getByRole('textbox', { name: '搜索词典' }).fill('117')
  await page.getByRole('button', { name: '编辑 界面词典 117' }).click()
  await expect(page.getByRole('heading', { name: '界面词典 117' })).toBeVisible()
  await expect(page.getByText('synthetic-inline')).toHaveCount(0)
  await page.getByRole('textbox', { name: '词典描述' }).fill('用于验证词典描述的保存链路')
  await page.getByRole('textbox', { name: 'Open 的译文' }).fill('开启')
  await page.getByRole('button', { name: '新增规则' }).click()
  await page.getByRole('textbox', { name: '新规则 的原文' }).fill('Export')
  await page.getByRole('textbox', { name: 'Export 的译文' }).fill('导出')
  await page.getByRole('button', { name: '保存词典' }).click()
  await expect(page.getByText(/修订 2 · 3 条规则/)).toBeVisible()
  await expect(page.getByRole('textbox', { name: '词典描述' })).toHaveValue('用于验证词典描述的保存链路')
  await expect(page.getByRole('textbox', { name: 'Open 的译文' })).toHaveValue('开启')
  await page.getByRole('button', { name: '返回词典' }).click()
  await page.getByRole('textbox', { name: '搜索词典' }).fill('用于验证词典描述的保存链路')
  await expect(page.getByText('用于验证词典描述的保存链路', { exact: true })).toBeVisible()
})

test('independent dictionary library follows the shared table management contract', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await expect(page.getByRole('button', { name: /软件筛选/ })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '筛选词典' })).toBeVisible()
  await expect(page.getByRole('button', { name: '显示列' })).toBeVisible()
  await page.getByRole('button', { name: '显示列' }).click()
  await page.getByRole('menu').getByRole('menuitemcheckbox', { name: '修订' }).click()
  await expect(page.getByRole('columnheader', { name: '修订' })).toHaveCount(0)
  await page.keyboard.press('Escape')
  await page.getByRole('checkbox', { name: '选择 界面词典 001' }).check()
  await expect(page.getByRole('button', { name: '批量删除' })).toBeVisible()
})

test('dictionary batch deletion removes unreferenced independent assets', async ({ page }) => {
  const value = productModel(3)
  value.workflows = []
  value.workflowDetails = {}
  value.activations = []
  value.workflowRuntimeStatus = {}
  await seedValue(page, value)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('checkbox', { name: '选择 界面词典 001' }).check()
  await page.getByRole('checkbox', { name: '选择 界面词典 002' }).check()
  await page.getByRole('button', { name: '批量删除' }).click()
  await page.getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('显示 1–1，共 1 份词典')).toBeVisible()
  await expect(page.getByText('界面词典 001', { exact: true })).toHaveCount(0)
  await expect(page.getByText('界面词典 002', { exact: true })).toHaveCount(0)
})

test('dictionary owns one hook type while rules stay focused on replacement content', async ({ page }) => {
  const value = productModel(1)
  await seedValue(page, value)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面词典 001' }).click()

  await expect(page.getByText('适用 Hook：GDI+', { exact: true })).toBeVisible()
  await expect(page.getByRole('combobox', { name: '适用 Hook' })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: 'Hook 类型' })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '位置' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '设置 Open 的 Hook 类型' })).toHaveCount(0)
})

test('dictionary editor uses the shared table management contract for replacement rules', async ({ page }) => {
  await seedProduct(page, 1)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面词典 001' }).click()

  await expect(page.getByRole('textbox', { name: '搜索词典规则' })).toBeVisible()
  await expect(page.getByRole('button', { name: '筛选词典规则' })).toBeVisible()
  await expect(page.getByRole('button', { name: '显示规则列' })).toBeVisible()
  await expect(page.getByRole('checkbox', { name: '选择本页规则' })).toBeVisible()

  const columnPositions = await Promise.all(['原文', '译文', '文字处理'].map(async name => {
    const box = await page.getByRole('columnheader', { name, exact: true }).boundingBox()
    expect(box).not.toBeNull()
    return box!.x
  }))
  expect(columnPositions[0]).toBeLessThan(columnPositions[1])
  expect(columnPositions[1]).toBeLessThan(columnPositions[2])

  for (const name of ['Open 的原文', 'Open 的译文']) {
    const input = page.getByRole('textbox', { name })
    await expect.poll(() => input.evaluate(element => getComputedStyle(element).boxShadow)).not.toBe('none')
  }

  await page.getByRole('checkbox', { name: '选择 Open' }).check()
  await expect(page.getByRole('button', { name: '批量删除规则' })).toBeVisible()
  await page.getByRole('button', { name: '显示规则列' }).click()
  await expect(page.getByRole('menu').getByRole('menuitemcheckbox', { name: 'Hook 类型' })).toHaveCount(0)
  await expect(page.getByRole('menu').getByRole('menuitemcheckbox', { name: '文字处理' })).toBeVisible()
})

test('long font menus use a thin scrollbar without native arrow buttons', async ({ page }) => {
  const value = productModel(1)
  value.fontFamilies = Array.from({ length: 80 }, (_, index) => `Synthetic Font ${String(index + 1).padStart(3, '0')}`)
  await seedValue(page, value)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面词典 001' }).click()
  await page.getByRole('button', { name: '默认字体' }).click()

  const viewport = page.locator('[data-slot="viewport"]:visible').last()
  await expect(viewport).toBeVisible()
  await expect.poll(() => viewport.evaluate(element => getComputedStyle(element).scrollbarWidth)).toBe('thin')
  await expect.poll(() => viewport.evaluate(element => getComputedStyle(element, '::-webkit-scrollbar-button').display)).toBe('none')
  await expect.poll(() => viewport.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
})

test('dictionary font menu stays above the sticky rule table', async ({ page }) => {
  const value = productModel(1)
  value.fontFamilies = Array.from({ length: 40 }, (_, index) => `Synthetic Font ${String(index + 1).padStart(3, '0')}`)
  await seedValue(page, value)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面词典 001' }).click()
  await page.getByRole('button', { name: '默认字体' }).click()
  await expect(page.getByRole('listbox')).toBeVisible()

  const stacking = await page.evaluate(() => {
    const listbox = document.querySelector<HTMLElement>('[role="listbox"]')
    const menuLayer = listbox?.closest<HTMLElement>('[data-reka-popper-content-wrapper]') ?? null
    const tableHeader = document.querySelector<HTMLElement>('thead th')
    const zIndex = (target: HTMLElement | null) => Number.parseInt(target ? getComputedStyle(target).zIndex : '0', 10) || 0
    return { menu: zIndex(menuLayer), table: zIndex(tableHeader) }
  })
  expect(stacking.menu).toBeGreaterThan(stacking.table)
})

test('dictionary editor loads local fonts and edits text and font behavior independently', async ({ page }) => {
  await seedProduct(page, 1)
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面词典 001' }).click()

  const defaultFont = page.getByRole('button', { name: '默认字体' })
  await defaultFont.click()
  await page.getByRole('option', { name: 'Synthetic Serif' }).click()

  const textBehavior = page.getByRole('combobox', { name: 'Open 的文字处理' })
  await textBehavior.click()
  await page.getByRole('option', { name: '保持原文' }).click()
  await expect(page.getByRole('textbox', { name: 'Open 的译文' })).toBeDisabled()
  await textBehavior.click()
  await page.getByRole('option', { name: '替换文字' }).click()
  await page.getByRole('textbox', { name: 'Open 的译文' }).fill('开启')

  await page.getByRole('combobox', { name: 'Open 的字体处理' }).click()
  await page.getByRole('option', { name: '替换为指定字体' }).click()
  const entryFont = page.getByRole('button', { name: 'Open 的字体', exact: true })
  await entryFont.click()
  await page.getByRole('option', { name: 'Synthetic Sans' }).click()

  await page.getByRole('button', { name: '保存词典' }).click()
  await expect(defaultFont).toContainText('Synthetic Serif')
  await expect(textBehavior).toContainText('替换文字')
  await expect(entryFont).toContainText('Synthetic Sans')
})

test('dictionary library creates the reusable asset required by workflows', async ({ page }) => {
  await seedValue(page, productModel(0))
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '新建词典' }).first().click()
  const dialog = page.getByRole('dialog', { name: '新建词典' })
  await dialog.getByRole('textbox', { name: '词典名称' }).fill('合成界面词典')
  await dialog.getByRole('textbox', { name: '词典描述' }).fill('用于合成软件菜单汉化')
  await dialog.getByRole('textbox', { name: '词典语言' }).fill('zh-CN')
  await dialog.getByRole('combobox', { name: '适用 Hook' }).click()
  await page.getByRole('option', { name: 'GDI+', exact: true }).click()
  await dialog.getByRole('button', { name: '创建词典' }).click()
  await expect(page.getByText('合成界面词典', { exact: true })).toBeVisible()
  await expect(page.getByText('用于合成软件菜单汉化', { exact: true })).toBeVisible()
  await expect(page.getByRole('cell', { name: 'GDI+', exact: true })).toBeVisible()
})

test('dictionary hook menu stays above the create modal footer', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await seedValue(page, productModel(0))
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '新建词典' }).first().click()
  const dialog = page.getByRole('dialog', { name: '新建词典' })
  const footerBox = await dialog.locator('[data-slot="footer"]').boundingBox()
  expect(footerBox).not.toBeNull()
  await dialog.getByRole('combobox', { name: '适用 Hook' }).click()
  await page.waitForTimeout(300)

  const menuBox = await page.getByRole('listbox').boundingBox()
  expect(menuBox).not.toBeNull()
  expect(menuBox!.y + menuBox!.height).toBeLessThanOrEqual(footerBox!.y)
  const stacking = await page.evaluate(() => {
    const listbox = document.querySelector<HTMLElement>('[role="listbox"]')
    const menuLayer = listbox?.closest<HTMLElement>('[data-reka-popper-content-wrapper]') ?? null
    const modalLayer = document.querySelector<HTMLElement>('[role="dialog"][data-slot="content"]')
    const zIndex = (target: HTMLElement | null) => Number.parseInt(target ? getComputedStyle(target).zIndex : '0', 10) || 0
    return { menu: zIndex(menuLayer), modal: zIndex(modalLayer) }
  })
  expect(stacking.menu).toBeGreaterThan(stacking.modal)
  const optionBoxes = await Promise.all(['全部 Hook', 'GDI', 'GDI+'].map(name => page.getByRole('option', { name, exact: true }).boundingBox()))
  expect(optionBoxes.every(box => box !== null)).toBe(true)
  expect(menuBox!.height).toBeGreaterThanOrEqual(optionBoxes.reduce((height, box) => height + (box?.height ?? 0), 0))
})

test('management pages share icon-title-description headers and accessible primary actions', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  for (const name of ['工作流', '软件', '词典']) {
    if (name !== '工作流') await page.getByRole('button', { name, exact: true }).click()
    await expect(page.getByRole('img', { name: `${name}图标` })).toBeVisible()
  }

  await expect.poll(() => contrastRatio(page.getByRole('button', { name: '新建词典' }).first())).toBeGreaterThanOrEqual(4.5)
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect.poll(() => contrastRatio(page.getByRole('button', { name: '新增软件' }).first())).toBeGreaterThanOrEqual(4.5)
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect.poll(() => contrastRatio(page.getByRole('button', { name: '新建工作流' }))).toBeGreaterThanOrEqual(4.5)
})

test('management primary actions use the low-luminance Yotta contained surface', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')

  const actions = [
    { page: '工作流', button: '新建工作流' },
    { page: '软件', button: '新增软件' },
    { page: '词典', button: '新建词典' },
  ]
  for (const action of actions) {
    await page.getByRole('button', { name: action.page, exact: true }).click()
    const button = page.getByRole('button', { name: action.button }).first()
    await expect.poll(() => backgroundLuminance(button)).toBeLessThan(0.12)
    await expect.poll(() => contrastRatio(button)).toBeGreaterThanOrEqual(4.5)
  }
})

test('software library only manages software identity and program binding', async ({ page }) => {
  await seedProduct(page, 4)
  await page.goto('/')
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByText('管理软件名称、用途说明和程序位置。', { exact: true })).toBeVisible()
  await expect(page.getByRole('columnheader', { name: '程序位置' })).toBeVisible()
  await expect(page.getByRole('columnheader', { name: '可用功能' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '筛选软件' })).toHaveCount(0)
  await expect(page.getByText(/修改界面文字|字体可用|界面文字可用|当前不可用/)).toHaveCount(0)
})

test('software selection boxes match the transparent workflow selection control', async ({ page }) => {
  await seedProduct(page, 3)
  await page.goto('/')
  const workflowCheckbox = page.getByRole('checkbox', { name: '选择 创作工作流 001' })
  const workflowVisual = await workflowCheckbox.evaluate(element => {
    const style = getComputedStyle(element)
    return {
      tag: element.tagName,
      background: style.backgroundColor,
      border: style.borderColor,
      radius: style.borderRadius,
      width: style.width,
      height: style.height,
    }
  })

  await page.getByRole('button', { name: '软件', exact: true }).click()
  const softwareCheckbox = page.getByRole('checkbox', { name: '选择 Vector Studio 001' })
  await expect.poll(() => softwareCheckbox.evaluate(element => {
    const style = getComputedStyle(element)
    return {
      tag: element.tagName,
      background: style.backgroundColor,
      border: style.borderColor,
      radius: style.borderRadius,
      width: style.width,
      height: style.height,
    }
  })).toEqual(workflowVisual)
})

test('title bar exposes product navigation and icon-only settings', async ({ page }) => {
  await seedProduct(page)
  await page.goto('/')
  await expect(page.getByRole('navigation', { name: '主导航' })).toBeVisible()
  for (const name of ['工作流', '软件', '词典']) await expect(page.getByRole('button', { name, exact: true })).toBeVisible()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
})

test('compact desktop viewport stays inside one application surface', async ({ page }) => {
  await seedProduct(page)
  await page.setViewportSize({ width: 960, height: 640 })
  await page.goto('/')
  const metrics = await page.evaluate(() => ({
    bodyWidth: document.body.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
    bodyHeight: document.body.scrollHeight,
    viewportHeight: document.documentElement.clientHeight,
  }))
  expect(metrics.bodyWidth).toBeLessThanOrEqual(metrics.viewportWidth)
  expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight)
})

test('desktop API mismatch asks for a restart before loading product commands', async ({ page }) => {
  await page.addInitScript(() => {
    const commands: string[] = []
    Object.defineProperty(window, '__glyphshiftTestCommands', { value: commands })
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {
        invoke: async (command: string) => {
          commands.push(command)
          if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 1 }
          throw new Error(`unexpected command: ${command}`)
        },
      },
    })
  })
  await page.goto('/')

  await expect(page.getByRole('alert')).toContainText('桌面接口已更新，请重新启动 Glyphshift')
  await expect.poll(() => page.evaluate(() => (window as unknown as { __glyphshiftTestCommands: string[] }).__glyphshiftTestCommands)).toEqual(['desktop_status'])
})
