import { expect, test } from '@playwright/test'

const storageKey = 'glyphshift.composable-product-model.v2'

const model = {
  selectedSoftwareId: 'software-proof',
  software: [{
    id: 'software-proof',
    name: 'Vector Studio',
    description: '用于动态图形与合成项目',
    vendor: 'Sample Creative Tools',
    version: 'v4.2',
    executableName: 'VectorStudio.exe',
    executablePath: 'X:\\SyntheticFixtures\\VectorStudio.exe',
    monogram: 'VS',
    lastUsed: null,
    locale: 'zh-CN',
    connected: false,
    translation: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    font: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    observe: { state: 'unavailable', enabled: false, coverage: 0, detail: '等待运行实例', generation: null },
    locations: [{ id: 'main-ui', label: '界面文字' }, { id: 'dialogs', label: '对话框' }],
  }],
  dictionaries: [{
    metadata: {
      id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化',
      sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'],
    },
    revision: 3,
    entryCount: 2,
  }],
  fontProfiles: [{
    metadata: { id: 'font-profile-proof', name: '中文界面字体', description: '优先使用可变黑体' },
    revision: 2,
    families: ['Synthetic Sans', 'Synthetic Serif'],
    resolvedFamily: 'Synthetic Sans',
  }],
  adapters: [{
    id: 'synthetic.ext-text-out', name: 'ExtTextOutW', summary: '拦截 GDI 高级文本输出；覆盖字距数组、裁剪选项和部分字形索引绘制',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!ExtTextOutW', configuration: 'none',
  }, {
    id: 'synthetic.text-out', name: 'TextOutW', summary: '拦截基础 GDI 文本输出；常见于传统 Win32 控件和简单自绘界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!TextOutW', configuration: 'none',
  }, {
    id: 'synthetic.draw-text', name: 'DrawTextW / DrawTextExW', summary: '拦截矩形内文本布局绘制；常见于按钮、标签和传统窗口界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['USER32 / GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'user32.dll!DrawTextW + DrawTextExW', configuration: 'none',
  }, {
    id: 'synthetic.gdip-draw-string', name: 'GdipDrawString', summary: '拦截 GDI+ 浮点布局文本绘制；常见于自绘面板和图形化桌面界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI+'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdiplus.dll!GdipDrawString', configuration: 'none',
  }],
  workflows: [{
    id: 'workflow-proof', name: '默认创作工作流', description: '组合语言与字体资产', revision: 5,
    softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'],
    targets: [{
      softwareId: 'software-proof',
      adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
      dictionaryIds: ['dictionary-proof'],
      fontBindings: [{ fontProfileId: 'font-profile-proof', scope: { kind: 'all' } }],
    }],
  }],
  activations: [],
  workflowRuntimeStatus: {},
  dictionaryDetails: {
    'dictionary-proof': {
      metadata: {
        id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化',
        sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'],
      },
      revision: 3,
      entries: [
        { source: 'Open', translation: '打开' },
        { source: 'Save As…', translation: '另存为…' },
      ],
    },
  },
  fontProfileDetails: {
    'font-profile-proof': {
      metadata: { id: 'font-profile-proof', name: '中文界面字体', description: '优先使用可变黑体' },
      revision: 2,
      families: ['Synthetic Sans', 'Synthetic Serif'],
      resolvedFamily: 'Synthetic Sans',
    },
  },
  workflowDetails: {
    'workflow-proof': {
      id: 'workflow-proof', name: '默认创作工作流', description: '组合语言与字体资产', revision: 5,
      targets: [{
        softwareId: 'software-proof',
        adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
        dictionaryIds: ['dictionary-proof'],
        fontBindings: [{ fontProfileId: 'font-profile-proof', scope: { kind: 'all' } }],
      }],
    },
  },
  fontFamilies: ['Synthetic Sans', 'Synthetic Serif', 'Synthetic Mono'],
  capture: null,
}

