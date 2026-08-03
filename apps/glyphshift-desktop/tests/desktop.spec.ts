import { expect, test } from '@playwright/test'

const storageKey = 'glyphshift.composable-product-model.v3'

const model = {
  selectedSoftwareId: 'software-proof',
  software: [{
    id: 'software-proof',
    name: 'Vector Studio',
    description: '用于动态图形与合成项目',
    vendor: 'Sample Creative Tools',
    version: 'v4.2',
    executableName: 'VectorStudio.exe',
    executablePath: 'X:\\SyntheticFixtures\\VectorStudio.exe',
    monogram: 'VS',
    lastUsed: null,
    locale: 'zh-CN',
    connected: false,
    translation: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    font: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    observe: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
  }],
  dictionaries: [{
    metadata: {
      id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化',
      sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'],
    },
    revision: 3,
    entryCount: 2,
  }],
  adapters: [{
    id: 'synthetic.ext-text-out', name: 'ExtTextOutW', summary: '拦截 GDI 高级文本输出；覆盖字距数组、裁剪选项和部分字形索引绘制',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!ExtTextOutW', configuration: 'none',
  }, {
    id: 'synthetic.text-out', name: 'TextOutW', summary: '拦截基础 GDI 文本输出；常见于传统 Win32 控件和简单自绘界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!TextOutW', configuration: 'none',
  }, {
    id: 'synthetic.draw-text', name: 'DrawTextW / DrawTextExW', summary: '拦截矩形内文本布局绘制；常见于按钮、标签和传统窗口界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['USER32 / GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'user32.dll!DrawTextW + DrawTextExW', configuration: 'none',
  }, {
    id: 'synthetic.gdip-draw-string', name: 'GdipDrawString', summary: '拦截 GDI+ 浮点布局文本绘制；常见于自绘面板和图形化桌面界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI+'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdiplus.dll!GdipDrawString', configuration: 'none',
  }],
  workflows: [{
    id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5,
    softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'],
    targets: [{
      softwareId: 'software-proof',
      adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
      dictionaryIds: ['dictionary-proof'],
      fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' },
    }],
  }],
  activations: [],
  workflowRuntimeStatus: {},
  dictionaryDetails: {
    'dictionary-proof': {
      metadata: {
        id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化',
        sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'],
      },
      revision: 3,
      entries: [
        { source: 'Open', translation: '打开' },
        { source: 'Save As…', translation: '另存为…' },
      ],
    },
  },
  workflowDetails: {
    'workflow-proof': {
      id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5,
      targets: [{
        softwareId: 'software-proof',
        adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
        dictionaryIds: ['dictionary-proof'],
        fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' },
      }],
    },
  },
  fontFamilies: ['Synthetic Sans', 'Synthetic Serif', 'Synthetic Mono'],
  capture: null,
}

function expandedModel() {
  const next = JSON.parse(JSON.stringify(model))
  next.software.push({
    ...next.software[0],
    id: 'software-secondary',
    name: 'Pixel Studio',
    executableName: 'PixelStudio.exe',
    executablePath: 'X:\\SyntheticFixtures\\PixelStudio.exe',
    monogram: 'PS',
  })
  next.dictionaries.push({
    metadata: {
      ...next.dictionaries[0].metadata,
      id: 'dictionary-effects',
      name: '效果词典',
      description: '效果与属性文字',
      tags: ['效果'],
    },
    revision: 1,
    entryCount: 1,
  })
  next.dictionaryDetails['dictionary-effects'] = {
    metadata: { ...next.dictionaries[1].metadata },
    revision: 1,
    entries: [{ source: 'Effect', translation: '效果' }],
  }
  const secondaryTarget = {
    softwareId: 'software-secondary',
    adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
    dictionaryIds: ['dictionary-proof'],
    fontPolicy: null,
  }
  next.workflows[0].softwareIds.push('software-secondary')
  next.workflows[0].targets.push(secondaryTarget)
  next.workflowDetails['workflow-proof'].targets.push(JSON.parse(JSON.stringify(secondaryTarget)))
  return next
}

