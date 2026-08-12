import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

const aiStorageKey = 'glyphshift.ai-profiles.v1'
const filterPolicy = {
  skipPureNumbersOrSymbols: true,
  skipNumericMeasurements: true,
  skipSingleCharacter: true,
  skipTextContainingDigits: false,
  skipUrls: true,
  skipEmails: true,
  skipFilePaths: true,
  skipShortcuts: true,
  maxSourceChars: null,
  excludedPatterns: [],
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => {
    localStorage.setItem(key, JSON.stringify(value))
  }, { key: storageKey, value: model })
})

test('settings creates a default Ollama AI profile without asking for an API key', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: 'AI 翻译' })).toBeVisible()
  await page.getByRole('button', { name: '添加 AI Profile' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI Profile' })
  await dialog.getByRole('textbox', { name: 'Profile 名称' }).fill('本地 Ollama')
  await dialog.getByRole('combobox', { name: '协议' }).click()
  await page.getByRole('option', { name: 'Ollama', exact: true }).click()
  await expect(dialog.getByRole('textbox', { name: 'API Key' })).toHaveCount(0)
  await dialog.getByRole('textbox', { name: '模型 ID' }).fill('qwen3:8b')
  await dialog.getByRole('button', { name: '保存 Profile' }).click()

  await expect(page.getByText('本地 Ollama', { exact: true })).toBeVisible()
  await expect(page.getByText('默认', { exact: true })).toBeVisible()
  await expect(page.getByText(/qwen3:8b/)).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByText('本地 Ollama', { exact: true })).toBeVisible()
})

test('AI profile connection test reports model invocation separately from optional discovery', async ({ page }) => {
  const profiles = {
    defaultProfileId: 'profile.local',
    profiles: [{
      id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
      baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
      timeoutMs: 60_000, maxConcurrency: 1, maxItemsPerRequest: 20,
      maxInputCharsPerRequest: 12_000, filterPolicy, hasCredential: false, credentialRequired: false,
    }],
  }
  await page.addInitScript(({ key, value }) => {
    localStorage.setItem(key, JSON.stringify(value))
  }, { key: aiStorageKey, value: profiles })
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  await page.getByRole('button', { name: '测试“本地 Ollama”连接（会调用模型）' }).click()

  await expect(page.getByText('网络、模型调用与结构化输出正常')).toBeVisible()
  await expect(page.getByText('模型发现未测试，不影响手工填写的模型 ID')).toBeVisible()
})

test('dictionary AI fill translates only eligible blank entries', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries.push(
    { source: 'Close', translation: '' },
    { source: '1920 x 1080', translation: '' },
  )
  snapshot.dictionaries[0].entryCount = 4
  const profiles = {
    defaultProfileId: 'profile.local',
    profiles: [{
      id: 'profile.local',
      name: '本地 Ollama',
      protocol: 'ollama_chat',
      baseUrl: 'http://127.0.0.1:11434/api',
      modelId: 'qwen3:8b',
      timeoutMs: 60_000,
      maxConcurrency: 1,
      maxItemsPerRequest: 20,
      maxInputCharsPerRequest: 12_000,
      filterPolicy,
      hasCredential: false,
      credentialRequired: false,
    }],
  }
  await page.addInitScript(({ modelKey, modelValue, profileKey, profileValue }) => {
    localStorage.setItem(modelKey, JSON.stringify(modelValue))
    localStorage.setItem(profileKey, JSON.stringify(profileValue))
  }, { modelKey: storageKey, modelValue: snapshot, profileKey: aiStorageKey, profileValue: profiles })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 翻译选项' }).click()
  await page.getByRole('menuitem', { name: '预览候选' }).click()
  const preview = page.getByRole('dialog', { name: 'AI 翻译预览' })
  await expect(preview.getByText('将翻译 1 条，跳过 3 条')).toBeVisible()
  await expect(preview.getByText('Close', { exact: true })).toBeVisible()
  await preview.getByRole('button', { name: '翻译 1 条' }).click()

  await expect(page.getByRole('textbox', { name: '编辑译文：Close' })).toHaveValue('关闭')
  await expect(page.getByRole('textbox', { name: '编辑译文：Open' })).toHaveValue('打开')
  await expect(page.getByRole('textbox', { name: '编辑译文：1920 x 1080' })).toHaveValue('')
  await expect(page.getByText('AI 已补全 1 条译文')).toBeVisible()
  await expect(page.getByRole('button', { name: '保存词典' })).toBeEnabled()
})

