import { expect, test, type Page } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

const candidate = {
  packageId: 'glyphshift-adapter-synthetic', version: '1.0.0', sha256: 'a'.repeat(64),
  architectures: ['x86', 'x86_64'], adapters: ['Synthetic inline'],
  selected: false, loaded: false, problem: null,
}

async function setup(page: Page, desktop = true, installed = false) {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
  await page.getByRole('button', { name: '设置', exact: true }).click()
  if (desktop) await page.evaluate(({ candidate, installed }) => {
    const w = window as any
    w.__pluginCalls = []
    w.__pluginFailure = false
    w.__pluginSnapshot = { packages: installed ? [candidate] : [], restartRequired: false, runtimeReady: true }
    w.__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      w.__pluginCalls.push({ command, args })
      if (command === 'plugin:dialog|open') return 'synthetic.gsp'
      if (command === 'desktop_prepare_plugin') return candidate
      if (command === 'desktop_install_plugin') w.__pluginSnapshot.packages = [{ ...candidate }]
      if (command === 'desktop_select_plugin') {
        if (w.__pluginFailure) throw { schemaVersion: 1, code: 'plugin.incompatible', args: {} }
        w.__pluginSnapshot.packages[0].selected = args.enabled
        w.__pluginSnapshot.restartRequired = args.enabled
      }
      if (command === 'desktop_plugins' || command === 'desktop_install_plugin' || command === 'desktop_select_plugin') return structuredClone(w.__pluginSnapshot)
      return null
    } }
  }, { candidate, installed })
  await page.getByRole('tab', { name: '插件管理', exact: true }).click()
}

test('local package installation and enabling require separate confirmations', async ({ page }, info) => {
  await setup(page)
  await expect(page.getByText('还没有安装插件。')).toBeVisible()
  await page.getByRole('button', { name: '安装本地插件', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('Synthetic inline')
  await expect.poll(() => page.evaluate(() => (window as any).__pluginCalls.filter((v: any) => v.command === 'desktop_install_plugin').length)).toBe(0)
  await dialog.getByRole('button', { name: '确认安装', exact: true }).click()
  const row = page.getByTestId('plugin-glyphshift-adapter-synthetic-1.0.0')
  await expect(row).toContainText('下次启动停用')
  await expect(page.getByTestId('plugin-restart')).toHaveCount(0)
  await expect.poll(() => page.evaluate(() => (window as any).__pluginCalls.find((v: any) => v.command === 'desktop_install_plugin').args)).toEqual({ approvedSha256: candidate.sha256 })
  await row.getByRole('button', { name: '启用', exact: true }).click()
  await expect(dialog).toContainText('原生代码')
  await expect.poll(() => page.evaluate(() => (window as any).__pluginCalls.filter((v: any) => v.command === 'desktop_select_plugin').length)).toBe(0)
  await dialog.getByRole('button', { name: '启用', exact: true }).click()
  await expect(row).toContainText('下次启动启用')
  await expect(row.getByText('本次已加载')).toHaveCount(0)
  await expect(page.getByTestId('plugin-restart')).toBeVisible()
  await page.screenshot({ path: info.outputPath('plugins-pending-restart.png') })
  await row.getByRole('button', { name: '停用', exact: true }).click()
  await dialog.getByRole('button', { name: '停用', exact: true }).click()
  await expect(page.getByTestId('plugin-restart')).toHaveCount(0)
})

test('cancelled preview does not install; incompatible enable keeps its state', async ({ page }) => {
  await setup(page, true, true)
  await page.getByRole('button', { name: '安装本地插件', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: '取消', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__pluginCalls.filter((v: any) => v.command === 'desktop_cancel_plugin_install').length)).toBe(1)
  await expect.poll(() => page.evaluate(() => (window as any).__pluginCalls.filter((v: any) => v.command === 'desktop_install_plugin').length)).toBe(0)
  await page.evaluate(() => { (window as any).__pluginFailure = true })
  const row = page.getByTestId('plugin-glyphshift-adapter-synthetic-1.0.0')
  await row.getByRole('button', { name: '启用', exact: true }).click()
  await page.getByRole('dialog').getByRole('button', { name: '启用', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('选择未保存')
  await expect(row).toContainText('下次启动停用')
  await expect(page.getByTestId('plugin-restart')).toHaveCount(0)
})

test('loaded version and damaged versions stay distinguishable in a compact window', async ({ page }, info) => {
  await page.setViewportSize({ width: 720, height: 700 })
  await setup(page, true, true)
  await page.evaluate(() => {
    const w = window as any
    const old = w.__pluginSnapshot.packages[0]
    old.loaded = true
    w.__pluginSnapshot.packages.unshift({ ...old, version: '2.0.0', sha256: 'b'.repeat(64), loaded: false, selected: true, problem: { schemaVersion: 1, code: 'plugin.integrity', args: {} } })
    w.__pluginSnapshot.restartRequired = true
  })
  await page.getByRole('button', { name: '刷新', exact: true }).click()
  await expect(page.getByTestId('plugin-glyphshift-adapter-synthetic-1.0.0')).toContainText('本次已加载')
  const damaged = page.getByTestId('plugin-glyphshift-adapter-synthetic-2.0.0')
  await expect(damaged).toContainText('校验失败')
  await expect(damaged.getByRole('button', { name: '停用', exact: true })).toBeEnabled()
  await expect(page.getByRole('button', { name: '安装本地插件', exact: true })).toBeInViewport()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: info.outputPath('plugins-compact.png') })
})

test('browser mode explains desktop-only management', async ({ page }) => {
  await setup(page, false)
  await expect(page.getByText('请在桌面 App 中管理插件。')).toBeVisible()
  await expect(page.getByRole('button', { name: '安装本地插件', exact: true })).toBeDisabled()
})
