import { expect, test } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('quick probe turns an executable into a retained probe without prerequisite assets', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const softwareTemplate = structuredClone(snapshot.software[0])
    const dictionaryTemplate = structuredClone(snapshot.dictionaries[0])
    let currentSnapshot: any = {
      ...structuredClone(snapshot),
      selectedSoftwareId: null,
      software: [],
      dictionaries: [],
      workflows: [],
      activations: [],
      workflowRuntimeStatus: {},
    }
    let runs: any[] = []
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
        if (command === 'desktop_snapshot') return currentSnapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_preflight_software') {
          return {
            executablePath: args?.executablePath,
            executableName: 'QuickTarget.exe',
            suggestedName: 'QuickTarget',
            architecture: 'x86_64',
            running: true,
            canAdd: true,
            state: 'ready',
            existingName: null,
          }
        }
        if (command === 'desktop_start_quick_probe') {
          const summary = {
            id: 'quick-probe-ui', name: 'QuickTarget 快速测试', softwareId: 'software.quick-target',
            dictionaryId: 'quick-dictionary-ui', adapterIds: ['synthetic.ext-text-out'], status: 'running',
            livePreviewEnabled: false, observationRevision: 0, observedCount: 0, ignoredCount: 0,
            droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
            dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: true,
          }
          runs = [summary]
          currentSnapshot = {
            ...currentSnapshot,
            selectedSoftwareId: 'software.quick-target',
            software: [{ ...softwareTemplate, id: 'software.quick-target', name: 'QuickTarget', executableName: 'QuickTarget.exe', executablePath: args?.request.executablePath }],
            dictionaries: [{ ...dictionaryTemplate, metadata: { ...dictionaryTemplate.metadata, id: 'quick-dictionary-ui', name: 'QuickTarget 临时词典' }, revision: 1, entryCount: 0 }],
          }
          return summary
        }
        if (command === 'desktop_retain_quick_probe') {
          runs = [{ ...runs[0], quickProbe: false, updatedAtMs: 2 }]
          return runs[0]
        }
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_probe_run_summary') return runs[0]
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await page.setViewportSize({ width: 1440, height: 900 })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await expect(page.getByRole('button', { name: '新建探针任务' })).toBeDisabled()
  await page.getByRole('button', { name: '快速测试' }).click()
  const dialog = page.getByRole('dialog', { name: '快速测试一个程序' })
  await expect(dialog.getByText('任务名称', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('绑定词典', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('探针技术', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('目标语言', { exact: true })).toHaveCount(0)
  await dialog.getByRole('textbox', { name: '目标程序' }).fill('X:\\SyntheticFixtures\\QuickTarget.exe')
  await dialog.getByRole('button', { name: '检查' }).click()
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('已识别 QuickTarget')
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('x86_64')
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/quick-probe-ready-zh.png' })
  await dialog.getByRole('button', { name: '高级选项' }).click()
  await expect(dialog.getByText('目标语言', { exact: true })).toBeVisible()
  await dialog.getByRole('button', { name: '开始测试' }).click()

  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'QuickTarget 快速测试', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '保留', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '结束并清理', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保留', exact: true }).click()
  await expect(page.getByRole('button', { name: '探针设置', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '保留', exact: true })).toHaveCount(0)
})

test('quick probe foreground capture can end and clean up at compact English layout', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let runs: any[] = []
    const preflight = {
      executablePath: 'X:\\SyntheticFixtures\\CapturedTarget.exe', executableName: 'CapturedTarget.exe',
      suggestedName: 'CapturedTarget', architecture: 'x86_64', running: true, canAdd: false,
      state: 'already_added', existingName: 'Captured Target',
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'en-US', themePreference: 'light' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_arm_software_capture') return 'Ctrl+Shift+F8'
        if (command === 'desktop_cancel_software_capture') return null
        if (command === 'desktop_start_quick_probe') {
          const summary = {
            id: 'quick-probe-captured', name: 'Captured Target quick test', softwareId: 'software-proof',
            dictionaryId: 'quick-dictionary-captured', adapterIds: ['synthetic.ext-text-out'], status: 'running',
            livePreviewEnabled: false, observationRevision: 0, observedCount: 0, ignoredCount: 0,
            droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
            dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: true,
          }
          runs = [summary]
          return summary
        }
        if (command === 'desktop_cleanup_quick_probe') {
          runs = []
          ;(window as unknown as { __quickProbeCleaned?: boolean }).__quickProbeCleaned = true
          return { software: 'reused', dictionary: 'removed' }
        }
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_probe_run_summary') return runs[0]
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    ;(window as unknown as { __quickProbePreflight?: typeof preflight }).__quickProbePreflight = preflight
  }, { snapshot: model })
  await page.setViewportSize({ width: 960, height: 640 })
  await replaceModel(page, model)

  await page.getByRole('button', { name: 'Capture', exact: true }).click()
  await page.getByRole('button', { name: 'Quick test' }).click()
  const dialog = page.getByRole('dialog', { name: 'Quick-test an application' })
  await dialog.getByRole('button', { name: 'Capture foreground app' }).click()
  await expect(dialog.getByText('Waiting to capture the target')).toBeVisible()
  await page.evaluate(() => {
    const preflight = (window as unknown as { __quickProbePreflight: unknown }).__quickProbePreflight
    window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
      detail: { state: 'captured', shortcut: 'Ctrl+Shift+F8', preflight },
    }))
  })
  await expect(dialog.getByRole('textbox', { name: 'Target application' })).toHaveValue('X:\\SyntheticFixtures\\CapturedTarget.exe')
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('reuse “Captured Target”')
  await dialog.getByRole('button', { name: 'Start test' }).click()
  await expect(page.getByRole('heading', { name: 'Captured Target quick test', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'End and clean up', exact: true }).click()
  const confirmation = page.getByRole('dialog', { name: 'End quick test' })
  await expect(confirmation).toContainText('Assets referenced elsewhere are kept')
  await confirmation.getByRole('button', { name: 'End and clean up' }).click()

  await expect(confirmation).toHaveCount(0)
  await expect(page.getByText('No probe runs yet')).toBeVisible()
  await expect(page.getByRole('status')).toContainText('The software record that existed before the test is unchanged')
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as { __quickProbeCleaned?: boolean }).__quickProbeCleaned))).toBe(true)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/quick-probe-compact-en.png' })
})

