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
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!ExtTextOutW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-exttextoutw', configuration: 'none',
  }, {
    id: 'synthetic.text-out', name: 'TextOutW', summary: '拦截基础 GDI 文本输出；常见于传统 Win32 控件和简单自绘界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!TextOutW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-textoutw', configuration: 'none',
  }, {
    id: 'synthetic.draw-text', name: 'DrawTextW / DrawTextExW', summary: '拦截矩形内文本布局绘制；常见于按钮、标签和传统窗口界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['USER32 / GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'user32.dll!DrawTextW + DrawTextExW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-drawtextw', configuration: 'none',
  }, {
    id: 'synthetic.gdip-draw-string', name: 'GdipDrawString', summary: '拦截 GDI+ 浮点布局文本绘制；常见于自绘面板和图形化桌面界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI+'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdiplus.dll!GdipDrawString', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/gdiplus/-gdiplus-drawing-text-use', configuration: 'none',
  }, {
    id: 'synthetic.console-observer', name: 'WriteConsoleW 观察器', summary: '观察 Console 客户端 Unicode 输出；不执行翻译写回',
    version: '1.0.0', platforms: ['windows'], technologies: ['Windows Console'], features: ['textObserve'], technicalTarget: 'KernelBase!WriteConsoleW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/console/writeconsole', configuration: 'none',
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

function largeWorkflowCatalogModel() {
  const next = expandedModel()
  for (let index = 3; index <= 10; index += 1) {
    const softwareId = `software-batch-${index}`
    next.software.push({
      ...next.software[0],
      id: softwareId,
      name: `Batch Studio ${index}`,
      executableName: `BatchStudio${index}.exe`,
      executablePath: `X:\\SyntheticFixtures\\BatchStudio${index}.exe`,
      monogram: `B${index}`,
    })
    const target = {
      softwareId,
      adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
      dictionaryIds: ['dictionary-proof'],
      fontPolicy: null,
    }
    next.workflows[0].softwareIds.push(softwareId)
    next.workflows[0].targets.push(target)
    next.workflowDetails['workflow-proof'].targets.push(JSON.parse(JSON.stringify(target)))
  }
  for (let index = 3; index <= 100; index += 1) {
    next.dictionaries.push({
      metadata: {
        ...next.dictionaries[0].metadata,
        id: `dictionary-batch-${index}`,
        name: `批量词典 ${index}`,
        description: `第 ${index} 份合成测试词典`,
        tags: ['批量测试'],
      },
      revision: 1,
      entryCount: index,
    })
  }
  return next
}

async function replaceModel(page: import('@playwright/test').Page, value: ReturnType<typeof expandedModel>) {
  await page.addInitScript(({ key, modelValue }) => localStorage.setItem(key, JSON.stringify(modelValue)), { key: storageKey, modelValue: value })
  await page.goto('/')
}

function workflowEditor(page: import('@playwright/test').Page) {
  return page.getByTestId('workflow-editor')
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

test('software creation requires preflight and supports two-step foreground capture', async ({ page }) => {
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('button', { name: '快速捕获' })).toBeVisible()

  await page.getByRole('button', { name: '新增软件' }).click()
  const dialog = page.getByRole('dialog', { name: '新增软件' })
  await dialog.getByRole('textbox', { name: '软件名称' }).fill('Synthetic Editor')
  await dialog.getByRole('textbox', { name: '程序路径' }).fill('X:\\SyntheticFixtures\\SyntheticEditor.exe')
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeDisabled()
  await expect(dialog.getByText('通过接入检查后才能添加。')).toBeVisible()
  await dialog.getByRole('button', { name: '检查' }).click()
  await expect(dialog.getByText('基础接入条件已通过')).toBeVisible()
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeEnabled()
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
    detail: { state: 'armed', shortcut: 'Ctrl+Shift+F8' },
  })))
  await expect(page.getByRole('alert', { name: '等待选择软件' })).toContainText('Ctrl+Shift+F8')
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
    detail: {
      state: 'captured',
      shortcut: 'Ctrl+Shift+F8',
      preflight: {
        executablePath: 'X:\\SyntheticFixtures\\CapturedEditor.exe',
        executableName: 'CapturedEditor.exe',
        suggestedName: 'CapturedEditor',
        architecture: 'x86_64',
        running: true,
        canAdd: true,
        state: 'ready',
        existingName: null,
      },
    },
  })))
  await expect(dialog.getByRole('textbox', { name: '软件名称' })).toHaveValue('CapturedEditor')
  await expect(dialog.getByRole('textbox', { name: '程序路径' })).toHaveValue('X:\\SyntheticFixtures\\CapturedEditor.exe')
  await expect(dialog.getByText('基础接入条件已通过')).toBeVisible()
  await dialog.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(dialog).toBeHidden()
})

test('software rows expose a direct delete action', async ({ page }) => {
  await page.getByRole('button', { name: '软件', exact: true }).click()
  const row = page.getByRole('row').filter({ hasText: 'Vector Studio' })

  await expect(row.getByRole('button', { name: '删除 Vector Studio' })).toBeVisible()
})

test('software direct and batch delete remove unreferenced records', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.software.push({
    ...snapshot.software[0],
    id: 'software-disposable-one',
    name: 'Disposable One',
    executableName: 'DisposableOne.exe',
    executablePath: 'X:\\SyntheticFixtures\\DisposableOne.exe',
  }, {
    ...snapshot.software[0],
    id: 'software-disposable-two',
    name: 'Disposable Two',
    executableName: 'DisposableTwo.exe',
    executablePath: 'X:\\SyntheticFixtures\\DisposableTwo.exe',
  })
  await replaceModel(page, snapshot)
  await page.getByRole('button', { name: '软件', exact: true }).click()

  await page.getByRole('button', { name: '删除 Disposable One' }).click()
  await page.getByRole('dialog', { name: '删除软件' }).getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('Disposable One', { exact: true })).toBeHidden()

  await page.getByRole('checkbox', { name: '选择 Disposable Two' }).click()
  await page.getByRole('button', { name: '批量删除' }).click()
  await page.getByRole('dialog', { name: '删除软件' }).getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('Disposable Two', { exact: true })).toBeHidden()
})

