import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

for (const library of ['workflows', 'dictionaries'] as const) {
  test(`${library} sorts by usage and title and remembers the selection`, async ({ page }, testInfo) => {
    const snapshot = structuredClone(model)
    if (library === 'workflows') {
      snapshot.workflows = ['Alpha', 'Zulu'].map(name => ({ ...structuredClone(model.workflows[0]), id: name, name }))
      for (const item of snapshot.workflows) (snapshot.workflowDetails as any)[item.id] = { ...structuredClone(model.workflowDetails['workflow-proof']), ...item }
    } else {
      snapshot.dictionaries = ['Alpha', 'Zulu'].map(name => ({ ...structuredClone(model.dictionaries[0]), metadata: { ...model.dictionaries[0].metadata, id: name, name } }))
      for (const item of snapshot.dictionaries) (snapshot.dictionaryDetails as any)[item.metadata.id] = { ...structuredClone(model.dictionaryDetails['dictionary-proof']), metadata: item.metadata }
    }
    await page.addInitScript(({ snapshot, key, library }) => {
      localStorage.setItem(key, JSON.stringify(snapshot))
      if (!localStorage.getItem('glyphshift.library-usage')) localStorage.setItem('glyphshift.library-usage', JSON.stringify({ [`${library}:Zulu`]: 100, [`${library}:Alpha`]: 10 }))
    }, { snapshot, key: storageKey, library })
    await page.goto('/')
    const navigate = async () => { if (library === 'dictionaries') await page.getByRole('button', { name: '字典', exact: true }).click() }
    await navigate()
    const table = page.getByTestId(library === 'workflows' ? 'workflow-management-table' : 'dictionary-management-table')
    const names = library === 'workflows' ? table.getByRole('button', { name: /^(Alpha|Zulu)$/ }) : table.getByRole('button', { name: /^(Alpha|Zulu) / }).locator('span.font-semibold')
    await expect(names).toHaveText(['Zulu', 'Alpha'])
    for (const [label, expected] of [['名称升序', ['Alpha', 'Zulu']], ['名称降序', ['Zulu', 'Alpha']], ['最早使用', ['Alpha', 'Zulu']]] as const) {
      await page.getByRole('button', { name: '排序', exact: true }).click()
      await page.getByRole('menuitem', { name: label, exact: true }).click()
      await expect(names).toHaveText([...expected])
    }
    await page.reload()
    await navigate()
    await expect(page.getByRole('button', { name: '排序', exact: true })).toContainText('最早使用')
    await page.getByRole('button', { name: '排序', exact: true }).click()
    await page.getByRole('menuitem', { name: '最近使用', exact: true }).click()
    if (library === 'workflows') await page.getByRole('button', { name: '编辑 Alpha', exact: true }).click()
    else await names.filter({ hasText: 'Alpha' }).click()
    await expect.poll(() => page.evaluate(library => JSON.parse(localStorage.getItem('glyphshift.library-usage')!)[`${library}:Alpha`], library)).toBeGreaterThan(100)
    await page.reload()
    await navigate()
    await expect(names).toHaveText(['Alpha', 'Zulu'])
    await page.screenshot({ path: testInfo.outputPath(`${library}-sort.png`) })
  })
}

test('empty dictionary can explicitly clear captured rows on save', async ({ page }) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaryDetails['dictionary-proof'].entries = []
  snapshot.dictionaries[0].entryCount = 0
  await page.addInitScript(snapshot => {
    const w = window as any
    w.__saveCalls = []
    w.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { localePreference: 'zh-CN', checkUpdatesOnStartup: false }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_dictionary') return structuredClone(snapshot.dictionaryDetails['dictionary-proof'])
      if (command === 'desktop_update_dictionary') { w.__saveCalls.push(args); snapshot.dictionaryDetails['dictionary-proof'].revision++; return snapshot }
      return null
    } }
  }, snapshot)
  await page.goto('/')
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典', exact: true }).click()
  await page.getByRole('button', { name: '字典操作', exact: true }).click()
  await page.getByRole('menuitem', { name: '清空字典', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: '确认清空', exact: true }).click()
  await page.getByRole('button', { name: '保存字典', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__saveCalls)).toEqual([expect.objectContaining({ clearCaptured: true, edit: expect.objectContaining({ entries: [] }) })])
  await expect(page.getByRole('button', { name: '保存字典', exact: true })).toBeDisabled()
})