async function replaceModel(page: import('@playwright/test').Page, value: ReturnType<typeof expandedModel>) {
  await page.addInitScript(({ key, modelValue }) => localStorage.setItem(key, JSON.stringify(modelValue)), { key: storageKey, modelValue: value })
  await page.goto('/')
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('management table body stays continuous for empty and populated states', async ({ page }) => {
  const emptyModel = JSON.parse(JSON.stringify(model))
  emptyModel.workflows = []
  emptyModel.workflowDetails = {}
  await replaceModel(page, emptyModel)

  const tableBody = page.getByTestId('management-table-body')
  const emptyState = tableBody.locator('[data-slot="empty"] > [data-slot="root"]')
  await expect(page.getByText('还没有工作流')).toBeVisible()
  await expect.poll(async () => {
    const bodyBox = await tableBody.boundingBox()
    const emptyBox = await emptyState.boundingBox()
    if (!bodyBox || !emptyBox) return false
    return Math.abs(emptyBox.y - (bodyBox.y + 32)) <= 1
      && Math.abs((emptyBox.y + emptyBox.height) - (bodyBox.y + bodyBox.height)) <= 1
  }).toBe(true)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/management-table-empty-continuous.png' })

  await replaceModel(page, model)
  const lastRow = page.getByTestId('management-table-body').locator('tbody > tr').last()
  await expect(lastRow).toBeVisible()
  await expect.poll(() => lastRow.evaluate(element => getComputedStyle(element).borderBottomWidth)).toBe('1px')
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/management-table-populated-continuous.png' })
})

test('navigation keeps fonts inside workflow targets instead of a separate asset page', async ({ page }) => {
  await expect(page.getByRole('heading', { name: '工作流' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('桌面服务已连接')).toHaveCount(0)
  await expect(page.getByText('本地预览')).toHaveCount(0)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await expect(page.getByRole('heading', { name: '词典' })).toBeVisible()
  await expect(page.getByText('en-US')).toBeVisible()
  await expect(page.getByText('v1.2.0')).toBeVisible()

  await expect(page.getByRole('button', { name: '字体', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect(page.getByText('字体策略：Synthetic Sans')).toBeVisible()
})

test('probe run uses the shared searchable selectable paginated table flow', async ({ page }) => {
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await expect(page.getByRole('heading', { name: '探针', exact: true })).toBeVisible()
  await expect(page.getByText('还没有探针任务')).toBeVisible()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(dialog.getByText('使用已有词典', { exact: true })).toBeVisible()
  await expect(dialog.getByText('界面基础词典', { exact: true })).toBeVisible()
  await expect(dialog.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('实时预览')).toBeVisible()
  await expect(page.getByText('gdi32.dll!TextOutW', { exact: true })).toHaveCount(0)
  await expect(page.getByText(/位置|语境/)).toHaveCount(0)

  await dialog.getByRole('button', { name: '创建并连接' }).click()
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回探针管理' })).toBeVisible()
  await expect(page.getByPlaceholder('搜索原文、译文或探针技术')).toBeVisible()
  await expect(page.getByText(/技术目录|字典草稿/)).toHaveCount(0)
  await expect(page.getByText('还没有捕获到文字')).toBeVisible()
  await expect(page.getByText('每页')).toBeVisible()
  await expect(page.getByRole('button', { name: /开始监听|停止并生成/ })).toHaveCount(0)
})

test('probe run renders only one backend page for 5000 joined entries', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const translations: Record<string, string> = {}
    let summary = {
      id: 'probe-scale', name: '5000 条性能任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'],
      status: 'running', livePreviewEnabled: true, observationRevision: 12, observedCount: 5000,
      ignoredCount: 0, droppedObservations: 0, previewGeneration: 8,
      createdAtMs: 1, updatedAtMs: 2,
      dictionaryRevision: 8, dictionaryEntryCount: 2500,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 12 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_edit_probe_translation') {
          const request = args?.request as { source: string; translation: string }
          ;(window as unknown as { __captureEditRequests?: unknown[] }).__captureEditRequests ??= []
          ;(window as unknown as { __captureEditRequests: unknown[] }).__captureEditRequests.push(request)
          translations[request.source] = request.translation
          summary = { ...summary, dictionaryRevision: summary.dictionaryRevision + 1, dictionaryEntryCount: summary.dictionaryEntryCount + 1, previewGeneration: summary.previewGeneration + 1 }
          return summary
        }
        const request = args?.request as { page: number; pageSize: number }
        if (command === 'desktop_probe_run_entries') {
          const start = (request.page - 1) * request.pageSize
          return {
            observationRevision: 12, dictionaryRevision: summary.dictionaryRevision,
            page: request.page, pageSize: request.pageSize, total: 5000,
            rows: Array.from({ length: request.pageSize }, (_, offset) => {
              const source = `Source ${String(start + offset + 1).padStart(4, '0')}`
              const translation = translations[source] ?? (offset % 2 ? `译文 ${start + offset + 1}` : '')
              return {
                source, translation,
                state: translation ? 'translated' : 'pending',
                adapterIds: ['synthetic.text-out'], count: start + offset + 1,
                firstSeenMs: 1, lastSeenMs: 2,
              }
            }),
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', 'probe-scale')
  }, { snapshot: model })
  await page.setViewportSize({ width: 1180, height: 760 })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  const paginationSummary = page.getByText('显示 1–50，共 5000 条目')
  await expect(paginationSummary).toBeVisible()
  await expect(paginationSummary).toBeInViewport()
  const tableScroller = page.getByTestId('capture-table-scroll')
  await expect(tableScroller).toBeVisible()
  await expect.poll(() => tableScroller.evaluate(element => {
    const style = getComputedStyle(element)
    return ['auto', 'scroll'].includes(style.overflowY) && element.scrollHeight > element.clientHeight
  })).toBe(true)
  await expect.poll(() => tableScroller.evaluate(element => (element as HTMLElement).offsetWidth - element.clientWidth >= 8)).toBe(true)
  await expect(page.getByTestId('capture-scrollbar')).toBeVisible()
  const initialThumbTransform = await page.getByTestId('capture-scrollbar-thumb').evaluate(element => getComputedStyle(element).transform)
  await expect(page.locator('tbody tr')).toHaveCount(50)
  await expect(page.locator('tbody input')).toHaveCount(50)
  const firstTranslation = page.getByLabel('“Source 0001”的译文')
  await firstTranslation.fill('即时译文')
  await firstTranslation.blur()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureEditRequests?: Array<{ translation: string }> }).__captureEditRequests?.[0]?.translation
  ))).toBe('即时译文')
  await expect(page.getByText('预览 G9')).toBeVisible()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/capture-workspace-live-edit-scroll.png' })
  await tableScroller.evaluate(element => { element.scrollTop = 600 })
  await expect.poll(() => page.getByTestId('capture-scrollbar-thumb').evaluate(element => getComputedStyle(element).transform)).not.toBe(initialThumbTransform)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/capture-workspace-scrollbar-pagination.png' })
  await page.getByLabel('选择当前页').click()
  await expect(page.getByText('50 条目已选择')).toBeVisible()
  await expect(page.getByText(/技术目录|字典草稿/)).toHaveCount(0)
})

