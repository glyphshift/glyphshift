import { expect, test, type Page } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

async function openProbeTaskActions(page: Page) {
  await page.getByTestId('probe-task-actions').click()
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('probe detail can launch its bound software', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const run = {
      id: 'probe-launch', name: '启动目标探针', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'disconnected', livePreviewEnabled: false,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: null, quickProbe: false,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, unknown>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [run]
        if (command === 'desktop_probe_run_entries') return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        if (command === 'desktop_probe_run_summary') return run
        if (command === 'desktop_launch_software') {
          if ((window as unknown as { __launchedSoftwareId?: unknown }).__launchedSoftwareId) {
            throw { schemaVersion: 1, code: 'software.executable_missing', args: {} }
          }
          ;(window as unknown as { __launchedSoftwareId?: unknown }).__launchedSoftwareId = args?.softwareId
          return null
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await page.setViewportSize({ width: 960, height: 640 })
  await page.goto('/')

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '启动目标探针' }).dblclick()
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.getByRole('button', { name: '运行软件', exact: true }).click()

  await expect.poll(() => page.evaluate(() => (window as unknown as { __launchedSoftwareId?: unknown }).__launchedSoftwareId)).toBe('software-proof')
  await page.getByRole('button', { name: '运行软件', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('绑定的程序文件已不存在，请在软件页重新选择程序位置。')
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/probe-launch-software-compact.png' })
})

test('empty libraries stay actionable and a running application can create a temporary probe', async ({ page }) => {
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return currentSnapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_arm_software_capture') return 'Ctrl+Shift+F8'
        if (command === 'desktop_running_software_targets') {
          return [{
            executablePath: 'X:\\SyntheticFixtures\\QuickTarget.exe',
            executableName: 'QuickTarget.exe',
            suggestedName: 'QuickTarget',
            architecture: 'x86_64',
            running: true,
            canAdd: true,
            state: 'ready',
            existingName: null,
          }]
        }
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
        if (command === 'desktop_create_probe_from_sources') {
          ;(window as unknown as { __probeCreateRequest?: unknown }).__probeCreateRequest = args?.request
          const summary = {
            id: 'quick-probe-ui', name: 'QuickTarget 探针', softwareId: 'software.quick-target',
            dictionaryId: 'quick-dictionary-ui', adapterIds: ['synthetic.ext-text-out'], status: 'running',
            livePreviewEnabled: false, observationRevision: 0, observedCount: 0, ignoredCount: 0,
            droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
            dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: true,
          }
          runs = [summary]
          currentSnapshot = {
            ...currentSnapshot,
            selectedSoftwareId: 'software.quick-target',
            software: [{ ...softwareTemplate, id: 'software.quick-target', name: 'QuickTarget', executableName: 'QuickTarget.exe', executablePath: args?.request.target.executablePath }],
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
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(dialog.getByRole('button', { name: '软件列表' })).toBeEnabled()
  await dialog.getByRole('button', { name: '软件列表' }).click()
  await expect(dialog.getByText('还没有添加软件。')).toBeVisible()
  await dialog.getByRole('button', { name: '运行中软件' }).click()
  await dialog.getByRole('combobox', { name: '运行中软件' }).click()
  await page.getByRole('option', { name: 'QuickTarget · QuickTarget.exe · x86_64' }).click()
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('已识别 QuickTarget')

  await expect(dialog.getByRole('button', { name: '使用已有词典' })).toBeEnabled()
  await dialog.getByRole('button', { name: '使用已有词典' }).click()
  await expect(dialog.getByText('还没有可用词典。')).toBeVisible()
  await dialog.getByRole('button', { name: '使用临时词典' }).click()
  await expect(dialog.getByRole('button', { name: '使用临时词典' })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByRole('textbox', { name: '任务名称' })).toBeVisible()
  await expect(dialog.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: '目标语言' })).toHaveValue('zh-CN')
  await dialog.getByRole('textbox', { name: '源语言' }).fill('ja-JP')
  await dialog.getByRole('textbox', { name: '目标语言' }).fill('ko-KR')
  await expect(dialog.getByRole('button', { name: '高级选项' })).toHaveCount(0)
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('x86_64')
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/quick-probe-ready-zh.png' })
  await dialog.getByRole('button', { name: '创建并连接' }).click()

  await expect(dialog).toHaveCount(0)
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeCreateRequest?: { dictionary?: unknown } }
  ).__probeCreateRequest?.dictionary)).toEqual({
    kind: 'temporary',
    sourceLocale: 'ja-JP',
    targetLocale: 'ko-KR',
  })
  const activeCaptureTab = page.getByRole('button', { name: '探针', exact: true })
  await expect(activeCaptureTab).toHaveAccessibleDescription('运行中')
  await expect(activeCaptureTab.getByText('运行中', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect(activeCaptureTab).toBeVisible()
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/probe-tab-running-zh.png' })
  await activeCaptureTab.click()
  await expect(page.getByRole('heading', { name: 'QuickTarget 探针', exact: true })).toBeVisible()
  await expect(page.getByTestId('probe-software-path')).toContainText('X:\\SyntheticFixtures\\QuickTarget.exe')
  await expect(page.getByTestId('probe-detail-actions').getByRole('button')).toHaveCount(4)
  await expect(page.getByTestId('probe-ai-actions')).toBeVisible()
  await expect(page.getByTestId('probe-temporary-task-badge')).toHaveText('临时')
  await expect(page.getByTestId('probe-bound-dictionary-name')).toHaveText('临时词典')
  await expect(page.getByTestId('probe-temporary-dictionary-badge')).toHaveText('临时')
  await openProbeTaskActions(page)
  await expect(page.getByRole('menuitem', { name: '保留为常规任务', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '结束并清理临时内容', exact: true })).toBeVisible()
  await page.getByRole('menuitem', { name: '保留为常规任务', exact: true }).click()
  await expect(page.getByTestId('probe-task-actions')).toHaveText('任务操作')
  await openProbeTaskActions(page)
  await expect(page.getByRole('menuitem', { name: '探针设置', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '保留为常规任务', exact: true })).toHaveCount(0)
})

