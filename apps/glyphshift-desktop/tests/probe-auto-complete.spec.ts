import { expect, test, type Page } from '@playwright/test'
import { model } from './fixtures/productModel'

async function setup(page: Page, interval = 5) {
  await page.clock.install()
  await page.addInitScript(({ snapshot, interval }) => {
    const state = { observedCount: 1, starts: 0, plans: 0, startedScopes: [] as string[], plannedCounts: [] as number[], candidates: true, holdPlan: false, release: null as any, task: null as any, status: 'running', phase: 'running', enabled: true, cancelled: [] as string[] }
    ;(window as any).__auto = state
    const profile = { id: 'profile.auto', name: '自动测试', protocol: 'ollama_chat', baseUrl: 'http://127.0.0.1:11434/api', modelId: 'synthetic', reasoningEffort: 'disabled', timeoutMs: 60000, maxItemsPerRequest: 50, maxConcurrency: 1, maxRetries: 0, filterPolicy: {}, hasCredential: false, credentialRequired: false }
    const run = { id: 'probe-auto', workflowId: 'workflow-proof', name: '自动探针', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'], status: 'running', livePreviewEnabled: false, observationRevision: 1, observedCount: 1, ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: null, excludedDictionaryIds: [], exclusionRevisions: {} }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', autoCompleteIntervalSeconds: interval }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_probe_runs') return [{ ...run, status: state.status }]
      if (command === 'desktop_probe_run_summary' || command === 'desktop_workflow_collection') return { ...run, status: state.status, observedCount: state.observedCount, workflowRuntime: { workflowId: run.workflowId, targets: [], errors: {}, revision: state.observedCount, lifecycle: { phase: state.phase, enabled: state.enabled, collectNewSources: state.status !== 'paused', checkedAtMs: 1, revision: state.observedCount } } }
      if (command === 'desktop_ai_profiles') return { defaultProfileId: profile.id, profiles: [profile] }
      if (command === 'desktop_ai_translation_tasks') return { current: structuredClone(state.task), history: [] }
      if (command === 'desktop_probe_run_entries') { (state as any).lastEntryRequest = args; return { observationRevision: 1, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] } }
      if (command === 'desktop_plan_probe_ai_translation') {
        state.plans++
        state.plannedCounts.push(state.observedCount)
        if (state.holdPlan) await new Promise(resolve => { state.release = resolve })
        return { token: args.request.runId, scopeId: `probe:${args.request.runId}`, snapshotRevision: 1, sourceLocale: 'en-US', targetLocale: 'zh-CN', candidates: state.candidates ? [{ itemId: 'one', source: 'Open', protectedTokens: [] }] : [], skipped: [] }
      }
      if (command === 'desktop_start_ai_translation') {
        state.starts++
        state.startedScopes.push(`probe:${args.request.planToken}`)
        state.task = { jobId: `job-${state.starts}`, planToken: args.request.planToken, scopeId: `probe:${args.request.planToken}`, snapshotRevision: 1, profileName: profile.name, modelId: profile.modelId, protocol: profile.protocol, status: 'running', totalCount: 1, completedCount: 0, failedCount: 0, totalBatches: 1, finishedBatches: 0, failedBatches: 0, batchSize: 1, maxConcurrency: 1, maxRetries: 0, elapsedMs: 0, peakConcurrency: 1, usage: null, batches: [], results: [], errors: [], targetDictionaryId: 'dictionary-proof', origin: 'probe', appliedCount: 0, skippedCount: 0, writebackError: null, dictionaryLocked: true }
        return structuredClone(state.task)
      }
      if (command === 'desktop_cancel_ai_translation') { state.cancelled.push(args.jobId); return null }
      return null
    } }
    Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
  }, { snapshot: model, interval })
  await page.goto('/')
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
}
async function toggle(page: Page) {
  await page.getByRole('checkbox', { name: '自动补全', exact: true }).click()

}