test('software batch delete keeps a rejected record and explains why', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_remove_software') {
          throw {
            schemaVersion: 1,
            code: 'software.referenced',
            args: { workflowCount: 1, probeCount: 0 },
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.getByRole('checkbox', { name: '选择 Vector Studio' }).click()
  await page.getByRole('button', { name: '批量删除' }).click()
  const confirmation = page.getByRole('dialog', { name: '删除软件' })
  await confirmation.getByRole('button', { name: '确认删除' }).click()

  await expect(page.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(page.getByRole('alert')).toContainText('仍被 1 个工作流使用')
})

test('workflow names a stopped software and exposes its actionable Runtime error', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  snapshot.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: false, active: false,
        translationRequested: true, fontRequested: true,
        translationActive: false, fontActive: false, appliedGeneration: null,
      }],
      errors: {
        'software-proof': {
          schemaVersion: 1,
          code: 'runtime.target_not_found',
          args: {},
        },
      },
    },
  }
  await replaceModel(page, snapshot)

  await expect(page.getByText('需要处理', { exact: true })).toHaveCount(0)
  const stoppedStatus = page.getByRole('button', { name: '软件未启动', exact: true })
  await expect(stoppedStatus).toBeVisible()
  await stoppedStatus.click()
  const details = page.getByTestId('workflow-runtime-issues')
  await expect(details.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(details.getByText('没有找到与该程序路径匹配的运行实例；请先启动这个版本的软件。')).toBeVisible()
  await expect(details.getByRole('button', { name: '刷新状态', exact: true })).toBeVisible()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-runtime-stopped.png' })
})

test('workflow keeps permission failures distinct from a stopped software', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  snapshot.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: true, active: false,
        translationRequested: true, fontRequested: true,
        translationActive: false, fontActive: false, appliedGeneration: null,
      }],
      errors: {
        'software-proof': {
          schemaVersion: 1,
          code: 'runtime.target_access_failed',
          args: {},
        },
      },
    },
  }
  await replaceModel(page, snapshot)

  const failedStatus = page.getByRole('button', { name: '权限不匹配', exact: true })
  await expect(failedStatus).toBeVisible()
  await failedStatus.click()
  await expect(page.getByTestId('workflow-runtime-issues').getByText('无法写入目标软件。它可能已经退出，或正以管理员权限运行。请确认软件仍在运行；若权限更高，请在设置中开启“始终以管理员身份启动”。')).toBeVisible()
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

test('dictionary library separates local provenance from the offline catalog mode', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()

  const localMode = page.getByRole('button', { name: /本地词典/ })
  const catalogMode = page.getByRole('button', { name: '在线目录', exact: true })
  await expect(localMode).toHaveAttribute('aria-pressed', 'true')
  await expect(localMode).toHaveClass(/text-primary/)
  await expect(page.getByRole('columnheader', { name: '来源状态' })).toBeVisible()
  await expect(page.getByText('本地创建', { exact: true })).toBeVisible()
  await expect(page.getByText('未关联在线发布', { exact: true })).toBeVisible()

  await catalogMode.click()
  await expect(catalogMode).toHaveAttribute('aria-pressed', 'true')
  await expect(catalogMode).toHaveClass(/text-primary/)
  await expect(localMode).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByText('在线目录不可用', { exact: true })).toBeVisible()
  await expect(page.getByText('在线词典目录尚未配置或暂时不可用，本地词典不受影响。')).toBeVisible()
  await expect(page.getByText('第 1 页，本页 0 个版本')).toBeVisible()
  await expect(page.getByRole('button', { name: '新建词典' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '重试' })).toBeVisible()
})

test('dictionary library imports and exports one portable JSON file', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  const importedSnapshot = JSON.parse(JSON.stringify(snapshot))
  importedSnapshot.dictionaries.push({
    metadata: {
      ...snapshot.dictionaries[0].metadata,
      id: 'dictionary-imported',
      name: '导入词典',
      description: '标准 Dictionary /2 文件',
      releaseVersion: '1.0.0',
    },
    revision: 1,
    entryCount: 1,
    installation: {
      state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
    },
  })
  await page.addInitScript(({ current, imported }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return current
        if (command === 'plugin:dialog|open') return 'X:\\SyntheticFixtures\\dictionary-imported.json'
        if (command === 'desktop_import_dictionary') {
          ;(window as unknown as { __dictionaryImport?: unknown }).__dictionaryImport = args
          return imported
        }
        if (command === 'plugin:dialog|save') {
          ;(window as unknown as { __dictionarySaveDialog?: unknown }).__dictionarySaveDialog = args
          return 'X:\\SyntheticFixtures\\dictionary-imported.published.json'
        }
        if (command === 'desktop_export_dictionary') {
          ;(window as unknown as { __dictionaryExport?: unknown }).__dictionaryExport = args
          return null
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot, imported: importedSnapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByText('导入词典', { exact: true })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionaryImport?: { inputPath?: string } }).__dictionaryImport
  ))).toEqual(expect.objectContaining({ inputPath: 'X:\\SyntheticFixtures\\dictionary-imported.json' }))

  await page.getByRole('button', { name: '导出发布文件 导入词典' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionaryExport?: { dictionaryId?: string, outputPath?: string } }).__dictionaryExport
  ))).toEqual(expect.objectContaining({
    dictionaryId: 'dictionary-imported',
    outputPath: 'X:\\SyntheticFixtures\\dictionary-imported.published.json',
  }))
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionarySaveDialog?: { options?: { defaultPath?: string } } }).__dictionarySaveDialog
  ))).toEqual(expect.objectContaining({
    options: expect.objectContaining({ defaultPath: 'dictionary-imported.json' }),
  }))
})

test('dictionary export reports when the native save dialog cannot open', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'plugin:dialog|save') throw new Error('synthetic save dialog failure')
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '导出发布文件 界面基础词典', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('无法打开保存窗口，请重试')
})