test('current-app source can end and clean up at compact English layout', async ({ page }) => {
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_arm_software_capture') return 'Ctrl+Shift+F8'
        if (command === 'desktop_cancel_software_capture') return null
        if (command === 'desktop_create_probe_from_sources') {
          const summary = {
            id: 'quick-probe-captured', name: 'Captured Target probe', softwareId: 'software-proof',
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
  await page.getByRole('button', { name: 'New probe run' }).click()
  const dialog = page.getByRole('dialog', { name: 'New probe run' })
  await dialog.getByRole('button', { name: 'Running apps' }).click()
  await dialog.getByRole('button', { name: 'Use temporary dictionary' }).click()
  await expect(dialog.getByRole('textbox', { name: 'Source locale' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: 'Target locale' })).toHaveValue('zh-CN')
  await expect(dialog.getByText(/detected automatically/i)).toHaveCount(0)
  await dialog.getByRole('button', { name: 'Capture with shortcut' }).click()
  await expect(dialog.getByText('Waiting to capture the target')).toBeVisible()
  await page.evaluate(() => {
    const preflight = (window as unknown as { __quickProbePreflight: unknown }).__quickProbePreflight
    window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
      detail: { state: 'captured', shortcut: 'Ctrl+Shift+F8', preflight },
    }))
  })
  await expect(dialog.getByTestId('quick-probe-preflight')).toContainText('reuse “Captured Target”')
  await dialog.getByRole('button', { name: 'Create and connect' }).click()
  const compactHeading = page.getByRole('heading', { name: 'Captured Target probe', exact: true })
  await expect(compactHeading).toBeVisible()
  await expect.poll(() => compactHeading.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  const activeCaptureTab = page.getByRole('button', { name: 'Capture', exact: true })
  await expect(activeCaptureTab).toHaveAccessibleDescription('Running')
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/probe-tab-running-compact-en.png' })
  await expect(page.getByTestId('probe-detail-actions').getByRole('button')).toHaveCount(4)
  await expect(page.getByTestId('probe-bound-dictionary-name')).toHaveText('Temporary dictionary')
  await expect(page.getByText('quick-dictionary-captured', { exact: false })).toHaveCount(0)
  await openProbeTaskActions(page)
  await page.getByRole('menuitem', { name: 'End and clean up temporary content', exact: true }).click()
  const confirmation = page.getByRole('dialog', { name: 'End temporary probe' })
  await expect(confirmation).toContainText('Existing content and anything still used elsewhere are kept')
  await confirmation.getByRole('button', { name: 'End and clean up' }).click()

  await expect(confirmation).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Capture', exact: true })).toBeVisible()
  await expect(page.getByText('Running', { exact: true })).toHaveCount(0)
  await expect(page.getByText('No probe runs yet')).toBeVisible()
  await expect(page.getByRole('status')).toContainText('Existing software and dictionaries are unchanged')
  await expect.poll(() => page.evaluate(() => Boolean((window as unknown as { __quickProbeCleaned?: boolean }).__quickProbeCleaned))).toBe(true)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/quick-probe-compact-en.png' })
})