function expandedModel() {
  const next = JSON.parse(JSON.stringify(model))
  next.software.push({
    ...next.software[0],
    id: 'software-secondary',
    name: 'Pixel Studio',
    executableName: 'PixelStudio.exe',
    executablePath: 'X:\\SyntheticFixtures\\PixelStudio.exe',
    monogram: 'PS',
  })
  next.dictionaries.push({
    metadata: {
      ...next.dictionaries[0].metadata,
      id: 'dictionary-effects',
      name: '效果词典',
      description: '效果与属性文字',
      tags: ['效果'],
    },
    revision: 1,
    entryCount: 1,
  })
  next.dictionaryDetails['dictionary-effects'] = {
    metadata: { ...next.dictionaries[1].metadata },
    revision: 1,
    entries: [{ source: 'Effect', translation: '效果' }],
  }
  next.fontProfiles.push({
    metadata: { id: 'font-profile-secondary', name: '备用中文字体', description: '用于对话框' },
    revision: 1,
    families: ['Synthetic Serif'],
    resolvedFamily: 'Synthetic Serif',
  })
  next.fontProfileDetails['font-profile-secondary'] = { ...next.fontProfiles[1] }
  const secondaryTarget = {
    softwareId: 'software-secondary',
    adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
    dictionaryIds: ['dictionary-proof'],
    fontBindings: [],
  }
  next.workflows[0].softwareIds.push('software-secondary')
  next.workflows[0].targets.push(secondaryTarget)
  next.workflowDetails['workflow-proof'].targets.push(JSON.parse(JSON.stringify(secondaryTarget)))
  return next
}

