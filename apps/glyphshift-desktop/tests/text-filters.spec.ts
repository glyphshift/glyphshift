import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test('dictionary keeps existing entries visible without the obsolete hide-skipped control', async ({ page }, testInfo) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaryDetails['dictionary-proof'].entries = [
    {source:'123', translation:'123'}, {source:'Open menu', translation:'打开菜单'}, {source:'Chapter 2', translation:''},
  ]
  snapshot.dictionaries[0].entryCount = 3
  await page.addInitScript(({snapshot, key}) => { if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(snapshot)) }, {snapshot, key:storageKey})
  await page.goto('/')
  await page.getByRole('button', {name:'字典', exact:true}).click()
  await page.getByRole('row').filter({hasText:'界面基础词典'}).dblclick()
  await expect(page.getByRole('checkbox', {name:'隐藏跳过的条目', exact:true})).toHaveCount(0)
  await expect(page.getByRole('textbox', {name:'编辑原文：123', exact:true})).toBeVisible()
  await expect(page.getByRole('textbox', {name:'编辑原文：Open menu', exact:true})).toBeVisible()
  await expect(page.getByRole('textbox', {name:'编辑原文：Chapter 2', exact:true})).toBeVisible()
  await page.getByRole('button', {name:'设置', exact:true}).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: '文字处理', exact: true }).click()
  const filters = page.getByTestId('settings-text-filters')
  await expect(filters.getByText('捕获黑名单', { exact: true })).toBeVisible()
  await expect(filters).not.toContainText('已收录的条目保留')
  await filters.getByRole('switch', {name:/数字/}).last().check()
  await filters.getByRole('button', {name:'保存规则', exact:true}).click()
  await page.waitForTimeout(300)
  await filters.screenshot({path:testInfo.outputPath('global-filters.png')})
  await page.getByRole('button', {name:'字典', exact:true}).click()
  await page.getByRole('row').filter({hasText:'界面基础词典'}).dblclick()
  await expect(page.getByRole('textbox', {name:'编辑原文：Chapter 2', exact:true})).toBeVisible()
  await expect(page.getByRole('checkbox', {name:'隐藏跳过的条目', exact:true})).toHaveCount(0)
  await expect(page.getByRole('textbox', {name:/编辑原文：/})).toHaveCount(3)
  expect(await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries.length, storageKey)).toBe(3)
})
