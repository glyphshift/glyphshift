import { test, expect, type Page } from '@playwright/test'
import { model } from './fixtures/productModel'

async function setup(page: Page) {
  await page.addInitScript(snapshot => {
    const state = { failRefresh: false, phase: 'running', warning: false, revision: 1, refreshes: 0, commands: [] as string[], hold: false, release: null as any, collect: true }
    ;(window as any).__lifecycle = state
    Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
    const runtime = () => ({ workflowId: 'workflow-proof', revision: state.revision,
      lifecycle: { phase: state.phase, enabled: !['stopped', 'stop_failed'].includes(state.phase), collectNewSources: state.collect, revision: state.revision, checkedAtMs: 1 },
      targets: [{ softwareId: 'software-proof', active: ['running', 'stop_failed'].includes(state.phase), discovered: state.phase === 'running', translationActive: state.phase === 'running' }],
      errors: state.phase === 'stop_failed' ? { 'software-proof': { schemaVersion: 1, code: 'runtime.stop_unconfirmed', args: {} } } : {},
      warnings: state.warning ? { 'software-proof': { schemaVersion: 1, code: 'runtime.no_compatibility_signal', args: {} } } : {},
    })
    const run = () => ({ id: 'record-one', workflowId: 'workflow-proof', workflowRuntime: runtime(), name: 'Synthetic collection', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'], status: 'running', livePreviewEnabled: true, observationRevision: 1, observedCount: 3, ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 3, excludedDictionaryIds: [], exclusionRevisions: [] })
    const current = () => structuredClone({ ...snapshot, activations: runtime().lifecycle.enabled ? [{ workflowId: 'workflow-proof', revision: 1 }] : [], workflowRuntimeStatus: { 'workflow-proof': runtime() } })
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 36 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_snapshot') return current()
      if (command === 'desktop_refresh_workflows') { if (state.failRefresh) throw new Error('Synthetic status failure'); state.refreshes++; state.revision++; return current() }
      if (command === 'desktop_probe_runs') return [run()]
      if (command === 'desktop_probe_run_summary' || command === 'desktop_workflow_collection') return run()
      if (command === 'desktop_probe_run_entries') return { rows: [], page: 1, pageSize: 50, total: 0, observationRevision: 1, dictionaryRevision: 1 }
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_enable_workflow' || command === 'desktop_disable_workflow') {
        state.commands.push(command)
        if (state.hold) await new Promise(resolve => { state.release = resolve })
        state.phase = command === 'desktop_enable_workflow' ? 'waiting' : 'stopped'
        state.revision++
        return { definition: snapshot.workflows[0], activation: { workflowId: 'workflow-proof', enabled: runtime().lifecycle.enabled }, runtime: runtime() }
      }
      if (command === 'desktop_set_workflow_collection') { state.collect = args.enabled; state.revision++; return current() }
      return null
    } }
  }, structuredClone(model))
  await page.goto('/')
  await expect(page.locator('#probe-activity-status')).toHaveText('运行中 1')
}

async function refreshManually(page: Page) {
  await page.getByRole('button', { name: '刷新状态', exact: true }).first().click()
}

async function makeBackgroundRefreshStale(page: Page) {
  await page.evaluate(async () => {
    const path = '/src/workspace/state.ts'
    const state = await import(path)
    state.lastWorkflowRefreshAt.value = Date.now() - 10_000
  })
}

test('workflow exit refresh replaces stale capture status across header, list and detail', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => { (window as any).__lifecycle.phase = 'waiting' })
  await refreshManually(page)
  await expect(page.locator('#probe-activity-status')).toHaveText('等待软件启动 1')
  await expect(page.getByRole('columnheader', { name: '启用', exact: true })).toHaveCount(0)
  await expect(page.getByRole('cell', { name: '等待软件启动', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByText('等待软件启动', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '停止运行', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '暂停收集', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '任务操作', exact: true }).click()
  await page.getByRole('menuitemcheckbox', { name: '收集新原文' }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__lifecycle.collect)).toBe(false)
})

