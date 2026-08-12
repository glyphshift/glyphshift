import { expect, test } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})
test('dictionary library separates local provenance from the offline catalog mode', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()

  const localMode = page.getByRole('button', { name: /本地词典/ })
  const catalogMode = page.getByRole('button', { name: '在线目录', exact: true })
  await expect(localMode).toHaveAttribute('aria-pressed', 'true')
  await expect(localMode).toHaveClass(/text-primary/)
  await expect(page.getByRole('columnheader', { name: '来源状态' })).toBeVisible()
  await expect(page.getByText('本地创建', { exact: true })).toBeVisible()
  await expect(page.getByText('未关联在线发布', { exact: true })).toBeVisible()

  await catalogMode.click()
  await expect(catalogMode).toHaveAttribute('aria-pressed', 'true')
  await expect(catalogMode).toHaveClass(/text-primary/)
  await expect(localMode).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByText('在线目录不可用', { exact: true })).toBeVisible()
  await expect(page.getByText('在线词典目录尚未配置或暂时不可用，本地词典不受影响。')).toBeVisible()
  await expect(page.getByText('第 1 页，本页 0 个版本')).toBeVisible()
  await expect(page.getByRole('button', { name: '新建词典' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '重试' })).toBeVisible()
})

test('dictionary library imports and exports one portable JSON file', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  const importedSnapshot = JSON.parse(JSON.stringify(snapshot))
  importedSnapshot.dictionaries.push({
    metadata: {
      ...snapshot.dictionaries[0].metadata,
      id: 'dictionary-imported',
      name: '导入词典',
      description: '标准 Dictionary /3 文件',
      releaseVersion: '1.0.0',
    },
    revision: 1,
    entryCount: 1,
    installation: {
      state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
    },
  })
  await page.addInitScript(({ current, imported }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return current
        if (command === 'plugin:dialog|open') return 'X:\\SyntheticFixtures\\dictionary-imported.json'
        if (command === 'desktop_import_dictionary') {
          ;(window as unknown as { __dictionaryImport?: unknown }).__dictionaryImport = args
          return imported
        }
        if (command === 'plugin:dialog|save') {
          ;(window as unknown as { __dictionarySaveDialog?: unknown }).__dictionarySaveDialog = args
          return 'X:\\SyntheticFixtures\\dictionary-imported.published.json'
        }
        if (command === 'desktop_export_dictionary') {
          ;(window as unknown as { __dictionaryExport?: unknown }).__dictionaryExport = args
          return null
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot, imported: importedSnapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByText('导入词典', { exact: true })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionaryImport?: { inputPath?: string } }).__dictionaryImport
  ))).toEqual(expect.objectContaining({ inputPath: 'X:\\SyntheticFixtures\\dictionary-imported.json' }))

  await page.getByRole('button', { name: '导出发布文件 导入词典' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionaryExport?: { dictionaryId?: string, outputPath?: string } }).__dictionaryExport
  ))).toEqual(expect.objectContaining({
    dictionaryId: 'dictionary-imported',
    outputPath: 'X:\\SyntheticFixtures\\dictionary-imported.published.json',
  }))
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionarySaveDialog?: { options?: { defaultPath?: string } } }).__dictionarySaveDialog
  ))).toEqual(expect.objectContaining({
    options: expect.objectContaining({ defaultPath: 'dictionary-imported.json' }),
  }))
})

