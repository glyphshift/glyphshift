import { expect, test, type Page } from '@playwright/test'
import { model } from './fixtures/productModel'

function collectionModel() {
  const snapshot = structuredClone(model)
  Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
  return snapshot
}

async function openWorkflowText(page: Page) {
  await page.goto('/')
  await page.getByRole('button', { name: '查看文字', exact: true }).click()
}

async function openTaskActions(page: Page) {
  await page.getByTestId('probe-task-actions').click()
}

test('workflow text view can launch its bound software', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const run = {
      id: 'workflow-launch', workflowId: 'workflow-proof', name: '默认创作工作流',
      softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: false,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: null, quickProbe: false,
    }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      if (command === 'desktop_settings') return {
        settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1,
        localePreference: 'zh-CN', themePreference: 'dark',
      }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_probe_runs') return [run]
      if (command === 'desktop_workflow_collection' || command === 'desktop_probe_run_summary') return run
      if (command === 'desktop_probe_run_entries') return {
        observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [],
      }
      if (command === 'desktop_launch_software') {
        if ((window as any).__launchedSoftwareId) {
          throw { schemaVersion: 1, code: 'software.executable_missing', args: {} }
        }
        ;(window as any).__launchedSoftwareId = args?.softwareId
        return null
      }
      return null
    } }
  }, { snapshot: collectionModel() })

  await page.setViewportSize({ width: 960, height: 640 })
  await openWorkflowText(page)
  await openTaskActions(page)
  await page.getByRole('menuitem', { name: '运行软件', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__launchedSoftwareId)).toBe('software-proof')

  await openTaskActions(page)
  await page.getByRole('menuitem', { name: '运行软件', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('绑定的程序文件已不存在，请在工作流的“设置软件”中重新选择程序位置。')
})

test('workflow text view keeps backend paging while adapter filters and view state recover', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const translations: Record<string, string> = {}
    let summary = {
      id: 'workflow-scale', workflowId: 'workflow-proof', name: '默认创作工作流',
      softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out', 'synthetic.draw-text'], status: 'running', livePreviewEnabled: true,
      observationRevision: 12, observedCount: 5000, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 8, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 8,
      dictionaryEntryCount: 2500, runtimeCapability: 'direct_replace', quickProbe: false,
    }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      if (command === 'desktop_settings') return {
        settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1,
        localePreference: 'zh-CN', themePreference: 'dark',
      }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_probe_runs') return [summary]
      if (command === 'desktop_workflow_collection' || command === 'desktop_probe_run_summary') return summary
      if (command === 'desktop_set_probe_run_paused') {
        summary = { ...summary, status: args?.paused ? 'paused' : 'running' }
        return summary
      }
      if (command === 'desktop_disconnect_probe_run') {
        summary = { ...summary, status: 'ready' }
        ;(window as any).__probeDisconnected = true
        return summary
      }
      if (command === 'desktop_edit_probe_translation') {
        const request = args?.request as { source: string; translation: string }
        ;(window as any).__captureEditRequests ??= []
        ;(window as any).__captureEditRequests.push(structuredClone(request))
        translations[request.source] = request.translation
        summary = {
          ...summary,
          dictionaryRevision: summary.dictionaryRevision + 1,
          dictionaryEntryCount: summary.dictionaryEntryCount + 1,
          previewGeneration: summary.previewGeneration + 1,
        }
        return summary
      }
      if (command === 'desktop_probe_run_entries') {
        const request = args?.request as {
          search: string
          adapterIds: string[]
          translationFilter: 'all' | 'untranslated' | 'translated'
          page: number
          pageSize: number
        }
        ;(window as any).__captureQueryRequests ??= []
        ;(window as any).__captureQueryRequests.push(structuredClone(request))
        const filtered = request.adapterIds.includes('synthetic.draw-text')
        const sourceIndices = Array.from({ length: filtered ? 30 : 5000 }, (_, index) => index + 1)
          .filter(sourceIndex => {
            const source = `Source ${String(sourceIndex).padStart(4, '0')}`
            const translation = translations[source] ?? (sourceIndex % 2 === 0 ? `译文 ${sourceIndex}` : '')
            if (request.search && !source.includes(request.search)) return false
            if (request.translationFilter === 'untranslated') return !translation
            if (request.translationFilter === 'translated') return Boolean(translation)
            return true
          })
        const start = (request.page - 1) * request.pageSize
        const pageIndices = sourceIndices.slice(start, start + request.pageSize)
        return {
          observationRevision: 12,
          dictionaryRevision: summary.dictionaryRevision,
          page: request.page,
          pageSize: request.pageSize,
          total: sourceIndices.length,
          rows: pageIndices.map(sourceIndex => {
            const source = `Source ${String(sourceIndex).padStart(4, '0')}`
            const translation = translations[source] ?? (sourceIndex % 2 === 0 ? `译文 ${sourceIndex}` : '')
            return {
              source,
              translation,
              state: translation ? 'translated' : 'pending',
              adapterIds: filtered ? ['synthetic.draw-text'] : ['synthetic.text-out'],
              count: sourceIndex,
              firstSeenMs: 1,
              lastSeenMs: 2,
            }
          }),
        }
      }
      return null
    } }
  }, { snapshot: collectionModel() })

  await page.setViewportSize({ width: 1180, height: 760 })
  await openWorkflowText(page)

  await expect(page.getByText('显示 1–50，共 5000 条目')).toBeVisible()
  await expect(page.locator('tbody tr')).toHaveCount(50)
  const firstTranslation = page.getByRole('textbox', { name: '“Source 0001”的译文' })
  await firstTranslation.fill('即时译文')
  await firstTranslation.blur()
  await expect.poll(() => page.evaluate(() => (window as any).__captureEditRequests?.[0]?.translation)).toBe('即时译文')

  const translationFilter = page.getByRole('button', { name: '按翻译状态筛选' })
  await translationFilter.click()
  await page.getByRole('menuitem', { name: '未翻译', exact: true }).click()
  await expect(page.getByText('显示 1–50，共 2499 条目')).toBeVisible()

  const adapterFilter = page.getByTestId('capture-adapter-filter')
  await adapterFilter.click()
  await page.getByRole('menuitemcheckbox', { name: 'Windows 按钮与标签' }).click()
  await expect(adapterFilter).toContainText('Windows 按钮与标签')
  await expect.poll(() => page.evaluate(() => (window as any).__captureQueryRequests?.at(-1)?.adapterIds)).toEqual(['synthetic.draw-text'])

  await page.keyboard.press('Escape')
  const search = page.getByPlaceholder('搜索原文、译文或适配器')
  await search.fill('Source 00')
  await expect.poll(() => page.evaluate(() => (window as any).__captureQueryRequests?.at(-1)?.search)).toBe('Source 00')

  await page.reload()
  await page.getByRole('button', { name: '查看文字', exact: true }).click()
  await expect(page.getByPlaceholder('搜索原文、译文或适配器')).toHaveValue('Source 00')
  await expect(page.getByRole('button', { name: '按翻译状态筛选' })).toContainText('未翻译')
  await expect(page.getByTestId('capture-adapter-filter')).toContainText('Windows 按钮与标签')
})