test('local management items support double-click editing while keeping explicit actions', async ({ page }) => {
  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await expect(page.getByRole('heading', { name: '默认创作工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回工作流列表' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '编辑工作流' })).toHaveCount(0)
  await page.getByRole('button', { name: '返回工作流列表' }).click()

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.getByRole('row').filter({ hasText: 'Vector Studio' }).dblclick()
  await expect(page.getByRole('heading', { name: 'Vector Studio' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回软件列表' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '编辑软件' })).toHaveCount(0)
  await page.getByRole('button', { name: '返回软件列表' }).click()

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回词典列表' })).toBeVisible()

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const createProbe = page.getByRole('dialog', { name: '新建探针任务' })
  await createProbe.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await createProbe.getByRole('button', { name: '创建并连接' }).click()
  await page.getByRole('button', { name: '返回探针管理' }).click()
  await page.getByRole('row').filter({ hasText: 'Vector Studio 探针' }).dblclick()
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针' })).toBeVisible()

  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect(page.getByRole('button', { name: '编辑 默认创作工作流' })).toBeVisible()
})

test('escape returns from each independent item page and protects dirty forms', async ({ page }) => {
  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.getByRole('textbox', { name: '工作流名称' }).fill('尚未保存的工作流')
  await page.keyboard.press('Escape')
  const workflowDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(workflowDiscard).toBeVisible()
  await workflowDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.getByRole('textbox', { name: '工作流名称' }).fill('切换菜单前尚未保存')
  await page.getByRole('button', { name: '软件', exact: true }).click()
  const navigationDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(navigationDiscard).toBeVisible()
  await navigationDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '软件', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: 'Vector Studio' }).dblclick()
  await page.getByRole('textbox', { name: '显示名称' }).fill('尚未保存的软件')
  await page.keyboard.press('Escape')
  const softwareDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(softwareDiscard).toBeVisible()
  await softwareDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '软件', exact: true })).toBeVisible()

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '词典', exact: true })).toBeVisible()

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const createProbe = page.getByRole('dialog', { name: '新建探针任务' })
  await createProbe.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await createProbe.getByRole('button', { name: '创建并连接' }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '探针', exact: true })).toBeVisible()
})

test('configured dictionary catalog queries and installs through the desktop seam', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  const installedSnapshot = JSON.parse(JSON.stringify(snapshot))
  installedSnapshot.dictionaries.push({
    metadata: {
      ...snapshot.dictionaries[0].metadata,
      id: 'dictionary-catalog',
      name: '目录词典',
      description: '菜单翻译',
    },
    revision: 1,
    entryCount: 1,
    installation: {
      state: 'verified', installedRelease: '1.2.0', verifiedPublisher: 'publisher.example', updateRelease: null,
    },
  })
  await page.addInitScript(({ current, installed }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') {
          ;(window as unknown as { __catalogQuery?: unknown }).__catalogQuery = args?.request
          return {
            releases: [{
              catalogId: 'glyphshift.official', dictionaryId: 'dictionary-catalog', releaseVersion: '1.2.0',
              sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'zh-CN',
              name: '目录词典', summary: '菜单翻译', tags: ['菜单'], publisherIdentity: 'publisher.example',
            }],
            nextCursor: null,
          }
        }
        if (command === 'desktop_install_dictionary_release') {
          ;(window as unknown as { __catalogInstall?: unknown }).__catalogInstall = args?.request
          return installed
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot, installed: installedSnapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '在线目录', exact: true }).click()
  await expect(page.getByText('目录词典', { exact: true })).toBeVisible()
  await expect(page.getByText('publisher.example', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '按标签筛选 菜单' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogQuery?: { tag?: string | null } }).__catalogQuery
  ))).toEqual(expect.objectContaining({ tag: '菜单' }))
  await page.getByRole('button', { name: '清除标签筛选 菜单' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogQuery?: { tag?: string | null } }).__catalogQuery
  ))).toEqual(expect.objectContaining({ tag: null }))
  await page.getByRole('button', { name: '安装', exact: true }).click()
  await expect(page.getByRole('button', { name: '已安装', exact: true })).toBeDisabled()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogInstall?: { replacement?: string } }).__catalogInstall
  ))).toEqual(expect.objectContaining({
    dictionaryId: 'dictionary-catalog',
    releaseVersion: '1.2.0',
    replacement: 'reject_existing',
  }))
})

test('catalog requires explicit confirmation before replacing local dictionary changes', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'modified', installedRelease: '1.0.0', verifiedPublisher: 'publisher.example', updateRelease: '1.2.0',
  }
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') return {
          releases: [{
            catalogId: 'glyphshift.official', dictionaryId: 'dictionary-proof', releaseVersion: '1.2.0',
            sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'zh-CN',
            name: '界面基础词典', summary: '菜单与面板汉化', tags: ['菜单'], publisherIdentity: 'publisher.example',
          }],
          nextCursor: null,
        }
        if (command === 'desktop_install_dictionary_release') {
          ;(window as unknown as { __catalogReplacement?: unknown }).__catalogReplacement = args?.request
          return current
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '在线目录', exact: true }).click()
  await page.getByRole('button', { name: '更新', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '替换本地词典' })
  await expect(dialog.getByText(/包含本地修改/)).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogReplacement?: unknown }).__catalogReplacement
  ))).toBeUndefined()
  await dialog.getByRole('button', { name: '替换并安装' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogReplacement?: { replacement?: string } }).__catalogReplacement
  ))).toEqual(expect.objectContaining({ replacement: 'replace_any' }))
})

test('catalog presentation follows the English interface locale', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'en-US', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') {
          ;(window as unknown as { __catalogLocale?: string }).__catalogLocale = args?.request?.requestedPresentationLocale
          return {
            releases: [{
              catalogId: 'glyphshift.official', dictionaryId: 'dictionary-catalog', releaseVersion: '1.2.0',
              sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'en-US',
              name: 'Simplified Chinese menus', summary: 'Common menu and dialog text', tags: ['menus'], publisherIdentity: 'publisher.example',
            }],
            nextCursor: null,
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: 'Dictionaries', exact: true }).click()
  await page.getByRole('button', { name: 'Online catalog', exact: true }).click()
  await expect(page.getByText('Simplified Chinese menus', { exact: true })).toBeVisible()
  await expect(page.getByText('Common menu and dialog text', { exact: true })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogLocale?: string }).__catalogLocale
  ))).toBe('en-US')
})

