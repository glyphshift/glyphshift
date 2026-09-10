import { selectLanguage } from './fixtures/languageSelect'
import { expect, test } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

test('large dictionary renders bounded pages and preserves edits across pages', async ({ page }, testInfo) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaryDetails['dictionary-proof'].entries = Array.from({ length: 550 }, (_, index) => ({ source: `Entry ${index}`, translation: `Translation ${index}` }))
  snapshot.dictionaries[0].entryCount = 550
  await replaceModel(page, snapshot)
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(50)
  await page.getByRole('textbox', { name: '编辑译文：Entry 0', exact: true }).fill('Edited')
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('textbox', { name: '编辑原文：Entry 50', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '上一页', exact: true }).click()
  await expect(page.getByRole('textbox', { name: '编辑译文：Entry 0', exact: true })).toHaveValue('Edited')
  await page.getByRole('combobox', { name: '每页数量' }).click()
  await expect(page.getByRole('option')).toHaveText(['50', '100', '200'])
  await page.getByRole('option', { name: '200', exact: true }).click()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(200)
  await page.getByRole('button', { name: '末页', exact: true }).click()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(150)
  await page.getByRole('textbox', { name: '搜索字典词条' }).fill('Entry 549')
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(1)
  await page.getByRole('textbox', { name: '编辑原文：Entry 549', exact: true }).fill('Entry 0')
  await expect(page.getByRole('button', { name: '保存字典', exact: true })).toBeDisabled()
  await page.getByRole('textbox', { name: '搜索字典词条' }).clear()
  await page.getByRole('combobox', { name: '每页数量' }).click()
  await page.getByRole('option', { name: '100', exact: true }).click()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(100)
  await page.screenshot({ path: testInfo.outputPath('dictionary-pagination.png') })

})

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('dictionary library reports skipped artifacts while keeping valid dictionaries usable', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.artifactWarnings = [{
    artifactKind: 'dictionary',
    artifactId: 'dictionary.legacy',
    issue: 'invalid',
  }]
  await page.addInitScript(({ key, value }) => {
    localStorage.setItem(key, JSON.stringify(value))
  }, { key: storageKey, value: snapshot })
  await page.goto('/')
  await page.getByRole('button', { name: '字典', exact: true }).click()

  await expect(page.getByText('部分字典已跳过', { exact: true })).toBeVisible()
  await expect(page.getByText(/dictionary\.legacy/)).toBeVisible()
  await expect(page.getByTestId('dictionary-management-table')).toBeVisible()
})
test('dictionary library separates local provenance from the offline catalog mode', async ({ page }) => {
  await page.getByRole('button', { name: '字典', exact: true }).click()

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
  await expect(page.getByText('在线字典目录尚未配置或暂时不可用，本地字典不受影响。')).toBeVisible()
  await expect(page.getByText('第 1 页，本页 0 个版本')).toBeVisible()
  await expect(page.getByRole('button', { name: '新建字典' })).toHaveCount(0)
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
      name: '导入字典',
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 35 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_dictionary') return { ...current.dictionaryDetails['dictionary-proof'], metadata: imported.dictionaries[1].metadata }
        if (command === 'plugin:dialog|open') return 'X:\\SyntheticFixtures\\dictionary-imported.json'
        if (command === 'desktop_preview_dictionary_import') return { metadata: imported.dictionaries[1].metadata, entries: [{ source: 'Save', translation: '保存' }] }
        if (command === 'desktop_create_dictionary') {
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

  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await page.getByRole('dialog', { name: '导入字典' }).getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByRole('row').filter({ hasText: '导入字典' })).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    (window as unknown as { __dictionaryImport?: { inputPath?: string } }).__dictionaryImport
  ))).toEqual(expect.objectContaining({ create: expect.objectContaining({ entries: [{ source: 'Save', translation: '保存' }], metadata: expect.objectContaining({ name: '导入字典' }) }) }))

  await page.getByRole('row').filter({ hasText: '导入字典' }).dblclick()
  await page.getByRole('button', { name: '字典操作' }).click()
  await page.getByRole('menuitem', { name: '导出 JSON', exact: true }).click()
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 35 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_dictionary') return snapshot.dictionaryDetails['dictionary-proof']
        if (command === 'plugin:dialog|save') throw new Error('synthetic save dialog failure')
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot: model })
  await replaceModel(page, model)

  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await page.getByRole('button', { name: '字典操作' }).click()
  await page.getByRole('menuitem', { name: '导出 JSON', exact: true }).click()
  await expect(page.getByRole('alert')).toContainText('导出失败')
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

  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByRole('button', { name: '返回字典列表' })).toBeVisible()

  await expect(page.getByRole('button', { name: '探针', exact: true })).toHaveCount(0)

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

  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('row').filter({ hasText: '界面基础词典' }).dblclick()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('heading', { name: '字典', exact: true })).toBeVisible()

  await expect(page.getByRole('button', { name: '探针', exact: true })).toHaveCount(0)

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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 35 }
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

  await page.getByRole('button', { name: '字典', exact: true }).click()
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 35 }
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

  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '在线目录', exact: true }).click()
  await page.getByRole('button', { name: '更新', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '替换本地字典' })
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 35 }
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

