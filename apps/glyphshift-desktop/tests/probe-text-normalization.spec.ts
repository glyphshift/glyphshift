import { test, expect } from '@playwright/test'
import { model } from './fixtures/productModel'

test('canonical workflow text row resolves an existing translation without deleting legacy entries', async ({ page }, testInfo) => {
  const workflowSnapshot = structuredClone(model)
  Object.assign(workflowSnapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
  await page.addInitScript(({ snapshot }) => {
    const source = 'Strong flexible material for tools.'
    const variants = [
      { source: 'Strong flexible \r\nmaterial for tools.', translation: '结实的材料' },
      { source: 'Strong \r\nflexible material for tools.', translation: '更坚韧的材料' },
    ]
    const run = { id: 'probe-wrap', workflowId: 'workflow-proof', name: '默认创作工作流', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: true,
      observationRevision: 1, observedCount: 1, ignoredCount: 0, droppedObservations: 0, previewGeneration: 1,
      createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 2, runtimeCapability: null, quickProbe: false }
    let row = { source, translation: '', state: 'pending', adapterIds: ['synthetic.text-out'], count: 7, firstSeenMs: 1, lastSeenMs: 2, translationVariants: variants }
    const app = window as any
    app.__legacyEntries = variants
    app.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_probe_runs') return [run]
      if (command === 'desktop_probe_run_summary' || command === 'desktop_workflow_collection') return run
      if (command === 'desktop_probe_run_entries') return { observationRevision: 1, dictionaryRevision: run.dictionaryRevision, page: 1, pageSize: 50, total: 1, rows: [row] }
      if (command === 'desktop_edit_probe_translation') {
        app.__resolvedEntry = args.request
        row = { ...row, translation: args.request.translation, state: 'translated', translationVariants: [] }
        run.dictionaryRevision++; run.dictionaryEntryCount++
        return run
      }
      return null
    } }
  }, { snapshot: workflowSnapshot })
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/')
  await page.getByRole('button', { name: '查看文字', exact: true }).click()
  await expect(page.getByText('多个已有译文', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '选择已有译文（2）' }).click()
  await expect(page.getByRole('menuitem', { name: '更坚韧的材料', exact: true })).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('conflict-desktop.png'), animations: 'disabled' })
  await page.keyboard.press('Escape')
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '选择已有译文（2）' }).click()
  await page.screenshot({ path: testInfo.outputPath('conflict-compact.png'), animations: 'disabled' })
  await page.getByRole('menuitem', { name: '更坚韧的材料', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__resolvedEntry)).toEqual({ runId: 'probe-wrap', source: 'Strong flexible material for tools.', translation: '更坚韧的材料', translationContext: null })
  await expect(page.getByText('多个已有译文', { exact: true })).toHaveCount(0)
  await expect.poll(() => page.evaluate(() => (window as any).__legacyEntries.length)).toBe(2)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})
