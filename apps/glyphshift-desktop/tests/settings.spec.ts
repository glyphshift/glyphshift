import { expect, test } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { model, storageKey } from './fixtures/productModel'

const appVersion = (JSON.parse(readFileSync(join(import.meta.dirname, '..', 'package.json'), 'utf8')) as { version: string }).version

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('browser settings keep valid fields when other fields are unknown or invalid', async ({ page }) => {
  await page.evaluate(() => localStorage.setItem('glyphshift.app-settings.v1', JSON.stringify({
    settingsSchemaVersion: 999,
    safetyNoticeVersion: 1,
    onboardingVersion: 1,
    localePreference: 'en-US',
    themePreference: 42,
    launchAtStartup: true,
    closeBehavior: 'future-option',
    unknownFutureField: true,
  })))
  await page.reload()
  await page.getByRole('button', { name: 'Settings', exact: true }).click()

  await expect(page.getByRole('combobox', { name: 'Display language' })).toContainText('English')
  await expect(page.getByRole('combobox', { name: 'Theme' })).toContainText('Dark')
  await expect(page.getByRole('switch', { name: 'Launch at startup' })).toBeChecked()
  await expect(page.getByRole('combobox', { name: 'When closing the window' })).toContainText('Quit completely')
})

test('settings shares the full-width primary page axis across wide and compact windows', async ({ page }) => {
  await page.setViewportSize({ width: 1520, height: 720 })
  const workflowHeader = await page.getByTestId('management-page-header').boundingBox()
  expect(workflowHeader).not.toBeNull()
  await page.getByRole('button', { name: '设置', exact: true }).click()

  const pageHeader = page.getByTestId('management-page-header')
  const settingsLayout = page.getByTestId('settings-layout')
  const appearanceSection = page.getByTestId('settings-section-appearance')

  await expect(pageHeader).toBeVisible()
  await expect(page.getByTestId('management-page-header-icon')).toBeVisible()
  await expect(settingsLayout).toBeVisible()

  const geometry = await page.evaluate(() => {
    const box = (selector: string) => {
      const rect = document.querySelector<HTMLElement>(selector)?.getBoundingClientRect()
      if (!rect) throw new Error(`Missing ${selector}`)
      return { left: rect.left, right: rect.right, width: rect.width }
    }
    return {
      header: box('[data-testid="management-page-header"]'),
      layout: box('[data-testid="settings-layout"]'),
      appearance: box('[data-testid="settings-section-appearance"]'),
      headerBottom: document.querySelector<HTMLElement>('[data-testid="management-page-header"]')!.getBoundingClientRect().bottom,
      appearanceTop: document.querySelector<HTMLElement>('[data-testid="settings-section-appearance"]')!.getBoundingClientRect().top,
      navigationBottom: document.querySelector<HTMLElement>('[aria-label="设置分区"]')!.getBoundingClientRect().bottom,
    }
  })
  expect(geometry.header.width).toBeGreaterThan(1400)
  expect(Math.abs(geometry.header.left - workflowHeader!.x)).toBeLessThanOrEqual(1)
  expect(Math.abs(geometry.header.right - (workflowHeader!.x + workflowHeader!.width))).toBeLessThanOrEqual(1)
  expect(Math.abs(geometry.header.left - geometry.layout.left)).toBeLessThanOrEqual(1)
  expect(geometry.header.right - geometry.layout.right).toBeGreaterThanOrEqual(0)
  expect(geometry.header.right - geometry.layout.right).toBeLessThanOrEqual(12)
  expect(Math.abs(geometry.header.left - geometry.appearance.left)).toBeLessThanOrEqual(1)
  expect(Math.abs(geometry.layout.right - geometry.appearance.right)).toBeLessThanOrEqual(1)
  expect(geometry.appearanceTop - geometry.navigationBottom).toBeGreaterThanOrEqual(15)
  expect(geometry.appearanceTop - geometry.navigationBottom).toBeLessThanOrEqual(17)

  await page.setViewportSize({ width: 960, height: 640 })
  const compactGeometry = await page.evaluate(() => {
    const header = document.querySelector<HTMLElement>('[data-testid="management-page-header"]')!.getBoundingClientRect()
    const layout = document.querySelector<HTMLElement>('[data-testid="settings-layout"]')!.getBoundingClientRect()
    return { headerLeft: header.left, headerRight: header.right, layoutLeft: layout.left, layoutRight: layout.right }
  })
  expect(Math.abs(compactGeometry.headerLeft - compactGeometry.layoutLeft)).toBeLessThanOrEqual(1)
  expect(compactGeometry.headerRight - compactGeometry.layoutRight).toBeGreaterThanOrEqual(0)
  expect(compactGeometry.headerRight - compactGeometry.layoutRight).toBeLessThanOrEqual(12)
  await expect.poll(() => settingsLayout.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('settings navigation separates AI and management surfaces in the intended order', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  const tabs = page.getByTestId('settings-tabs').getByRole('tab')
  await expect(tabs).toHaveCount(6)
  expect(await tabs.allTextContents()).toEqual(['通用', 'AI 配置', '文字处理', '软件管理', '字体管理', '语言管理'])
  await expect(page.getByTestId('settings-section-ai')).toBeHidden()
  await tabs.filter({ hasText: 'AI 配置' }).click()
  await expect(page.getByTestId('settings-section-ai')).toBeVisible()
  await expect(page.getByTestId('settings-section-appearance')).toBeHidden()
})

test('help shares the full-width primary page axis and first-content rhythm', async ({ page }) => {
  await page.setViewportSize({ width: 1520, height: 720 })
  const workflowHeader = await page.getByTestId('management-page-header').boundingBox()
  expect(workflowHeader).not.toBeNull()
  await page.getByRole('button', { name: '帮助', exact: true }).click()

  const pageHeader = page.getByTestId('management-page-header')
  const helpLayout = page.getByTestId('help-layout')
  const gettingStartedSection = page.getByTestId('help-section-getting-started')

  await expect(pageHeader).toBeVisible()
  await expect(page.getByTestId('management-page-header-icon')).toBeVisible()
  await expect(helpLayout).toBeVisible()
  await expect(page.getByText('从第一次界面翻译到故障排查，按实际任务找到下一步。Glyphshift 不修改目标软件安装文件。', { exact: true })).toHaveCount(0)
  const geometry = await page.evaluate(() => {
    const rect = (selector: string) => {
      const box = document.querySelector<HTMLElement>(selector)?.getBoundingClientRect()
      if (!box) throw new Error(`Missing ${selector}`)
      return { left: box.left, right: box.right, top: box.top, bottom: box.bottom, width: box.width }
    }
    return {
      header: rect('[data-testid="management-page-header"]'),
      layout: rect('[data-testid="help-layout"]'),
      tabs: rect('[data-testid="help-tabs"] [role="tablist"]'),
      gettingStarted: rect('[data-testid="help-section-getting-started"]'),
    }
  })
  expect(geometry.layout.width).toBeGreaterThan(1400)
  expect(Math.abs(geometry.header.left - workflowHeader!.x)).toBeLessThanOrEqual(1)
  expect(Math.abs(geometry.header.right - (workflowHeader!.x + workflowHeader!.width))).toBeLessThanOrEqual(1)
  expect(Math.abs(geometry.header.left - geometry.layout.left)).toBeLessThanOrEqual(1)
  expect(geometry.header.right - geometry.layout.right).toBeGreaterThanOrEqual(0)
  expect(geometry.header.right - geometry.layout.right).toBeLessThanOrEqual(12)
  expect(Math.abs(geometry.layout.left - geometry.gettingStarted.left)).toBeLessThanOrEqual(1)
  expect(geometry.tabs.top - geometry.header.bottom).toBeGreaterThanOrEqual(15)
  expect(geometry.tabs.top - geometry.header.bottom).toBeLessThanOrEqual(17)
  expect(geometry.gettingStarted.top).toBeGreaterThan(geometry.tabs.bottom)

  await page.setViewportSize({ width: 960, height: 640 })
  const compactGeometry = await page.evaluate(() => {
    const header = document.querySelector<HTMLElement>('[data-testid="management-page-header"]')!.getBoundingClientRect()
    const layout = document.querySelector<HTMLElement>('[data-testid="help-layout"]')!.getBoundingClientRect()
    return { headerLeft: header.left, headerRight: header.right, layoutLeft: layout.left, layoutRight: layout.right }
  })
  expect(Math.abs(compactGeometry.headerLeft - compactGeometry.layoutLeft)).toBeLessThanOrEqual(1)
  expect(compactGeometry.headerRight - compactGeometry.layoutRight).toBeGreaterThanOrEqual(0)
  expect(compactGeometry.headerRight - compactGeometry.layoutRight).toBeLessThanOrEqual(12)
  await expect.poll(() => helpLayout.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('help teaches the workflow and progressively exposes AI, recovery, and adapter details', async ({ page }) => {
  test.slow()
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '帮助' }).click()

  await expect(page.getByRole('heading', { name: '帮助' })).toBeVisible()
  const gettingStartedHeading = page.getByRole('heading', { name: '完成第一次界面翻译' })
  await expect(gettingStartedHeading).toBeVisible()
  await expect(page.getByRole('tab', { name: '使用指南' })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('heading', { name: '新建工作流，设置软件' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '选择适配器和字典' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '操作目标界面并收集原文' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '填写并校对译文' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '在目标软件中确认效果' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '保存并启用工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '打开工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '新建工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '打开字典' })).toBeVisible()
  await expect(page.getByRole('button', { name: '创建工作流' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '翻译生效后怎么维护' })).toBeVisible()
  await expect(page.getByRole('button', { name: '查看任务' })).toBeVisible()

  await page.getByRole('tab', { name: 'AI 翻译' }).click()
  await expect(page.getByRole('heading', { name: '用 AI 补全空白译文' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '任务里的 Token 怎么看' })).toBeVisible()
  await expect(page.getByText('思考 Token', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '查看翻译任务' })).toBeVisible()

  await page.getByRole('tab', { name: '故障排查' }).click()
  await expect(page.getByRole('heading', { name: '软件未启动' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '权限不匹配' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '没有捕获到文字' })).toBeVisible()
  const recoverySection = page.getByTestId('help-section-recovery')
  await expect(recoverySection.getByRole('button', { name: '打开工作流' })).toHaveCount(2)
  await expect(page.getByRole('button', { name: '检查权限设置' })).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Glyphshift 如何组织工作' })).toHaveCount(0)

  await page.getByRole('tab', { name: '技术与兼容' }).click()
  const adapterHeading = page.getByRole('heading', { name: '当前适配器' })
  await expect(adapterHeading).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText(/适合传统 Windows 软件，支持字距、裁剪和部分特殊文字/)).toBeVisible()
  await expect(page.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(page.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(page.getByText('Unity Mono 标准界面', { exact: true })).toBeVisible()
  await expect(page.getByText('Unity IL2CPP 标准界面', { exact: true })).toBeVisible()
  await expect(page.getByText('WriteConsoleW 观察器', { exact: true })).toHaveCount(0)
  await expect(page.getByText('UI Automation 观察器', { exact: true })).toHaveCount(0)
  await expect(page.getByText('Windows', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI+', { exact: true })).toBeVisible()
  await expect(page.getByText('文字观察', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('文字替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('字体替换', { exact: true }).first()).toBeVisible()
  const adapterList = page.getByTestId('help-adapter-list')
  await expect(adapterList).toBeVisible()
  await expect.poll(() => adapterList.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  const extTextOut = page.getByTestId('help-adapter-item').filter({ hasText: 'ExtTextOutW' })
  await expect(extTextOut.getByText('v1.0.0', { exact: true })).toHaveCount(0)
  await extTextOut.getByRole('button', { name: '查看 ExtTextOutW 详情' }).click()
  const extDetails = extTextOut.getByRole('region', { name: 'ExtTextOutW 详情' })
  await expect(extDetails).toContainText('v1.0.0')
  await expect(extDetails).toContainText('无需配置')
  await expect(extTextOut.getByRole('button', { name: '查看 ExtTextOutW 技术文档' })).toBeVisible()
  await page.evaluate(() => {
    window.open = ((url?: string | URL) => {
      ;(window as unknown as { __openedAdapterDocumentation?: string }).__openedAdapterDocumentation = String(url)
      return window
    }) as typeof window.open
  })
  await extTextOut.getByRole('button', { name: '查看 ExtTextOutW 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-exttextoutw')
  const unity = page.getByTestId('help-adapter-item').filter({ hasText: 'Unity Mono 标准界面' })
  await unity.getByRole('button', { name: '查看 Unity Mono 标准界面 详情' }).click()
  await unity.getByRole('button', { name: '查看 Unity Mono 标准界面 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://docs.unity3d.com/cn/current/Manual/scripting-backends-mono.html')
  const unityIl2cpp = page.getByTestId('help-adapter-item').filter({ hasText: 'Unity IL2CPP 标准界面' })
  await unityIl2cpp.getByRole('button', { name: '查看 Unity IL2CPP 标准界面 详情' }).click()
  await unityIl2cpp.getByRole('button', { name: '查看 Unity IL2CPP 标准界面 技术文档' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAdapterDocumentation?: string }
  ).__openedAdapterDocumentation)).toBe('https://docs.unity3d.com/6000.0/Documentation/Manual/scripting-backends-il2cpp.html')
  await expect(page.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)
  await expect(page.getByText('synthetic.ext-text-out')).toHaveCount(0)

  await page.getByRole('tab', { name: '关于' }).click()
  const about = page.getByTestId('help-section-about')
  await expect(about.getByRole('heading', { name: 'Glyphshift' })).toBeVisible()
  await expect(about.getByText(`版本 v${appVersion}`, { exact: true })).toBeVisible()
  await expect(about.getByText('https://github.com/Yuelioi/glyphshift', { exact: true })).toBeVisible()
  await expect(about.getByText('https://space.bilibili.com/4279370', { exact: true })).toBeVisible()
  await page.evaluate(() => {
    window.open = ((url?: string | URL) => {
      ;(window as unknown as { __openedAboutUrl?: string }).__openedAboutUrl = String(url)
      return window
    }) as typeof window.open
  })
  await about.getByRole('button', { name: '打开GitHub 仓库' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAboutUrl?: string }
  ).__openedAboutUrl)).toBe('https://github.com/Yuelioi/glyphshift')
  await about.getByRole('button', { name: '打开哔哩哔哩主页' }).click()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __openedAboutUrl?: string }
  ).__openedAboutUrl)).toBe('https://space.bilibili.com/4279370')
  await expect.poll(() => about.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)

  await page.getByRole('tab', { name: '故障排查' }).click()
  await page.getByRole('button', { name: '检查权限设置' }).click()
  await expect(page.getByRole('heading', { name: '设置', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '帮助' }).click()
  await page.getByRole('tab', { name: '故障排查' }).click()
  await page.getByTestId('help-section-recovery').getByRole('listitem').filter({ hasText: '没有捕获到文字' }).getByRole('button', { name: '打开工作流' }).click()
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '帮助' }).click()
  await page.getByRole('tab', { name: '故障排查' }).click()
  await page.getByTestId('help-section-recovery').getByRole('listitem').filter({ hasText: '软件未启动' }).getByRole('button', { name: '打开工作流' }).click()
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()
})

test('settings applies and persists the real locale and theme preferences', async ({ page }) => {
  test.slow()
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.getByRole('button', { name: '切换到浅色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await page.getByRole('button', { name: '切换到深色主题' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)

  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
  await expect(page.getByText(/在线翻译/)).toHaveCount(0)
  await expect(page.getByText(/取词翻译/)).toHaveCount(0)
  await expect(page.getByText(/F9/)).toHaveCount(0)
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
  await expect(page.getByRole('heading', { name: 'Complete your first interface translation' })).toBeVisible()
  await expect(page.getByRole('tab', { name: 'AI translation' })).toBeVisible()
  await page.getByRole('tab', { name: 'Troubleshooting' }).click()
  await expect(page.getByRole('heading', { name: 'Solve a problem' })).toBeVisible()
  await page.getByRole('tab', { name: 'Technology & compatibility' }).click()
  await expect(page.getByRole('heading', { name: 'Adapters' })).toBeVisible()
  await expect(page.getByText(/For traditional Windows applications, including spacing, clipping, and some special text/)).toBeVisible()
  await expect(page.getByText(/For standard Unity IL2CPP TMP\/uGUI text on Windows x64/)).toBeVisible()
  await expect(page.getByText(/拦截 GDI/)).toHaveCount(0)
  await page.getByRole('button', { name: 'View details for ExtTextOutW' }).click()
  await expect(page.getByRole('button', { name: 'View technical documentation for ExtTextOutW' })).toBeVisible()
  await page.getByRole('button', { name: 'Workflows', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Workflows' })).toBeVisible()
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

  await expect(page.getByRole('heading', { name: '应用与权限' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '应用行为' })).toHaveCount(0)
  await expect(page.getByRole('heading', { name: '权限', exact: true })).toHaveCount(0)
  const launchAtStartup = page.getByRole('switch', { name: '开机自动启动' })
  await expect(launchAtStartup).not.toBeChecked()
  await expect(page.getByRole('combobox', { name: '关闭窗口时' })).toContainText('彻底退出')

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
          safetyNoticeVersion: 1,
          onboardingVersion: 1,
          localePreference: 'zh-CN',
          themePreference: 'dark',
          launchAtStartup: false,
          launchElevated: false,
          closeBehavior: 'quit',
        }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 36 }
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


test('AI add action shares its section header', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  const section = page.getByTestId('settings-section-ai')
  await expect(section.locator('header').getByRole('button', { name: '添加 AI 配置' })).toBeVisible()
  const heading = await section.getByRole('heading').boundingBox()
  const button = await section.getByRole('button', { name: '添加 AI 配置' }).boundingBox()
  expect(Math.abs(heading!.y + heading!.height / 2 - button!.y - button!.height / 2)).toBeLessThan(3)
  if (process.env.GLYPHSHIFT_HEADER_SCREENSHOT) await page.screenshot({ path: process.env.GLYPHSHIFT_HEADER_SCREENSHOT })
})


test('auto fill interval is configured in settings and survives reload', async ({ page }) => {
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  const input = page.getByRole('spinbutton', { name: '自动补全间隔（秒）' })
  await expect(input).toHaveValue('10')
  await input.fill('25')
  await input.press('Tab')
  await page.reload()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  await expect(input).toHaveValue('25')
  await input.fill('0')
  await input.press('Tab')
  await expect(input).toHaveValue('0')
  await page.reload()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByTestId('settings-tabs').getByRole('tab', { name: 'AI 配置', exact: true }).click()
  await expect(input).toHaveValue('0')
  await input.fill('60')
  await input.press('Tab')
  await expect(input).toHaveValue('60')
  await input.fill('61')
  await input.press('Tab')
  await expect(input).toHaveValue('60')
  if (process.env.GLYPHSHIFT_AUTO_SETTINGS_SCREENSHOT) await page.screenshot({ path: process.env.GLYPHSHIFT_AUTO_SETTINGS_SCREENSHOT })
})

test('help groups current features into cards at desktop and compact sizes', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: '帮助', exact: true }).click()
  await expect(page.getByRole('heading', { name: '字典导入、导出与分页' })).toBeVisible()
  await page.getByRole('tab', { name: 'AI 翻译', exact: true }).click()
  for (const width of [1280, 800]) {
    await page.setViewportSize({ width, height: 900 })
    await expect(page.getByRole('region', { name: '边玩边自动补全' })).toHaveClass(/help-card/)
    await expect(page.getByRole('heading', { name: '服务预设与翻译提示词' })).toBeVisible()
    await expect(page.getByTestId('help-layout')).toBeVisible()
    expect(await page.getByTestId('help-layout').evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true)
    await page.screenshot({ path: testInfo.outputPath(`help-${width}.png`) })
  }
  await page.getByRole('tab', { name: '故障排查', exact: true }).click()
  await expect(page.getByRole('heading', { name: '已有译文，界面却没变' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '实验适配器怎么理解' })).toBeVisible()
})


test('data settings open the native dictionary directory and allow retry', async ({ page }, testInfo) => {
  await page.addInitScript(({ snapshot }) => {
    let attempts = 0
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 36 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_probe_runs') return []
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_open_dictionary_directory') {
        ;(window as any).__directoryOpenAttempts = ++attempts
        if (attempts === 1) throw { code: 'data.open_dictionary_failed' }
      }
      return null
    } }
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '设置', exact: true }).click()
  const section = page.getByTestId('settings-section-data')
  const application = page.getByTestId('settings-section-application')
  const applicationBox = await application.boundingBox()
  const dataBox = await section.boundingBox()
  expect(dataBox!.y).toBeGreaterThan(applicationBox!.y + applicationBox!.height)
  await section.getByRole('button', { name: '打开字典文件夹', exact: true }).click()
  await expect(section.getByRole('alert')).toContainText('无法打开字典文件夹')
  await section.getByRole('button', { name: '打开字典文件夹', exact: true }).click()
  await expect(section.getByRole('alert')).toHaveCount(0)
  await expect.poll(() => page.evaluate(() => (window as any).__directoryOpenAttempts)).toBe(2)
  await section.scrollIntoViewIfNeeded()
  await page.screenshot({ path: testInfo.outputPath('settings-data.png') })
})
