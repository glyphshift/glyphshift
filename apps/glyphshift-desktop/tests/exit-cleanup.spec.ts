import { expect, test } from '@playwright/test'
import { model } from './fixtures/productModel'

for (const behavior of ['quit', 'minimize', 'tray'] as const) {
test(`${behavior} respects runtime cleanup and the window close preference`, async ({ page }) => {
  await page.addInitScript(({ snapshot, behavior }) => {
    const state = { fail: true, calls: [] as string[] }
    ;(window as any).__exitTest = state
    ;(window as any).__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } }, invoke: async (command: string) => {
      state.calls.push(command)
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', closeBehavior: behavior }
      if (['desktop_snapshot', 'desktop_refresh_workflows'].includes(command)) return snapshot
      if (command === 'desktop_probe_runs') return []
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_prepare_exit' && state.fail) throw { schemaVersion: 1, code: 'runtime.exit_stop_failed', args: {} }
      return null
    } }
  }, { snapshot: model, behavior })
  await page.goto('/')
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click()
  if (behavior !== 'quit') {
    await expect.poll(() => page.evaluate(behavior => (window as any).__exitTest.calls.includes(behavior === 'tray' ? 'desktop_hide_to_tray' : 'desktop_minimize_window'), behavior)).toBe(true)
    expect(await page.evaluate(() => (window as any).__exitTest.calls.includes('desktop_prepare_exit'))).toBe(false)
    return
  }
  await expect.poll(() => page.evaluate(() => (window as any).__exitTest.calls.includes('desktop_prepare_exit'))).toBe(true)
  expect(await page.evaluate(() => (window as any).__exitTest.calls.includes('plugin:window|close'))).toBe(false)
  await expect(page.getByRole('alert')).toContainText('未能停止')
  await page.evaluate(() => { (window as any).__exitTest.fail = false })
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__exitTest.calls.includes('plugin:window|close'))).toBe(true)
})

}

test('tray exit bypasses hide preference and still prepares runtime cleanup', async ({ page }) => {
  await page.addInitScript(snapshot => {
    const callbacks = new Map<number, (event: any) => void>()
    const listeners = new Map<string, number>()
    const state = { calls: [] as string[], emit: (name: string) => callbacks.get(listeners.get(name)!)?.({ event: name, payload: null, id: 1 }) }
    ;(window as any).__trayExit = state
    ;(window as any).__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main', windowLabel: 'main' } },
      transformCallback: (callback: (event: any) => void) => { const id = callbacks.size + 1; callbacks.set(id, callback); return id },
      unregisterCallback: () => {},
      invoke: async (command: string, args: any) => {
        state.calls.push(command)
        if (command === 'plugin:event|listen') { listeners.set(args.event, args.handler); return args.handler }
        if (command === 'desktop_status') return { shellReady: true, apiVersion: 35 }
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', closeBehavior: 'tray' }
        if (['desktop_snapshot', 'desktop_refresh_workflows'].includes(command)) return snapshot
        if (command === 'desktop_probe_runs') return []
        if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
        if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
        return null
      },
    }
  }, model)
  await page.goto('/')
  await page.getByRole('button', { name: '关闭窗口', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__trayExit.calls.includes('desktop_hide_to_tray'))).toBe(true)
  await page.evaluate(() => { (window as any).__trayExit.calls = []; (window as any).__trayExit.emit('glyphshift-request-exit') })
  await expect.poll(() => page.evaluate(() => (window as any).__trayExit.calls.includes('desktop_prepare_exit'))).toBe(true)
  await expect.poll(() => page.evaluate(() => (window as any).__trayExit.calls.includes('plugin:window|close'))).toBe(true)
  expect(await page.evaluate(() => (window as any).__trayExit.calls.includes('desktop_hide_to_tray'))).toBe(false)
})