test('workflow text contextual rows keep observation identity while sharing one source translation', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let summary = {
      id: 'workflow-context', workflowId: 'workflow-proof', name: '默认创作工作流',
      softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'],
      status: 'running', livePreviewEnabled: false, observationRevision: 3, observedCount: 3,
      ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 2,
      dictionaryRevision: 2, dictionaryEntryCount: 1, runtimeCapability: 'direct_replace', quickProbe: false,
    }
    const rows = [
      {
        source: 'Open', translation: '打开', translationContext: { context: 'MainMenu', disambiguation: null, pluralN: null },
        state: 'translated', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 3,
      },
      {
        source: 'Open', translation: '打开', translationContext: { context: 'Toolbar', disambiguation: 'button', pluralN: null },
        state: 'translated', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 2,
      },
      {
        source: '%n files', translation: '', translationContext: { context: 'Counter', disambiguation: null, pluralN: 2 },
        state: 'pending', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 1,
        resolution: { kind: 'pending', dictionaryIds: [], ruleIndex: null, editable: false },
      },
    ]
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      if (command === 'desktop_settings') return {
        settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1,
        localePreference: 'zh-CN', themePreference: 'dark',
      }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_probe_runs') return [summary]
      if (command === 'desktop_workflow_collection' || command === 'desktop_probe_run_summary') return summary
      if (command === 'desktop_probe_run_entries') return {
        observationRevision: 3, dictionaryRevision: summary.dictionaryRevision,
        page: 1, pageSize: 50, total: rows.length, rows,
      }
      if (command === 'desktop_edit_probe_translation') {
        const request = structuredClone(args?.request)
        ;(window as any).__contextEditRequests ??= []
        ;(window as any).__contextEditRequests.push(request)
        for (const row of rows.filter(row => row.source === request.source)) {
          row.translation = request.translation
          row.state = request.translation ? 'translated' : 'pending'
        }
        summary = { ...summary, dictionaryRevision: summary.dictionaryRevision + 1 }
        return summary
      }
      return null
    } }
  }, { snapshot: collectionModel() })

  await openWorkflowText(page)
  const mainMenuRow = page.locator('tbody tr').filter({ hasText: 'MainMenu' })
  const toolbarRow = page.locator('tbody tr').filter({ hasText: 'Toolbar · button' })
  const pluralRow = page.locator('tbody tr').filter({ hasText: 'Counter · n=2' })
  await expect(mainMenuRow).toBeVisible()
  await expect(toolbarRow).toBeVisible()
  await expect(pluralRow.getByRole('textbox')).toHaveAttribute('readonly', '')

  const toolbarTranslation = toolbarRow.getByRole('textbox', { name: '“Open”的译文' })
  await toolbarTranslation.fill('打开工具栏更新')
  await toolbarTranslation.blur()
  await expect.poll(() => page.evaluate(() => (window as any).__contextEditRequests?.[0])).toEqual({
    runId: 'workflow-context', source: 'Open', translation: '打开工具栏更新',
    translationContext: { context: 'Toolbar', disambiguation: 'button', pluralN: null },
  })
  await expect(mainMenuRow.getByRole('textbox', { name: '“Open”的译文' })).toHaveValue('打开工具栏更新')

  await mainMenuRow.getByRole('checkbox').click()
  await page.getByRole('button', { name: '从字典移除' }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__contextEditRequests?.[1])).toEqual({
    runId: 'workflow-context', source: 'Open', translation: '',
    translationContext: { context: 'MainMenu', disambiguation: null, pluralN: null },
  })
})