test('dictionary editor separates portable font preferences from adapter configuration', async ({ page }) => {
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByRole('heading', { name: '界面基础词典' })).toBeVisible()
  await expect(page.getByText('en-US → zh-CN')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Save As…' })).toHaveValue('另存为…')
  await expect(page.getByRole('columnheader', { name: '位置', exact: true })).toHaveCount(0)
  await expect(page.getByRole('columnheader', { name: '语境', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '字典操作' }).click()
  await page.getByRole('menuitem', { name: '字典设置' }).click()
  const settings = page.getByRole('dialog', { name: '字典设置' })
  await expect(settings.getByRole('button', { name: '源语言', exact: true })).toContainText('en-US')
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

test('dictionary editor saves the complete portable metadata set', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '字典操作' }).click()
  await page.getByRole('menuitem', { name: '字典设置' }).click()
  let settings = page.getByRole('dialog', { name: '字典设置' })
  await expect(settings.getByRole('button', { name: '字典字体', exact: true })).toHaveCount(0)
  await expect(settings.getByRole('switch', { name: '使用字典字号', exact: true })).toHaveCount(0)
  await selectLanguage(page, settings.getByRole('button', { name: '源语言', exact: true }), 'fr-FR')
  await selectLanguage(page, settings.getByRole('button', { name: '目标语言', exact: true }), 'de-DE')
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
  await page.getByRole('button', { name: '保存字典' }).click()
  await expect(page.getByText('未保存', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '返回字典列表' }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('button', { name: '字典操作' }).click()
  await page.getByRole('menuitem', { name: '字典设置' }).click()
  settings = page.getByRole('dialog', { name: '字典设置' })
  await expect(settings.getByRole('button', { name: '源语言', exact: true })).toContainText('fr-FR')
  await expect(settings.getByRole('button', { name: '目标语言', exact: true })).toContainText('de-DE')
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
  await page.getByRole('button', { name: '字典', exact: true }).click()
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

  await page.getByRole('button', { name: '返回字典列表' }).click()
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
  await page.getByRole('button', { name: '返回字典列表' }).click()
  await page.getByRole('dialog', { name: '放弃未保存更改？' }).getByRole('button', { name: '放弃更改' }).click()
  await expect(page.getByRole('heading', { name: '字典', exact: true })).toBeVisible()
})

test('dictionary text editing saves only source and translation', async ({ page }) => {
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await page.getByRole('textbox', { name: '编辑译文：Save As…' }).fill('另存一个副本…')
  await page.getByRole('textbox', { name: '新词条原文' }).fill('Close')
  await page.getByRole('textbox', { name: '新词条译文' }).fill('关闭')
  await expect(page.getByText('未保存', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '保存字典' }).click()

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


test('dictionary list batch exports only selected dictionaries into one new folder', async ({ page }) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaries.push({ ...snapshot.dictionaries[0]!, metadata: { ...snapshot.dictionaries[0]!.metadata, id: 'dictionary-second', name: '第二份字典' } })
  await page.addInitScript(({ snapshot }) => {
    (window as any).__batchExports = []
    ;(window as any).__TAURI_INTERNALS__ = { invoke: async (command: string, args: any) => {
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
      if (command === 'plugin:dialog|open') return (window as any).__cancelBatch ? null : 'X:/SyntheticFixtures/exports'
      if (command === 'desktop_export_dictionary') { if ((window as any).__failBatch && args.dictionaryId === 'dictionary-second') throw new Error('synthetic export failure'); (window as any).__batchExports.push(args); return null }
      return null
    } }
  }, { snapshot })
  await page.reload()
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await expect(page.getByRole('button', { name: /^导出字典 / })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '批量导出' })).toHaveCount(0)
  await page.getByRole('checkbox', { name: /选择.*界面基础词典/ }).check()
  await page.getByRole('checkbox', { name: /选择.*第二份字典/ }).check()
  await page.getByRole('button', { name: '批量导出' }).click()
  const dialog = page.getByRole('dialog', { name: '批量导出' })
  await dialog.getByRole('combobox', { name: '文件格式' }).click()
  await page.getByRole('option', { name: 'CSV', exact: true }).click()
  await dialog.getByRole('button', { name: '导出', exact: true }).click()
  await expect(dialog).toContainText('已导出 2/2')
  const exports = await page.evaluate(() => (window as any).__batchExports)
  expect(exports.map((item: any) => item.dictionaryId).sort()).toEqual(['dictionary-proof', 'dictionary-second'])
  expect(exports[0].outputPath).toMatch(/^X:\/SyntheticFixtures\/exports\/glyphshift-export-[^/]+\/dictionary-proof.csv$/)
  expect(exports[1].outputPath.replace(/[^/]+$/, '')).toBe(exports[0].outputPath.replace(/[^/]+$/, ''))
  await dialog.getByRole('button', { name: '关闭', exact: true }).last().click()
  await page.evaluate(() => { (window as any).__cancelBatch = true })
  await page.getByRole('button', { name: '批量导出' }).click()
  await dialog.getByRole('button', { name: '导出', exact: true }).click()
  await expect(dialog).toBeVisible()
  expect(await page.evaluate(() => (window as any).__batchExports.length)).toBe(2)
  await page.evaluate(() => { (window as any).__cancelBatch = false; (window as any).__failBatch = true })
  await dialog.getByRole('button', { name: '导出', exact: true }).click()
  await expect(dialog).toContainText('已导出 1/2')
  await expect(dialog.getByRole('alert')).toContainText('部分字典导出失败')

})