test('running workflow opens a bounded local decision diagnostics table', async ({ page }) => {
  const active = JSON.parse(JSON.stringify(model))
  active.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  active.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: true, active: true,
        translationRequested: true, fontRequested: true,
        translationActive: true, fontActive: true, appliedGeneration: 4,
      }],
      errors: {},
    },
  }
  await page.addInitScript(({ snapshot }) => {
    const controls: boolean[] = []
    const internals = {
      invoke: async (command: string, args?: Record<string, unknown>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_control_workflow_diagnostics') {
          controls.push(Boolean(args?.enabled))
          ;(window as unknown as { __diagnosticControls: boolean[] }).__diagnosticControls = controls
          return null
        }
        if (command === 'desktop_workflow_diagnostics') {
          return {
            workflowId: 'workflow-proof',
            records: [{
              softwareId: 'software-proof', softwareName: 'Vector Studio', adapterName: 'ExtTextOutW',
              sourceText: 'Open', status: 'matched', text: 'replaced', font: 'protected', generation: 4,
              publicationIdentity: '71'.repeat(32), translationDigest: '72'.repeat(32), fontPolicyDigest: '73'.repeat(32),
            }],
            dropped: 2,
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: active })
  await replaceModel(page, active)

  await page.getByRole('button', { name: '诊断 默认创作工作流' }).click()
  const dialog = page.getByRole('dialog', { name: '运行诊断 · 默认创作工作流' })
  await expect(dialog.getByText('显示目标 Runtime 最近生成的决策')).toBeVisible()
  await expect(dialog.getByRole('columnheader', { name: '原文' })).toBeVisible()
  await expect(dialog.getByText('Open', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('替换决策已生成', { exact: true })).toBeVisible()
  await expect(dialog).toContainText('不代表目标界面已显示译文')
  await expect(dialog.getByText('字体保持原样', { exact: true })).toBeVisible()
  await expect(dialog.getByText('G4 · 71717171')).toBeVisible()
  await expect(dialog.getByText('2 条诊断因缓冲限制被丢弃')).toBeVisible()
  await expect(dialog.getByText('synthetic.ext-text-out')).toHaveCount(0)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/runtime-diagnostics-modal.png' })
  await dialog.getByRole('button', { name: '关闭诊断' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __diagnosticControls?: boolean[] }).__diagnosticControls
  ))).toEqual([true, false])
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
  await expect(dialog.getByText('可实时翻译', { exact: true })).toBeVisible()
  await expect(dialog.getByText('仅采集原文', { exact: true })).toBeVisible()
  await expect(dialog.getByText('可在 Glyphshift 中编辑、导出和用于 AI 翻译，暂不写回目标软件。', { exact: true })).toBeVisible()
  await expect(dialog.getByText('实时预览', { exact: true })).toBeVisible()
  await expect(page.getByText('gdi32.dll!TextOutW', { exact: true })).toHaveCount(0)
  await expect(page.getByText(/位置|语境/)).toHaveCount(0)

  await dialog.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await dialog.getByRole('button', { name: '创建并连接' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回探针管理' })).toBeVisible()
  await expect(page.getByPlaceholder('搜索原文、译文或探针技术')).toBeVisible()
  await expect(page.getByText(/技术目录|字典草稿/)).toHaveCount(0)
  await expect(page.getByText('还没有捕获到文字')).toBeVisible()
  await expect(page.getByText('每页')).toBeVisible()
  await expect(page.getByRole('button', { name: /开始监听|停止并生成/ })).toHaveCount(0)
})

test('probe list hides technical detail behind one accessible hover target and uses binary connection status', async ({ page }) => {
  const probeRuns = [{
    id: 'probe-detail',
    name: 'Vector Studio 探针',
    softwareId: 'software-proof',
    dictionaryId: 'dictionary-proof',
    adapterIds: ['synthetic.ext-text-out', 'synthetic.text-out', 'synthetic.draw-text', 'synthetic.gdip-draw-string'],
    status: 'interrupted',
    livePreviewEnabled: true,
    observationRevision: 4,
    observedCount: 37,
    ignoredCount: 0,
    droppedObservations: 0,
    previewGeneration: 2,
    createdAtMs: 1,
    updatedAtMs: 2,
    dictionaryRevision: 3,
    dictionaryEntryCount: 2,
  }]
  await page.addInitScript(({ snapshot, runs }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model, runs: probeRuns })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '探针', exact: true }).click()
  const row = page.getByRole('row').filter({ hasText: 'Vector Studio 探针' })
  await expect(row).toContainText('未连接')
  await expect(row).not.toContainText('ExtTextOutW · TextOutW')
  await expect(page.getByText('连接已中断', { exact: true })).toHaveCount(0)

  const details = row.getByRole('button', { name: /查看 Vector Studio 探针.*技术详情/ })
  await details.hover()
  await expect(page.getByText('程序位置', { exact: true })).toBeVisible()
  await expect(page.getByText('X:\\SyntheticFixtures\\VectorStudio.exe', { exact: true })).toBeVisible()
  await expect(page.getByText('探针技术', { exact: true })).toBeVisible()
  await expect(page.getByText('GDI · ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('GDI+ · GdipDrawString', { exact: true })).toBeVisible()
})

test('persisted probe closes creation modal even when its initial connection fails', async ({ page }) => {
  const persistedRun = {
    id: 'probe-persisted-after-connect-error',
    name: '离线软件探针',
    softwareId: 'software-proof',
    dictionaryId: 'dictionary-proof',
    adapterIds: ['synthetic.ext-text-out'],
    status: 'ready',
    livePreviewEnabled: true,
    observationRevision: 0,
    observedCount: 0,
    ignoredCount: 0,
    droppedObservations: 0,
    previewGeneration: 0,
    createdAtMs: 1,
    updatedAtMs: 1,
    dictionaryRevision: 3,
    dictionaryEntryCount: 2,
  }
  await page.addInitScript(({ snapshot, persisted }) => {
    let creationAttempted = false
    let createdRun = persisted
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return creationAttempted ? [createdRun] : []
        if (command === 'desktop_probe_run_summary') return createdRun
        if (command === 'desktop_create_probe_run') {
          creationAttempted = true
          createdRun = {
            ...persisted,
            id: args?.request.id,
            name: args?.request.name,
            softwareId: args?.request.softwareId,
            adapterIds: args?.request.adapterIds,
          }
          throw { schemaVersion: 1, code: 'runtime.component_incompatible', args: {} }
        }
        if (command === 'desktop_probe_run_entries') return {
          observationRevision: 0, dictionaryRevision: 3, page: 1, pageSize: 50, total: 0, rows: [],
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model, persisted: persistedRun })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await dialog.getByRole('textbox', { name: '任务名称' }).fill('离线软件探针')
  await dialog.getByRole('button', { name: '创建并连接' }).click()

  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('heading', { name: '离线软件探针', exact: true })).toBeVisible()
  await expect(page.getByRole('alert')).toContainText('所选探针技术不适用于当前软件')
  await expect(page.getByRole('alert')).not.toContainText('升级')
  await expect(page.getByRole('alert')).not.toContainText('重启')
})

test('new probe discards a cancelled inline dictionary draft', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()

  let dialog = page.getByRole('dialog', { name: '新建探针任务' })
  const existingDictionaryMode = dialog.getByRole('button', { name: '使用已有词典' })
  const newDictionaryMode = dialog.getByRole('button', { name: '新建空词典' })
  await expect(dialog.getByRole('textbox', { name: '任务名称' })).toHaveValue('')
  await expect(existingDictionaryMode).toHaveAttribute('aria-pressed', 'true')
  await expect(existingDictionaryMode).toHaveClass(/text-primary/)
  await expect(newDictionaryMode).toHaveAttribute('aria-pressed', 'false')
  await dialog.getByRole('textbox', { name: '任务名称' }).fill('不应保留的任务')
  await dialog.getByRole('checkbox', { name: 'TextOutW', exact: true }).uncheck()
  await newDictionaryMode.click()
  await expect(newDictionaryMode).toHaveAttribute('aria-pressed', 'true')
  await expect(newDictionaryMode).toHaveClass(/text-primary/)
  await expect(existingDictionaryMode).toHaveAttribute('aria-pressed', 'false')
  const scrollableRegions = await dialog.locator('*').evaluateAll(elements => elements.filter((element) => {
    const style = getComputedStyle(element)
    return ['auto', 'scroll'].includes(style.overflowY) && element.scrollHeight > element.clientHeight + 1
  }).length)
  expect(scrollableRegions).toBe(1)
  await dialog.getByRole('textbox', { name: '词典名称' }).fill('不应保留的词典')
  await dialog.getByRole('textbox', { name: '源语言' }).fill('ja-JP')
  await dialog.getByRole('textbox', { name: '目标语言' }).fill('ko-KR')
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.getByRole('button', { name: '新建探针任务' }).click()
  dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(dialog.getByRole('textbox', { name: '任务名称' })).toHaveValue('')
  await expect(dialog.getByRole('checkbox', { name: 'TextOutW', exact: true })).toBeChecked()
  await dialog.getByRole('button', { name: '新建空词典' }).click()
  await expect(dialog.getByRole('textbox', { name: '词典名称' })).toHaveValue('')
  await expect(dialog.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: '目标语言' })).toHaveValue('zh-CN')
  await expect(dialog.getByText('词典标识', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('说明', { exact: true })).toHaveCount(0)
})