test('dictionary export reports when the native save dialog cannot open', async ({ page }) => {
  await page.addInitScript(({ snapshot }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'plugin:dialog|save') throw new Error('synthetic save dialog failure')
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '导出发布文件 界面基础词典', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('无法打开保存窗口，请重试')
})

test('local management items support double-click editing while keeping explicit actions', async ({ page }) => {
  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await expect(page.getByRole('heading', { name: '默认创作工作流' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回工作流列表' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '编辑工作流' })).toHaveCount(0)
  await page.getByRole('button', { name: '返回工作流列表' }).click()

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.getByRole('row').filter({ hasText: 'Vector Studio' }).dblclick()
  await expect(page.getByRole('heading', { name: 'Vector Studio' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回软件列表' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '编辑软件' })).toHaveCount(0)
  await page.getByRole('button', { name: '返回软件列表' }).click()

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回词典列表' })).toBeVisible()

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const createProbe = page.getByRole('dialog', { name: '新建探针任务' })
  await createProbe.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await createProbe.getByRole('button', { name: '创建并连接' }).click()
  await page.getByRole('button', { name: '返回探针管理' }).click()
  await page.getByRole('row').filter({ hasText: 'Vector Studio 探针' }).dblclick()
  await expect(page.getByRole('heading', { name: 'Vector Studio 探针' })).toBeVisible()

  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect(page.getByRole('button', { name: '编辑 默认创作工作流' })).toBeVisible()
})

test('escape returns from each independent item page and protects dirty forms', async ({ page }) => {
  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.getByRole('textbox', { name: '工作流名称' }).fill('尚未保存的工作流')
  await page.keyboard.press('Escape')
  const workflowDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(workflowDiscard).toBeVisible()
  await workflowDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '工作流', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: '默认创作工作流' }).dblclick()
  await page.getByRole('textbox', { name: '工作流名称' }).fill('切换菜单前尚未保存')
  await page.getByRole('button', { name: '软件', exact: true }).click()
  const navigationDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(navigationDiscard).toBeVisible()
  await navigationDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '软件', exact: true })).toBeVisible()

  await page.getByRole('row').filter({ hasText: 'Vector Studio' }).dblclick()
  await page.getByRole('textbox', { name: '显示名称' }).fill('尚未保存的软件')
  await page.keyboard.press('Escape')
  const softwareDiscard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(softwareDiscard).toBeVisible()
  await softwareDiscard.getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '软件', exact: true })).toBeVisible()

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '词典', exact: true })).toBeVisible()

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await page.getByRole('button', { name: '新建探针任务' }).click()
  const createProbe = page.getByRole('dialog', { name: '新建探针任务' })
  await createProbe.getByRole('textbox', { name: '任务名称' }).fill('Vector Studio 探针')
  await createProbe.getByRole('button', { name: '创建并连接' }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '探针', exact: true })).toBeVisible()
})

test('configured dictionary catalog queries and installs through the desktop seam', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  const installedSnapshot = JSON.parse(JSON.stringify(snapshot))
  installedSnapshot.dictionaries.push({
    metadata: {
      ...snapshot.dictionaries[0].metadata,
      id: 'dictionary-catalog',
      name: '目录词典',
      description: '菜单翻译',
    },
    revision: 1,
    entryCount: 1,
    installation: {
      state: 'verified', installedRelease: '1.2.0', verifiedPublisher: 'publisher.example', updateRelease: null,
    },
  })
  await page.addInitScript(({ current, installed }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') {
          ;(window as unknown as { __catalogQuery?: unknown }).__catalogQuery = args?.request
          return {
            releases: [{
              catalogId: 'glyphshift.official', dictionaryId: 'dictionary-catalog', releaseVersion: '1.2.0',
              sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'zh-CN',
              name: '目录词典', summary: '菜单翻译', tags: ['菜单'], publisherIdentity: 'publisher.example',
            }],
            nextCursor: null,
          }
        }
        if (command === 'desktop_install_dictionary_release') {
          ;(window as unknown as { __catalogInstall?: unknown }).__catalogInstall = args?.request
          return installed
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot, installed: installedSnapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '在线目录', exact: true }).click()
  await expect(page.getByText('目录词典', { exact: true })).toBeVisible()
  await expect(page.getByText('publisher.example', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '按标签筛选 菜单' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogQuery?: { tag?: string | null } }).__catalogQuery
  ))).toEqual(expect.objectContaining({ tag: '菜单' }))
  await page.getByRole('button', { name: '清除标签筛选 菜单' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogQuery?: { tag?: string | null } }).__catalogQuery
  ))).toEqual(expect.objectContaining({ tag: null }))
  await page.getByRole('button', { name: '安装', exact: true }).click()
  await expect(page.getByRole('button', { name: '已安装', exact: true })).toBeDisabled()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogInstall?: { replacement?: string } }).__catalogInstall
  ))).toEqual(expect.objectContaining({
    dictionaryId: 'dictionary-catalog',
    releaseVersion: '1.2.0',
    replacement: 'reject_existing',
  }))
})

