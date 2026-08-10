import type { Page } from '@playwright/test'

export const storageKey = 'glyphshift.composable-product-model.v3'

export const model = {
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
  }],
  dictionaries: [{
    metadata: {
      id: 'dictionary-proof', releaseVersion: '1.2.0', name: '界面基础词典', description: '菜单与面板汉化',
      sourceLocale: 'en-US', targetLocale: 'zh-CN', authors: ['Glyphshift'], license: 'MIT', homepage: null, tags: ['菜单', '面板'],
    },
    revision: 3,
    entryCount: 2,
  }],
  adapters: [{
    id: 'synthetic.ext-text-out', name: 'ExtTextOutW', summary: '拦截 GDI 高级文本输出；覆盖字距数组、裁剪选项和部分字形索引绘制',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!ExtTextOutW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-exttextoutw', configuration: 'none',
  }, {
    id: 'synthetic.text-out', name: 'TextOutW', summary: '拦截基础 GDI 文本输出；常见于传统 Win32 控件和简单自绘界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdi32.dll!TextOutW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-textoutw', configuration: 'none',
  }, {
    id: 'synthetic.draw-text', name: 'DrawTextW / DrawTextExW', summary: '拦截矩形内文本布局绘制；常见于按钮、标签和传统窗口界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['USER32 / GDI'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'user32.dll!DrawTextW + DrawTextExW', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-drawtextw', configuration: 'none',
  }, {
    id: 'synthetic.gdip-draw-string', name: 'GdipDrawString', summary: '拦截 GDI+ 浮点布局文本绘制；常见于自绘面板和图形化桌面界面',
    version: '1.0.0', platforms: ['windows'], technologies: ['GDI+'], features: ['textObserve', 'textReplace', 'fontSubstitute'], technicalTarget: 'gdiplus.dll!GdipDrawString', documentationUrl: 'https://learn.microsoft.com/en-us/windows/win32/gdiplus/-gdiplus-drawing-text-use', configuration: 'none',
  }, {
    id: 'windows.unity.mono.standard-ui', name: 'Unity Mono 标准界面', summary: '翻译 Windows x64 Unity Mono 应用中的 TMP/uGUI 标准 text 属性；不适用于 IL2CPP、UI Toolkit、NGUI、自绘文字或 TMP SetText 快捷入口',
    version: '0.1.0', platforms: ['windows'], technologies: ['Unity Mono / TextMeshPro / uGUI'], features: ['textObserve', 'textReplace'], technicalTarget: 'Mono JIT!TMP_Text.set_text + UnityEngine.UI.Text.set_text', documentationUrl: 'https://docs.unity3d.com/cn/current/Manual/scripting-backends-mono.html', configuration: 'none',
  }],
  workflows: [{
    id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5,
    softwareIds: ['software-proof'], dictionaryIds: ['dictionary-proof'],
    targets: [{
      softwareId: 'software-proof',
      adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
      dictionaryIds: ['dictionary-proof'],
      fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' },
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
  workflowDetails: {
    'workflow-proof': {
      id: 'workflow-proof', name: '默认创作工作流', description: '组合词典与字体策略', revision: 5,
      targets: [{
        softwareId: 'software-proof',
        adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
        dictionaryIds: ['dictionary-proof'],
        fontPolicy: { families: ['Synthetic Sans', 'Synthetic Serif'], coverage: 'dictionary_matches' },
      }],
    },
  },
  fontFamilies: ['Synthetic Sans', 'Synthetic Serif', 'Synthetic Mono'],
  capture: null,
}

export function expandedModel() {
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
  const secondaryTarget = {
    softwareId: 'software-secondary',
    adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
    dictionaryIds: ['dictionary-proof'],
    fontPolicy: null,
  }
  next.workflows[0].softwareIds.push('software-secondary')
  next.workflows[0].targets.push(secondaryTarget)
  next.workflowDetails['workflow-proof'].targets.push(JSON.parse(JSON.stringify(secondaryTarget)))
  return next
}

export function largeWorkflowCatalogModel() {
  const next = expandedModel()
  for (let index = 3; index <= 10; index += 1) {
    const softwareId = `software-batch-${index}`
    next.software.push({
      ...next.software[0],
      id: softwareId,
      name: `Batch Studio ${index}`,
      executableName: `BatchStudio${index}.exe`,
      executablePath: `X:\\SyntheticFixtures\\BatchStudio${index}.exe`,
      monogram: `B${index}`,
    })
    const target = {
      softwareId,
      adapterPlan: { strategy: 'parallel', adapterIds: ['synthetic.ext-text-out'] },
      dictionaryIds: ['dictionary-proof'],
      fontPolicy: null,
    }
    next.workflows[0].softwareIds.push(softwareId)
    next.workflows[0].targets.push(target)
    next.workflowDetails['workflow-proof'].targets.push(JSON.parse(JSON.stringify(target)))
  }
  for (let index = 3; index <= 100; index += 1) {
    next.dictionaries.push({
      metadata: {
        ...next.dictionaries[0].metadata,
        id: `dictionary-batch-${index}`,
        name: `批量词典 ${index}`,
        description: `第 ${index} 份合成测试词典`,
        tags: ['批量测试'],
      },
      revision: 1,
      entryCount: index,
    })
  }
  return next
}

export async function replaceModel(page: Page, value: ReturnType<typeof expandedModel>) {
  await page.addInitScript(({ key, modelValue }) => localStorage.setItem(key, JSON.stringify(modelValue)), { key: storageKey, modelValue: value })
  await page.goto('/')
}

export function workflowEditor(page: Page) {
  return page.getByTestId('workflow-editor')
}
