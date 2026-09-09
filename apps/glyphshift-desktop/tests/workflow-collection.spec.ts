import { expect, test } from '@playwright/test'
import { expandedModel, model, storageKey } from './fixtures/productModel'

test('workflow writer is selected from its dictionaries and persists', async ({ page }, testInfo) => {
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value: model, key: storageKey })
  await page.goto('/')
  await expect(page.getByRole('button', { name: '探针', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '翻译字典', exact: true }).click()
  await expect(page.getByRole('radio', { name: '设为写入字典：界面基础词典', exact: true })).toBeChecked()
  await page.getByRole('button', { name: '写入与优先级说明', exact: true }).hover()
  await expect(page.getByText(/^写入：新获取的内容/)).toBeVisible()
  await page.waitForTimeout(250)
  await page.screenshot({ path: testInfo.outputPath('dictionary-help-aligned.png') })
  await page.getByRole('button', { name: '保存工作流', exact: true }).click()
  await page.getByRole('button', { name: '返回工作流列表', exact: true }).click()
  await expect(page.getByRole('button', { name: '收集与翻译', exact: true })).toBeVisible()
})

test('workflow collection opens its owned record and uses common text controls', async ({ page }, testInfo) => {
  const snapshot = structuredClone(model)
  Object.assign(snapshot.workflows[0].targets[0], { writeDictionaryId: 'dictionary-proof' })
  await page.addInitScript(({ snapshot }) => {
    const run = { id: 'owned-record', workflowId: 'workflow-proof', name: '默认创作工作流', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: true, observationRevision: 1, observedCount: 1, ignoredCount: 0, droppedObservations: 0, previewGeneration: 1, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 1, runtimeCapability: 'direct_replace', excludedDictionaryIds: [], exclusionRevisions: [] }
    const calls: string[] = []
    ;(window as any).__collectionCalls = calls
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args?: any) => {
      calls.push(command)
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_probe_runs') return []
      if (command === 'desktop_workflow') return { ...snapshot.workflows[0], revision: 1 }
      if (command === 'desktop_set_probe_run_paused') { run.status = args.paused ? 'paused' : 'running'; return { ...run } }
      if (command === 'desktop_refresh_probe_text' && (window as any).__refreshFailure) throw { schemaVersion: 1, code: 'runtime.session_rejected', args: {} }
      if (['desktop_workflow_collection', 'desktop_probe_run_summary', 'desktop_refresh_probe_text'].includes(command)) return { ...run }
      if (command === 'desktop_disconnect_probe_run') { run.status = 'ready'; return { ...run } }
      if (command === 'desktop_resume_probe_run') { if ((window as any).__startFailure) throw { schemaVersion: 1, code: 'runtime.bundle_unavailable', args: {} }; run.status = 'running'; return { ...run } }
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      if (command === 'desktop_probe_run_entries') return { observationRevision: 1, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
      return null
    } }
  }, { snapshot })
  await page.goto('/')
  await expect(page.getByRole('button', { name: '探针', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '默认创作工作流', exact: true }).click()
  await expect(page.getByRole('heading', { name: '默认创作工作流', exact: true })).toBeVisible()
  await expect(page.locator('main')).not.toContainText('探针')
  await page.getByRole('combobox', { name: '每页' }).click()
  await expect(page.getByRole('option', { name: '20', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: '200', exact: true })).toBeVisible()
  await page.getByRole('option', { name: '200', exact: true }).click()

  await page.evaluate(() => { (window as any).__startFailure = true })
  await page.getByRole('button', { name: '开始运行', exact: true }).click()
  await expect(page.getByText('Glyphshift 的运行组件尚未就绪。请修复或重新安装后重启。', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '开始运行', exact: true })).toBeEnabled()
  await page.evaluate(() => { (window as any).__startFailure = false })
  await page.getByRole('button', { name: '开始运行', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__collectionCalls.includes('desktop_resume_probe_run'))).toBe(true)
  await page.getByRole('button', { name: '暂停收集', exact: true }).click()
  await expect(page.getByRole('button', { name: '继续收集', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '继续收集', exact: true }).click()
  await page.getByTestId('probe-task-actions').click()
  await page.getByRole('menuitem', { name: '强制刷新文字', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__collectionCalls.includes('desktop_refresh_probe_text'))).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('workflow-collection.png') })
  await page.evaluate(() => { (window as any).__refreshFailure = true })
  await page.getByTestId('probe-task-actions').click()
  await page.getByRole('menuitem', { name: '强制刷新文字', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('强制刷新未完成：当前翻译会话未接受更新')
  await page.getByRole('button', { name: '关闭提示', exact: true }).click()
  await page.getByRole('button', { name: '工作流设置', exact: true }).click()
  await expect(page.getByRole('tab', { name: '翻译字典', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '返回工作流列表', exact: true }).click()
  await expect(page.getByTestId('probe-task-actions')).toBeVisible()
  await page.getByRole('button', { name: '停止运行', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__collectionCalls.includes('desktop_disconnect_probe_run'))).toBe(true)
  await page.keyboard.press('Escape')
  await expect(page.getByTestId('workflow-management-table')).toBeVisible()
})


test('a workflow can create an ordinary dictionary as its writer', async ({ page }, testInfo) => {
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value: model, key: storageKey })
  await page.goto('/')
  await expect(page.getByRole('button', { name: '探针', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '翻译字典', exact: true }).click()
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dialog.getByRole('tab', { name: '创建新字典', exact: true }).click()
  await dialog.getByRole('textbox', { name: '字典名称', exact: true }).fill('Collected dictionary')
  await dialog.getByRole('button', { name: '创建并添加', exact: true }).click()
  await expect(dialog).not.toBeVisible()
  await expect(page.getByRole('radio', { name: '设为写入字典：Collected dictionary', exact: true })).toBeChecked()
  await page.getByRole('button', { name: '保存工作流', exact: true }).click()
  await page.screenshot({ path: testInfo.outputPath('workflow-writer.png') })
  await page.getByRole('button', { name: '返回工作流列表', exact: true }).click()
  await expect(page.getByRole('button', { name: '收集与翻译', exact: true })).toBeVisible()
})


