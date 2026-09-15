import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 800 })
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('settings uses wide descriptions and optional help without duplicate shortcuts', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.getByText(/^当前：/)).toHaveCount(0)
  const description = page.getByText('打开本机保存字典的文件夹。', { exact: true })
  await expect(description).toBeVisible()
  expect((await description.boundingBox())!.height).toBeLessThanOrEqual(20)
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  const help = page.getByRole('button', { name: '自动补全间隔（秒）说明', exact: true })
  await help.hover()
  await expect(page.getByText(/^0 秒时每 250 毫秒检查/)).toBeVisible()
  await page.mouse.move(5, 5)
  await page.getByTestId('settings-tabs').getByRole('tab', { name: '通用', exact: true }).click()
  await description.scrollIntoViewIfNeeded()
  await page.screenshot({ path: testInfo.outputPath('settings-wide-descriptions.png') })
})

test('workflow uses dictionary terminology and moves shortcut instructions into help', async ({ page }, testInfo) => {
  await expect(page.getByRole('button', { name: '字典', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await expect(page.getByRole('tab', { name: '翻译字典', exact: true })).toBeVisible()
  await page.getByRole('tab', { name: '基础配置', exact: true }).click()
  const hint = page.getByText('快捷键要带 Ctrl、Alt 或 Win。保存后，在其他软件里也能按它开关这个工作流。', { exact: true })
  await expect(hint).toHaveCount(0)
  await page.getByRole('button', { name: '全局快捷键说明', exact: true }).hover()
  await expect(hint).toBeVisible()
  await page.waitForTimeout(250)
  await page.screenshot({ path: testInfo.outputPath('workflow-shortcut-help.png') })
})
