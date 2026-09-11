import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test('pin button and minimize-to-tray preferences persist and agree with settings', async ({ page }, info) => {
  await page.addInitScript(({ key, value }) => { if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value)) }, { key: storageKey, value: model })
  await page.goto('/')
  await page.getByRole('button', { name: '窗口置顶', exact: true }).click()
  await expect(page.getByRole('button', { name: '取消置顶', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.getByRole('switch', { name: '窗口置顶', exact: true })).toBeChecked()
  await page.getByRole('switch', { name: '最小化到托盘', exact: true }).click()
  await page.getByRole('combobox', { name: '关闭窗口时', exact: true }).click()
  await page.getByRole('option', { name: '收进托盘', exact: true }).click()
  await page.reload()
  await expect(page.getByRole('button', { name: '取消置顶', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.getByRole('switch', { name: '最小化到托盘', exact: true })).toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时', exact: true })).toContainText('收进托盘')
  await page.getByTestId('settings-section-application').screenshot({ path: info.outputPath('window-preferences.png') })
  await page.getByRole('switch', { name: '窗口置顶', exact: true }).click()
  await expect(page.getByRole('button', { name: '窗口置顶', exact: true })).toHaveAttribute('aria-pressed', 'false')
})