for (const scenario of [
  {
    title: 'elevated workflow text reconnect explains the protected target',
    code: 'runtime.target_access_failed',
    args: { operation: 'remoteMemory', controllerElevated: true },
    expected: 'Glyphshift 已有管理员权限，但这个软件仍拒绝连接。请尝试其他适配器；受保护的软件可能不支持实时替换。',
  },
  {
    title: 'workflow text reconnect explains a desktop and Runtime Bundle build mismatch',
    code: 'runtime.bundle_incompatible',
    args: {},
    expected: 'Glyphshift 的运行组件版本不一致',
  },
  {
    title: 'workflow text reconnect explains that an enabled workflow owns the target',
    code: 'capture.target_in_use_by_workflow',
    args: {},
    expected: '这个软件当前正由已启用的工作流使用；请先停用该工作流，再连接工作流。',
  },
  {
    title: 'workflow text reconnect gives restart-first recovery when no selected component activates',
    code: 'runtime.component_incompatible',
    args: {},
    expected: '请完整退出并重新启动目标软件后重试',
  },
]) {
  test(scenario.title, async ({ page }) => {
    await page.addInitScript(({ snapshot, errorCode, errorArgs }) => {
      const summary = {
        id: 'workflow-reconnect', workflowId: 'workflow-proof', name: '默认创作工作流',
        softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.ext-text-out'],
        status: 'ready', livePreviewEnabled: true, observationRevision: 0, observedCount: 0,
        ignoredCount: 0, droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2,
        dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: null,
      }
      ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string) => {
        if (command === 'desktop_settings') return {
          settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1,
          localePreference: 'zh-CN', themePreference: 'dark',
        }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_workflow_collection' || command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') return {
          observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [],
        }
        if (command === 'desktop_enable_workflow') {
          throw { schemaVersion: 1, code: errorCode, args: errorArgs }
        }
        return null
      } }
    }, { snapshot: collectionModel(), errorCode: scenario.code, errorArgs: scenario.args })

    await openWorkflowText(page)
    await page.getByRole('button', { name: '开始运行' }).click()
    await expect(page.getByRole('alert')).toContainText(scenario.expected)
  })
}

test('workflow text import confirms overwrite by default and sends the selected conflict strategy', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const run = {
      id: 'workflow-transfer', workflowId: 'workflow-proof', name: '默认创作工作流',
      softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'],
      status: 'ready', livePreviewEnabled: false, observationRevision: 0, observedCount: 0,
      ignoredCount: 0, droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
      dictionaryRevision: 1, dictionaryEntryCount: 0,
    }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_settings') return {
        settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1,
        localePreference: 'zh-CN', themePreference: 'dark',
      }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_probe_runs') return [run]
      if (command === 'desktop_workflow_collection' || command === 'desktop_probe_run_summary') return run
      if (command === 'desktop_probe_run_entries') return {
        observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [],
      }
      if (command === 'plugin:dialog|open') return 'X:/SyntheticFixtures/entries.json'
      if (command === 'desktop_import_probe_entries') {
        ;(window as any).__importRequest = args.request
        return run
      }
      return null
    } }
  }, { snapshot: collectionModel() })

  await openWorkflowText(page)
  await openTaskActions(page)
  await page.getByRole('menuitem', { name: '导入 JSON', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '导入词条' })
  await expect(dialog.getByText('同原文覆盖（默认）', { exact: true }).first()).toBeVisible()
  await dialog.getByRole('combobox', { name: '同原文处理方式' }).click()
  await page.getByRole('option', { name: '保留已有词条' }).click()
  await dialog.getByRole('button', { name: '导入', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__importRequest)).toEqual({
    runId: 'workflow-transfer', inputPath: 'X:/SyntheticFixtures/entries.json', format: 'json', mode: 'keep_existing',
  })
  await expect(dialog).toHaveCount(0)
})
