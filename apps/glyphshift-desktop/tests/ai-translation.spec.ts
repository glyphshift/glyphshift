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
  await expect(dialog.getByRole('spinbutton', { name: '单批超时（秒）' })).toHaveValue('300')
  await expect(dialog.getByRole('spinbutton', { name: '单批最多条目' })).toHaveValue('50')
  await expect(dialog.getByRole('spinbutton', { name: '并发批数' })).toHaveValue('1')
  await expect(dialog.getByRole('spinbutton', { name: '失败重试次数' })).toHaveValue('2')
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
      timeoutMs: 60_000, maxItemsPerRequest: 50, maxConcurrency: 1,
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

test('settings keeps confirmation global while batch size belongs to each AI profile', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: 'AI 翻译执行' })).toBeVisible()
  const confirmation = page.getByRole('switch', { name: '翻译前询问' })
  await expect(page.getByRole('spinbutton', { name: '单批最多条目' })).toHaveCount(0)
  await expect(confirmation).toBeChecked()
  await expect(page.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await confirmation.click()

  await page.getByRole('button', { name: '添加 AI Profile' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI Profile' })
  await dialog.getByRole('textbox', { name: 'Profile 名称' }).fill('自定义批次')
  await dialog.getByRole('combobox', { name: '协议' }).click()
  await page.getByRole('option', { name: 'Ollama', exact: true }).click()
  await dialog.getByRole('textbox', { name: '模型 ID' }).fill('local-model')
  const items = dialog.getByRole('spinbutton', { name: '单批最多条目' })
  await expect(items).toHaveValue('50')
  await items.fill('75')
  await dialog.getByRole('spinbutton', { name: '并发批数' }).fill('3')
  await expect(dialog.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await dialog.getByRole('button', { name: '保存 Profile' }).click()

  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('spinbutton', { name: '单批最多条目' })).toHaveCount(0)
  await expect(page.getByRole('switch', { name: '翻译前询问' })).not.toBeChecked()
  await expect(page.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await page.getByRole('button', { name: '编辑 AI Profile：自定义批次' }).click()
  const editor = page.getByRole('dialog', { name: '编辑 AI Profile' })
  await expect(editor.getByRole('spinbutton', { name: '单批最多条目' })).toHaveValue('75')
  await expect(editor.getByRole('spinbutton', { name: '并发批数' })).toHaveValue('3')
})

test('AI fill asks with token and request policy before submitting', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries.push({ source: 'Close', translation: '' })
  snapshot.dictionaries[0].entryCount = 3
  const profiles = {
    defaultProfileId: 'profile.local',
    profiles: [{
      id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
      baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
      timeoutMs: 60_000, maxItemsPerRequest: 25, maxConcurrency: 1,
      filterPolicy, hasCredential: false, credentialRequired: false,
    }],
  }
  await page.addInitScript(({ modelKey, modelValue, profileKey, profileValue }) => {
    localStorage.setItem(modelKey, JSON.stringify(modelValue))
    localStorage.setItem(profileKey, JSON.stringify(profileValue))
  }, { modelKey: storageKey, modelValue: snapshot, profileKey: aiStorageKey, profileValue: profiles })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()
  const preflight = page.getByRole('dialog', { name: '确认 AI 翻译' })
  await expect(preflight.getByText('预计输入约 402 Token')).toBeVisible()
  await expect(preflight.getByText('1 条 · 1 批')).toBeVisible()
  await expect(preflight.getByText('每批最多 25 条 · 并发 1 批')).toBeVisible()
  await expect(preflight.getByText('单批 60 秒 · 失败重试 2 次')).toBeVisible()
  await expect(preflight.getByText(/秒后自动开始/)).toHaveCount(0)
  await preflight.getByRole('button', { name: '取消' }).click()
  await expect(page.getByRole('textbox', { name: '编辑译文：Close' })).toHaveValue('')

  await page.getByRole('button', { name: 'AI 补全' }).click()
  await preflight.getByRole('button', { name: '开始翻译' }).click()
  await expect(page.getByRole('textbox', { name: '编辑译文：Close' })).toHaveValue('关闭')
})

test('stopping an AI job immediately leaves the running state', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries = [{ source: 'Pending source', translation: '' }]
  snapshot.dictionaries[0].entryCount = 1
  const profile = {
    id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
    baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
    timeoutMs: 60_000, maxItemsPerRequest: 50, maxConcurrency: 3, maxRetries: 2,
    filterPolicy, hasCredential: false, credentialRequired: false,
  }
  await page.addInitScript(({ snapshot, profile }) => {
    let cancelled = false
    const job = () => ({
      jobId: 'job-stop', planToken: 'plan-stop', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
      status: cancelled ? 'cancelled' : 'running', totalCount: 258, completedCount: 0, failedCount: 0,
      totalBatches: 6, batchSize: 50, maxConcurrency: 3, maxRetries: 2,
      finishedBatches: 0, failedBatches: 0, elapsedMs: 31_000, peakConcurrency: 3,
      batches: [
        { batchNumber: 1, itemCount: 50, status: cancelled ? 'cancelled' : 'running', attemptCount: 1, startedAfterMs: 0, elapsedMs: 31_000, lastError: null },
        { batchNumber: 2, itemCount: 50, status: cancelled ? 'cancelled' : 'running', attemptCount: 1, startedAfterMs: 1, elapsedMs: 30_999, lastError: null },
        { batchNumber: 3, itemCount: 50, status: cancelled ? 'cancelled' : 'running', attemptCount: 1, startedAfterMs: 2, elapsedMs: 30_998, lastError: null },
        { batchNumber: 4, itemCount: 50, status: cancelled ? 'cancelled' : 'queued', attemptCount: 0, startedAfterMs: null, elapsedMs: 0, lastError: null },
        { batchNumber: 5, itemCount: 50, status: cancelled ? 'cancelled' : 'queued', attemptCount: 0, startedAfterMs: null, elapsedMs: 0, lastError: null },
        { batchNumber: 6, itemCount: 8, status: cancelled ? 'cancelled' : 'queued', attemptCount: 0, startedAfterMs: null, elapsedMs: 0, lastError: null },
      ],
      results: [], errors: [],
    })
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', confirmAiTranslation: false }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_dictionary') return snapshot.dictionaryDetails[args?.dictionaryId as string]
        if (command === 'desktop_ai_profiles') return { defaultProfileId: profile.id, profiles: [profile] }
        if (command === 'desktop_plan_ai_translation') return {
          token: 'plan-stop', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
          sourceLocale: 'en-US', targetLocale: 'zh-CN',
          candidates: [{ itemId: 'dictionary-row-1', source: 'Pending source', protectedTokens: [] }], skipped: [],
        }
        if (command === 'desktop_start_ai_translation') return 'job-stop'
        if (command === 'desktop_ai_translation_job') return job()
        if (command === 'desktop_cancel_ai_translation') {
          cancelled = true
          return job()
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

  await expect(page.getByText('AI 正在翻译')).toBeVisible()
  await expect(page.getByText('当前请求 3/3 · 已观测并发峰值 3')).toBeVisible()
  await expect(page.getByTestId('ai-batch-1')).toContainText('请求中')
  await expect(page.getByTestId('ai-batch-4')).toContainText('排队')
  await expect(page.getByText('已结束批次').locator('..')).toContainText('0/6')
  await page.getByRole('button', { name: '立即停止' }).click()
  await expect(page.getByText('AI 正在翻译')).toHaveCount(0, { timeout: 500 })
  await expect(page.getByText('AI 翻译已停止')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Pending source' })).toHaveValue('')
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
  await expect(page.getByText(/已补全全部 1 条译文，自动完成 1 批；用时 \d+:\d{2}。/)).toBeVisible()
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
      timeoutMs: 60_000, maxItemsPerRequest: 20, maxConcurrency: 1,
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
      confirmAiTranslation: false,
    }))
  }, { modelKey: storageKey, modelValue: snapshot, profileKey: aiStorageKey, profileValue: profiles, settingsKey: appSettingsStorageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  await expect(page.getByText(/已补全全部 51 条译文，自动完成 3 批；用时 \d+:\d{2}。/)).toBeVisible()
  await expect(page.getByText('AI 翻译批次详情')).toBeVisible()
  await expect(page.getByTestId('ai-batch-3')).toContainText('已完成')
  await page.getByRole('button', { name: '关闭批次详情' }).click()
  await expect(page.getByRole('region', { name: 'AI 翻译实时进度' })).toHaveCount(0)
  await expect(page.getByRole('textbox', { name: '编辑译文：Source 51' })).toHaveValue('AI · Source 51')
})

