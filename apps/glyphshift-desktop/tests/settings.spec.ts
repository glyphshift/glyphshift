import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})
test('help exposes adapter information without internal targets', async ({ page }) => {
  await page.getByRole('button', { name: '帮助' }).click()

  await expect(page.getByRole('heading', { name: '帮助' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '当前适配器' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(page.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(page.getByText('Unity Mono 标准界面', { exact: true })).toBeVisible()
  await expect(page.getByText('Windows', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI+', { exact: true })).toBeVisible()
  await expect(page.getByText('文字观察', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('文字替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('字体替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('无需配置', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('v1.0.0', { exact: true }).first()).toBeVisible()
  await expect(page.getByRole('button', { name: /技术文档/ })).toHaveCount(6)
  await page.evaluate(() => {
    window.open = ((url?: string | URL) => {
      ;(window as unknown as { __openedAdapterDocumentation?: string }).__openedAdapterDocumentation = String(url)
      return window
    }) as typeof window.open
  })
  await page.getByRole('button', { name: '查看 ExtTextOutW 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-exttextoutw')
  await page.getByRole('button', { name: '查看 Unity Mono 标准界面 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://docs.unity3d.com/cn/current/Manual/scripting-backends-mono.html')
  await expect(page.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)
  await expect(page.getByText('synthetic.ext-text-out')).toHaveCount(0)
})

test('settings applies and persists the real locale and theme preferences', async ({ page }) => {
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.getByRole('button', { name: '切换到浅色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await page.getByRole('button', { name: '切换到深色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
  await expect(page.getByText(/在线翻译/)).toHaveCount(0)
  await expect(page.getByRole('textbox')).toHaveCount(0)
  await expect(page.getByRole('combobox')).toHaveCount(3)

  await page.getByRole('combobox', { name: '界面语言' }).click()
  await page.getByRole('option', { name: 'English', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US')
  await expect(page.getByRole('button', { name: 'Workflows', exact: true })).toBeVisible()

  await page.getByRole('combobox', { name: 'Theme' }).click()
  await page.getByRole('option', { name: 'Dark', exact: true }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.reload()
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await expect(page.locator('html')).toHaveAttribute('lang', 'en-US')
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.getByRole('button', { name: 'Help', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Help' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Available adapters' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'View technical documentation for ExtTextOutW' })).toBeVisible()
  await page.getByRole('button', { name: 'Software', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Software' })).toBeVisible()
  await page.getByRole('button', { name: 'Dictionaries', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Dictionaries' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Fonts', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Workflows', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Workflows' })).toBeVisible()

  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  await page.getByRole('combobox', { name: 'Theme' }).click()
  await page.getByRole('option', { name: 'Light', exact: true }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await expect(page.locator('html')).not.toHaveClass(/dark/)
})

test('settings persists startup close behavior and the administrator launch preference', async ({ page }) => {
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: '应用行为' })).toBeVisible()
  const launchAtStartup = page.getByRole('switch', { name: '开机自动启动' })
  await expect(launchAtStartup).not.toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时' })).toContainText('彻底退出')

  await expect(page.getByRole('heading', { name: '权限' })).toBeVisible()
  await expect(page.getByText('普通权限', { exact: true })).toBeVisible()
  const launchElevated = page.getByRole('switch', { name: '始终以管理员身份启动' })
  await expect(launchElevated).not.toBeChecked()

  await launchAtStartup.click()
  await launchElevated.click()
  await page.getByRole('combobox', { name: '关闭窗口时' }).click()
  await page.getByRole('option', { name: '最小化到任务栏' }).click()
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('switch', { name: '开机自动启动' })).toBeChecked()
  await expect(page.getByRole('switch', { name: '始终以管理员身份启动' })).toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时' })).toContainText('最小化到任务栏')
})

test('administrator launch preference persists before elevation and disables without another restart', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const elevationCommands: Array<{ command: string; launchElevated?: boolean }> = []
    const internals = {
      invoke: async (command: string, args?: { update?: { launchElevated?: boolean } }) => {
        if (command === 'desktop_settings') return {
          settingsSchemaVersion: 1,
          localePreference: 'zh-CN',
          themePreference: 'dark',
          launchAtStartup: false,
          launchElevated: false,
          closeBehavior: 'quit',
        }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 27 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_privilege_status') return { elevated: false }
        if (command === 'desktop_update_settings') {
          elevationCommands.push({ command, launchElevated: args?.update?.launchElevated })
          return { settingsSchemaVersion: 1, ...args?.update }
        }
        if (command === 'desktop_restart_elevated') elevationCommands.push({ command })
        return null
      },
    }
    ;(window as unknown as {
      __TAURI_INTERNALS__: typeof internals
      __elevationCommands: typeof elevationCommands
    }).__TAURI_INTERNALS__ = internals
    ;(window as unknown as { __elevationCommands: typeof elevationCommands }).__elevationCommands = elevationCommands
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()

  const launchElevated = page.getByRole('switch', { name: '始终以管理员身份启动' })
  await launchElevated.click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __elevationCommands: Array<{ command: string }> }).__elevationCommands.map(entry => entry.command)
  ))).toEqual(['desktop_update_settings', 'desktop_restart_elevated'])

  await launchElevated.click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __elevationCommands: Array<{ command: string; launchElevated?: boolean }> }).__elevationCommands
  ))).toEqual([
    { command: 'desktop_update_settings', launchElevated: true },
    { command: 'desktop_restart_elevated' },
    { command: 'desktop_update_settings', launchElevated: false },
  ])
})

test('title bar reports a theme persistence failure without changing the active theme', async ({ page }) => {
  await page.evaluate(() => {
    Storage.prototype.setItem = () => {
      throw new DOMException('synthetic storage failure', 'QuotaExceededError')
    }
  })

  await page.getByRole('button', { name: '切换到浅色主题' }).click()
  await expect(page.getByText('设置未保存', { exact: true })).toBeVisible()
  await expect(page.getByText('操作失败，请重试。', { exact: true })).toBeVisible()
  await expect(page.locator('html')).toHaveClass(/dark/)
})

test('compact viewport keeps the application shell bounded', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await expect(page.getByRole('heading', { name: '工作流' })).toBeVisible()
  const metrics = await page.evaluate(() => ({
    bodyHeight: document.body.scrollHeight,
    viewportHeight: window.innerHeight,
    bodyWidth: document.body.scrollWidth,
    viewportWidth: window.innerWidth,
  }))
  expect(metrics.bodyHeight).toBeLessThanOrEqual(metrics.viewportHeight)
  expect(metrics.bodyWidth).toBeLessThanOrEqual(metrics.viewportWidth)
})
