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
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
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

  await page.getByRole('button', { name: '诊断 默认创作工作流' }).click()
  const dialog = page.getByRole('dialog', { name: '运行诊断 · 默认创作工作流' })
  await expect(dialog.getByText('显示目标 Runtime 最近生成的决策')).toBeVisible()
  await expect(dialog.getByRole('columnheader', { name: '原文' })).toBeVisible()
  await expect(dialog.getByText('Open', { exact: true })).toBeVisible()
  await expect(dialog.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('替换决策已生成', { exact: true })).toBeVisible()
  await expect(dialog).toContainText('不代表目标界面已显示译文')
  await expect(dialog.getByText('字体保持原样', { exact: true })).toBeVisible()
  await expect(dialog.getByText('G4 · 71717171')).toBeVisible()
  await expect(dialog.getByText('2 条诊断因缓冲限制被丢弃')).toBeVisible()
  await expect(dialog.getByText('synthetic.ext-text-out')).toHaveCount(0)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/runtime-diagnostics-modal.png' })
  await dialog.getByRole('button', { name: '关闭诊断' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __diagnosticControls?: boolean[] }).__diagnosticControls
  ))).toEqual([true, false])
})

test('workflow target independently selects adapters dictionaries and one font policy', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await expect(dialog.getByRole('tab')).toHaveCount(4)
  await expect(dialog.getByRole('tab', { name: '基础配置' })).toHaveAttribute('aria-selected', 'true')

  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  const extTextOutRow = dialog.getByTestId('workflow-adapter-table').locator('tbody > tr').filter({ hasText: 'ExtTextOutW' })
  await expect(extTextOutRow.getByText('Windows', { exact: true })).toBeVisible()
  await expect(extTextOutRow.getByText('GDI', { exact: true })).toBeVisible()
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(dialog.getByText('WriteConsoleW 观察器', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('UI Automation 观察器', { exact: true })).toHaveCount(0)
  await expect(dialog.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)

  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await expect(dialog.getByText('以下词典只应用于 Vector Studio')).toBeVisible()
  await expect(dialog.getByText('界面基础词典', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '提高 界面基础词典 的优先级' })).toBeDisabled()

  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).toBeChecked()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByPlaceholder('搜索本机字体')).toBeVisible()
  await expect(dialog.getByText(/位置|main-ui/)).toHaveCount(0)
})

test('workflow adapter catalog is one table with classification columns and select all', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '软件与拦截' }).click()

  const adapterTable = dialog.getByTestId('workflow-adapter-table')
  await expect(adapterTable.getByRole('table')).toHaveCount(1)
  await expect(adapterTable.locator('tbody > tr')).toHaveCount(model.adapters.length)
  await expect(adapterTable.getByRole('columnheader', { name: '拦截方式', exact: true })).toBeVisible()
  await expect(adapterTable.getByRole('columnheader', { name: '平台', exact: true })).toBeVisible()
  await expect(adapterTable.getByRole('columnheader', { name: '技术分类', exact: true })).toBeVisible()
  await expect(adapterTable.getByText('windows · GDI', { exact: true })).toHaveCount(0)
  await expect.poll(() => adapterTable.evaluate(element => element.scrollWidth >= element.clientWidth)).toBe(true)

  const adapterCheckboxes = adapterTable.getByRole('checkbox', { name: /^选择拦截方式 / })
  const checkedAdapterCheckboxes = adapterTable.getByRole('checkbox', { name: /^选择拦截方式 /, checked: true })
  await expect(adapterCheckboxes).toHaveCount(model.adapters.length)
  await expect(checkedAdapterCheckboxes).toHaveCount(1)
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/workflow-adapter-table-960.png' })
  await adapterTable.getByText('全选', { exact: true }).click()
  await expect(checkedAdapterCheckboxes).toHaveCount(model.adapters.length)
  await adapterTable.getByText('取消全选', { exact: true }).click()
  await expect(checkedAdapterCheckboxes).toHaveCount(0)
})

test('new workflow selects a searched font by clicking its visible row', async ({ page }) => {
  await page.getByRole('button', { name: '新建工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await dialog.getByRole('button', { name: '添加 Vector Studio' }).click()
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await dialog.getByRole('switch', { name: '启用字体策略' }).click()
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

  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await dialog.getByRole('checkbox', { name: '选择拦截方式 ExtTextOutW' }).click()
  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await dialog.getByRole('checkbox', { name: '选择词典 界面基础词典' }).click()
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
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await dialog.getByRole('button', { name: '添加 Vector Studio' }).click()
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await dialog.getByRole('switch', { name: '启用字体策略' }).click()

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

  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByRole('button', { name: '仅词典命中' })).toHaveCount(0)
  await expect(dialog.getByRole('button', { name: 'Hook 捕获的全部文字' })).toHaveCount(0)
  await expect(dialog.getByText('已缓存 3 种字体', { exact: true })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '刷新字体列表' })).toBeVisible()
})

