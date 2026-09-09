import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => { if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value)) }, { key: storageKey, value: model })
  await page.goto('/')
})

test('software is configured first, names follow selection but preserve manual edits, and history clears independently', async ({ page }, testInfo) => {
  await expect(page.getByRole('button', { name: '软件', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await expect(page.getByRole('tab', { name: /设置软件/ })).toHaveAttribute('data-state', 'active')
  await page.getByTestId('workflow-current-software').click()
  let picker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await picker.getByRole('button', { name: /Vector Studio/ }).click()
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  await page.getByRole('tab', { name: /基础配置/ }).click()
  await expect(page.getByRole('textbox', { name: '工作流名称' })).toHaveValue('Vector Studio')
  await page.getByRole('textbox', { name: '工作流名称' }).fill('我的翻译')
  await page.getByRole('tab', { name: /设置软件/ }).click()
  await page.getByTestId('workflow-current-software').click()
  picker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await picker.getByRole('textbox', { name: '程序路径' }).fill('X:/SyntheticFixtures/NewEditor.exe')
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  await page.getByRole('tab', { name: /基础配置/ }).click()
  await expect(page.getByRole('textbox', { name: '工作流名称' })).toHaveValue('我的翻译')
  await page.getByRole('tab', { name: /设置软件/ }).click()
  await page.getByTestId('workflow-current-software').click()
  await expect(picker.getByTestId('workflow-software-catalog').getByRole('button').first()).toContainText('NewEditor')
  await page.waitForTimeout(300)
  await page.screenshot({ path: testInfo.outputPath('software-picker.png') })
  await picker.getByRole('button', { name: '清空记录', exact: true }).click()
  await expect(picker.getByTestId('workflow-software-catalog').getByRole('button')).toHaveCount(0)
  await picker.getByRole('button', { name: '取消', exact: true }).click()
  await expect(page.getByTestId('workflow-current-software')).toContainText('NewEditor')
  await page.reload()
  await expect(page.getByRole('button', { name: '默认创作工作流', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await page.getByTestId('workflow-current-software').click()
  await expect(picker.getByTestId('workflow-software-catalog').getByRole('button')).toHaveCount(0)
})

test('create dictionary is first and selected by default; existing dictionaries remain available', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await page.getByTestId('workflow-current-software').click()
  const picker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await picker.getByRole('textbox', { name: '程序路径' }).fill('X:/SyntheticFixtures/FreshEditor.exe')
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.getByTestId('workflow-current-software')).toContainText('FreshEditor')
  await page.getByRole('tab', { name: /字典/ }).click()
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '添加字典', exact: true })
  await expect(dialog.getByRole('tab').first()).toHaveText('创建新字典')
  await expect(dialog.getByRole('tab').first()).toHaveAttribute('data-state', 'active')
  await expect(dialog.getByRole('textbox', { name: '字典名称' })).toHaveValue('FreshEditor 字典')
  await page.waitForTimeout(300)
  await page.screenshot({ path: testInfo.outputPath('dictionary-picker.png') })
  await dialog.getByRole('tab', { name: '已有字典' }).click()
  await expect(dialog).toContainText('界面基础词典')
  await dialog.getByRole('tab', { name: '创建新字典' }).click()
  await dialog.getByRole('button', { name: '创建并添加', exact: true }).click()
  await expect(page.getByTestId('workflow-selected-dictionaries')).toContainText('FreshEditor 字典')
  await expect(page.getByRole('radio')).toBeChecked()
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  await expect(dialog.getByRole('tab', { name: '已有字典', exact: true })).toHaveAttribute('data-state', 'active')
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await page.getByRole('tab', { name: /设置软件/ }).click()
  await page.getByTestId('workflow-adapter-config').getByRole('checkbox').nth(1).click()
  await page.getByRole('button', { name: '创建工作流', exact: true }).click()
  await expect(page.getByRole('button', { name: 'FreshEditor', exact: true })).toBeVisible()
  const saved = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!), storageKey)
  expect(saved.workflows.find((item: any) => item.name === 'FreshEditor').targets[0].writeDictionaryId).toBeTruthy()
})


test('running application and shortcut capture reuse the same binding without leaving the workflow', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const selected = snapshot.software[0]
    const preflight = { executablePath: selected.executablePath, executableName: selected.executableName, suggestedName: selected.name, architecture: 'x86_64', running: true, canAdd: false, state: 'already_added', existingName: selected.name }
    const calls: string[] = []
    ;(window as any).__softwareCalls = calls
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string) => {
      calls.push(command)
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_running_software_targets') return [preflight]
      if (command === 'desktop_preflight_software') return preflight
      if (command === 'desktop_arm_software_capture') return 'Ctrl+Shift+F8'
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_probe_runs') return []
      return null
    } }
    ;(window as any).__captured = preflight
  }, { snapshot: model })
  await page.reload()
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await page.getByTestId('workflow-current-software').click()
  const picker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await picker.getByRole('tab', { name: '运行中软件', exact: true }).click()
  await picker.getByRole('button', { name: /Vector Studio/ }).click()
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  await expect(page.getByTestId('workflow-current-software')).toContainText('Vector Studio')
  await page.getByTestId('workflow-current-software').click()
  await picker.getByRole('button', { name: '按键捕获', exact: true }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', { detail: { state: 'captured', shortcut: 'Ctrl+Shift+F8', preflight: (window as any).__captured } })))
  await expect(picker.getByRole('textbox', { name: '程序路径' })).toHaveValue(model.software[0].executablePath)
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  expect(await page.evaluate(() => (window as any).__softwareCalls.includes('desktop_add_software'))).toBe(false)
})
