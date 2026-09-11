import { expect, test } from '@playwright/test'
import { model } from './fixtures/productModel'

test('collection displays translation provenance and keeps automatic dictionary writes', async ({ page }, testInfo) => {
  await page.addInitScript(({ initial }) => {
    const snapshot = structuredClone(initial)
    snapshot.dictionaries.push({ ...structuredClone(snapshot.dictionaries[0]), metadata: { ...snapshot.dictionaries[0].metadata, id: 'dictionary-extra', name: '额外词典' } })
    snapshot.workflows[0].targets[0].dictionaryIds.push('dictionary-extra')
    Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
    const run = { id: 'resolution-record', workflowId: 'workflow-proof', name: '默认创作工作流', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: true, observationRevision: 1, observedCount: 5, ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 1, runtimeCapability: 'direct_replace', excludedDictionaryIds: [], exclusionRevisions: [] }
    const row = (source: string, translation: string, kind: string, ids: string[], ruleIndex: number | null, editable = true) => ({ source, translation, state: translation ? 'translated' : 'pending', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 1, resolution: { kind, dictionaryIds: ids, ruleIndex, editable } })
    const rows = [row('Selected:0', '选择', 'dictionary', ['dictionary-proof'], null), row('Total: 33', '总计: 33', 'rule_translated', ['dictionary-proof'], 0), row('Hidden:0', '', 'rule_pending', [], 0), { ...row('0.1818', '', 'filtered', [], null), resolution: { kind: 'filtered', dictionaryIds: [], ruleIndex: null, editable: true, skipReason: 'pure_number_or_symbols' } }, row('Other', '其他', 'dictionary', ['dictionary-extra'], null, false)]
    Object.assign(rows[1]!.resolution, { editSource: 'Total', editTranslation: '总计' })
    Object.assign(rows[2]!.resolution, { editSource: 'Hidden', editTranslation: '' })
    ;(window as any).__resolution = { rows, run, merges: [] as boolean[], filters: [] as string[], saves: [] as any[] }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_probe_runs') return []
      if (command === 'desktop_dictionary') return structuredClone(snapshot.dictionaryDetails['dictionary-proof'])
      if (command === 'desktop_workflow') return { ...snapshot.workflows[0], revision: 1 }
      if (['desktop_workflow_collection', 'desktop_probe_run_summary'].includes(command)) return { ...run }
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_edit_probe_translation') {
        ;(window as any).__resolution.saves.push(args)
        const edited = rows.find(row => row.source === args.request.source)
        if (edited && (edited.resolution as any).editSource) {
          Object.assign(edited.resolution, { editTranslation: args.request.translation, kind: args.request.translation ? 'rule_translated' : 'rule_pending' })
          edited.translation = args.request.translation ? args.request.translation + edited.source.slice(edited.source.indexOf(':')) : ''
        }
        return { ...run }
      }
      if (command === 'desktop_probe_run_entries') {
        ;(window as any).__resolution.merges.push(args.request.mergeRules)
        ;(window as any).__resolution.filters.push(args.request.translationFilter)
        return { observationRevision: 1, dictionaryRevision: 1, page: 1, pageSize: 50, total: rows.length, rows }
      }
      return null
    } }
  }, { initial: model })
  await page.goto('/')
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByRole('checkbox', { name: '隐藏跳过的条目', exact: true })).toHaveCount(0)
  const total = page.getByRole('row').filter({ hasText: 'Total: 33' })
  await expect(total.getByRole('textbox')).toHaveValue('总计: 33')
  await expect(total).toContainText('规则翻译')
  await expect(total).toContainText('规则 1')
  await expect(total).not.toContainText('译文来自 界面基础词典')
  await expect(page.getByRole('row').filter({ hasText: 'Selected:0' })).not.toContainText('已在 界面基础词典')
  await expect(page.getByRole('row').filter({ hasText: 'Other' })).toContainText('已在 额外词典')
  await expect(page.getByRole('row').filter({ hasText: 'Hidden:0' })).toContainText('规则待译')
  await expect(page.getByRole('row').filter({ hasText: '0.1818' })).toContainText('纯数字或符号过滤')
  await expect(page.getByRole('row').filter({ hasText: 'Other' }).getByRole('textbox')).toHaveAttribute('readonly')
  await expect(page.locator('[class*="i-tabler-book-upload"]')).toHaveCount(0)
  const selected = page.getByRole('row').filter({ hasText: 'Selected:0' })
  await selected.getByRole('textbox').fill('已选择')
  await selected.getByRole('textbox').blur()
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.saves.length)).toBe(1)
  const hidden = page.getByRole('row').filter({ hasText: 'Hidden:0' }).getByRole('textbox')
  await expect(hidden).toBeEditable()
  await hidden.fill('隐藏')
  await hidden.blur()
  await expect(hidden).toHaveValue('隐藏:0')
  await total.getByRole('textbox').focus()
  await expect(total.getByRole('textbox')).toHaveValue('总计')
  await total.getByRole('textbox').fill('合计')
  await total.getByRole('textbox').blur()
  await expect(total.getByRole('textbox')).toHaveValue('合计: 33')
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.saves.at(-1).request.translation)).toBe('合计')
  await page.getByRole('button', { name: '按翻译状态筛选', exact: true }).click()
  await page.getByRole('menuitem', { name: '规则匹配', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.filters.at(-1))).toBe('rule_matched')
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.merges.at(-1))).toBe(true)
  await page.getByRole('button', { name: '任务操作', exact: true }).click()
  const merge = page.getByRole('menuitemcheckbox', { name: '合并规则翻译', exact: true })
  await expect(merge).toHaveAttribute('aria-checked', 'true')
  await merge.click()
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.merges.at(-1))).toBe(false)
  await merge.click()
  await expect.poll(() => page.evaluate(() => (window as any).__resolution.merges.at(-1))).toBe(true)
  await page.keyboard.press('Escape')
  await page.getByRole('heading', { name: '默认创作工作流', exact: true }).click()
  await page.waitForTimeout(200)
  await page.screenshot({ path: testInfo.outputPath('entry-resolution.png'), fullPage: true })
  await page.getByRole('button', { name: '打开字典', exact: true }).click()
  await expect(page.getByRole('button', { name: '返回工作流', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回工作流', exact: true }).click()
  await expect(page.getByRole('heading', { name: '默认创作工作流', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '按翻译状态筛选', exact: true })).toContainText('规则匹配')
  await page.getByRole('button', { name: '打开字典', exact: true }).click()
  await page.getByRole('textbox', { name: /^编辑译文：/ }).first().fill('未保存修改')
  await page.getByRole('button', { name: '返回工作流', exact: true }).click()
  const discard = page.getByRole('dialog')
  await discard.getByRole('button', { name: '继续编辑', exact: true }).click()
  await expect(page.getByRole('button', { name: '返回工作流', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回工作流', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: '放弃更改', exact: true }).click()
  await expect(page.getByRole('heading', { name: '默认创作工作流', exact: true })).toBeVisible()
  await page.setViewportSize({ width: 900, height: 720 })
  await page.screenshot({ path: testInfo.outputPath('entry-resolution-narrow.png'), fullPage: true })
})