test('paused probe remains visible on the navigation tab after startup', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const pausedRun = {
      id: 'probe-paused', name: '暂停任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'paused', livePreviewEnabled: false,
      observationRevision: 1, observedCount: 4, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: false,
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [pausedRun]
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await page.reload()

  const captureTab = page.getByRole('button', { name: '探针', exact: true })
  await expect(captureTab).toHaveAccessibleDescription('已暂停')
  await expect(captureTab.getByText('已暂停', { exact: true })).toBeVisible()
})

test('current-app source keeps creation recoverable after a target stops during startup', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let attempts = 0
    let runs: any[] = []
    const summary = {
      id: 'quick-probe-retry', name: 'Vector Studio 探针', softwareId: 'software-proof',
      dictionaryId: 'quick-dictionary-retry', adapterIds: ['synthetic.ext-text-out'], status: 'running',
      livePreviewEnabled: false, observationRevision: 0, observedCount: 0, ignoredCount: 0,
      droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1,
      dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: 'direct_replace', quickProbe: true,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_arm_software_capture') return 'Ctrl+Shift+F8'
        if (command === 'desktop_preflight_software') {
          return {
            executablePath: args?.executablePath, executableName: 'VectorStudio.exe', suggestedName: 'Vector Studio',
            architecture: 'x86_64', running: true, canAdd: false, state: 'already_added', existingName: 'Vector Studio',
          }
        }
        if (command === 'desktop_create_probe_from_sources') {
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
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await dialog.getByRole('button', { name: '运行中软件' }).click()
  await dialog.getByRole('button', { name: '使用临时词典' }).click()
  await dialog.getByRole('button', { name: '使用快捷键捕获' }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
    detail: {
      state: 'captured', shortcut: 'Ctrl+Shift+F8', preflight: {
        executablePath: 'X:\\SyntheticFixtures\\VectorStudio.exe', executableName: 'VectorStudio.exe',
        suggestedName: 'Vector Studio', architecture: 'x86_64', running: true, canAdd: false,
        state: 'already_added', existingName: 'Vector Studio',
      },
    },
  })))
  await dialog.getByRole('button', { name: '创建并连接' }).click()

  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('alert')).toContainText('请确认它仍在运行，再重新创建探针')
  await expect(dialog.getByRole('button', { name: '创建并连接' })).toBeEnabled()
  await dialog.getByRole('button', { name: '创建并连接' }).click()
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针', exact: true })).toBeVisible()
})
test('probe run uses the shared searchable selectable paginated table flow', async ({ page }) => {
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await expect(page.getByRole('heading', { name: '探针', exact: true })).toBeVisible()
  await expect(page.getByText('还没有探针任务')).toBeVisible()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(dialog.getByRole('button', { name: '软件列表' })).toHaveAttribute('aria-pressed', 'true')
  await expect(dialog.getByRole('button', { name: '运行中软件' })).toHaveAttribute('aria-pressed', 'false')
  await expect(dialog.getByText('使用已有词典', { exact: true })).toBeVisible()
  await expect(dialog.getByText('使用临时词典', { exact: true })).toBeVisible()
  await expect(dialog.getByText('界面基础词典', { exact: true })).toBeVisible()
  await expect(dialog.getByText('探针技术', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('实时预览', { exact: true })).toHaveCount(0)
  await expect(page.getByText('gdi32.dll!TextOutW', { exact: true })).toHaveCount(0)
  await expect(page.getByText(/位置|语境/)).toHaveCount(0)

  await dialog.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await dialog.getByRole('button', { name: '创建并连接' }).click()
  await expect(dialog).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回探针管理' })).toBeVisible()
  await expect(page.getByPlaceholder('搜索原文、译文或兼容方式')).toBeVisible()
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
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
  await expect(page.getByText('兼容方式', { exact: true })).toBeVisible()
  await expect(page.getByText(/支持字距、裁剪和部分特殊文字.*传统 Windows 文字（高级）/)).toBeVisible()
  await expect(page.getByText(/适合传统自绘面板和图形界面.*Windows 自绘图形界面/)).toBeVisible()
})

test('library sources create a normal probe without temporary ownership', async ({ page }) => {
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
    quickProbe: false,
  }
  await page.addInitScript(({ snapshot, persisted }) => {
    let creationAttempted = false
    let createdRun = persisted
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return creationAttempted ? [createdRun] : []
        if (command === 'desktop_probe_run_summary') return createdRun
        if (command === 'desktop_create_probe_from_sources') {
          creationAttempted = true
          createdRun = {
            ...persisted,
            name: args?.request.name,
            softwareId: args?.request.target.softwareId,
            dictionaryId: args?.request.dictionary.dictionaryId,
            quickProbe: false,
          }
          return createdRun
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
  await expect(page.getByTestId('probe-task-actions')).toHaveText('任务操作')
  await openProbeTaskActions(page)
  await expect(page.getByRole('menuitem', { name: '探针设置', exact: true })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '保留为常规任务', exact: true })).toHaveCount(0)
})

test('new probe resets temporary source choices after cancellation', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()

  let dialog = page.getByRole('dialog', { name: '新建探针任务' })
  const existingDictionaryMode = dialog.getByRole('button', { name: '使用已有词典' })
  const temporaryDictionaryMode = dialog.getByRole('button', { name: '使用临时词典' })
  await expect(existingDictionaryMode).toHaveAttribute('aria-pressed', 'true')
  await expect(existingDictionaryMode).toHaveClass(/text-primary/)
  await expect(temporaryDictionaryMode).toHaveAttribute('aria-pressed', 'false')
  await temporaryDictionaryMode.click()
  await expect(temporaryDictionaryMode).toHaveAttribute('aria-pressed', 'true')
  await expect(temporaryDictionaryMode).toHaveClass(/text-primary/)
  await expect(existingDictionaryMode).toHaveAttribute('aria-pressed', 'false')
  await dialog.getByRole('textbox', { name: '任务名称' }).fill('不应保留的任务')
  await dialog.getByRole('textbox', { name: '源语言' }).fill('ja-JP')
  await dialog.getByRole('textbox', { name: '目标语言' }).fill('ko-KR')
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.getByRole('button', { name: '新建探针任务' }).click()
  dialog = page.getByRole('dialog', { name: '新建探针任务' })
  await expect(dialog.getByRole('button', { name: '使用已有词典' })).toHaveAttribute('aria-pressed', 'true')
  await dialog.getByRole('button', { name: '使用临时词典' }).click()
  await expect(dialog.getByRole('textbox', { name: '任务名称' })).toHaveValue('')
  await expect(dialog.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: '目标语言' })).toHaveValue('zh-CN')
  await expect(dialog.getByText('词典名称', { exact: true })).toHaveCount(0)
})