test('probe AI fill uses the backend full-run plan and CAS writeback', async ({ page }) => {
  const profile = {
    id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
    baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
    timeoutMs: 60_000, maxConcurrency: 1, maxItemsPerRequest: 20,
    maxInputCharsPerRequest: 12_000, filterPolicy, hasCredential: false, credentialRequired: false,
  }
  await page.addInitScript(({ snapshot, aiProfile }) => {
    let applied = false
    let summary = {
      id: 'probe-ai', name: 'AI 探针', softwareId: 'software-proof', dictionaryId: 'dictionary-proof',
      adapterIds: ['synthetic.text-out'], status: 'ready', livePreviewEnabled: false,
      observationRevision: 4, observedCount: 3, ignoredCount: 0, droppedObservations: 0,
      previewGeneration: 1, createdAtMs: 1, updatedAtMs: 2, dictionaryRevision: 3,
      dictionaryEntryCount: 2, runtimeCapability: null, quickProbe: false,
    }
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 29 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_probe_runs') return [summary]
        if (command === 'desktop_probe_run_summary') return summary
        if (command === 'desktop_ai_profiles') return { defaultProfileId: aiProfile.id, profiles: [aiProfile] }
        if (command === 'desktop_probe_run_entries') return {
          observationRevision: 4, dictionaryRevision: summary.dictionaryRevision, page: 1, pageSize: 50, total: 3,
          rows: [
            { source: 'Open', translation: '打开', state: 'translated', adapterIds: ['synthetic.text-out'], count: 2, firstSeenMs: 1, lastSeenMs: 2 },
            { source: 'Close', translation: applied ? '关闭' : '', state: applied ? 'translated' : 'pending', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 2 },
            { source: '25.00fps', translation: '', state: 'pending', adapterIds: ['synthetic.text-out'], count: 1, firstSeenMs: 1, lastSeenMs: 2 },
          ],
        }
        if (command === 'desktop_plan_probe_ai_translation') {
          ;(window as unknown as { __probeAiPlan?: unknown }).__probeAiPlan = args?.request
          return {
            token: 'plan-1', scopeId: 'probe:probe-ai', snapshotRevision: 3,
            sourceLocale: 'en-US', targetLocale: 'zh-CN',
            candidates: [{ itemId: 'probe-row-2', source: 'Close', protectedTokens: [] }],
            skipped: [
              { itemId: 'probe-row-1', source: 'Open', reason: 'already_translated' },
              { itemId: 'probe-row-3', source: '25.00fps', reason: 'numeric_measurement' },
            ],
          }
        }
        if (command === 'desktop_start_ai_translation') return 'job-1'
        if (command === 'desktop_ai_translation_job') return {
          jobId: 'job-1', planToken: 'plan-1', scopeId: 'probe:probe-ai', snapshotRevision: 3,
          status: 'completed', totalCount: 1, completedCount: 1, failedCount: 0,
          results: [{ itemId: 'probe-row-2', source: 'Close', translation: '关闭' }], errors: [],
        }
        if (command === 'desktop_apply_probe_ai_results') {
          ;(window as unknown as { __probeAiApply?: unknown }).__probeAiApply = args?.request
          applied = true
          summary = { ...summary, dictionaryRevision: 4, dictionaryEntryCount: 3 }
          return { probe: summary, appliedCount: 1, skippedCount: 0 }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
    localStorage.setItem('glyphshift.probe.selectedRun', summary.id)
  }, { snapshot: model, aiProfile: profile })
  await page.goto('/')
  await page.getByRole('button', { name: '探针', exact: true }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  await expect(page.getByRole('textbox', { name: /Close.*译文/ })).toHaveValue('关闭')
  await expect(page.getByRole('textbox', { name: /25\.00fps.*译文/ })).toHaveValue('')
  await expect(page.getByText('AI 已写入 1 条译文，保留 0 条期间新增的人工译文。')).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiPlan?: { runId?: string } }
  ).__probeAiPlan)).toEqual(expect.objectContaining({ runId: 'probe-ai' }))
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiApply?: { snapshotRevision?: number } }
  ).__probeAiApply)).toEqual(expect.objectContaining({ snapshotRevision: 3 }))
})