test('new dictionary discards a cancelled metadata draft', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '新建词典' }).click()

  let dialog = page.getByRole('dialog', { name: '新建词典' })
  await dialog.getByRole('textbox', { name: '词典名称' }).fill('不应保留的词典')
  await dialog.getByRole('textbox', { name: '源语言' }).fill('ja-JP')
  await dialog.getByRole('textbox', { name: '目标语言' }).fill('ko-KR')
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.getByRole('button', { name: '新建词典' }).click()
  dialog = page.getByRole('dialog', { name: '新建词典' })
  await expect(dialog.getByRole('textbox', { name: '词典名称' })).toHaveValue('')
  await expect(dialog.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: '目标语言' })).toHaveValue('zh-CN')
  await expect(dialog.getByRole('textbox', { name: '发布版本' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '作者' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '许可证' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '主页' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '说明' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '标签' })).toHaveCount(0)
})

test('probe detail edits settings and clears all joined entries behind confirmation', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let cleared = false
    let summary = {
      id: 'probe-settings', name: '可配置探针', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'],
      status: 'ready', livePreviewEnabled: true, observationRevision: 4, observedCount: 1,
      ignoredCount: 0, droppedObservations: 0, previewGeneration: 2,
      createdAtMs: 1, updatedAtMs: 2,
      dictionaryRevision: 3, dictionaryEntryCount: 2,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_update_probe_run') {
          const request = structuredClone(args?.request)
          ;(window as unknown as { __probeSettingsRequest?: unknown }).__probeSettingsRequest = request
          summary = {
            ...summary,
            name: request.name,
            adapterIds: request.adapterIds,
            livePreviewEnabled: request.livePreviewEnabled,
            updatedAtMs: summary.updatedAtMs + 1,
          }
          return summary
        }
        if (command === 'desktop_clear_probe_run_entries') {
          cleared = true
          ;(window as unknown as { __probeClearCount?: number }).__probeClearCount = ((window as unknown as { __probeClearCount?: number }).__probeClearCount ?? 0) + 1
          summary = {
            ...summary,
            observationRevision: summary.observationRevision + 1,
            observedCount: 0,
            ignoredCount: 0,
            dictionaryRevision: summary.dictionaryRevision + 1,
            dictionaryEntryCount: 0,
            updatedAtMs: summary.updatedAtMs + 1,
          }
          return summary
        }
        if (command === 'desktop_probe_run_entries') {
          return {
            observationRevision: summary.observationRevision,
            dictionaryRevision: summary.dictionaryRevision,
            page: 1,
            pageSize: 50,
            total: cleared ? 0 : 1,
            rows: cleared
              ? []
              : [{
                  source: 'Open', translation: '打开', state: 'translated',
                  adapterIds: ['synthetic.text-out'], count: 3, firstSeenMs: 1, lastSeenMs: 2,
                }],
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', 'probe-settings')
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '探针设置' }).click()
  const settings = page.getByRole('dialog', { name: '探针设置' })
  await settings.getByRole('textbox', { name: '任务名称' }).fill('整理后的探针')
  await settings.getByRole('checkbox', { name: /WriteConsoleW 观察器/ }).check()
  await settings.getByRole('button', { name: '保存设置' }).click()
  await expect(page.getByRole('heading', { name: '整理后的探针' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __probeSettingsRequest?: { adapterIds: string[]; livePreviewEnabled: boolean } }).__probeSettingsRequest
  ))).toMatchObject({
    adapterIds: ['synthetic.text-out', 'synthetic.console-observer'],
    livePreviewEnabled: true,
  })

  await page.getByRole('button', { name: '探针设置' }).click()
  await page.getByTestId('capture-clear-all').click()
  const confirmation = page.getByRole('dialog', { name: '清空全部探针条目' })
  await expect(confirmation.getByText(/界面基础词典/)).toBeVisible()
  await confirmation.getByRole('button', { name: '确认清空' }).click()
  await expect(page.getByText('还没有捕获到文字')).toBeVisible()
  await expect(page.getByText('0 条已观察 · 词典 0 条')).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __probeClearCount?: number }).__probeClearCount
  ))).toBe(1)
})

