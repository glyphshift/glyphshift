import { expect, test } from '@playwright/test'
import { expandedModel, largeWorkflowCatalogModel, model, replaceModel, storageKey, workflowEditor } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})
test('running workflow opens a bounded local decision diagnostics table', async ({ page }) => {
  const active = JSON.parse(JSON.stringify(model))
  active.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  active.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: true, active: true,
        translationRequested: true, fontRequested: true,
        translationActive: true, fontActive: true, appliedGeneration: 4,
      }],
      errors: {},
    },
  }
  await page.addInitScript(({ snapshot }) => {
    const controls: boolean[] = []
    const internals = {
      invoke: async (command: string, args?: Record<string, unknown>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, safetyNoticeVersion: 1, onboardingVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.5.2', apiVersion: 38 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_control_workflow_diagnostics') {
          controls.push(Boolean(args?.enabled))
          ;(window as unknown as { __diagnosticControls: boolean[] }).__diagnosticControls = controls
          return null
        }
        if (command === 'desktop_workflow_diagnostics') {
          return {
            workflowId: 'workflow-proof',
            records: [{
              softwareId: 'software-proof', softwareName: 'Vector Studio', adapterName: 'ExtTextOutW',
              sourceText: 'Open', status: 'matched', text: 'replaced', font: 'protected', generation: 4,
              publicationIdentity: '71'.repeat(32), translationDigest: '72'.repeat(32), fontPolicyDigest: '73'.repeat(32),
            }],
            dropped: 2,
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: active })
  await replaceModel(page, active)

  await page.getByRole('button', { name: '更多操作：默认创作工作流' }).click()
  await page.getByRole('menuitem', { name: '诊断 默认创作工作流' }).click()
  const dialog = page.getByRole('dialog', { name: '翻译诊断 · 默认创作工作流' })
  await expect(dialog.getByText(/查看最近匹配到的原文和字典/)).toBeVisible()
  await expect(dialog.getByRole('columnheader', { name: '原文' })).toBeVisible()
  await expect(dialog.getByText('Open', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(dialog.getByText('传统 Windows 文字（高级）', { exact: true })).toBeVisible()
  await expect(dialog.getByText('已准备替换文字', { exact: true })).toBeVisible()
  await expect(dialog).toContainText('最终效果仍以目标软件为准')
  await expect(dialog.getByText('字体保持原样', { exact: true })).toBeVisible()
  await expect(dialog.getByText('G4 · 71717171')).toHaveCount(0)
  await expect(dialog.getByText('有 2 条较早记录未显示')).toBeVisible()
  await expect(dialog.getByText('synthetic.ext-text-out')).toHaveCount(0)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/runtime-diagnostics-modal.png' })
  await dialog.getByRole('button', { name: '关闭诊断' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __diagnosticControls?: boolean[] }).__diagnosticControls
  ))).toEqual([true, false])
})

test('workflow configures adapters dictionaries and one font policy for its software target', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await expect(dialog.getByRole('tab')).toHaveCount(4)
  await expect(dialog.getByRole('tab', { name: '设置软件' })).toHaveAttribute('aria-selected', 'true')

  await dialog.getByRole('tab', { name: '设置软件' }).click()
  const extTextOutRow = dialog.getByTestId('workflow-adapter-table').locator('tbody > tr').filter({ hasText: '传统 Windows 文字（高级）' })
  await expect(extTextOutRow.getByText(/支持字距、裁剪和部分特殊文字/)).toBeVisible()
  await expect(dialog.getByText('传统 Windows 文字（高级）', { exact: true })).toBeVisible()
  await expect(dialog.getByText('传统 Windows 文字（基础）', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Windows 按钮与标签', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Windows 自绘图形界面', { exact: true })).toBeVisible()
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('WriteConsoleW 观察器', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('UI Automation 观察器', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)

  await dialog.getByRole('tab', { name: '翻译字典' }).click()
  const selectedDictionaries = dialog.getByTestId('workflow-selected-dictionaries')
  await expect(selectedDictionaries.locator('[data-dictionary-id]')).toHaveCount(1)
  await expect(selectedDictionaries.getByText('界面基础词典', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '提高 界面基础词典 的优先级' })).toBeDisabled()

  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('combobox', { name: '字体策略' })).toContainText('使用工作流配置')
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅已翻译文字')
  await expect(dialog.getByPlaceholder('搜索本机字体')).toBeVisible()
  await expect(dialog.getByText(/位置|main-ui/)).toHaveCount(0)
})

