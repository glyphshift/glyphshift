import { selectLanguage } from './fixtures/languageSelect'
import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

async function setup(page: import('@playwright/test').Page) {
  await page.addInitScript(({ snapshot, key }) => {
    localStorage.setItem(key, JSON.stringify(snapshot))
    const detail = structuredClone(snapshot.dictionaryDetails['dictionary-proof'])
    detail.entries = [{ source: 'Same', translation: 'Original' }, { source: 'Keep', translation: 'Keep translation' }]
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_privilege_status') return { elevated: false }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_dictionary') return detail
      if (command === 'plugin:dialog|save') return 'X:/SyntheticFixtures/export.json'
      if (command === 'desktop_export_dictionary') { (window as any).__exportedDictionary = args; return null }
      if (command === 'plugin:dialog|open') return 'X:/SyntheticFixtures/entries.csv'
      if (command === 'desktop_preview_dictionary_import') {
        if ((window as any).__invalidImport) throw { schemaVersion: 1, code: 'dictionary.import_invalid', args: {} }
        return { metadata: null, entries: [{ source: 'Same', translation: 'Imported' }, { source: 'New', translation: '' }] }
      }
      if (command === 'desktop_create_dictionary') { (window as any).__createdDictionary = args.create; return snapshot }
      if (command === 'desktop_update_dictionary') { (window as any).__savedDictionary = args.edit; Object.assign(detail, { entries: args.edit.entries, revision: detail.revision + 1 }); return snapshot }
      return null
    } }
  }, { snapshot: model, key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '字典', exact: true }).click()
}

test('CSV is validated before metadata confirmation and invalid CSV creates nothing', async ({ page }) => {
  await setup(page)
  await page.evaluate(() => { (window as any).__invalidImport = true })
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('文件格式无效')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.evaluate(() => { (window as any).__invalidImport = false })
  await page.getByRole('button', { name: '导入', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '导入字典' })
  await expect(dialog.getByRole('button', { name: '导入', exact: true })).toBeDisabled()
  await selectLanguage(page, dialog.getByRole('button', { name: '源语言', exact: true }), 'ja-JP')
  await selectLanguage(page, dialog.getByRole('button', { name: '目标语言', exact: true }), 'zh-CN')
  await dialog.getByRole('button', { name: '导入', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__createdDictionary)).toMatchObject({ metadata: { name: 'entries', sourceLocale: 'ja-JP', targetLocale: 'zh-CN' }, entries: [{ source: 'Same', translation: 'Imported' }, { source: 'New', translation: '' }] })
})

for (const [mode, same, keep] of [['追加', 'Original', true], ['覆盖', 'Imported', true], ['替换（危险）', 'Imported', false]] as const) {
  test(`dictionary import ${mode} updates draft before explicit save`, async ({ page }) => {
    await setup(page)
    await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
    await page.getByRole('button', { name: '字典操作' }).click()
    await page.getByRole('menuitem', { name: '导入 CSV', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: '导入词条' })
    await dialog.getByRole('combobox', { name: '导入方式' }).click()
    await page.getByRole('option', { name: mode, exact: true }).click()
    if (!keep) {
      await expect(dialog.getByRole('button', { name: '导入', exact: true })).toBeDisabled()
      await dialog.getByRole('checkbox').check()
    }
    await dialog.getByRole('button', { name: '导入', exact: true }).click()
    expect(await page.evaluate(() => (window as any).__savedDictionary)).toBeUndefined()
    await page.getByRole('button', { name: '保存字典', exact: true }).click()
    await expect.poll(() => page.evaluate(() => (window as any).__savedDictionary?.entries)).toEqual([
      { source: 'Same', translation: same }, ...(keep ? [{ source: 'Keep', translation: 'Keep translation' }] : []), { source: 'New', translation: '' },
    ])
  })
}

test('icons render with external icon APIs blocked and actions expose hover labels', async ({ page }) => {
  const requests: string[] = []
  await page.route(/https?:\/\/(?:api\.)?(?:iconify\.design|api\.simplesvg\.com|api\.unisvg\.com)/, async route => { requests.push(route.request().url()); await route.abort() })
  await setup(page)
  const edit = page.getByRole('button', { name: /编辑.*界面基础词典/ })
  await expect(edit).toHaveAttribute('title', /界面基础词典/)
  await expect(edit.locator('svg path').first()).toBeAttached()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'AI 配置', exact: true })).toHaveCount(1)
  await expect(page.getByRole('button', { name: '添加 AI 配置' }).locator('svg path').first()).toBeAttached()
  expect(requests).toEqual([])
  await expect(page.getByRole('alert')).toHaveCount(0)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/settings-bundled-icons.png' })
})


test('dictionary detail offers grouped JSON and CSV import and export actions', async ({ page }) => {
  await setup(page)
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await page.getByRole('button', { name: '字典操作' }).click()
  for (const name of ['导入 JSON', '导入 CSV', '导出 JSON', '导出 CSV']) await expect(page.getByRole('menuitem', { name, exact: true })).toBeVisible()
  if (process.env.GLYPHSHIFT_EXPORT_SCREENSHOT) await page.screenshot({ path: process.env.GLYPHSHIFT_EXPORT_SCREENSHOT })
  await page.getByRole('menuitem', { name: '导出 CSV', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__exportedDictionary)).toEqual({ dictionaryId: 'dictionary-proof', outputPath: 'X:/SyntheticFixtures/export.csv' })
})
