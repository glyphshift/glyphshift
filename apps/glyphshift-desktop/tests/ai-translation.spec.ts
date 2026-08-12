import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

const aiStorageKey = 'glyphshift.ai-profiles.v2'
const appSettingsStorageKey = 'glyphshift.app-settings.v1'
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

  await expect(page.getByRole('heading', { name: 'AI 翻译', exact: true })).toBeVisible()
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
      timeoutMs: 60_000, maxConcurrency: 1,
      filterPolicy, hasCredential: false, credentialRequired: false,
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

test('settings owns one global AI batch policy instead of repeating it per profile', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: 'AI 翻译批次' })).toBeVisible()
  const items = page.getByRole('spinbutton', { name: '单批最多条目' })
  const tokens = page.getByRole('spinbutton', { name: '单批输入 Token 预算' })
  await expect(items).toHaveValue('100')
  await expect(tokens).toHaveAttribute('aria-valuenow', '16000')
  await items.fill('120')
  await tokens.fill('32000')

  await page.getByRole('button', { name: '添加 AI Profile' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI Profile' })
  await expect(dialog.getByRole('spinbutton', { name: '单批最多条目' })).toHaveCount(0)
  await expect(dialog.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await page.getByRole('button', { name: '取消' }).click()

  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('spinbutton', { name: '单批最多条目' })).toHaveValue('120')
  await expect(page.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveAttribute('aria-valuenow', '32000')
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
  await expect(page.getByText('已补全全部 1 条译文，自动完成 1 批。')).toBeVisible()
  await expect(page.getByRole('button', { name: '保存词典' })).toBeEnabled()
})

test('dictionary AI fill reports all candidates completed across automatic batches', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries = Array.from({ length: 51 }, (_, index) => ({
    source: `Source ${index + 1}`,
    translation: '',
  }))
  snapshot.dictionaries[0].entryCount = 51
  const profiles = {
    defaultProfileId: 'profile.local',
    profiles: [{
      id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
      baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
      timeoutMs: 60_000, maxConcurrency: 1,
      filterPolicy, hasCredential: false, credentialRequired: false,
    }],
  }
  await page.addInitScript(({ modelKey, modelValue, profileKey, profileValue, settingsKey }) => {
    localStorage.setItem(modelKey, JSON.stringify(modelValue))
    localStorage.setItem(profileKey, JSON.stringify(profileValue))
    localStorage.setItem(settingsKey, JSON.stringify({
      settingsSchemaVersion: 1,
      localePreference: 'zh-CN',
      themePreference: 'dark',
      aiTranslationBatch: { maxItemsPerRequest: 20, maxInputTokensPerRequest: 16_000 },
    }))
  }, { modelKey: storageKey, modelValue: snapshot, profileKey: aiStorageKey, profileValue: profiles, settingsKey: appSettingsStorageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  await expect(page.getByText('已补全全部 51 条译文，自动完成 3 批。')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Source 51' })).toHaveValue('AI · Source 51')
})

test('dictionary AI fill exposes partial batches and retries only remaining blanks', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries = [
    { source: 'First source', translation: '' },
    { source: 'Second source', translation: '' },
    { source: 'Third source', translation: '' },
  ]
  snapshot.dictionaries[0].entryCount = 3
  const profile = {
    id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
    baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
    timeoutMs: 60_000, maxConcurrency: 1,
    filterPolicy, hasCredential: false, credentialRequired: false,
  }
  await page.addInitScript(({ snapshot, profile }) => {
    let attempt = 0
    let latestCandidates: Array<{ itemId: string; source: string }> = []
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_dictionary') return snapshot.dictionaryDetails[args?.dictionaryId as string]
        if (command === 'desktop_ai_profiles') return { defaultProfileId: profile.id, profiles: [profile] }
        if (command === 'desktop_plan_ai_translation') {
          latestCandidates = (args?.request?.items ?? [])
            .filter((item: { translation?: string | null }) => !item.translation?.trim())
            .map((item: { itemId: string; source: string }) => ({ itemId: item.itemId, source: item.source }))
          return {
            token: `plan-${attempt + 1}`, scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
            sourceLocale: 'en-US', targetLocale: 'zh-CN',
            candidates: latestCandidates.map(item => ({ ...item, protectedTokens: [] })), skipped: [],
          }
        }
        if (command === 'desktop_start_ai_translation') {
          attempt += 1
          return `job-${attempt}`
        }
        if (command === 'desktop_ai_translation_job') {
          if (attempt === 1) {
            const translated = latestCandidates.slice(0, 2)
            return {
              jobId: 'job-1', planToken: 'plan-1', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
              status: 'completed_with_failures', totalCount: 3, completedCount: 2, failedCount: 1,
              totalBatches: 2, finishedBatches: 2, failedBatches: 1,
              results: translated.map(item => ({ ...item, translation: `AI · ${item.source}` })),
              errors: [{ category: 'invalid_request', retryable: false, retryAfterMs: null, providerCode: null, requestId: null, httpStatus: 400, safeMessage: 'Request rejected' }],
            }
          }
          return {
            jobId: 'job-2', planToken: 'plan-2', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
            status: 'completed', totalCount: 1, completedCount: 1, failedCount: 0,
            totalBatches: 1, finishedBatches: 1, failedBatches: 0,
            results: latestCandidates.map(item => ({ ...item, translation: `AI · ${item.source}` })), errors: [],
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot, profile })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  await expect(page.getByText('已完成 2/3 条，1 条失败；可重试剩余空白项。')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Third source' })).toHaveValue('')
  await page.getByRole('button', { name: '重试剩余' }).click()
  await expect(page.getByRole('textbox', { name: '编辑译文：Third source' })).toHaveValue('AI · Third source')
  await expect(page.getByText('已补全全部 1 条译文，自动完成 1 批。')).toBeVisible()
})

test('probe AI fill uses the backend full-run plan and CAS writeback', async ({ page }) => {
  const profile = {
    id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
    baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
    timeoutMs: 60_000, maxConcurrency: 1,
    filterPolicy, hasCredential: false, credentialRequired: false,
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
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
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
          totalBatches: 1, finishedBatches: 1, failedBatches: 0,
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
  await expect(page.getByText('已写入全部 1 条译文，自动完成 1 批；保留 0 条期间新增的人工译文。')).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiPlan?: { runId?: string } }
  ).__probeAiPlan)).toEqual(expect.objectContaining({ runId: 'probe-ai' }))
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiApply?: { snapshotRevision?: number } }
  ).__probeAiApply)).toEqual(expect.objectContaining({ snapshotRevision: 3 }))
})