test('workflow adapter catalog is one table with classification columns and select all', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '设置软件' }).click()

  const adapterTable = dialog.getByTestId('workflow-adapter-table')
  await expect(adapterTable.getByRole('table')).toHaveCount(1)
  await expect(adapterTable.locator('tbody > tr')).toHaveCount(model.adapters.length)
  await expect(adapterTable.getByRole('columnheader', { name: '适配器', exact: true })).toBeVisible()
  await expect(adapterTable.getByRole('columnheader', { name: '平台', exact: true })).toHaveCount(0)
  await expect(adapterTable.getByRole('columnheader', { name: '技术分类', exact: true })).toHaveCount(0)
  await expect(adapterTable.getByText('windows · GDI', { exact: true })).toHaveCount(0)
  await expect.poll(() => adapterTable.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)

  const adapterCheckboxes = adapterTable.getByRole('checkbox', { name: /^选择适配器 / })
  const checkedAdapterCheckboxes = adapterTable.getByRole('checkbox', { name: /^选择适配器 /, checked: true })
  await expect(adapterCheckboxes).toHaveCount(model.adapters.length)
  await expect(checkedAdapterCheckboxes).toHaveCount(1)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/workflow-adapter-table-960.png' })
  await adapterTable.getByText('全选', { exact: true }).click()
  await expect(checkedAdapterCheckboxes).toHaveCount(model.adapters.length)
  await adapterTable.getByText('取消全选', { exact: true }).click()
  await expect(checkedAdapterCheckboxes).toHaveCount(0)
})

test('disabled plugin adapter stays visible only while an existing workflow still references it', async ({ page }) => {
  const snapshot = structuredClone(model)
  Object.assign(snapshot.adapters[0], { availableForNewUsage: false, restartRequired: true })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '设置软件' }).click()
  const adapterTable = dialog.getByTestId('workflow-adapter-table')
  const unavailableRow = adapterTable.locator('tbody > tr').filter({ hasText: '传统 Windows 文字（高级）' })

  await expect(unavailableRow.getByText('不可用', { exact: true })).toBeVisible()
  await expect(unavailableRow.getByText(/当前 App 不再允许新增使用/)).toBeVisible()
  await expect(dialog.getByText('工作流引用了当前不可用的适配器', { exact: true })).toBeVisible()
  const checkbox = unavailableRow.getByRole('checkbox', { name: '选择适配器 传统 Windows 文字（高级）' })
  await expect(checkbox).toBeChecked()
  await checkbox.click()

  await expect(unavailableRow).toHaveCount(0)
  await expect(dialog.getByText('工作流引用了当前不可用的适配器', { exact: true })).toHaveCount(0)
})

test('missing adapter reference remains removable after restart', async ({ page }) => {
  const snapshot = structuredClone(model)
  snapshot.adapters = snapshot.adapters.filter(adapter => adapter.id !== 'synthetic.ext-text-out')
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '设置软件' }).click()
  const adapterTable = dialog.getByTestId('workflow-adapter-table')
  const missingRow = adapterTable.locator('tbody > tr').filter({ hasText: 'synthetic.ext-text-out' })

  await expect(missingRow).toBeVisible()
  await expect(missingRow.getByText('不可用', { exact: true })).toBeVisible()
  await expect(missingRow).toContainText('当前启动没有加载它')
  await expect(dialog.getByText('工作流引用了当前不可用的适配器', { exact: true })).toBeVisible()
  const checkbox = missingRow.getByRole('checkbox', { name: '选择适配器 synthetic.ext-text-out' })
  await expect(checkbox).toBeChecked()
  await checkbox.click()
  await expect(missingRow).toHaveCount(0)
})

test('new workflow does not offer adapters disabled by plugin selection', async ({ page }) => {
  const snapshot = structuredClone(model)
  Object.assign(snapshot.adapters[0], { availableForNewUsage: false, restartRequired: true })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '新建工作流' }).click()
  const softwarePicker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await softwarePicker.getByRole('button', { name: /Vector Studio/ }).click()
  await softwarePicker.getByRole('button', { name: '确认', exact: true }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '设置软件' }).click()

  await expect(dialog.getByTestId('workflow-adapter-table').getByText('传统 Windows 文字（高级）', { exact: true })).toHaveCount(0)
  await expect(dialog.getByRole('checkbox', { name: /^选择适配器 /, checked: true })).toHaveCount(model.adapters.length - 1)
})