test('probe detail states when the active runtime can only collect text', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-collection-only', name: '仅采集任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.console-observer'], status: 'running', livePreviewEnabled: false,
      observationRevision: 1, observedCount: 12, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 3,
      dictionaryEntryCount: 2, runtimeCapability: 'collection_only',
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') return {
          observationRevision: 1, dictionaryRevision: 3, page: 1, pageSize: 50, total: 0, rows: [],
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await expect(page.getByTestId('probe-runtime-capability')).toHaveText('仅采集')
  await expect(page.getByText(/只采集原文，目标软件界面不会被修改/)).toBeVisible()
})

test('probe run keeps backend paging while adapter filters and view state recover', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const translations: Record<string, string> = {}
    let summary = {
      id: 'probe-scale', name: '5000 条性能任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out', 'synthetic.draw-text'],
      status: 'running', livePreviewEnabled: true, observationRevision: 12, observedCount: 5000,
      ignoredCount: 0, droppedObservations: 0, previewGeneration: 8,
      createdAtMs: 1, updatedAtMs: 2,
      dictionaryRevision: 8, dictionaryEntryCount: 2500,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_set_probe_run_paused') {
          summary = { ...summary, status: args?.paused ? 'paused' : 'running' }
          return summary
        }
        if (command === 'desktop_edit_probe_translation') {
          const request = args?.request as { source: string; translation: string }
          ;(window as unknown as { __captureEditRequests?: unknown[] }).__captureEditRequests ??= []
          ;(window as unknown as { __captureEditRequests: unknown[] }).__captureEditRequests.push(request)
          translations[request.source] = request.translation
          summary = { ...summary, dictionaryRevision: summary.dictionaryRevision + 1, dictionaryEntryCount: summary.dictionaryEntryCount + 1, previewGeneration: summary.previewGeneration + 1 }
          return summary
        }
        const request = args?.request as { search: string; adapterIds: string[]; page: number; pageSize: number }
        if (command === 'desktop_probe_run_entries') {
          ;(window as unknown as { __captureQueryRequests?: unknown[] }).__captureQueryRequests ??= []
          ;(window as unknown as { __captureQueryRequests: unknown[] }).__captureQueryRequests.push(structuredClone(request))
          const start = (request.page - 1) * request.pageSize
          const filtered = request.adapterIds.includes('synthetic.draw-text')
          const total = filtered ? 30 : 5000
          const rowCount = Math.max(0, Math.min(request.pageSize, total - start))
          return {
            observationRevision: 12, dictionaryRevision: summary.dictionaryRevision,
            page: request.page, pageSize: request.pageSize, total,
            rows: Array.from({ length: rowCount }, (_, offset) => {
              const source = `Source ${String(start + offset + 1).padStart(4, '0')}`
              const translation = translations[source] ?? (offset % 2 ? `译文 ${start + offset + 1}` : '')
              return {
                source, translation,
                state: translation ? 'translated' : 'pending',
                adapterIds: filtered ? ['synthetic.draw-text'] : ['synthetic.text-out'], count: start + offset + 1,
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
  await expect(page.locator('tbody tr').first()).toContainText('词典未命中')
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

  const adapterFilter = page.getByTestId('capture-adapter-filter')
  await expect(adapterFilter).toContainText('全部技术')
  await adapterFilter.click()
  await page.getByRole('menuitemcheckbox', { name: 'DrawTextW / DrawTextExW' }).click()
  await expect(adapterFilter).toContainText('DrawTextW / DrawTextExW')
  await expect(page.getByText('显示 1–30，共 30 条目')).toBeVisible()
  await expect(page.locator('tbody tr')).toHaveCount(30)
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ adapterIds: string[] }> }).__captureQueryRequests?.at(-1)?.adapterIds
  ))).toEqual(['synthetic.draw-text'])

  await page.keyboard.press('Escape')
  const search = page.getByPlaceholder('搜索原文、译文或探针技术')
  await search.fill('Source 00')
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ search: string }> }).__captureQueryRequests?.at(-1)?.search
  ))).toBe('Source 00')
  await page.getByRole('button', { name: '暂停收集' }).click()
  await expect(page.getByRole('button', { name: '继续收集' })).toBeVisible()
  const pausedTranslation = page.getByLabel('“Source 0001”的译文')
  await pausedTranslation.fill('暂停时译文')
  await pausedTranslation.blur()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureEditRequests?: Array<{ translation: string }> }).__captureEditRequests?.at(-1)?.translation
  ))).toBe('暂停时译文')
  await expect(page.getByRole('button', { name: '导出' })).toBeEnabled()

  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await expect(page.getByPlaceholder('搜索原文、译文或探针技术')).toHaveValue('Source 00')
  await expect(page.getByTestId('capture-adapter-filter')).toContainText('DrawTextW / DrawTextExW')
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ adapterIds: string[] }> }).__captureQueryRequests?.at(-1)?.adapterIds
  ))).toEqual(['synthetic.draw-text'])
})

test('probe reconnect reports an actionable target Runtime failure', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-reconnect', name: '连接诊断任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 3, observedCount: 433, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 28, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 6,
      dictionaryEntryCount: 6,
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') {
          return {
            observationRevision: 3, dictionaryRevision: 6, page: 1, pageSize: 50, total: 0, rows: [],
          }
        }
        if (command === 'desktop_resume_probe_run') {
          throw {
            schemaVersion: 1,
            code: 'runtime.target_access_failed',
            args: {},
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', 'probe-reconnect')
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '连接并继续' }).click()

  await expect(page.getByText('无法写入目标软件。它可能已经退出，或正以管理员权限运行。请确认软件仍在运行；若权限更高，请在设置中开启“始终以管理员身份启动”。')).toBeVisible()
  await expect(page.getByRole('button', { name: '连接并继续' })).toBeEnabled()
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
  await expect(page.getByRole('button', { name: /技术文档/ })).toHaveCount(5)
  await page.evaluate(() => {
    window.open = ((url?: string | URL) => {
      ;(window as unknown as { __openedAdapterDocumentation?: string }).__openedAdapterDocumentation = String(url)
      return window
    }) as typeof window.open
  })
  await page.getByRole('button', { name: '查看 ExtTextOutW 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-exttextoutw')
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
  await expect(page.getByRole('combobox')).toHaveCount(3)

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
  await expect(page.getByRole('button', { name: 'View technical documentation for ExtTextOutW' })).toBeVisible()
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

test('settings persists startup close behavior and the administrator launch preference', async ({ page }) => {
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: '应用行为' })).toBeVisible()
  const launchAtStartup = page.getByRole('switch', { name: '开机自动启动' })
  await expect(launchAtStartup).not.toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时' })).toContainText('彻底退出')

  await expect(page.getByRole('heading', { name: '权限' })).toBeVisible()
  await expect(page.getByText('普通权限', { exact: true })).toBeVisible()
  const launchElevated = page.getByRole('switch', { name: '始终以管理员身份启动' })
  await expect(launchElevated).not.toBeChecked()

  await launchAtStartup.click()
  await launchElevated.click()
  await page.getByRole('combobox', { name: '关闭窗口时' }).click()
  await page.getByRole('option', { name: '最小化到任务栏' }).click()
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('switch', { name: '开机自动启动' })).toBeChecked()
  await expect(page.getByRole('switch', { name: '始终以管理员身份启动' })).toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时' })).toContainText('最小化到任务栏')
})