test('dictionary AI fill shows live and final elapsed time', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.dictionaryDetails['dictionary-proof'].entries = Array.from({ length: 60 }, (_, index) => ({
    source: `Timed source ${index + 1}`,
    translation: '',
  }))
  snapshot.dictionaries[0].entryCount = 60
  const profiles = {
    defaultProfileId: 'profile.local',
    profiles: [{
      id: 'profile.local', name: '本地 Ollama', protocol: 'ollama_chat',
      baseUrl: 'http://127.0.0.1:11434/api', modelId: 'qwen3:8b',
      timeoutMs: 60_000, maxItemsPerRequest: 1, maxConcurrency: 1,
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
      confirmAiTranslation: false,
    }))
  }, { modelKey: storageKey, modelValue: snapshot, profileKey: aiStorageKey, profileValue: profiles, settingsKey: appSettingsStorageKey })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  const progress = page.getByRole('region', { name: 'AI 翻译实时进度' })
  await expect(progress.getByText(/已完成 \d+\/60 条 · 已用时 0:01/)).toBeVisible()
  await expect(progress.getByText('当前请求 1/1 · 已观测并发峰值 1')).toBeVisible()
  await expect(page.getByText(/已补全全部 60 条译文，自动完成 60 批；用时 0:0[4-6]。/)).toBeVisible()
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
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', confirmAiTranslation: false }
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
              batchSize: 2, maxConcurrency: 1, maxRetries: 2, elapsedMs: 2_500, peakConcurrency: 1,
              batches: [
                { batchNumber: 1, itemCount: 2, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 400, lastError: null },
                { batchNumber: 2, itemCount: 1, status: 'failed', attemptCount: 3, startedAfterMs: 401, elapsedMs: 2_099, lastError: { category: 'invalid_request', retryable: false, retryAfterMs: null, providerCode: null, requestId: null, httpStatus: 400, safeMessage: 'Request rejected' } },
              ],
              results: translated.map(item => ({ ...item, translation: `AI · ${item.source}` })),
              errors: [{ category: 'invalid_request', retryable: false, retryAfterMs: null, providerCode: null, requestId: null, httpStatus: 400, safeMessage: 'Request rejected' }],
            }
          }
          return {
            jobId: 'job-2', planToken: 'plan-2', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
            status: 'completed', totalCount: 1, completedCount: 1, failedCount: 0,
            totalBatches: 1, finishedBatches: 1, failedBatches: 0,
            batchSize: 1, maxConcurrency: 1, maxRetries: 2, elapsedMs: 300, peakConcurrency: 1,
            batches: [{ batchNumber: 1, itemCount: 1, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 300, lastError: null }],
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

  await expect(page.getByText(/已完成 2\/3 条，1 条失败；用时 \d+:\d{2}，可重试剩余空白项。/)).toBeVisible()
  await expect(page.getByTestId('ai-batch-2')).toContainText('失败')
  await expect(page.getByTestId('ai-batch-2')).toContainText('第 3 次请求')
  await expect(page.getByTestId('ai-batch-2')).toContainText('Request rejected')
  await expect(page.getByRole('textbox', { name: '编辑译文：Third source' })).toHaveValue('')
  await page.getByRole('button', { name: '重试剩余' }).click()
  await expect(page.getByRole('textbox', { name: '编辑译文：Third source' })).toHaveValue('AI · Third source')
  await expect(page.getByText(/已补全全部 1 条译文，自动完成 1 批；用时 \d+:\d{2}。/)).toBeVisible()
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
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', confirmAiTranslation: false }
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
  await expect(page.getByText(/已写入全部 1 条译文，自动完成 1 批；用时 \d+:\d{2}，保留 0 条期间新增的人工译文。/)).toBeVisible()
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiPlan?: { runId?: string } }
  ).__probeAiPlan)).toEqual(expect.objectContaining({ runId: 'probe-ai' }))
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiApply?: { snapshotRevision?: number } }
  ).__probeAiApply)).toEqual(expect.objectContaining({ snapshotRevision: 3 }))
})
