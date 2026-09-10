import { selectLanguage } from './fixtures/languageSelect'
import { expect, test } from '@playwright/test'
import { model, replaceModel } from './fixtures/productModel'
import { combineImportEntries } from '../src/dictionaryImport'

test('merge preserves non-empty translations and respects file priority', () => {
  const files = [[{ source: 'A', translation: 'First' }], [{ source: 'A', translation: '' }, { source: 'B', translation: '' }], [{ source: 'A', translation: 'Last' }]]
  expect(combineImportEntries(files, 'first')[0]!.translation).toBe('First')
  expect(combineImportEntries(files, 'last')[0]!.translation).toBe('Last')
  expect(combineImportEntries(files.slice(0, 2), 'last')[0]!.translation).toBe('First')
})

for (const merge of [false, true]) test(`batch import ${merge ? 'merged' : 'separate'} files validates languages and creates expected dictionaries`, async ({ page }, testInfo) => {
  await page.addInitScript(snapshot => {
    const w = window as any
    w.__created = []
    w.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'plugin:dialog|open') { w.__openOptions = args; return ['X:/SyntheticFixtures/one.csv', 'X:/SyntheticFixtures/two.srt'] }
      if (command === 'desktop_preview_dictionary_import') return { metadata: null, entries: [{ source: 'Hello', translation: args.inputPath.endsWith('.csv') ? '你好' : '' }] }
      if (command === 'desktop_create_dictionary') { w.__created.push(args.create); return snapshot }
      return null
    } }
  }, model)
  await replaceModel(page, model)
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '导入字典', exact: true })
  const confirm = dialog.getByRole('button', { name: '导入', exact: true })
  await expect(confirm).toBeDisabled()
  if (merge) {
    await dialog.getByRole('combobox', { name: '创建方式' }).click()
    await page.getByRole('option', { name: '合并成一个字典' }).click()
  }
  await selectLanguage(page, dialog.getByRole('button', { name: '源语言', exact: true }).first(), 'en-US')
  await selectLanguage(page, dialog.getByRole('button', { name: '目标语言', exact: true }).first(), 'zh-CN')
  if (!merge) await dialog.getByRole('button', { name: '应用语言到所有文件' }).click()
  await expect(confirm).toBeEnabled()
  await page.screenshot({ path: testInfo.outputPath('batch-import.png') })
  await confirm.click()
  await expect(dialog).not.toBeVisible()
  const created = await page.evaluate(() => (window as any).__created)
  expect(created).toHaveLength(merge ? 1 : 2)
  expect(created.every((item: any) => item.metadata.sourceLocale === 'en-US' && item.metadata.targetLocale === 'zh-CN')).toBeTruthy()
  expect(created[0].entries[0].translation).toBe('你好')
  expect(await page.evaluate(() => (window as any).__openOptions.options.multiple)).toBe(true)
})

test('batch import retries only unfinished dictionaries and reports preview filenames', async ({ page }) => {
  await page.addInitScript(snapshot => {
    const w = window as any
    w.__created = []; w.__failOnce = true; w.__badPreview = true
    w.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN' }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'plugin:dialog|open') return ['X:/SyntheticFixtures/one.json', 'X:/SyntheticFixtures/two.srt']
      if (command === 'desktop_preview_dictionary_import') {
        if (w.__badPreview && args.inputPath.endsWith('.srt')) throw { schemaVersion: 1, code: 'import.srt_timing', args: { line: 2 } }
        return { metadata: { ...snapshot.dictionaries[0].metadata, name: args.inputPath.endsWith('.json') ? 'one' : 'two' }, entries: [{ source: 'Hello', translation: '' }] }
      }
      if (command === 'desktop_create_dictionary') {
        if (args.create.metadata.name === 'two' && w.__failOnce) { w.__failOnce = false; throw new Error('synthetic failure') }
        w.__created.push(args.create.metadata.name); return snapshot
      }
      return null
    } }
  }, model)
  await replaceModel(page, model)
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('two.srt: 第 2 行')
  expect(await page.evaluate(() => (window as any).__created)).toEqual([])
  await page.evaluate(() => { (window as any).__badPreview = false })
  await page.getByRole('button', { name: '导入', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '导入字典', exact: true })
  await dialog.getByRole('button', { name: '导入', exact: true }).click()
  await expect(dialog).toContainText('已创建 1 / 2')
  await dialog.getByRole('button', { name: '导入', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  expect(await page.evaluate(() => (window as any).__created)).toEqual(['one', 'two'])
})