test('administrator launch preference persists before elevation and disables without another restart', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const elevationCommands: Array<{ command: string; launchElevated?: boolean }> = []
    const internals = {
      invoke: async (command: string, args?: { update?: { launchElevated?: boolean } }) => {
        if (command === 'desktop_settings') return {
          settingsSchemaVersion: 1,
          localePreference: 'zh-CN',
          themePreference: 'dark',
          launchAtStartup: false,
          launchElevated: false,
          closeBehavior: 'quit',
        }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 19 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_privilege_status') return { elevated: false }
        if (command === 'desktop_update_settings') {
          elevationCommands.push({ command, launchElevated: args?.update?.launchElevated })
          return { settingsSchemaVersion: 1, ...args?.update }
        }
        if (command === 'desktop_restart_elevated') elevationCommands.push({ command })
        return null
      },
    }
    ;(window as unknown as {
      __TAURI_INTERNALS__: typeof internals
      __elevationCommands: typeof elevationCommands
    }).__TAURI_INTERNALS__ = internals
    ;(window as unknown as { __elevationCommands: typeof elevationCommands }).__elevationCommands = elevationCommands
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()

  const launchElevated = page.getByRole('switch', { name: '始终以管理员身份启动' })
  await launchElevated.click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __elevationCommands: Array<{ command: string }> }).__elevationCommands.map(entry => entry.command)
  ))).toEqual(['desktop_update_settings', 'desktop_restart_elevated'])

  await launchElevated.click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __elevationCommands: Array<{ command: string; launchElevated?: boolean }> }).__elevationCommands
  ))).toEqual([
    { command: 'desktop_update_settings', launchElevated: true },
    { command: 'desktop_restart_elevated' },
    { command: 'desktop_update_settings', launchElevated: false },
  ])
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
  await expect(page.getByRole('textbox', { name: '编辑译文：Save As…' })).toHaveValue('另存为…')
  await expect(page.getByRole('columnheader', { name: '位置', exact: true })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '语境', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '词典设置' }).click()
  const settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(settings.getByRole('textbox', { name: '作者' })).toHaveValue('Glyphshift')
  await settings.getByRole('button', { name: '取消' }).click()
  await expect(page.getByRole('button', { name: '添加词条' })).toHaveCount(0)
  await expect(page.getByRole('textbox', { name: '新词条原文' })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '新词条译文' })).toBeVisible()
  await expect(page.getByText('语义位置')).toHaveCount(0)
  await expect(page.getByText('限定语境')).toHaveCount(0)
  await expect(page.getByText('默认字体')).toHaveCount(0)
  await expect(page.getByText(/Hook/)).toHaveCount(0)
})

test('workflow target independently selects adapters dictionaries and one font policy', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await expect(dialog.getByRole('tab')).toHaveCount(4)
  await expect(dialog.getByRole('tab', { name: '基础配置' })).toHaveAttribute('aria-selected', 'true')

  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await expect(dialog.getByText('windows · GDI', { exact: true })).toBeVisible()
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(dialog.getByText('WriteConsoleW 观察器', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)

  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await expect(dialog.getByText('以下词典只应用于 Vector Studio')).toBeVisible()
  await expect(dialog.getByText('界面基础词典', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '提高 界面基础词典 的优先级' })).toBeDisabled()

  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).toBeChecked()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByPlaceholder('搜索本机字体')).toBeVisible()
  await expect(dialog.getByText(/位置|main-ui/)).toHaveCount(0)
})

test('observe-only adapter stays available and disables live preview only when selected alone', async ({ page }) => {
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()

  const form = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(form.getByText('WriteConsoleW 观察器', { exact: true })).toBeVisible()
  await expect(form.getByRole('switch', { name: '实时预览' })).toBeEnabled()
  for (const name of ['ExtTextOutW', 'TextOutW', 'DrawTextW / DrawTextExW', 'GdipDrawString']) {
    await form.getByRole('checkbox', { name, exact: true }).uncheck()
  }
  await expect(form.getByRole('switch', { name: '实时预览' })).toBeDisabled()
  await expect(form.getByText('所选技术中没有可写回目标软件的技术，无法开启实时预览。')).toBeVisible()
})

test('font policy uses a compact coverage selector and an explicit cached refresh', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()

  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByRole('button', { name: '仅词典命中' })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: 'Hook 捕获的全部文字' })).toHaveCount(0)
  await expect(dialog.getByText('已缓存 3 种字体', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '刷新字体列表' })).toBeVisible()
})