test('catalog requires explicit confirmation before replacing local dictionary changes', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'modified', installedRelease: '1.0.0', verifiedPublisher: 'publisher.example', updateRelease: '1.2.0',
  }
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') return {
          releases: [{
            catalogId: 'glyphshift.official', dictionaryId: 'dictionary-proof', releaseVersion: '1.2.0',
            sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'zh-CN',
            name: '界面基础词典', summary: '菜单与面板汉化', tags: ['菜单'], publisherIdentity: 'publisher.example',
          }],
          nextCursor: null,
        }
        if (command === 'desktop_install_dictionary_release') {
          ;(window as unknown as { __catalogReplacement?: unknown }).__catalogReplacement = args?.request
          return current
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '在线目录', exact: true }).click()
  await page.getByRole('button', { name: '更新', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '替换本地词典' })
  await expect(dialog.getByText(/包含本地修改/)).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogReplacement?: unknown }).__catalogReplacement
  ))).toBeUndefined()
  await dialog.getByRole('button', { name: '替换并安装' }).click()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogReplacement?: { replacement?: string } }).__catalogReplacement
  ))).toEqual(expect.objectContaining({ replacement: 'replace_any' }))
})

test('catalog presentation follows the English interface locale', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaries[0].installation = {
    state: 'unmanaged', installedRelease: null, verifiedPublisher: null, updateRelease: null,
  }
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'en-US', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_query_dictionary_catalog') {
          ;(window as unknown as { __catalogLocale?: string }).__catalogLocale = args?.request?.requestedPresentationLocale
          return {
            releases: [{
              catalogId: 'glyphshift.official', dictionaryId: 'dictionary-catalog', releaseVersion: '1.2.0',
              sourceLocale: 'en-US', targetLocale: 'zh-CN', effectivePresentationLocale: 'en-US',
              name: 'Simplified Chinese menus', summary: 'Common menu and dialog text', tags: ['menus'], publisherIdentity: 'publisher.example',
            }],
            nextCursor: null,
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: 'Dictionaries', exact: true }).click()
  await page.getByRole('button', { name: 'Online catalog', exact: true }).click()
  await expect(page.getByText('Simplified Chinese menus', { exact: true })).toBeVisible()
  await expect(page.getByText('Common menu and dialog text', { exact: true })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __catalogLocale?: string }).__catalogLocale
  ))).toBe('en-US')
})

test('dictionary editor contains no adapter or font configuration', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByText('en-US → zh-CN')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Save As…' })).toHaveValue('另存为…')
  await expect(page.getByRole('columnheader', { name: '位置', exact: true })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '语境', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '词典设置' }).click()
  const settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('en-US')
  await settings.getByRole('button', { name: '更多发布信息' }).click()
  await expect(settings.getByText('Glyphshift', { exact: true })).toBeVisible()
  await settings.getByRole('button', { name: '取消' }).click()
  await expect(page.getByRole('button', { name: '添加词条' })).toHaveCount(0)
  await expect(page.getByRole('textbox', { name: '新词条原文' })).toBeVisible()
  await expect(page.getByRole('textbox', { name: '新词条译文' })).toBeVisible()
  await expect(page.getByText('语义位置')).toHaveCount(0)
  await expect(page.getByText('限定语境')).toHaveCount(0)
  await expect(page.getByText('默认字体')).toHaveCount(0)
  await expect(page.getByText(/Hook/)).toHaveCount(0)
})