test('new dictionary discards a cancelled metadata draft', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '新建词典' }).click()

  let dialog = page.getByRole('dialog', { name: '新建词典' })
  await dialog.getByRole('textbox', { name: '名称' }).fill('不应保留的词典')
  await dialog.getByRole('textbox', { name: '源语言' }).fill('ja-JP')
  await dialog.getByRole('textbox', { name: '目标语言' }).fill('ko-KR')
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.getByRole('button', { name: '新建词典' }).click()
  dialog = page.getByRole('dialog', { name: '新建词典' })
  await expect(dialog.getByRole('textbox', { name: '名称' })).toHaveValue('')
  await expect(dialog.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(dialog.getByRole('textbox', { name: '目标语言' })).toHaveValue('zh-CN')
  await expect(dialog.getByRole('textbox', { name: '发布版本' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '作者' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '许可证' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '主页' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: '说明' })).toHaveValue('')
  await expect(dialog.getByRole('textbox', { name: '标签' })).toHaveValue('')
})

test('paused probe can clear its entries without releasing the runtime', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let cleared = false
    let summary = {
      id: 'probe-paused-clear', name: '暂停后刷新', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'paused', livePreviewEnabled: true,
      observationRevision: 4, observedCount: 1, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 2, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 3,
      dictionaryEntryCount: 2, runtimeCapability: 'direct_replace',
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_clear_probe_run_entries') {
          cleared = true
          ;(window as unknown as { __pausedProbeClearCount?: number }).__pausedProbeClearCount = 1
          summary = {
            ...summary,
            observationRevision: summary.observationRevision + 1,
            observedCount: 0,
            dictionaryRevision: summary.dictionaryRevision + 1,
            dictionaryEntryCount: 0,
          }
          return summary
        }
        if (command === 'desktop_set_probe_run_paused') {
          ;(window as unknown as { __pausedProbeResumeCount?: number }).__pausedProbeResumeCount = 1
          summary = { ...summary, status: 'running' }
          return summary
        }
        if (command === 'desktop_probe_run_entries') return {
          observationRevision: summary.observationRevision,
          dictionaryRevision: summary.dictionaryRevision,
          page: 1,
          pageSize: 50,
          total: cleared ? 0 : 1,
          rows: cleared ? [] : [{
            source: 'Open', translation: '打开', state: 'translated',
            adapterIds: ['synthetic.text-out'], count: 3, firstSeenMs: 1, lastSeenMs: 2,
          }],
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await openProbeTaskActions(page)
  await page.getByRole('menuitem', { name: '探针设置' }).click()
  const clearButton = page.getByTestId('capture-clear-all')
  await expect(clearButton).toBeEnabled()
  await expect(page.getByText('删除全部收集记录和当前词典中的全部词条，保留任务设置。')).toBeVisible()
  await expect(page.getByText('先暂停收集，避免采集线程在清空时写入新条目。')).toHaveCount(0)
  await clearButton.click()
  await page.getByRole('dialog', { name: '清空全部探针条目' }).getByRole('button', { name: '确认清空' }).click()

  await expect(page.getByText('还没有捕获到文字')).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __pausedProbeClearCount?: number }).__pausedProbeClearCount
  ))).toBe(1)
  await page.getByRole('button', { name: '继续收集' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __pausedProbeResumeCount?: number }).__pausedProbeResumeCount
  ))).toBe(1)
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_update_probe_run') {
          const request = structuredClone(args?.request)
          ;(window as unknown as { __probeSettingsRequest?: unknown }).__probeSettingsRequest = request
          summary = {
            ...summary,
            name: request.name,
            dictionaryId: request.dictionaryId,
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

  await openProbeTaskActions(page)
  await page.getByRole('menuitem', { name: '探针设置' }).click()
  const settings = page.getByRole('dialog', { name: '探针设置' })
  await settings.getByRole('textbox', { name: '任务名称' }).fill('整理后的探针')
  await settings.getByRole('button', { name: '保存设置' }).click()
  await expect(page.getByRole('heading', { name: '整理后的探针' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __probeSettingsRequest?: { adapterIds: string[]; livePreviewEnabled: boolean } }).__probeSettingsRequest
  ))).toMatchObject({
    dictionaryId: 'dictionary-proof',
    adapterIds: ['synthetic.text-out'],
    livePreviewEnabled: true,
  })

  await openProbeTaskActions(page)
  await page.getByRole('menuitem', { name: '探针设置' }).click()
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_set_probe_run_paused') {
          summary = { ...summary, status: args?.paused ? 'paused' : 'running' }
          return summary
        }
        if (command === 'desktop_disconnect_probe_run') {
          summary = { ...summary, status: 'ready' }
          ;(window as unknown as { __probeDisconnected?: boolean }).__probeDisconnected = true
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
        const request = args?.request as { search: string; adapterIds: string[]; translationFilter: 'all' | 'untranslated' | 'translated'; page: number; pageSize: number }
        if (command === 'desktop_probe_run_entries') {
          ;(window as unknown as { __captureQueryRequests?: unknown[] }).__captureQueryRequests ??= []
          ;(window as unknown as { __captureQueryRequests: unknown[] }).__captureQueryRequests.push(structuredClone(request))
          const start = (request.page - 1) * request.pageSize
          const filtered = request.adapterIds.includes('synthetic.draw-text')
          const sourceIndices = Array.from({ length: filtered ? 30 : 5000 }, (_, index) => index + 1)
            .filter((sourceIndex) => {
              const source = `Source ${String(sourceIndex).padStart(4, '0')}`
              const translation = translations[source] ?? (sourceIndex % 2 === 0 ? `译文 ${sourceIndex}` : '')
              if (request.translationFilter === 'untranslated') return !translation
              if (request.translationFilter === 'translated') return Boolean(translation)
              return true
            })
          const total = sourceIndices.length
          const pageIndices = sourceIndices.slice(start, start + request.pageSize)
          return {
            observationRevision: 12, dictionaryRevision: summary.dictionaryRevision,
            page: request.page, pageSize: request.pageSize, total,
            rows: pageIndices.map((sourceIndex) => {
              const source = `Source ${String(sourceIndex).padStart(4, '0')}`
              const translation = translations[source] ?? (sourceIndex % 2 === 0 ? `译文 ${sourceIndex}` : '')
              return {
                source, translation,
                state: translation ? 'translated' : 'pending',
                adapterIds: filtered ? ['synthetic.draw-text'] : ['synthetic.text-out'], count: sourceIndex,
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
  await expect(page.locator('tbody tr').first()).toContainText('未进词典')
  const firstTranslation = page.getByRole('textbox', { name: '“Source 0001”的译文' })
  await firstTranslation.fill('即时译文')
  await firstTranslation.blur()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureEditRequests?: Array<{ translation: string }> }).__captureEditRequests?.[0]?.translation
  ))).toBe('即时译文')
  await expect(page.getByText(/预览 G\d+/)).toHaveCount(0)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/capture-workspace-live-edit-scroll.png' })
  await tableScroller.evaluate(element => { element.scrollTop = 600 })
  await expect.poll(() => page.getByTestId('capture-scrollbar-thumb').evaluate(element => getComputedStyle(element).transform)).not.toBe(initialThumbTransform)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/capture-workspace-scrollbar-pagination.png' })
  await page.getByLabel('选择当前页').click()
  await expect(page.getByText('50 条目已选择')).toBeVisible()
  await expect(page.getByText(/技术目录|字典草稿/)).toHaveCount(0)

  const translationFilter = page.getByRole('button', { name: '按翻译状态筛选' })
  await expect(translationFilter).toContainText('全部条目')
  await translationFilter.click()
  await expect(page.getByRole('menuitem', { name: '已翻译', exact: true })).toBeVisible()
  await page.getByRole('menuitem', { name: '未翻译', exact: true }).click()
  await expect(translationFilter).toContainText('未翻译')
  await expect(page.getByText('显示 1–50，共 2499 条目')).toBeVisible()
  await expect(page.locator('tbody tr')).toHaveCount(50)
  await expect(page.locator('tbody tr').filter({ hasText: '未进词典' })).toHaveCount(50)
  await expect(page.locator('tbody tr').filter({ hasText: '已在词典' })).toHaveCount(0)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/capture-untranslated-filter.png' })
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ translationFilter: string }> }).__captureQueryRequests?.at(-1)?.translationFilter
  ))).toBe('untranslated')

  const adapterFilter = page.getByTestId('capture-adapter-filter')
  await expect(adapterFilter).toContainText('全部兼容方式')
  await adapterFilter.click()
  await page.getByRole('menuitemcheckbox', { name: 'Windows 按钮与标签' }).click()
  await expect(adapterFilter).toContainText('Windows 按钮与标签')
  await expect(page.getByText('显示 1–14，共 14 条目')).toBeVisible()
  await expect(page.locator('tbody tr')).toHaveCount(14)
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ adapterIds: string[] }> }).__captureQueryRequests?.at(-1)?.adapterIds
  ))).toEqual(['synthetic.draw-text'])

  await page.keyboard.press('Escape')
  const search = page.getByPlaceholder('搜索原文、译文或兼容方式')
  await search.fill('Source 00')
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ search: string }> }).__captureQueryRequests?.at(-1)?.search
  ))).toBe('Source 00')
  await page.getByRole('button', { name: '暂停收集' }).click()
  await expect(page.getByRole('button', { name: '继续收集' })).toBeVisible()
  const disconnect = page.getByRole('button', { name: '释放当前连接', exact: true })
  await expect(disconnect).toBeVisible()
  await openProbeTaskActions(page)
  await expect(page.getByRole('menuitem', { name: '释放当前连接', exact: true })).toHaveCount(0)
  await page.keyboard.press('Escape')
  const pausedTranslation = page.getByRole('textbox', { name: '“Source 0003”的译文' })
  await pausedTranslation.fill('暂停时译文')
  await pausedTranslation.blur()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureEditRequests?: Array<{ translation: string }> }).__captureEditRequests?.at(-1)?.translation
  ))).toBe('暂停时译文')
  await openProbeTaskActions(page)
  await expect(page.getByRole('menuitem', { name: '当前联合表 CSV', exact: true })).toBeVisible()
  await page.keyboard.press('Escape')
  await disconnect.click()
  await expect(page.getByRole('button', { name: '连接并继续' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __probeDisconnected?: boolean }).__probeDisconnected
  ))).toBe(true)

  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await expect(page.getByPlaceholder('搜索原文、译文或兼容方式')).toHaveValue('Source 00')
  await expect(page.getByRole('button', { name: '按翻译状态筛选' })).toContainText('未翻译')
  await expect(page.getByTestId('capture-adapter-filter')).toContainText('Windows 按钮与标签')
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __captureQueryRequests?: Array<{ adapterIds: string[] }> }).__captureQueryRequests?.at(-1)?.adapterIds
  ))).toEqual(['synthetic.draw-text'])
})