test('workflow editor uses a left section rail for four focused large-catalog views', async ({ page }) => {
  await page.setViewportSize({ width: 1180, height: 760 })
  await replaceModel(page, largeWorkflowCatalogModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)

  const nameBox = await dialog.getByRole('textbox', { name: '工作流名称' }).boundingBox()
  const descriptionBox = await dialog.getByRole('textbox', { name: '工作流描述' }).boundingBox()
  expect(nameBox).not.toBeNull()
  expect(descriptionBox).not.toBeNull()
  expect(descriptionBox!.y).toBeGreaterThan(nameBox!.y + nameBox!.height)

  await expect(dialog).toHaveClass(/flex-1/)
  await page.waitForTimeout(250)
  const initialEditorHeight = await dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))
  const tabList = dialog.getByRole('tablist')
  await expect(dialog.getByTestId('workflow-editor-tabs')).toHaveClass(/h-full/)
  await expect.poll(() => tabList.evaluate(element => getComputedStyle(element).flexDirection)).toBe('column')
  const navigationBox = await tabList.boundingBox()
  const basicContentBox = await dialog.getByTestId('workflow-basic-tab').boundingBox()
  expect(navigationBox).not.toBeNull()
  expect(basicContentBox).not.toBeNull()
  expect(navigationBox!.x + navigationBox!.width).toBeLessThanOrEqual(basicContentBox!.x)
  await expect.poll(() => tabList.evaluate(element => element.scrollWidth === element.clientWidth && element.scrollHeight === element.clientHeight)).toBe(true)
  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  const softwareCatalog = dialog.getByTestId('workflow-software-catalog')
  await expect.poll(() => softwareCatalog.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
  await dialog.getByPlaceholder('搜索软件').fill('Batch Studio 10')
  await expect(softwareCatalog.getByText('Batch Studio 10', { exact: true })).toBeVisible()
  await expect(dialog.getByTestId('workflow-adapter-config')).toBeVisible()

  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  const targetSelect = dialog.getByRole('button', { name: '当前软件目标' })
  await targetSelect.click()
  await expect(page.getByRole('option')).toHaveCount(10)
  await page.keyboard.press('Escape')
  const dictionaryCatalog = dialog.getByTestId('workflow-dictionary-catalog')
  await expect.poll(() => dictionaryCatalog.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
  await dialog.getByPlaceholder('搜索词典').fill('批量词典 100')
  await expect(dialog.getByText('批量词典 100', { exact: true })).toBeVisible()

  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  await expect(dialog.getByRole('tab', { name: '字体策略' })).toHaveAttribute('aria-selected', 'true')
  await expect(dialog.getByTestId('workflow-font-tab')).toBeVisible()
  await expect(dialog.getByTestId('workflow-dictionary-catalog')).toHaveCount(0)
  await page.waitForTimeout(200)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-editor-tabbed-large-catalogs.png' })
})

test('workflow saves reordered dictionaries in explicit priority order', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await dialog.getByRole('checkbox', { name: '选择词典 效果词典' }).click()
  await dialog.getByRole('button', { name: '提高 效果词典 的优先级' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  const priorityItems = dialog.getByTestId('workflow-dictionary-catalog').locator('[data-dictionary-id]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-dictionary-id', 'dictionary-effects')
  await expect(priorityItems.nth(0)).toContainText('优先级 1')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-dictionary-id', 'dictionary-proof')
  await expect(priorityItems.nth(1)).toContainText('优先级 2')
})

test('workflow saves font coverage and ordered inline candidates', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toHaveCount(0)
  await dialog.getByRole('combobox', { name: '应用范围' }).click()
  await page.getByRole('option', { name: 'Hook 捕获的全部文字' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toContainText('未翻译文字、图标或符号')
  await dialog.getByRole('button', { name: '提高 Synthetic Serif 的优先级' }).click()
  await dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('Hook 捕获的全部文字')
  const priorityItems = dialog.getByTestId('workflow-font-catalog').locator('[data-font-family]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-font-family', 'Synthetic Serif')
  await expect(priorityItems.nth(0)).toContainText('优先级 1')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-font-family', 'Synthetic Sans')
  await expect(priorityItems.nth(2)).toHaveAttribute('data-font-family', 'Synthetic Mono')
})

test('inline font policy stays isolated between software targets', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  const currentTarget = dialog.getByRole('button', { name: '当前软件目标' })
  await currentTarget.click()
  await page.getByRole('option', { name: /^Pixel Studio/ }).click()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).not.toBeChecked()
  await dialog.getByRole('switch', { name: '启用字体策略' }).click()
  await dialog.getByRole('combobox', { name: '应用范围' }).click()
  await page.getByRole('option', { name: 'Hook 捕获的全部文字' }).click()
  await dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).click()
  await currentTarget.click()
  await page.getByRole('option', { name: /^Vector Studio/ }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Sans' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Serif' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).not.toBeChecked()
  await currentTarget.click()
  await page.getByRole('option', { name: /^Pixel Studio/ }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('Hook 捕获的全部文字')
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Sans' })).not.toBeChecked()
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
  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保存词典' }).click()
  await expect(page.getByText('未保存', { exact: true })).toHaveCount(0)
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

test('dictionary editor uses one guarded inline draft', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  const existingRow = page.getByRole('row').filter({ has: page.getByRole('textbox', { name: '编辑原文：Save As…' }) })
  await expect(existingRow.getByRole('textbox', { name: '编辑原文：Save As…' })).toHaveValue('Save As…')
  await expect(existingRow.getByRole('textbox', { name: '编辑译文：Save As…' })).toHaveValue('另存为…')
  await expect(page.getByRole('button', { name: '编辑 Save As…' })).toHaveCount(0)

  const blankRow = page.getByRole('row').filter({ has: page.getByRole('textbox', { name: '新词条原文' }) })
  await expect(blankRow.getByRole('textbox', { name: '新词条原文' })).toBeVisible()
  await blankRow.getByRole('textbox', { name: '新词条原文' }).fill('Close')
  await blankRow.getByRole('textbox', { name: '新词条译文' }).fill('关闭')
  await blankRow.getByRole('textbox', { name: '新词条译文' }).press('Enter')

  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await expect(blankRow.getByRole('textbox', { name: '新词条原文' })).toHaveValue('')
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-inline-draft.png' })

  await page.getByRole('button', { name: '返回词典列表' }).click()
  const guard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(guard).toBeVisible()
  await guard.getByRole('button', { name: '继续编辑' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '放弃未保存更改？' })).toBeVisible()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '继续编辑' }).click()
  await page.getByRole('button', { name: '关闭窗口' }).click()
  await expect(page.getByRole('dialog', { name: '放弃未保存更改？' })).toBeVisible()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '继续编辑' }).click()
  await page.getByRole('button', { name: '返回词典列表' }).click()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '词典', exact: true })).toBeVisible()
})

test('dictionary text editing saves only source and translation', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('textbox', { name: '编辑译文：Save As…' }).fill('另存一个副本…')
  await page.getByRole('textbox', { name: '新词条原文' }).fill('Close')
  await page.getByRole('textbox', { name: '新词条译文' }).fill('关闭')
  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保存词典' }).click()

  const saved = await page.evaluate(() => {
    const model = JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3') ?? '{}')
    return model.dictionaryDetails?.['dictionary-proof']?.entries
  })
  expect(saved[1]).toEqual({
    source: 'Save As…',
    translation: '另存一个副本…',
  })
  expect(saved[2]).toEqual({ source: 'Close', translation: '关闭' })
})

test('saved browser model contains inline policy without font assets or locations', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await workflowEditor(page).getByRole('button', { name: '保存工作流' }).click()
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