test('selecting another software replaces the workflow target', async ({ page }, testInfo) => {
  const value = structuredClone(model)
  value.software.push({ ...value.software[0], id: 'software-second', name: 'Second Studio', executablePath: 'X:/SyntheticFixtures/SecondStudio.exe' })
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value, key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '设置软件', exact: true }).click()
  await expect(page.getByTestId('workflow-software-catalog')).toHaveCount(0)
  await page.getByTestId('workflow-current-software').click()
  const picker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await picker.getByRole('button', { name: /Second Studio/ }).click()
  await page.waitForTimeout(250)
  await page.screenshot({ path: testInfo.outputPath('software-picker.png') })
  await picker.getByRole('button', { name: '取消', exact: true }).click()
  await expect(page.getByTestId('workflow-current-software')).toContainText('Vector Studio')
  await page.getByTestId('workflow-current-software').click()
  await picker.getByRole('button', { name: /Second Studio/ }).click()
  await picker.getByRole('button', { name: '确认', exact: true }).click()
  await expect(picker).not.toBeVisible()
  await expect(page.getByTestId('workflow-current-software')).toContainText('Second Studio')
  await page.screenshot({ path: testInfo.outputPath('current-software.png') })

})


test('dictionary picker keeps drafts isolated and rows order independently of the writer', async ({ page }, testInfo) => {
  await page.addInitScript(({ value, key }) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value))
  }, { value: expandedModel(), key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '翻译字典', exact: true }).click()
  const selected = page.getByTestId('workflow-selected-dictionaries')
  await expect(selected.locator('[data-dictionary-id]')).toHaveCount(1)
  await expect(page.getByRole('combobox', { name: '写入字典', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dialog.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dialog.getByRole('checkbox', { name: '选择字典 效果词典', exact: true }).check()
  await page.screenshot({ path: testInfo.outputPath('dictionary-picker.png') })
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(selected.locator('[data-dictionary-id]')).toHaveCount(1)
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  await dialog.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dialog.getByRole('checkbox', { name: '选择字典 效果词典', exact: true }).check()
  await dialog.getByRole('button', { name: '添加所选', exact: true }).click()
  await page.getByRole('radio', { name: '设为写入字典：效果词典', exact: true }).check()
  await selected.locator('[data-dictionary-id]').nth(1).getByRole('button').first().click()
  await expect(selected.locator('[data-dictionary-id]').first()).toContainText('效果词典')
  await expect(page.getByRole('radio', { name: '设为写入字典：效果词典', exact: true })).toBeChecked()
  await page.getByRole('button', { name: '保存工作流', exact: true }).click()
  await page.screenshot({ path: testInfo.outputPath('selected-dictionaries.png') })
  await page.reload()
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '翻译字典', exact: true }).click()
  await expect(selected.locator('[data-dictionary-id]').first()).toContainText('效果词典')
  await expect(page.getByRole('radio', { name: '设为写入字典：效果词典', exact: true })).toBeChecked()
  await page.getByRole('button', { name: '移除字典：效果词典', exact: true }).click()
  await expect(page.getByRole('radio', { name: '设为写入字典：界面基础词典', exact: true })).toBeChecked()
})