async function replaceModel(page: import('@playwright/test').Page, value: ReturnType<typeof expandedModel>) {
  await page.addInitScript(({ key, modelValue }) => localStorage.setItem(key, JSON.stringify(modelValue)), { key: storageKey, modelValue: value })
  await page.goto('/')
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('independent asset navigation exposes dictionaries and font profiles', async ({ page }) => {
  await expect(page.getByRole('heading', { name: '工作流' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('桌面服务已连接')).toHaveCount(0)
  await expect(page.getByText('本地预览')).toHaveCount(0)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await expect(page.getByRole('heading', { name: '词典' })).toBeVisible()
  await expect(page.getByText('en-US')).toBeVisible()
  await expect(page.getByText('v1.2.0')).toBeVisible()

  await page.getByRole('button', { name: '字体', exact: true }).click()
  await expect(page.getByRole('heading', { name: '字体' })).toBeVisible()
  await expect(page.getByText('Synthetic Sans', { exact: true })).toBeVisible()
  await expect(page.getByText('本机命中')).toBeVisible()
  await expect(page.getByText('被 1 个工作流引用')).toBeVisible()
})

test('capture probe selects observable adapters and generates both result views', async ({ page }) => {
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await expect(page.getByRole('heading', { name: '文字探针' })).toBeVisible()
  await expect(page.getByText('gdi32.dll!TextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('user32.dll!DrawTextW + DrawTextExW', { exact: true })).toBeVisible()
  await expect(page.getByText(/位置|语境/)).toHaveCount(0)

  await page.getByRole('button', { name: '开始监听' }).click()
  await expect(page.getByRole('heading', { name: '正在监听 Vector Studio' })).toBeVisible()
  await page.getByRole('button', { name: '停止并生成' }).click()

  await expect(page.getByRole('button', { name: '技术目录' })).toBeVisible()
  await expect(page.getByRole('button', { name: '字典草稿' })).toBeVisible()
  await expect(page.getByText('没有捕获到文字')).toBeVisible()
})

test('help exposes adapter information without internal targets', async ({ page }) => {
  await page.getByRole('button', { name: '帮助' }).click()

  await expect(page.getByRole('heading', { name: '帮助' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '当前适配器' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(page.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(page.getByText('Windows', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('GDI+', { exact: true })).toBeVisible()
  await expect(page.getByText('文字观察', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('文字替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('字体替换', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('无需配置', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('v1.0.0', { exact: true }).first()).toBeVisible()
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
  await expect(page.getByRole('combobox')).toHaveCount(2)

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
  await page.getByRole('button', { name: 'Software', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Software' })).toBeVisible()
  await page.getByRole('button', { name: 'Dictionaries', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Dictionaries' })).toBeVisible()
  await page.getByRole('button', { name: 'Fonts', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Fonts' })).toBeVisible()
  await page.getByRole('button', { name: 'Workflows', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Workflows' })).toBeVisible()

  await page.getByRole('button', { name: 'Settings', exact: true }).click()
  await page.getByRole('combobox', { name: 'Theme' }).click()
  await page.getByRole('option', { name: 'Light', exact: true }).click()
  await expect(page.locator('html')).toHaveClass(/light/)
  await expect(page.locator('html')).not.toHaveClass(/dark/)
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

test('dictionary editor contains no adapter or font configuration', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByText('en-US → zh-CN')).toBeVisible()
  await expect(page.getByText('另存为…')).toBeVisible()
  await expect(page.getByRole('columnheader', { name: '位置', exact: true })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '语境', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '词典设置' }).click()
  const settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await expect(settings.getByRole('textbox', { name: '作者' })).toHaveValue('Glyphshift')
  await settings.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '添加词条' }).click()
  const ruleEditor = page.getByRole('dialog', { name: '添加翻译词条' })
  await expect(ruleEditor.getByRole('textbox', { name: '原文' })).toBeVisible()
  await expect(ruleEditor.getByRole('textbox', { name: '译文' })).toBeVisible()
  await expect(ruleEditor.getByText('语义位置')).toHaveCount(0)
  await expect(ruleEditor.getByText('限定语境')).toHaveCount(0)
  await ruleEditor.getByRole('button', { name: '取消' }).click()
  await expect(page.getByText('默认字体')).toHaveCount(0)
  await expect(page.getByText(/Hook/)).toHaveCount(0)
})

test('workflow target independently selects adapters dictionaries and font scope', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await expect(page.getByText('此处配置仅属于这个软件目标')).toBeVisible()
  await expect(page.getByText('windows · GDI', { exact: true })).toBeVisible()
  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await expect(dialog.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('TextOutW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('DrawTextW / DrawTextExW', { exact: true })).toBeVisible()
  await expect(dialog.getByText('GdipDrawString', { exact: true })).toBeVisible()
  await expect(dialog.getByText('gdi32.dll!ExtTextOutW')).toHaveCount(0)
  await expect(dialog.getByText('界面基础词典', { exact: true }).first()).toBeVisible()
  await expect(page.getByRole('button', { name: '全部位置' })).toBeVisible()
  await expect(page.getByRole('button', { name: '全部位置' })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '提高 界面基础词典 的优先级' })).toBeDisabled()
  await expect(page.getByRole('combobox', { name: '选择要添加的字体方案' })).toBeVisible()
})

test('workflow saves reordered dictionaries in explicit priority order', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await dialog.getByText('效果词典', { exact: true }).click()
  await dialog.getByRole('button', { name: '提高 效果词典 的优先级' }).click()
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = page.getByRole('dialog', { name: '编辑工作流' })
  const priorityItems = dialog.locator('ol > li')
  await expect(priorityItems).toHaveCount(2)
  await expect(priorityItems.nth(0)).toContainText('效果词典')
  await expect(priorityItems.nth(1)).toContainText('界面基础词典')
})

test('workflow adds a second font binding and recovers from a location overlap', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  let dialog = page.getByRole('dialog', { name: '编辑工作流' })
  const primary = dialog.getByRole('group', { name: '字体绑定 中文界面字体' })
  await primary.getByRole('button', { name: '指定位置' }).click()
  await primary.getByText('界面文字', { exact: true }).click()

  await dialog.getByRole('combobox', { name: '选择要添加的字体方案' }).click()
  await page.getByRole('option', { name: '备用中文字体' }).click()
  await dialog.getByRole('button', { name: '添加', exact: true }).click()
  await expect(dialog.getByText(/字体绑定需要至少一个位置/)).toBeVisible()

  const secondary = dialog.getByRole('group', { name: '字体绑定 备用中文字体' })
  await secondary.getByText('界面文字', { exact: true }).click()
  await expect(dialog.getByText(/字体位置 main-ui 被重复绑定/)).toBeVisible()
  await secondary.getByText('界面文字', { exact: true }).click()
  await secondary.getByText('对话框', { exact: true }).click()
  await expect(dialog.getByText('还不能保存')).toHaveCount(0)
  await dialog.getByRole('button', { name: '保存工作流' }).click()

  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  dialog = page.getByRole('dialog', { name: '编辑工作流' })
  await expect(dialog.getByRole('group', { name: /^字体绑定 / })).toHaveCount(2)
  await expect(dialog.getByRole('group', { name: '字体绑定 备用中文字体' }).getByText('对话框', { exact: true })).toBeVisible()
})

test('pending font selection does not leak between software targets', async ({ page }) => {
  await replaceModel(page, expandedModel())
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  const dialog = page.getByRole('dialog', { name: '编辑工作流' })
  const selector = dialog.getByRole('combobox', { name: '选择要添加的字体方案' })
  await selector.click()
  await page.getByRole('option', { name: '备用中文字体' }).click()
  await expect(selector).toContainText('备用中文字体')
  await dialog.getByRole('button', { name: '切换到 Pixel Studio' }).click()
  await expect(selector).toContainText('选择字体方案')
})

test('dictionary editor saves the complete portable metadata set', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  let settings = page.getByRole('dialog', { name: '词典设置' })
  await settings.getByRole('textbox', { name: '源语言' }).fill('fr-FR')
  await settings.getByRole('textbox', { name: '目标语言' }).fill('de-DE')
  await settings.getByRole('textbox', { name: '发布版本' }).fill('2.0.0')
  await settings.getByRole('textbox', { name: '作者' }).fill('Alice, Bob')
  await settings.getByRole('textbox', { name: '许可证' }).fill('Apache-2.0')
  await settings.getByRole('textbox', { name: '主页' }).fill('https://example.invalid/dictionary')
  await settings.getByRole('button', { name: '应用设置' }).click()
  await page.getByRole('button', { name: '保存词典' }).click()
  await page.getByRole('button', { name: '返回词典列表' }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('fr-FR')
  await expect(settings.getByRole('textbox', { name: '目标语言' })).toHaveValue('de-DE')
  await expect(settings.getByRole('textbox', { name: '作者' })).toHaveValue('Alice, Bob')
  await expect(settings.getByRole('textbox', { name: '许可证' })).toHaveValue('Apache-2.0')
  await expect(settings.getByRole('textbox', { name: '主页' })).toHaveValue('https://example.invalid/dictionary')
})

test('dictionary text editing saves only source and translation', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '编辑 Save As…' }).click()
  const editor = page.getByRole('dialog', { name: '编辑翻译词条' })
  await editor.getByRole('textbox', { name: '译文' }).fill('另存一个副本…')
  await editor.getByRole('button', { name: '保存词条' }).click()
  await page.getByRole('button', { name: '保存词典' }).click()

  const saved = await page.evaluate(() => {
    const model = JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v2') ?? '{}')
    return model.dictionaryDetails?.['dictionary-proof']?.entries?.[1]
  })
  expect(saved).toEqual({
    source: 'Save As…',
    translation: '另存一个副本…',
  })
})

test('font profile editor preserves candidate order', async ({ page }) => {
  await page.getByRole('button', { name: '字体', exact: true }).click()
  await page.getByRole('button', { name: '编辑 中文界面字体' }).click()
  await expect(page.getByText('候选优先级 · 2')).toBeVisible()
  const candidates = page.locator('section').filter({ hasText: '候选优先级 · 2' })
  await expect(candidates.getByText('Synthetic Sans', { exact: true })).toBeVisible()
  await expect(candidates.getByText('Synthetic Serif', { exact: true })).toBeVisible()
  await candidates.getByRole('button', { name: '下移 Synthetic Sans' }).click()
  await page.getByRole('button', { name: '保存字体方案' }).click()
  await expect(page.getByText('Synthetic Serif → Synthetic Sans')).toBeVisible()
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