test('elevated probe rejection explains the protected target without observer recovery', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-reconnect', name: '连接诊断任务', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 3, observedCount: 433, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 28, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 6,
      dictionaryEntryCount: 6, runtimeCapability: null as string | null,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
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
            args: { operation: 'remoteMemory', controllerElevated: true },
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

  await expect(page.getByText('Glyphshift 已有管理员权限，但这个软件仍拒绝连接。请尝试其他兼容方式；受保护的软件可能不支持实时替换。')).toBeVisible()
  await expect(page.getByText(/开启“始终以管理员身份启动”/)).toHaveCount(0)
  await expect(page.getByRole('button', { name: '切换为仅采集并重试' })).toHaveCount(0)
})

test('probe reconnect explains a desktop and Runtime Bundle build mismatch', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-runtime-bundle-mismatch', name: 'Runtime 版本诊断', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: null,
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_resume_probe_run') {
          throw { schemaVersion: 1, code: 'runtime.bundle_incompatible', args: {} }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '连接并继续' }).click()

  await expect(page.getByRole('alert')).toContainText('Glyphshift 的运行组件版本不一致')
  await expect(page.getByRole('alert')).toContainText('只重启目标软件无法解决')
  await expect(page.getByRole('alert')).not.toContainText('选择的兼容方式当前不可用')
})