test('workflow editor uses a left section rail for four focused large-catalog views', async ({ page }) => {
  await page.setViewportSize({ width: 1180, height: 760 })
  await replaceModel(page, largeWorkflowCatalogModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)

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
  await dialog.getByRole('tab', { name: '软件与拦截' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  const softwareCatalog = dialog.getByTestId('workflow-software-catalog')
  await expect.poll(() => softwareCatalog.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
  await dialog.getByPlaceholder('搜索软件').fill('Batch Studio 10')
  await expect(softwareCatalog.getByText('Batch Studio 10', { exact: true })).toBeVisible()
  await expect(dialog.getByTestId('workflow-adapter-config')).toBeVisible()

  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await expect.poll(() => dialog.evaluate(element => Math.round(element.getBoundingClientRect().height))).toBe(initialEditorHeight)
  const targetSelect = dialog.getByRole('button', { name: '当前软件目标' })
  await targetSelect.click()
  await expect(page.getByRole('option')).toHaveCount(10)
  await page.keyboard.press('Escape')
  const dictionaryCatalog = dialog.getByTestId('workflow-dictionary-catalog')
  await expect.poll(() => dictionaryCatalog.evaluate(element => element.scrollHeight > element.clientHeight)).toBe(true)
  await dialog.getByPlaceholder('搜索词典').fill('批量词典 100')
  await expect(dialog.getByText('批量词典 100', { exact: true })).toBeVisible()

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
  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  await dialog.getByRole('checkbox', { name: '选择词典 效果词典' }).click()
  await dialog.getByRole('button', { name: '提高 效果词典 的优先级' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '翻译词典' }).click()
  const priorityItems = dialog.getByTestId('workflow-dictionary-catalog').locator('[data-dictionary-id]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-dictionary-id', 'dictionary-effects')
  await expect(priorityItems.nth(0)).toContainText('优先级 1')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-dictionary-id', 'dictionary-proof')
  await expect(priorityItems.nth(1)).toContainText('优先级 2')
})

test('workflow saves font coverage and ordered inline candidates', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toHaveCount(0)
  await dialog.getByRole('combobox', { name: '应用范围' }).click()
  await page.getByRole('option', { name: 'Hook 捕获的全部文字' }).click()
  await expect(dialog.getByRole('alert', { name: '高影响字体模式' })).toContainText('未翻译文字、图标或符号')
  await dialog.getByRole('button', { name: '提高 Synthetic Serif 的优先级' }).click()
  await dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('Hook 捕获的全部文字')
  const priorityItems = dialog.getByTestId('workflow-selected-fonts').locator('[data-selected-font-family]')
  await expect(priorityItems.nth(0)).toHaveAttribute('data-selected-font-family', 'Synthetic Serif')
  await expect(priorityItems.nth(1)).toHaveAttribute('data-selected-font-family', 'Synthetic Sans')
  await expect(priorityItems.nth(2)).toHaveAttribute('data-selected-font-family', 'Synthetic Mono')
})

test('inline font policy stays isolated between software targets', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = workflowEditor(page)
  await dialog.getByRole('tab', { name: '字体策略' }).click()
  const currentTarget = dialog.getByRole('button', { name: '当前软件目标' })
  await currentTarget.click()
  await page.getByRole('option', { name: /^Pixel Studio/ }).click()
  await expect(dialog.getByRole('switch', { name: '启用字体策略' })).not.toBeChecked()
  await dialog.getByRole('switch', { name: '启用字体策略' }).click()
  await dialog.getByRole('combobox', { name: '应用范围' }).click()
  await page.getByRole('option', { name: 'Hook 捕获的全部文字' }).click()
  await dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' }).click()
  await currentTarget.click()
  await page.getByRole('option', { name: /^Vector Studio/ }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('仅词典命中')
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Sans' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Serif' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).not.toBeChecked()
  await currentTarget.click()
  await page.getByRole('option', { name: /^Pixel Studio/ }).click()
  await expect(dialog.getByRole('combobox', { name: '应用范围' })).toContainText('Hook 捕获的全部文字')
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Mono' })).toBeChecked()
  await expect(dialog.getByRole('checkbox', { name: '选择字体 Synthetic Sans' })).not.toBeChecked()
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