test('new workflow selects a searched font by clicking its visible row', async ({ page }) => {
  await page.getByRole('button', { name: '新建工作流' }).click()
  const softwarePicker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await softwarePicker.getByRole('button', { name: /Vector Studio/ }).click()
  await softwarePicker.getByRole('button', { name: '确认', exact: true }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await dialog.getByRole('combobox', { name: '字体策略' }).click()
  await page.getByRole('option', { name: '使用工作流配置' }).click()
  await dialog.getByPlaceholder('搜索本机字体').fill('Mono')

  const fontRow = dialog.getByTestId('workflow-font-catalog').locator('[data-font-family="Synthetic Mono"]')
  await fontRow.getByText('Synthetic Mono', { exact: true }).click()

  await expect(fontRow.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).toBeChecked()
  const selectedFonts = dialog.getByTestId('workflow-selected-fonts')
  await expect(selectedFonts.locator('[data-selected-font-family="Synthetic Mono"]')).toBeVisible()
  await fontRow.getByText('Synthetic Mono', { exact: true }).click()
  await expect(fontRow.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).not.toBeChecked()
  await expect(selectedFonts.locator('[data-selected-font-family="Synthetic Mono"]')).toHaveCount(0)
  await fontRow.getByText('Synthetic Mono', { exact: true }).click()

  await dialog.getByRole('tab', { name: '设置软件' }).click()
  await dialog.getByRole('checkbox', { name: '选择适配器 传统 Windows 文字（高级）' }).click()
  await dialog.getByRole('tab', { name: '翻译字典' }).click()
  const dictionaryPicker = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dictionaryPicker.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dictionaryPicker.getByRole('checkbox', { name: '选择字典 界面基础词典', exact: true }).check()
  await dictionaryPicker.getByRole('button', { name: '添加所选', exact: true }).click()
  await dialog.getByRole('tab', { name: '基础配置' }).click()
  await dialog.getByTestId('workflow-basic-tab').locator('input').fill('字体选择工作流')
  await dialog.getByRole('button', { name: '创建工作流' }).click()

  await page.getByRole('button', { name: '编辑 字体选择工作流' }).click()
  const savedDialog = workflowEditor(page)
  await savedDialog.getByRole('tab', { name: '字体策略' }).click()
  await savedDialog.getByPlaceholder('搜索本机字体').fill('Mono')
  await expect(savedDialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).toBeChecked()
})

test('font catalog keeps its source order while selected fonts use separate priority tags', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '新建工作流' }).click()
  const softwarePicker = page.getByRole('dialog', { name: '设置软件', exact: true })
  await softwarePicker.getByRole('button', { name: /Vector Studio/ }).click()
  await softwarePicker.getByRole('button', { name: '确认', exact: true }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await dialog.getByRole('combobox', { name: '字体策略' }).click()
  await page.getByRole('option', { name: '使用工作流配置' }).click()

  const catalog = dialog.getByTestId('workflow-font-catalog')
  const catalogRows = catalog.locator('[data-font-family]')
  await expect(catalogRows.nth(0)).toHaveAttribute('data-font-family', 'Synthetic Sans')
  await expect(catalogRows.nth(1)).toHaveAttribute('data-font-family', 'Synthetic Serif')
  await expect(catalogRows.nth(2)).toHaveAttribute('data-font-family', 'Synthetic Mono')

  await catalog.locator('[data-font-family="Synthetic Mono"]').getByText('Synthetic Mono', { exact: true }).click()
  await expect(catalogRows.nth(0)).toHaveAttribute('data-font-family', 'Synthetic Sans')
  await expect(catalogRows.nth(1)).toHaveAttribute('data-font-family', 'Synthetic Serif')
  await expect(catalogRows.nth(2)).toHaveAttribute('data-font-family', 'Synthetic Mono')

  const selectedFonts = dialog.getByTestId('workflow-selected-fonts')
  const selectedTags = selectedFonts.locator('[data-selected-font-family]')
  await expect(selectedTags).toHaveCount(1)
  await expect(selectedTags.nth(0)).toHaveAttribute('data-selected-font-family', 'Synthetic Mono')

  await catalog.locator('[data-font-family="Synthetic Sans"]').getByText('Synthetic Sans', { exact: true }).click()
  await expect(catalogRows.nth(0)).toHaveAttribute('data-font-family', 'Synthetic Sans')
  await expect(catalogRows.nth(1)).toHaveAttribute('data-font-family', 'Synthetic Serif')
  await expect(catalogRows.nth(2)).toHaveAttribute('data-font-family', 'Synthetic Mono')
  await expect(selectedTags.nth(0)).toHaveAttribute('data-selected-font-family', 'Synthetic Mono')
  await expect(selectedTags.nth(1)).toHaveAttribute('data-selected-font-family', 'Synthetic Sans')
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/workflow-selected-font-tags-960.png' })

  await selectedFonts.getByRole('button', { name: '提高 Synthetic Sans 的优先级' }).click()
  await expect(selectedTags.nth(0)).toHaveAttribute('data-selected-font-family', 'Synthetic Sans')
  await expect(selectedTags.nth(1)).toHaveAttribute('data-selected-font-family', 'Synthetic Mono')
  await selectedFonts.getByRole('button', { name: '移除已选字体 Synthetic Mono' }).click()
  await expect(selectedTags).toHaveCount(1)
  await expect(catalog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).not.toBeChecked()
})