test('creating a workflow goes straight to collection and open failures are visible', async ({ page }) => {
  await page.addInitScript(({ initial }) => {
    const snapshot: any = structuredClone(initial)
    const state = { fail: false, owner: '', started: false }
    ;(window as any).__entry = state
    const run = { id: 'new-record', workflowId: '', name: '新收集工作流', softwareId: 'software-proof', dictionaryId: 'dictionary-proof', adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: true, observationRevision: 0, observedCount: 0, ignoredCount: 0, droppedObservations: 0, previewGeneration: 0, createdAtMs: 1, updatedAtMs: 1, dictionaryRevision: 1, dictionaryEntryCount: 0, runtimeCapability: null, excludedDictionaryIds: [], exclusionRevisions: [] }
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'desktop_snapshot' || command === 'desktop_refresh_workflows') return snapshot
      if (command === 'desktop_preflight_software') return { executablePath: snapshot.software[0].executablePath, executableName: snapshot.software[0].executableName, suggestedName: snapshot.software[0].name, architecture: 'x86_64', running: true, canAdd: false, state: 'already_added', existingName: snapshot.software[0].name }
      if (command === 'desktop_probe_runs') return []
      if (command === 'desktop_create_workflow') {
        const created = { ...args.create, revision: 1, softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'] }
        snapshot.workflows.push(created)
        state.owner = created.id
        run.workflowId = created.id
        return snapshot
      }
      if (command === 'desktop_workflow_collection') {
        if (state.fail) throw { schemaVersion: 1, code: 'capture.preview_publish_failed', args: {} }
        if (args.workflowId !== state.owner) throw new Error('Wrong collection owner')
        return { ...run }
      }
      if (command === 'desktop_probe_run_summary') return { ...run }
      if (command === 'desktop_probe_run_entries') return { observationRevision: 0, dictionaryRevision: 1, page: 1, pageSize: 50, total: 0, rows: [] }
      if (command === 'desktop_resume_probe_run') { state.started = true; run.status = 'running'; return { ...run } }
      if (command === 'desktop_ai_profiles') return { defaultProfileId: null, profiles: [] }
      if (command === 'desktop_ai_translation_tasks') return { current: null, history: [] }
      return null
    } }
  }, { initial: model })
  await page.goto('/')
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await page.getByRole('tab', { name: /基础配置/ }).click()
  await page.getByTestId('workflow-basic-tab').getByRole('textbox').first().fill('新收集工作流')
  await page.getByRole('tab', { name: /^设置软件/ }).click()
  await page.getByTestId('workflow-current-software').click()
  await page.getByRole('dialog', { name: '设置软件', exact: true }).getByRole('button', { name: /Vector Studio/ }).click()
  await page.getByRole('dialog', { name: '设置软件', exact: true }).getByRole('button', { name: '确认', exact: true }).click()
  await page.getByTestId('workflow-adapter-select-all').check()
  await page.getByRole('tab', { name: /^翻译字典/ }).click()
  await page.getByRole('button', { name: '添加字典', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dialog.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dialog.getByRole('checkbox', { name: '选择字典 界面基础词典', exact: true }).check()
  await dialog.getByRole('button', { name: '添加所选', exact: true }).click()
  await page.getByRole('button', { name: '创建工作流', exact: true }).click()
  await expect(page.getByRole('heading', { name: '新收集工作流', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '开始运行', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__entry.started)).toBe(true)
  await page.getByRole('button', { name: '工作流', exact: true }).first().click()
  await page.evaluate(() => { (window as any).__entry.fail = true })
  await page.getByRole('button', { name: '新收集工作流', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('实时更新没有发送到目标软件')
  await page.evaluate(() => { (window as any).__entry.fail = false })
  await page.getByRole('button', { name: '新收集工作流', exact: true }).click()
  await expect(page.getByTestId('probe-task-actions')).toBeVisible()
})

test('a workflow without a writer keeps a visible entry and explains how to enable collection', async ({ page }) => {
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value: model, key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '收集与翻译', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('指定写入字典并保存')
  await expect(page.getByRole('tab', { name: /^翻译字典/ })).toBeVisible()
})


test('workflow font modes persist dictionary inheritance and original fonts', async ({ page }, testInfo) => {
  await page.addInitScript(({ value, key }) => {
    if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value))
  }, { value: model, key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '字体策略', exact: true }).click()
  const mode = page.getByRole('combobox', { name: '字体策略', exact: true })
  await mode.click()
  await page.getByRole('option', { name: '优先使用字典字体', exact: true }).click()
  await page.getByRole('spinbutton', { name: '默认字号', exact: true }).fill('150')
  await page.getByRole('button', { name: '保存工作流', exact: true }).click()
  await page.reload()
  await page.getByRole('button', { name: '编辑 默认创作工作流', exact: true }).click()
  await page.getByRole('tab', { name: '字体策略', exact: true }).click()
  await expect(mode).toContainText('优先使用字典字体')
  await expect(page.getByRole('spinbutton', { name: '默认字号', exact: true })).toHaveValue('150')
  await page.screenshot({ path: testInfo.outputPath('workflow-fonts.png') })
  await mode.click()
  await page.getByRole('option', { name: '保留软件原字体', exact: true }).click()
  await expect(page.getByTestId('workflow-font-catalog')).toHaveCount(0)
  await page.getByRole('button', { name: '保存工作流', exact: true }).click()
})


test('workflow actions fit their column at desktop review width', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1100, height: 760 })
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value: model, key: storageKey })
  await page.goto('/')
  const action = page.getByRole('button', { name: '收集与翻译', exact: true })
  await expect(action).toBeVisible()
  const fits = await action.evaluate(el => {
    const cell = el.closest('td')!.getBoundingClientRect()
    const rect = el.getBoundingClientRect()
    return rect.left >= cell.left && rect.right <= cell.right
  })
  expect(fits).toBe(true)
  const narrowWidth = await action.evaluate(el => el.closest('td')!.getBoundingClientRect().width)
  await page.setViewportSize({ width: 1600, height: 760 })
  const wideWidth = await action.evaluate(el => el.closest('td')!.getBoundingClientRect().width)
  expect(Math.abs(wideWidth - narrowWidth)).toBeLessThan(4)
  await page.setViewportSize({ width: 1100, height: 760 })
  await page.screenshot({ path: testInfo.outputPath('workflow-list.png') })
  await page.getByText('字典：1 个', { exact: true }).hover()
  await expect(page.locator('[data-slot="text"]').filter({ hasText: '界面基础词典' })).toBeVisible()
})


test('recent software shows its path without placeholder metadata', async ({ page }) => {
  const value = structuredClone(model)
  value.software[0]!.vendor = '—'
  value.software[0]!.version = '-'
  await page.addInitScript(({ value, key }) => localStorage.setItem(key, JSON.stringify(value)), { value, key: storageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '新建工作流', exact: true }).click()
  await page.getByTestId('workflow-current-software').click()
  const entry = page.getByTestId('workflow-software-catalog').getByRole('button').first()
  await expect(entry).toContainText('SyntheticFixtures')
  await expect(entry).not.toContainText('—')
})