test('probe auto fill survives navigation, skips busy and empty cycles, and stops on failure', async ({ page }) => {
  await setup(page)
  await toggle(page)
  await page.clock.runFor(1200)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await expect(page.getByRole('dialog', { name: '确认 AI 翻译' })).toHaveCount(0)
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.clock.runFor(12000)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.evaluate(() => { const s = (window as any).__auto; s.task.status = 'completed'; s.task.dictionaryLocked = false; s.candidates = false })
  await page.clock.runFor(6000)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.evaluate(() => { const s = (window as any).__auto; s.candidates = true; s.observedCount++ })
  await page.clock.runFor(6000)
  await expect.poll(async () => { await page.clock.runFor(1000); return page.evaluate(() => (window as any).__auto.starts) }).toBe(2)
  await page.evaluate(() => { (window as any).__auto.task.status = 'completed_with_failures' })
  await page.clock.runFor(12000)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(2)
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: '自动补全', exact: true })).toHaveAttribute('aria-checked', 'false')
})

test('unchecking auto fill while planning prevents a late request', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => { (window as any).__auto.holdPlan = true })
  await toggle(page)
  await page.clock.runFor(1200)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.plans)).toBe(1)
  await toggle(page)
  await page.evaluate(() => (window as any).__auto.release())
  await page.clock.runFor(6000)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(0)
})


test('natural target exit keeps automatic selection and restart of app does not re-enable it', async ({ page }) => {
  await setup(page)
  await toggle(page)
  await expect(page.getByRole('checkbox', { name: '自动补全', exact: true })).toHaveAttribute('aria-checked', 'true')
  if (process.env.GLYPHSHIFT_AUTO_MENU_SCREENSHOT) await page.screenshot({ path: process.env.GLYPHSHIFT_AUTO_MENU_SCREENSHOT })
  await page.evaluate(() => { (window as any).__auto.phase = 'waiting' })
  await page.clock.runFor(1500)
  await expect(page.getByRole('checkbox', { name: '自动补全', exact: true })).toHaveAttribute('aria-checked', 'true')
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(0)
  await page.reload()
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: '自动补全', exact: true })).toHaveAttribute('aria-checked', 'false')
})


test('zero interval starts on new text without a five second delay or repeated empty plans', async ({ page }) => {
  await setup(page, 0)
  await page.evaluate(() => { (window as any).__auto.candidates = false })
  await toggle(page)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.plans)).toBe(1)
  await page.clock.runFor(1500)
  expect(await page.evaluate(() => (window as any).__auto.plans)).toBe(1)
  await page.evaluate(() => { const s = (window as any).__auto; s.candidates = true; s.observedCount++ })
  await page.clock.runFor(300)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.clock.runFor(1500)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await toggle(page)
  await page.clock.runFor(1500)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(1)
})

for (const interval of [1, 5, 60]) {
  test(`interval ${interval} skips identical empty plans and resumes for new content`, async ({ page }) => {
    await setup(page, interval)
    await page.evaluate(() => { (window as any).__auto.candidates = false })
    await toggle(page)
    await page.clock.runFor(1200)
    await expect.poll(() => page.evaluate(() => (window as any).__auto.plans)).toBe(1)
    await page.clock.runFor(interval * 3000 + 1500)
    expect(await page.evaluate(() => (window as any).__auto.plans)).toBe(1)
    await page.evaluate(() => { const s = (window as any).__auto; s.candidates = true; s.observedCount++ })
    await page.clock.runFor(interval * 1000 + 1500)
    await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  })
}