test('dictionary selects across pages and clears after dialog confirmation', async ({ page }, testInfo) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaryDetails['dictionary-proof'].entries = Array.from({ length: 120 }, (_, i) => ({ source: `Entry ${i}`, translation: '' }))
  snapshot.dictionaries[0].entryCount = 120
  await replaceModel(page, snapshot)
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  const all = page.getByRole('checkbox', { name: '全选当前搜索结果', exact: true })
  await all.check()
  await expect(page.getByText('120 条词条已选择', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(all).toBeChecked()
  await all.uncheck()
  await page.getByRole('textbox', { name: '搜索字典词条' }).fill('Entry 119')
  await all.check()
  await expect(page.getByText('1 条词条已选择', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: '字典操作', exact: true }).click()
  await page.getByRole('menuitem', { name: '清空字典', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '清空字典', exact: true })
  const confirm = dialog.getByRole('button', { name: '确认清空', exact: true })
  await expect(dialog).toContainText('120')
  await expect(confirm).toBeEnabled()
  await expect(dialog.getByRole('textbox')).toHaveCount(0)
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(page.getByRole('textbox', { name: '编辑原文：Entry 119', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '字典操作', exact: true }).click()
  await page.getByRole('menuitem', { name: '清空字典', exact: true }).click()
  await page.screenshot({ path: testInfo.outputPath('clear-dictionary-confirm.png') })
  await confirm.click()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(0)
  await page.getByRole('button', { name: '保存字典', exact: true }).click()
  const count = await page.evaluate(() => JSON.parse(localStorage.getItem('glyphshift.composable-product-model.v3')!).dictionaryDetails['dictionary-proof'].entries.length)
  expect(count).toBe(0)
})

test('dictionary import displays the native CSV line and reason without writing', async ({ page }, testInfo) => {
  await page.addInitScript(snapshot => {
    const w = window as any
    w.__importWrites = 0
    w.__TAURI_INTERNALS__ = { invoke: async (command: string) => {
      if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', checkUpdatesOnStartup: false }
      if (command === 'desktop_status') return { shellReady: true, productVersion: '0.3.0', apiVersion: 35 }
      if (command === 'desktop_snapshot') return snapshot
      if (command === 'plugin:dialog|open') return 'X:\\SyntheticFixtures\\malformed.csv'
      if (command === 'desktop_preview_dictionary_import') throw { schemaVersion: 1, code: 'import.csv_quote', args: { line: 478 } }
      if (command === 'desktop_create_dictionary') w.__importWrites++
      return null
    } }
  }, model)
  await page.goto('/')
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '导入', exact: true }).click()
  await expect(page.getByText(/^malformed.csv: 第 478 行：文字里的双引号没有转义/)).toBeVisible()
  expect(await page.evaluate(() => (window as any).__importWrites)).toBe(0)
  await page.screenshot({ path: testInfo.outputPath('csv-line-error.png') })
})