test('dictionary editor saves the complete portable metadata set', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  let settings = page.getByRole('dialog', { name: '词典设置' })
  await settings.getByRole('textbox', { name: '源语言' }).fill('fr-FR')
  await settings.getByRole('textbox', { name: '目标语言' }).fill('de-DE')
  await settings.getByRole('button', { name: '更多发布信息' }).click()
  await settings.getByRole('spinbutton', { name: '主版本号' }).fill('2')
  await settings.getByRole('spinbutton', { name: '次版本号' }).fill('0')
  await settings.getByRole('spinbutton', { name: '修订版本号' }).fill('0')
  const authorInput = settings.getByRole('textbox', { name: '作者' })
  await authorInput.fill('Alice')
  await authorInput.press('Enter')
  await authorInput.fill('Bob')
  await authorInput.press('Enter')
  await settings.getByRole('textbox', { name: '许可证' }).fill('Apache-2.0')
  await settings.getByRole('textbox', { name: '主页' }).fill('https://example.invalid/dictionary')
  await settings.getByRole('button', { name: '应用设置' }).click()
  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保存词典' }).click()
  await expect(page.getByText('未保存', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '返回词典列表' }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '词典设置' }).click()
  settings = page.getByRole('dialog', { name: '词典设置' })
  await expect(settings.getByRole('textbox', { name: '源语言' })).toHaveValue('fr-FR')
  await expect(settings.getByRole('textbox', { name: '目标语言' })).toHaveValue('de-DE')
  await settings.getByRole('button', { name: '更多发布信息' }).click()
  await expect(settings.getByRole('spinbutton', { name: '主版本号' })).toHaveValue('2')
  await expect(settings.getByRole('spinbutton', { name: '次版本号' })).toHaveValue('0')
  await expect(settings.getByRole('spinbutton', { name: '修订版本号' })).toHaveValue('0')
  await expect(settings.getByText('Alice', { exact: true })).toBeVisible()
  await expect(settings.getByText('Bob', { exact: true })).toBeVisible()
  await expect(settings.getByRole('textbox', { name: '许可证' })).toHaveValue('Apache-2.0')
  await expect(settings.getByRole('textbox', { name: '主页' })).toHaveValue('https://example.invalid/dictionary')
})

test('dictionary editor uses one guarded inline draft', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  const existingRow = page.getByRole('row').filter({ has: page.getByRole('textbox', { name: '编辑原文：Save As…' }) })
  await expect(existingRow.getByRole('textbox', { name: '编辑原文：Save As…' })).toHaveValue('Save As…')
  await expect(existingRow.getByRole('textbox', { name: '编辑译文：Save As…' })).toHaveValue('另存为…')
  await expect(page.getByRole('button', { name: '编辑 Save As…' })).toHaveCount(0)

  const blankRow = page.getByRole('row').filter({ has: page.getByRole('textbox', { name: '新词条原文' }) })
  await expect(blankRow.getByRole('textbox', { name: '新词条原文' })).toBeVisible()
  await blankRow.getByRole('textbox', { name: '新词条原文' }).fill('Close')
  await blankRow.getByRole('textbox', { name: '新词条译文' }).fill('关闭')
  await blankRow.getByRole('textbox', { name: '新词条译文' }).press('Enter')

  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await expect(blankRow.getByRole('textbox', { name: '新词条原文' })).toHaveValue('')
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/dictionary-inline-draft.png' })

  await page.getByRole('button', { name: '返回词典列表' }).click()
  const guard = page.getByRole('dialog', { name: '放弃未保存更改？' })
  await expect(guard).toBeVisible()
  await guard.getByRole('button', { name: '继续编辑' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '放弃未保存更改？' })).toBeVisible()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '继续编辑' }).click()
  await page.getByRole('button', { name: '关闭窗口' }).click()
  await expect(page.getByRole('dialog', { name: '放弃未保存更改？' })).toBeVisible()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '继续编辑' }).click()
  await page.getByRole('button', { name: '返回词典列表' }).click()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '词典', exact: true })).toBeVisible()
})

test('dictionary text editing saves only source and translation', async ({ page }) => {
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('textbox', { name: '编辑译文：Save As…' }).fill('另存一个副本…')
  await page.getByRole('textbox', { name: '新词条原文' }).fill('Close')
  await page.getByRole('textbox', { name: '新词条译文' }).fill('关闭')
  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保存词典' }).click()

  const saved = await page.evaluate(() => {
    const model = JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3') ?? '{}')
    return model.dictionaryDetails?.['dictionary-proof']?.entries
  })
  expect(saved[1]).toEqual({
    source: 'Save As…',
    translation: '另存一个副本…',
  })
  expect(saved[2]).toEqual({ source: 'Close', translation: '关闭' })
})