test('probe operation error closes when switching to another probe', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    let resumeAttempts = 0
    const makeRun = (id: string, name: string) => ({
      id, name, softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: null,
    })
    const runs = [makeRun('probe-a', '探针 A'), makeRun('probe-b', '探针 B')]
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return runs
        if (command === 'desktop_probe_run_summary') return runs.find(run => run.id === args?.runId)
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_compatible_probe_adapters') return []
        if (command === 'desktop_resume_probe_run' && args?.runId === 'probe-a') {
          resumeAttempts += 1
          if (resumeAttempts > 1) await new Promise(resolve => setTimeout(resolve, 100))
          throw {
            schemaVersion: 1,
            code: 'runtime.target_access_failed',
            args: { operation: 'remoteMemory', controllerElevated: true },
          }
        }
        return runs.find(run => run.id === args?.runId) ?? null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', 'probe-a')
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '连接并继续' }).click()
  await expect(page.getByRole('alert')).toContainText('这个软件仍拒绝连接')
  await page.getByRole('alert').getByRole('button', { name: '关闭提示' }).click()
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.getByRole('button', { name: '连接并继续' }).click()
  await expect(page.getByRole('alert')).toContainText('这个软件仍拒绝连接')

  await page.getByRole('button', { name: '返回探针管理' }).click()
  await page.getByRole('row').filter({ hasText: '探针 B' }).dblclick()

  await expect(page.getByRole('heading', { name: '探针 B', exact: true })).toBeVisible()
  await expect(page.getByRole('alert')).toHaveCount(0)

  await page.getByRole('button', { name: '返回探针管理' }).click()
  await page.getByRole('row').filter({ hasText: '探针 A' }).dblclick()
  await page.getByRole('button', { name: '连接并继续' }).click()
  await page.getByRole('button', { name: '返回探针管理' }).click()
  await page.getByRole('row').filter({ hasText: '探针 B' }).dblclick()

  await page.waitForTimeout(150)
  await expect(page.getByRole('heading', { name: '探针 B', exact: true })).toBeVisible()
  await expect(page.getByRole('alert')).toHaveCount(0)
})