test('quick probe keeps the launcher recoverable after a target stops during startup', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let attempts = 0
    let runs: any[] = []
    const summary = {
      id: 'quick-probe-retry', name: 'Vector Studio 快速测试', softwareId: 'software-proof',
      dictionaryId: 'quick-dictionary-retry', adapterIds: ['synthetic.ext-text-out'], status: 'running',
      livePreviewEnabled: false, observationRevision: 0, observedCount: 0, ignoredCount: 0,
      droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
      dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: true,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_preflight_software') {
          return {
            executablePath: args?.executablePath, executableName: 'VectorStudio.exe', suggestedName: 'Vector Studio',
            architecture: 'x86_64', running: true, canAdd: false, state: 'already_added', existingName: 'Vector Studio',
          }
        }
        if (command === 'desktop_start_quick_probe') {
          attempts += 1
          if (attempts === 1) throw { schemaVersion: 1, code: 'quick_probe.target_stopped', args: {} }
          runs = [summary]
          return summary
        }
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_probe_run_summary') return runs[0]
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '快速测试' }).click()
  const dialog = page.getByRole('dialog', { name: '快速测试一个程序' })
  await dialog.getByRole('textbox', { name: '目标程序' }).fill('X:\\SyntheticFixtures\\VectorStudio.exe')
  await dialog.getByRole('button', { name: '检查' }).click()
  await dialog.getByRole('button', { name: '开始测试' }).click()

  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('alert')).toContainText('请确认它仍在运行，再重新开始测试')
  await expect(dialog.getByRole('button', { name: '开始测试' })).toBeEnabled()
  await dialog.getByRole('button', { name: '开始测试' }).click()
  await expect(page.getByRole('heading', { name: 'Vector Studio 快速测试', exact: true })).toBeVisible()
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 22 }
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
