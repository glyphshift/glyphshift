import { test } from '@playwright/test'

const model = {
  selectedSoftwareId: 'software-proof',
  software: [{
    id: 'software-proof',
    name: 'Vector Studio',
    description: '用于动态图形与合成项目',
    vendor: 'Sample Creative Tools',
    version: 'v4.2',
    executableName: 'VectorStudio.exe',
    executablePath: 'X:\\SyntheticFixtures\\Vector Studio\\VectorStudio.exe',
    monogram: 'VS',
    lastUsed: '刚刚',
    locale: 'zh-CN',
    connected: true,
    translation: { state: 'ready', enabled: false, coverage: 94, detail: '合成能力证据', generation: null },
    font: { state: 'ready', enabled: false, coverage: 82, detail: '合成能力证据', generation: null },
    observe: { state: 'active', enabled: true, coverage: 100, detail: '合成观察证据', generation: 18 },
    locations: [{ id: 'main-ui', label: '界面文字' }],
  }],
  dictionaries: [{ id: 'dictionary-proof', name: '界面基础词典', description: '用于创作软件菜单与面板汉化', locale: 'zh-CN', hookTypeId: 'synthetic-gdi', revision: 3, entryCount: 2 }],
  workflows: [{ id: 'workflow-proof', name: '默认创作工作流', description: '默认合成软件与基础词典组合', revision: 5, softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'], targets: [{ softwareId: 'software-proof', dictionaryIds: ['dictionary-proof'] }] }],
  activations: [{ workflowId: 'workflow-proof', revision: 5 }],
  workflowRuntimeStatus: {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof',
        discovered: true,
        active: true,
        translationRequested: true,
        fontRequested: true,
        translationActive: true,
        fontActive: true,
        appliedGeneration: 18,
      }],
      errors: {},
    },
  },
  dictionaryDetails: {
    'dictionary-proof': {
      id: 'dictionary-proof',
      name: '界面基础词典',
      description: '用于创作软件菜单与面板汉化',
      locale: 'zh-CN',
      hookTypeId: 'synthetic-gdi',
      revision: 3,
      defaultFont: { kind: 'substitute', family: 'Synthetic Sans' },
      entries: [
        { location: 'main-ui', context: null, source: 'Open', translation: '打开', font: { kind: 'inherit' }, adapterIds: [] },
        { location: 'main-ui', context: null, source: 'Save As…', translation: '另存为…', font: { kind: 'substitute', family: 'Synthetic Serif' }, adapterIds: [] },
      ],
    },
  },
  workflowDetails: {
    'workflow-proof': { id: 'workflow-proof', name: '默认创作工作流', description: '默认合成软件与基础词典组合', revision: 5, targets: [{ softwareId: 'software-proof', dictionaryIds: ['dictionary-proof'] }] },
  },
  theme: 'dark',
  translationSource: '',
  hookTypes: [
    { id: 'synthetic-gdi', label: 'GDI' },
    { id: 'synthetic-gdiplus', label: 'GDI+' },
  ],
  fontFamilies: [
    'Synthetic Sans',
    'Synthetic Serif',
    'Synthetic Mono',
    ...Array.from({ length: 24 }, (_, index) => `Synthetic Family ${String(index + 1).padStart(2, '0')}`),
  ],
}

test('capture workflow product desktop and compact surfaces', async ({ page }) => {
  await page.addInitScript(value => localStorage.setItem('glyphshift.workflow-product-model', JSON.stringify(value)), model)
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await page.waitForTimeout(800)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflows-desktop.png' })
  await page.getByRole('button', { name: '显示列' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/display-columns-desktop.png' })
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-edit-desktop.png' })
  await page.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/software-desktop.png' })
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionaries-desktop.png' })
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-editor-desktop.png' })
  await page.getByRole('button', { name: '默认字体' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-font-menu-desktop.png' })
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/settings-desktop.png' })
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflows-compact.png' })
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionaries-compact.png' })
  await page.getByRole('checkbox', { name: '选择 界面基础词典' }).check()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionaries-batch-compact.png' })
})

test('capture empty asset creation dialogs', async ({ page }) => {
  await page.addInitScript((value) => {
    localStorage.setItem('glyphshift.workflow-product-model', JSON.stringify(value))
  }, {
    ...model,
    selectedSoftwareId: null,
    software: [],
    dictionaries: [],
    workflows: [],
    activations: [],
    workflowRuntimeStatus: {},
    dictionaryDetails: {},
    workflowDetails: {},
  })
  await page.setViewportSize({ width: 960, height: 640 })
  await page.goto('/')
  await page.getByRole('button', { name: '新建工作流' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-prerequisites-compact.png' })
  await page.getByRole('button', { name: '去添加软件' }).click()
  await page.getByRole('button', { name: '新增软件' }).first().click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/software-create-compact.png' })
  await page.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '新建词典' }).first().click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-create-compact.png' })
  await page.getByRole('combobox', { name: '适用 Hook' }).click()
  await page.waitForTimeout(300)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/dictionary-create-hook-menu-compact.png' })
})