test('help exposes adapter information without internal targets', async ({ page }) => {
  await page.getByRole('button', { name: '帮助' }).click()

  await expect(page.getByRole('heading', { name: '帮助' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '当前适配器' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(page.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(page.getByText('Windows', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI+', { exact: true })).toBeVisible()
  await expect(page.getByText('文字观察', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('文字替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('字体替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('无需配置', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('v1.0.0', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)
  await expect(page.getByText('synthetic.ext-text-out')).toHaveCount(0)
})

test('settings applies and persists the real locale and theme preferences', async ({ page }) => {
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.getByRole('button', { name: '切换到浅色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await page.getByRole('button', { name: '切换到深色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
  await expect(page.getByText(/在线翻译/)).toHaveCount(0)
  await expect(page.getByRole('textbox')).toHaveCount(0)
  await expect(page.getByRole('combobox')).toHaveCount(2)

  await page.getByRole('combobox', { name: '界面语言' }).click()
  await page.getByRole('option', { name: 'English', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US')
  await expect(page.getByRole('button', { name: 'Workflows', exact: true })).toBeVisible()

  await page.getByRole('combobox', { name: 'Theme' }).click()
  await page.getByRole('option', { name: 'Dark', exact: true }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.reload()
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US')
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.getByRole('button', { name: 'Help', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Help' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Available adapters' })).toBeVisible()
  await page.getByRole('button', { name: 'Software', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Software' })).toBeVisible()
  await page.getByRole('button', { name: 'Dictionaries', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Dictionaries' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Fonts', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Workflows', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Workflows' })).toBeVisible()

  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  await page.getByRole('combobox', { name: 'Theme' }).click()
  await page.getByRole('option', { name: 'Light', exact: true }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await expect(page.locator('html')).not.toHaveClass(/dark/)
})

test('title bar reports a theme persistence failure without changing the active theme', async ({ page }) => {
  await page.evaluate(() => {
    Storage.prototype.setItem = () => {
      throw new DOMException('synthetic storage failure', 'QuotaExceededError')
    }
  })

  await page.getByRole('button', { name: '切换到浅色主题' }).click()
  await expect(page.getByText('设置未保存', { exact: true })).toBeVisible()
  await expect(page.getByText('操作失败，请重试。', { exact: true })).toBeVisible()
  await expect(page.locator('html')).toHaveClass(/dark/)
})

test('dictionary editor contains no adapter or font configuration', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByText('en-US → zh-CN')).toBeVisible()
  await expect(page.getByText('另存为…')).toBeVisible()
  await expect(page.getByRole('columnheader', { name: '位置', exact: true })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '语境', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '词典设置' }).click()
  const settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(settings.getByRole('textbox', { name: '作者' })).toHaveValue('Glyphshift')
  await settings.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '添加词条' }).click()
  const ruleEditor = page.getByRole('dialog', { name: '添加翻译词条' })
  await expect(ruleEditor.getByRole('textbox', { name: '原文' })).toBeVisible()
  await expect(ruleEditor.getByRole('textbox', { name: '译文' })).toBeVisible()
  await expect(ruleEditor.getByText('语义位置')).toHaveCount(0)
  await expect(ruleEditor.getByText('限定语境')).toHaveCount(0)
  await ruleEditor.getByRole('button', { name: '取消' }).click()
  await expect(page.getByText('默认字体')).toHaveCount(0)
  await expect(page.getByText(/Hook/)).toHaveCount(0)
})

test('workflow target independently selects adapters dictionaries and one font policy', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await expect(page.getByText('此处配置仅属于这个软件目标')).toBeVisible()
  await expect(page.getByText('windows · GDI', { exact: true })).toBeVisible()
  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(dialog.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)
  await expect(dialog.getByText('界面基础词典', { exact: true }).first()).toBeVisible()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).toBeChecked()
  await expect(dialog.getByRole('button', { name: '仅词典命中' })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByRole('button', { name: 'Hook 捕获的全部文字' })).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByRole('button', { name: '提高 界面基础词典 的优先级' })).toBeDisabled()
  await expect(dialog.getByPlaceholder('搜索本机字体')).toBeVisible()
  await expect(dialog.getByText(/位置|main-ui/)).toHaveCount(0)
})

test('workflow saves reordered dictionaries in explicit priority order', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await dialog.getByText('效果词典', { exact: true }).click()
  await dialog.getByRole('button', { name: '提高 效果词典 的优先级' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = page.getByRole('dialog', { name: '编辑工作流' })
  const priorityItems = dialog.getByText('有序词典 · 2').locator('xpath=ancestor::section[1]').locator('ol > li')
  await expect(priorityItems).toHaveCount(2)
  await expect(priorityItems.nth(0)).toContainText('效果词典')
  await expect(priorityItems.nth(1)).toContainText('界面基础词典')
})

test('workflow saves font coverage and ordered inline candidates', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await dialog.getByRole('button', { name: 'Hook 捕获的全部文字' }).click()
  await dialog.getByRole('button', { name: '提高 Synthetic Serif 的优先级' }).click()
  await dialog.getByText('Synthetic Mono', { exact: true }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await expect(dialog.getByRole('button', { name: 'Hook 捕获的全部文字' })).toHaveAttribute('aria-pressed', 'true')
  const priorityItems = dialog.getByLabel('字体候选优先级').locator('li')
  await expect(priorityItems).toHaveCount(3)
  await expect(priorityItems.nth(0)).toContainText('Synthetic Serif')
  await expect(priorityItems.nth(1)).toContainText('Synthetic Sans')
  await expect(priorityItems.nth(2)).toContainText('Synthetic Mono')
})

test('inline font policy stays isolated between software targets', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await dialog.getByRole('button', { name: '切换到 Pixel Studio' }).click()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).not.toBeChecked()
  await dialog.getByRole('switch', { name: '启用字体策略' }).click()
  await dialog.getByRole('button', { name: 'Hook 捕获的全部文字' }).click()
  await dialog.getByText('Synthetic Mono', { exact: true }).click()
  await dialog.getByRole('button', { name: '切换到 Vector Studio' }).click()
  await expect(dialog.getByRole('button', { name: '仅词典命中' })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByLabel('字体候选优先级').locator('li')).toHaveCount(2)
  await dialog.getByRole('button', { name: '切换到 Pixel Studio' }).click()
  await expect(dialog.getByRole('button', { name: 'Hook 捕获的全部文字' })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByLabel('字体候选优先级').locator('li')).toHaveCount(1)
})

test('dictionary editor saves the complete portable metadata set', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  let settings = page.getByRole('dialog', { name: '词典设置' })
  await settings.getByRole('textbox', { name: '源语言' }).fill('fr-FR')
  await settings.getByRole('textbox', { name: '目标语言' }).fill('de-DE')
  await settings.getByRole('textbox', { name: '发布版本' }).fill('2.0.0')
  await settings.getByRole('textbox', { name: '作者' }).fill('Alice, Bob')
  await settings.getByRole('textbox', { name: '许可证' }).fill('Apache-2.0')
  await settings.getByRole('textbox', { name: '主页' }).fill('https://example.invalid/dictionary')
  await settings.getByRole('button', { name: '应用设置' }).click()
  await page.getByRole('button', { name: '保存词典' }).click()
  await page.getByRole('button', { name: '返回词典列表' }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('fr-FR')
  await expect(settings.getByRole('textbox', { name: '目标语言' })).toHaveValue('de-DE')
  await expect(settings.getByRole('textbox', { name: '作者' })).toHaveValue('Alice, Bob')
  await expect(settings.getByRole('textbox', { name: '许可证' })).toHaveValue('Apache-2.0')
  await expect(settings.getByRole('textbox', { name: '主页' })).toHaveValue('https://example.invalid/dictionary')
})

test('dictionary text editing saves only source and translation', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '编辑 Save As…' }).click()
  const editor = page.getByRole('dialog', { name: '编辑翻译词条' })
  await editor.getByRole('textbox', { name: '译文' }).fill('另存一个副本…')
  await editor.getByRole('button', { name: '保存词条' }).click()
  await page.getByRole('button', { name: '保存词典' }).click()

  const saved = await page.evaluate(() => {
    const model = JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3') ?? '{}')
    return model.dictionaryDetails?.['dictionary-proof']?.entries?.[1]
  })
  expect(saved).toEqual({
    source: 'Save As…',
    translation: '另存一个副本…',
  })
})

test('saved browser model contains inline policy without font assets or locations', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await page.getByRole('dialog', { name: '编辑工作流' }).getByRole('button', { name: '保存工作流' }).click()
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3') ?? '{}'))
  expect(saved.fontProfiles).toBeUndefined()
  expect(saved.software[0].locations).toBeUndefined()
  expect(saved.workflowDetails['workflow-proof'].targets[0].fontPolicy).toEqual({
    families: ['Synthetic Sans', 'Synthetic Serif'],
    coverage: 'dictionary_matches',
  })
})

test('compact viewport keeps the application shell bounded', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await expect(page.getByRole('heading', { name: '工作流' })).toBeVisible()
  const metrics = await page.evaluate(() => ({
    bodyHeight: document.body.scrollHeight,
    viewportHeight: window.innerHeight,
    bodyWidth: document.body.scrollWidth,
    viewportWidth: window.innerWidth,
  }))
  expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight)
  expect(metrics.bodyWidth).toBeLessThanOrEqual(metrics.viewportWidth)
})