test('font policy uses a compact coverage selector and an explicit cached refresh', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()

  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅已翻译文字')
  await expect(dialog.getByRole('button', { name: '仅已翻译文字' })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: '探针找到的全部文字' })).toHaveCount(0)
  await expect(dialog.getByText('已找到 3 种字体', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '刷新字体列表' })).toBeVisible()
})

test('workflow editor uses a left section rail while large catalogs stay in focused pickers', async ({ page }) => {
  await page.setViewportSize({ width: 1180, height: 760 })
  await replaceModel(page, largeWorkflowCatalogModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '基础配置' }).click()

  const nameBox = await dialog.getByRole('textbox', { name: '工作流名称' }).boundingBox()
  const descriptionBox = await dialog.getByRole('textbox', { name: '工作流描述' }).boundingBox()
  expect(nameBox).not.toBeNull()
  expect(descriptionBox).not.toBeNull()
  expect(descriptionBox!.y).toBeGreaterThan(nameBox!.y + nameBox!.height)

  await expect(dialog).toHaveClass(/flex-1/)
  await page.waitForTimeout(250)
  const initialEditorHeight = await dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))
  const tabList = dialog.getByRole('tablist')
  await expect(dialog.getByTestId('workflow-editor-tabs')).toHaveClass(/h-full/)
  await expect.poll(() => tabList.evaluate(element => getComputedStyle(element).flexDirection)).toBe('column')
  const navigationBox = await tabList.boundingBox()
  const basicContentBox = await dialog.getByTestId('workflow-basic-tab').boundingBox()
  expect(navigationBox).not.toBeNull()
  expect(basicContentBox).not.toBeNull()
  expect(navigationBox!.x + navigationBox!.width).toBeLessThanOrEqual(basicContentBox!.x)
  await expect.poll(() => tabList.evaluate(element => element.scrollWidth === element.clientWidth && element.scrollHeight === element.clientHeight)).toBe(true)
  await dialog.getByRole('tab', { name: '设置软件' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  await expect(dialog.getByTestId('workflow-software-catalog')).toHaveCount(0)
  await dialog.getByTestId('workflow-current-software').click()
  const softwarePicker = page.getByRole('dialog', { name: '设置软件', exact: true })
  const softwareCatalog = softwarePicker.getByTestId('workflow-software-catalog')
  await expect.poll(() => softwareCatalog.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
  await softwarePicker.getByPlaceholder('搜索软件').fill('Batch Studio 10')
  await expect(softwareCatalog.getByText('Batch Studio 10', { exact: true })).toBeVisible()
  await softwarePicker.getByRole('button', { name: '取消', exact: true }).click()
  await expect(dialog.getByTestId('workflow-adapter-config')).toBeVisible()

  await dialog.getByRole('tab', { name: '翻译字典' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  await expect(dialog.getByTestId('workflow-selected-dictionaries').locator('[data-dictionary-id]')).toHaveCount(1)
  await dialog.getByRole('button', { name: '添加字典', exact: true }).click()
  const dictionaryPicker = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dictionaryPicker.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dictionaryPicker.getByPlaceholder('搜索字典').fill('批量词典 100')
  await expect(dictionaryPicker.getByRole('checkbox', { name: '选择字典 批量词典 100', exact: true })).toBeVisible()
  await dictionaryPicker.getByRole('button', { name: '取消', exact: true }).click()

  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  await expect(dialog.getByRole('tab', { name: '字体策略' })).toHaveAttribute('aria-selected', 'true')
  await expect(dialog.getByTestId('workflow-font-tab')).toBeVisible()
  await expect(dialog.getByTestId('workflow-dictionary-catalog')).toHaveCount(0)
  await page.waitForTimeout(200)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/workflow-editor-tabbed-large-catalogs.png' })
})

test('workflow saves reordered dictionaries in explicit priority order', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '翻译字典' }).click()
  await dialog.getByRole('button', { name: '添加字典', exact: true }).click()
  const dictionaryPicker = page.getByRole('dialog', { name: '添加字典', exact: true })
  await dictionaryPicker.getByRole('tab', { name: '已有字典', exact: true }).click()
  await dictionaryPicker.getByRole('checkbox', { name: '选择字典 效果词典', exact: true }).check()
  await dictionaryPicker.getByRole('button', { name: '添加所选', exact: true }).click()
  await dialog.getByRole('button', { name: '提高 效果词典 的优先级' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await expect(dialog.getByRole('tab', { name: '翻译字典' })).toHaveAttribute('aria-selected', 'true')
  const priorityItems = dialog.getByTestId('workflow-selected-dictionaries').locator('[data-dictionary-id]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-dictionary-id', 'dictionary-effects')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-dictionary-id', 'dictionary-proof')
})

test('workflow saves font coverage and ordered inline candidates', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toHaveCount(0)
  await dialog.getByRole('combobox', { name: '应用范围' }).click()
  await page.getByRole('option', { name: '工作流找到的全部文字' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toContainText('未翻译文字、图标或符号')
  await dialog.getByRole('button', { name: '提高 Synthetic Serif 的优先级' }).click()
  await dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await expect(dialog.getByRole('tab', { name: '字体策略' })).toHaveAttribute('aria-selected', 'true')
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('工作流找到的全部文字')
  const priorityItems = dialog.getByTestId('workflow-selected-fonts').locator('[data-selected-font-family]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-selected-font-family', 'Synthetic Serif')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-selected-font-family', 'Synthetic Sans')
  await expect(priorityItems.nth(2)).toHaveAttribute('data-selected-font-family', 'Synthetic Mono')
})

test('saving fonts keeps the editor section and allows another save', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const editor = workflowEditor(page)
  await editor.getByRole('tab', { name: '字体策略' }).click()
  const search = editor.getByPlaceholder('搜索本机字体')
  await search.fill('Mono')
  await editor.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).check()
  await editor.getByRole('button', { name: '保存工作流' }).click()
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3')!).workflowDetails['workflow-proof'].revision)).toBe(6)
  await expect(editor.getByRole('tab', { name: '字体策略' })).toHaveAttribute('aria-selected', 'true')
  await expect(search).toHaveValue('Mono')
  await expect(editor.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).toBeChecked()
  await search.fill('Sans')
  await editor.getByRole('checkbox', { name: '选择字体 Synthetic Sans' }).uncheck()
  await editor.getByRole('button', { name: '保存工作流' }).click()
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3')!).workflowDetails['workflow-proof'].revision)).toBe(7)
  await expect(editor).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(editor).toHaveCount(0)
  await expect(page.getByRole('dialog', { name: '放弃未保存更改？' })).toHaveCount(0)
})

test('saved browser model contains inline policy without font assets or locations', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await workflowEditor(page).getByRole('button', { name: '保存工作流' }).click()
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3') ?? '{}'))
  expect(saved.fontProfiles).toBeUndefined()
  expect(saved.software[0].locations).toBeUndefined()
  expect(saved.workflowDetails['workflow-proof'].targets[0].fontPolicy).toEqual({
    families: ['Synthetic Sans', 'Synthetic Serif'],
    coverage: 'dictionary_matches',
  })
})
