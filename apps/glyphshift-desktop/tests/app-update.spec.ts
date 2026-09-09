import { test, expect } from '@playwright/test'
import { model } from './fixtures/productModel'

async function setup(page: import('@playwright/test').Page, enabled = true, delayed = false) {
  await page.addInitScript(({ snapshot, enabled, delayed }) => {
    const w = window as any
    w.__checks = 0
    w.__download = ''
    w.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_settings') return JSON.parse(localStorage.getItem('update-test-settings') ?? JSON.stringify({ settingsSchemaVersion: 1, localePreference: 'zh-CN', checkUpdatesOnStartup: enabled }))
      if (command === 'desktop_update_settings') {
        localStorage.setItem('update-test-settings', JSON.stringify({ settingsSchemaVersion: 1, ...args.update }))
        return { settingsSchemaVersion: 1, ...args.update }
      }
      if (command === 'desktop_privilege_status') return { elevated: false }
      if (command === 'desktop_status') return { shellReady: true, apiVersion: 35, productVersion: '0.3.0' }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_check_update') {
        w.__checks++
        if (w.__failUpdate) throw new Error('offline')
        const release = { version: '0.4.0', releaseNotes: '改进导入\n修复显示问题', downloadUrl: 'https://pan.quark.cn/s/synthetic-release' }
        if (delayed) return await new Promise(resolve => { w.__resolveUpdate = () => resolve(release) })
        return release
      }
      if (command === 'plugin:opener|open_url') { w.__download = args.url; return null }
      return null
    } }
  }, { snapshot: model, enabled, delayed })
  await page.setViewportSize({ width: 1100, height: 800 })
  await page.goto('/')
}

test('update notice supports notes, download, dismissal and persisted opt-out', async ({ page }, info) => {
  await setup(page)
  await expect(page.getByText('发现新版本 0.4.0')).toBeVisible()
  await page.getByRole('button', { name: '更新内容', exact: true }).click()
  await expect(page.getByText('改进导入')).toBeVisible()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '前往下载', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__download)).toBe('https://pan.quark.cn/s/synthetic-release')
  await page.getByRole('button', { name: '关闭更新提示' }).click()
  await expect(page.getByText('发现新版本 0.4.0')).toHaveCount(0)
  await page.getByRole('button', { name: '设置', exact: true }).click()
  const toggle = page.getByRole('switch', { name: '启动时检查更新' })
  await toggle.click()
  await expect(toggle).not.toBeChecked()
  await page.reload()
  await expect.poll(() => page.evaluate(() => (window as any).__checks)).toBe(0)
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('button', { name: '检查更新', exact: true }).click()
  await expect(page.getByText('发现新版本 0.4.0')).toBeVisible()
  await page.getByText('软件更新', { exact: true }).scrollIntoViewIfNeeded()
  await page.screenshot({ path: info.outputPath('update-settings.png') })
  await page.evaluate(() => { (window as any).__failUpdate = true })
  await page.getByRole('button', { name: '检查更新', exact: true }).click()
  await expect(page.getByText('没能检查更新，请稍后再试。')).toBeVisible()
})

test('turning off updates hides a late automatic response', async ({ page }) => {
  await setup(page, true, true)
  await expect.poll(() => page.evaluate(() => (window as any).__checks)).toBe(1)
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('switch', { name: '启动时检查更新' }).click()
  await page.evaluate(() => (window as any).__resolveUpdate())
  await expect(page.getByText('发现新版本 0.4.0')).toHaveCount(0)
})
