import { expect, test } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
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
