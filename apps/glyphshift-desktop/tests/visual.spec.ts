import { expect, test } from '@playwright/test'

const model = {
  selectedSoftwareId: 'software-proof',
  software: [{
    id: 'software-proof', name: 'Vector Studio', description: '用于动态图形与合成项目', vendor: 'Sample Creative Tools', version: 'v4.2',
    executableName: 'VectorStudio.exe', executablePath: 'X:\\SyntheticFixtures\\VectorStudio.exe', monogram: 'VS', lastUsed: null,
    locale: 'zh-CN', connected: false,
    translation: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    font: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    observe: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
  }],
  dictionaries: [{ metadata: { id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化', sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'] }, revision: 3, entryCount: 2 }],
  adapters: [
    { id: 'synthetic.ext-text-out', name: 'ExtTextOutW', summary: '拦截 GDI 高级文本输出；覆盖字距数组、裁剪选项和部分字形索引绘制', version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!ExtTextOutW', configuration: 'none' },
    { id: 'synthetic.text-out', name: 'TextOutW', summary: '拦截基础 GDI 文本输出；常见于传统 Win32 控件和简单自绘界面', version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!TextOutW', configuration: 'none' },
    { id: 'synthetic.draw-text', name: 'DrawTextW / DrawTextExW', summary: '拦截矩形内文本布局绘制；常见于按钮、标签和传统窗口界面', version: '1.0.0', platforms: ['windows'], technologies: ['USER32 / GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'user32.dll!DrawTextW + DrawTextExW', configuration: 'none' },
    { id: 'synthetic.gdip-draw-string', name: 'GdipDrawString', summary: '拦截 GDI+ 浮点布局文本绘制；常见于自绘面板和图形化桌面界面', version: '1.0.0', platforms: ['windows'], technologies: ['GDI+'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdiplus.dll!GdipDrawString', configuration: 'none' },
  ],
  workflows: [{ id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5, softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'], targets: [{ softwareId: 'software-proof', adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] }, dictionaryIds: ['dictionary-proof'], fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' } }] }],
  activations: [{ workflowId: 'workflow-proof', revision: 5 }],
  workflowRuntimeStatus: { 'workflow-proof': { workflowId: 'workflow-proof', targets: [{ softwareId: 'software-proof', discovered: true, active: true, translationRequested: true, fontRequested: true, translationActive: true, fontActive: true, appliedGeneration: 18 }], errors: {} } },
  dictionaryDetails: { 'dictionary-proof': { metadata: { id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化', sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'] }, revision: 3, entries: [{ source: 'Open', translation: '打开' }, { source: 'Save As…', translation: '另存为…' }] } },
  workflowDetails: { 'workflow-proof': { id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5, targets: [{ softwareId: 'software-proof', adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] }, dictionaryIds: ['dictionary-proof'], fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' } }] } },
  fontFamilies: ['Synthetic Sans', 'Synthetic Serif', 'Synthetic Mono', ...Array.from({ length: 18 }, (_, index) => `Synthetic Family ${String(index + 1).padStart(2, '0')}`)],
  capture: null,
}

async function waitForVisualStability(page: import('@playwright/test').Page) {
  await page.waitForLoadState('networkidle')
  await page.waitForTimeout(150)
}

test('capture composable workflow and asset surfaces', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.waitForTimeout(400)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflows-composable.png' })
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await expect(page.getByRole('dialog', { name: '编辑工作流' })).toBeVisible()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-target-composition.png' })
  await page.getByRole('dialog', { name: '编辑工作流' }).getByRole('button', { name: 'Hook 捕获的全部文字' }).click()
  await page.getByLabel('目标配置，可向下滚动').evaluate(element => { element.scrollTop = element.scrollHeight })
  await page.waitForTimeout(150)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-font-policy-all-observations.png' })
  await page.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.waitForTimeout(200)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionaries-metadata.png' })
})

test('capture probe run creation and joined table state', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.getByRole('button', { name: '探针', exact: true }).click()
  await waitForVisualStability(page)
  await page.getByRole('button', { name: '新建探针任务' }).click()
  await expect(page.getByRole('dialog', { name: '新建探针任务' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/probe-run-create.png' })
  await page.getByRole('dialog', { name: '新建探针任务' }).getByRole('button', { name: '创建并连接' }).click()
  await expect(page.getByPlaceholder('搜索原文、译文或探针技术')).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/probe-run-table.png' })
  await page.setViewportSize({ width: 960, height: 640 })
  await waitForVisualStability(page)
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/probe-run-table-compact.png' })
})

test('capture distilled dictionary editor and its focused modals', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-editor-distilled.png' })

  await page.getByRole('button', { name: '词典设置' }).click()
  await expect(page.getByRole('dialog', { name: '词典设置' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-settings-modal.png' })
  await page.getByRole('dialog', { name: '词典设置' }).getByRole('button', { name: '取消' }).click()

  await page.setViewportSize({ width: 1160, height: 527 })
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-editor-distilled-short.png' })
  await page.getByRole('button', { name: '词典设置' }).click()
  await expect(page.getByRole('dialog', { name: '词典设置' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-settings-modal-short.png' })
  await page.getByRole('dialog', { name: '词典设置' }).getByRole('button', { name: '取消' }).click()

  await page.setViewportSize({ width: 960, height: 640 })
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-editor-distilled-compact.png' })
  await page.getByRole('button', { name: '添加词条' }).click()
  await expect(page.getByRole('dialog', { name: '添加翻译词条' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-rule-modal-compact.png' })
})

test('capture help and settings surfaces', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: '帮助' }).click()
  await expect(page.getByRole('heading', { name: '当前适配器' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/help-adapters.png' })
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('heading', { name: '设置' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/settings-local-assets.png' })
  await page.setViewportSize({ width: 960, height: 640 })
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/settings-local-assets-compact.png' })
})

test('capture compact workflow composition', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 960, height: 640 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await expect(page.getByRole('dialog', { name: '编辑工作流' })).toBeVisible()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-target-composition-compact.png' })
})

test('capture compact help surface', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 960, height: 640 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: '帮助' }).click()
  await expect(page.getByRole('heading', { name: '当前适配器' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/help-adapters-compact.png' })
})

test('capture English light settings at wide and compact widths', async ({ page }) => {
  await page.addInitScript(({ productModel, appSettings }) => {
    localStorage.setItem('glyphshift.composable-product-model.v3', JSON.stringify(productModel))
    localStorage.setItem('glyphshift.app-settings.v1', JSON.stringify(appSettings))
  }, {
    productModel: model,
    appSettings: {
      settingsSchemaVersion: 1,
      localePreference: 'en-US',
      themePreference: 'light',
    },
  })
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/settings-english-light.png' })

  await page.setViewportSize({ width: 960, height: 640 })
  await waitForVisualStability(page)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/settings-english-light-compact.png' })
})