test('busy auto fill coalesces new text, gives the next workflow a turn, and removes cancelled waiting work', async ({ page }) => {
  await setup(page, 0)
  await toggle(page)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.evaluate(async () => {
    const path = '/src/useProbeAutoComplete.ts'
    const { useProbeAutoComplete } = await import(path)
    useProbeAutoComplete().start('probe-second', 'profile.auto')
  })
  for (let count = 2; count <= 6; count++) {
    await page.evaluate(value => { (window as any).__auto.observedCount = value }, count)
    await page.clock.runFor(300)
  }
  expect(await page.evaluate(() => (window as any).__auto.plans)).toBe(1)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.evaluate(() => { const state = (window as any).__auto; state.task.status = 'completed'; state.task.dictionaryLocked = false })
  await page.clock.runFor(600)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.startedScopes)).toEqual(['probe:probe-auto', 'probe:probe-second'])
  expect(await page.evaluate(() => (window as any).__auto.plannedCounts)).toEqual([1, 6])
  await toggle(page)
  await page.evaluate(() => { const state = (window as any).__auto; state.candidates = false; state.task.status = 'completed'; state.task.dictionaryLocked = false })
  await page.clock.runFor(1500)
  expect(await page.evaluate(() => (window as any).__auto.starts)).toBe(2)
  await page.evaluate(async () => {
    const path = '/src/useProbeAutoComplete.ts'
    const { useProbeAutoComplete } = await import(path)
    useProbeAutoComplete().stop('probe-second')
  })
})


test('workflow status menu omits obsolete skipped controls', async ({ page }) => {
  await setup(page)
  await expect(page.getByRole('checkbox', { name: '隐藏跳过的条目', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '按翻译状态筛选', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: '已跳过', exact: true })).toHaveCount(0)
  await page.getByRole('menuitem', { name: '规则匹配', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__auto.lastEntryRequest.request.translationFilter)).toBe('rule_matched')
  expect(await page.evaluate(() => 'hideSkipped' in (window as any).__auto.lastEntryRequest)).toBe(false)
})

test('changing filters during a slow page read sends only the latest follow-up', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => {
    const bridge = (window as any).__TAURI_INTERNALS__
    const original = bridge.invoke
    const state = { calls: 0, release: null as any, requests: [] as any[] }
    ;(window as any).__slowPage = state
    bridge.invoke = async (command: string, args: any) => {
      if (command === 'desktop_probe_run_entries') {
        state.calls++
        state.requests.push(structuredClone(args))
        if (state.calls === 1) await new Promise(resolve => { state.release = resolve })
      }
      return original(command, args)
    }
  })
  const search = page.getByPlaceholder('搜索原文、译文或适配器')
  await search.fill('Open')
  await page.clock.runFor(300)
  await expect.poll(() => page.evaluate(() => (window as any).__slowPage.calls)).toBe(1)
  await search.fill('Save')
  await page.clock.runFor(300)
  await page.getByRole('button', { name: '按翻译状态筛选', exact: true }).click()
  await page.getByRole('menuitem', { name: '规则匹配', exact: true }).click()
  expect(await page.evaluate(() => (window as any).__slowPage.calls)).toBe(1)
  await page.evaluate(() => (window as any).__slowPage.release())
  await expect.poll(() => page.evaluate(() => (window as any).__slowPage.calls)).toBe(2)
  expect(await page.evaluate(() => (window as any).__slowPage.requests[1].request.search)).toBe('Save')
  expect(await page.evaluate(() => (window as any).__slowPage.requests[1].request.translationFilter)).toBe('rule_matched')
})


test('natural exit finishes the owned AI round; explicit stop cancels only the owned job', async ({ page }) => {
  await setup(page, 0)
  await toggle(page)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
  await page.evaluate(() => { (window as any).__auto.phase = 'waiting' })
  await page.clock.runFor(1500)
  expect(await page.evaluate(() => (window as any).__auto.cancelled)).toEqual([])
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:workflow-stop', { detail: 'workflow-unrelated' })))
  expect(await page.evaluate(() => (window as any).__auto.cancelled)).toEqual([])
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:workflow-stop', { detail: 'workflow-proof' })))
  await expect.poll(() => page.evaluate(() => (window as any).__auto.cancelled)).toEqual(['job-1'])
})

test('collection disabled does not stop AI translation while workflow is running', async ({ page }) => {
  await setup(page, 0)
  await page.evaluate(() => { (window as any).__auto.status = 'paused' })
  await toggle(page)
  await expect.poll(() => page.evaluate(() => (window as any).__auto.starts)).toBe(1)
})