test('running workflow surfaces a 5 second compatibility warning and clears it after text is observed', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => { (window as any).__lifecycle.warning = true })
  await refreshManually(page)

  await expect(page.locator('#probe-activity-status')).toHaveText('运行中 1')
  const status = page.getByRole('button', { name: '未检测到界面文字', exact: true })
  await expect(status).toBeVisible()
  await status.click()
  await expect(page.getByTestId('workflow-runtime-issues')).toContainText('已检测 5 秒')
  await expect(page.getByTestId('workflow-runtime-issues')).toContainText('当前界面可能暂不受支持')

  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByTestId('workflow-compatibility-warning')).toContainText('已检测 5 秒')

  await page.evaluate(() => { (window as any).__lifecycle.warning = false })
  await makeBackgroundRefreshStale(page)
  await page.evaluate(() => window.dispatchEvent(new Event('focus')))
  await expect(page.getByTestId('workflow-compatibility-warning')).toHaveCount(0)
  await expect(page.getByText('运行中', { exact: true })).toBeVisible()
})

test('stop failure remains actionable after disabling and stale snapshots cannot revive a stopped workflow', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => { const state = (window as any).__lifecycle; state.phase = 'stop_failed' })
  await refreshManually(page)
  await expect(page.locator('#probe-activity-status')).toHaveText('停止失败 1')
  await page.getByRole('button', { name: '停止运行', exact: true }).click()
  await expect(page.locator('#probe-activity-status')).toHaveCount(0)
  await page.evaluate(async () => {
    const path = '/src/workspace/state.ts'; const state = await import(path)
    const old = JSON.parse(JSON.stringify(state.model.value))
    old.workflowRuntimeStatus['workflow-proof'].revision = 0
    old.workflowRuntimeStatus['workflow-proof'].lifecycle.phase = 'running'
    old.workflowRuntimeStatus['workflow-proof'].lifecycle.enabled = true
    state.applyDesktopSnapshot(old)
  })
  await expect(page.locator('#probe-activity-status')).toHaveCount(0)
  await expect(page.getByRole('button', { name: '开始运行', exact: true })).toBeVisible()
})

test('start arriving during stop is serialized and multiple workflows count independently', async ({ page }) => {
  await setup(page)
  await page.evaluate(async () => {
    const path = '/src/workspace/workflows.ts'; const api = (await import(path)).useWorkflowWorkspace()
    const state = (window as any).__lifecycle; state.hold = true
    ;(window as any).__stop = api.setWorkflowEnabled('workflow-proof', false)
    ;(window as any).__start = api.setWorkflowEnabled('workflow-proof', true)
  })
  await expect(page.locator('#probe-activity-status')).toHaveText('正在停止 1')
  expect(await page.evaluate(() => (window as any).__lifecycle.commands.length)).toBe(1)
  await page.evaluate(async () => { const state = (window as any).__lifecycle; state.hold = false; state.release(); await (window as any).__start })
  await expect(page.locator('#probe-activity-status')).toHaveText('等待软件启动 1')
  await page.evaluate(async () => {
    const path = '/src/workspace/state.ts'; const state = await import(path)
    state.model.value.workflows.push({ ...state.model.value.workflows[0], id: 'workflow-second' })
    state.applyWorkflowRuntime({ workflowId: 'workflow-second', targets: [], errors: {}, revision: 100, lifecycle: { phase: 'running', enabled: true, collectNewSources: true, checkedAtMs: 1, revision: 100 } })
  })
  await expect(page.locator('#probe-activity-status')).toContainText('等待软件启动 1')
  await expect(page.locator('#probe-activity-status')).toContainText('运行中 1')
  await page.screenshot({ path: test.info().outputPath('workflow-lifecycle.png') })
})


test('failed status checks show unconfirmed and recover on the next focus refresh', async ({ page }) => {
  await setup(page)
  await makeBackgroundRefreshStale(page)
  await page.evaluate(() => { (window as any).__lifecycle.failRefresh = true; window.dispatchEvent(new Event('focus')) })
  await expect(page.locator('#probe-activity-status')).toHaveText('状态待确认 1')
  await makeBackgroundRefreshStale(page)
  await page.evaluate(() => { (window as any).__lifecycle.failRefresh = false; window.dispatchEvent(new Event('focus')) })
  await expect(page.locator('#probe-activity-status')).toHaveText('运行中 1')
})

test('current workflow navigation stays inert while manual refresh remains forced', async ({ page }) => {
  await setup(page)
  const before = await page.evaluate(() => (window as any).__lifecycle.refreshes)

  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.waitForTimeout(200)
  expect(await page.evaluate(() => (window as any).__lifecycle.refreshes)).toBe(before)

  await refreshManually(page)
  await expect.poll(() => page.evaluate(() => (window as any).__lifecycle.refreshes)).toBe(before + 1)
})
