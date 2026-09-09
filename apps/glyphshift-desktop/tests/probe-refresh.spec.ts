import { expect, test } from '@playwright/test'
import { model } from './fixtures/productModel'

for (const scenario of ['running', 'paused', 'disconnected', 'read-only', 'failure'] as const) {
  test(`target refresh: ${scenario}`, async ({ page }) => {
    await page.addInitScript(({ snapshot, scenario }) => {
      const run = {
        id: 'probe-refresh', workflowId: 'workflow-proof', name: '刷新验证', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
        adapterIds: ['synthetic.text-out'], status: scenario === 'paused' ? 'paused' : scenario === 'disconnected' ? 'disconnected' : 'running',
        livePreviewEnabled: scenario !== 'read-only', observationRevision: 1, observedCount: 1,
        ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 1,
        dictionaryRevision: 1, dictionaryEntryCount: 1, runtimeCapability: 'direct_replace', quickProbe: false,
      }
      const calls: string[] = []
      Object.assign(window, { __refreshCalls: calls })
      Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
      let translation = ''
      Object.assign(window, { __TAURI_INTERNALS__: {
        invoke: async (command: string, args?: Record<string, any>) => {
          if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
          if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
          if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
          if (command === 'desktop_probe_runs') return [run]
          if (command === 'desktop_probe_run_summary' || command === 'desktop_workflow_collection') return run
          if (command === 'desktop_probe_run_entries') return {
            observationRevision: 1, dictionaryRevision: 1, page: 1, pageSize: 50, total: 1,
            rows: [{ source: 'Synthetic dialogue', translation, state: translation ? 'translated' : 'pending', adapterIds: run.adapterIds, count: 1, lastSeenMs: 1, translationVariants: [] }],
          }
          if (command === 'desktop_edit_probe_translation') {
            await new Promise(resolve => setTimeout(resolve, 150))
            translation = args?.request.translation
            calls.push('save')
            return run
          }
          if (command === 'desktop_refresh_probe_text') {
            calls.push(`refresh:${args?.runId}:${translation}`)
            if (scenario === 'failure') throw { schemaVersion: 1, code: 'capture.preview_publish_failed', args: {} }
            return { ...run, previewGeneration: ++run.previewGeneration }
          }
          if (command === 'desktop_disconnect_probe_run' || command === 'desktop_resume_probe_run') calls.push('unexpected-reconnect')
          return null
        },
      } })
    }, { snapshot: model, scenario })
    await page.setViewportSize({ width: 960, height: 720 })
    await page.goto('/')
    await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
    const button = page.getByTestId('probe-refresh-text')
    await expect(button).toBeVisible()
    await expect(button).toHaveAttribute('title', '让软件重新显示译文。如果没变化，试试重新打开界面，或进入下一句对话。')
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    if (scenario === 'disconnected' || scenario === 'read-only') {
      await expect(button).toBeDisabled()
      return
    }
    await page.getByRole('textbox', { name: /Synthetic dialogue/ }).fill('新的译文')
    await button.click()
    await expect.poll(() => page.evaluate(() => (window as any).__refreshCalls)).toEqual(['save', 'refresh:probe-refresh:新的译文'])
    if (scenario === 'failure') {
      await expect(page.getByRole('alert')).toContainText('实时更新没有发送到目标软件')
      await expect(page.getByText('已请求刷新目标文字。', { exact: false })).toHaveCount(0)
    }
    else {
      await expect(page.getByText('已请求刷新目标文字。', { exact: false })).toHaveCount(0)
      if (scenario === 'running') await page.screenshot({ path: '../../local-test/evidence/desktop-screens/probe-refresh.png' })
    }
  })
}