test('probe reconnect explains that an enabled workflow owns the target', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-workflow-conflict', name: '工作流占用诊断', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 1,
      dictionaryEntryCount: 0,
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') {
          return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        }
        if (command === 'desktop_resume_probe_run') {
          throw { schemaVersion: 1, code: 'capture.target_in_use_by_workflow', args: {} }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '连接并继续' }).click()

  await expect(page.getByText('这个软件当前正由已启用的工作流使用；请先停用该工作流，再连接探针。')).toBeVisible()
  await expect(page.getByRole('button', { name: '连接并继续' })).toBeEnabled()
})

test('probe reconnect gives restart-first recovery when no selected component activates', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const summary = {
      id: 'probe-component-incompatible', name: '组件恢复诊断', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.ext-text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 0, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 1,
      dictionaryEntryCount: 0, runtimeCapability: null,
    }
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 32 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_probe_run_entries') return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
        if (command === 'desktop_resume_probe_run') throw { schemaVersion: 1, code: 'runtime.component_incompatible', args: {} }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: '连接并继续' }).click()

  const alert = page.getByRole('alert')
  await expect(alert).toContainText('请完整退出并重新启动目标软件后重试')
  await expect(alert).toContainText('当前界面暂不受支持')
  await expect(alert).not.toContainText('请在探针设置中重新选择')
})
