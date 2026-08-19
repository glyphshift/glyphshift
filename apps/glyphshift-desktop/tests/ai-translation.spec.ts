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

test('translation task empty state uses the shared management surface', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '翻译任务' }).click()

  const currentTask = page.getByTestId('translation-task-current')
  const surface = currentTask.getByTestId('management-workspace-surface')
  await expect(surface).toBeVisible()
  await expect(surface).toHaveClass(/rounded-\[8px\]/)
  await expect(currentTask.getByText('还没有翻译任务', { exact: true })).toBeVisible()

  if (process.env.GLYPHSHIFT_TASK_EMPTY_SCREENSHOT) {
    await page.setViewportSize({ width: 1280, height: 720 })
    await page.screenshot({ path: process.env.GLYPHSHIFT_TASK_EMPTY_SCREENSHOT, fullPage: true })
  }
})

test('settings creates a default Ollama AI profile without asking for an API key', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  await expect(page.getByRole('heading', { name: 'AI 翻译', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '添加 AI 配置' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI 配置' })
  await dialog.getByRole('textbox', { name: '配置名称' }).fill('本地 Ollama')
  await dialog.getByRole('combobox', { name: 'AI 服务' }).click()
  await page.getByRole('option', { name: 'Ollama（本机）', exact: true }).click()
  await expect(dialog.getByRole('textbox', { name: 'API Key（密钥）' })).toHaveCount(0)
  await expect(dialog.getByRole('spinbutton', { name: '每次请求超时（分钟）' })).toHaveCount(0)
  await dialog.getByRole('button', { name: '高级设置' }).click()
  await expect(dialog.getByRole('spinbutton', { name: '每次请求超时（分钟）' })).toHaveValue('30')
  await expect(dialog.getByRole('spinbutton', { name: '每批最多翻译' })).toHaveValue('50')
  await expect(dialog.getByRole('spinbutton', { name: '同时请求数' })).toHaveValue('1')
  await expect(dialog.getByRole('spinbutton', { name: '失败后重试' })).toHaveValue('2')
  await dialog.getByRole('textbox', { name: '模型' }).fill('qwen3:8b')
  await dialog.getByRole('button', { name: '保存配置' }).click()

  await expect(page.getByText('本地 Ollama', { exact: true })).toBeVisible()
  await expect(page.getByText('默认', { exact: true })).toBeVisible()
  await expect(page.getByText(/qwen3:8b/)).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByText('本地 Ollama', { exact: true })).toBeVisible()
})

test('settings creates a Codex subscription profile without endpoint or credential fields', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()
  await page.getByRole('button', { name: '添加 AI 配置' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI 配置' })
  await dialog.getByRole('textbox', { name: '配置名称' }).fill('我的 Codex')
  await dialog.getByRole('combobox', { name: 'AI 服务' }).click()
  await page.getByRole('option', { name: 'Codex 订阅', exact: true }).click()

  await expect(dialog.getByRole('textbox', { name: '服务地址' })).toHaveCount(0)
  await expect(dialog.getByRole('textbox', { name: 'API Key（密钥）' })).toHaveCount(0)
  await expect(dialog.getByText(/使用这台电脑上已经登录的 Codex/)).toBeVisible()
  await expect(dialog.getByRole('combobox', { name: '思考模式' })).toContainText('关闭（翻译推荐）')
  await dialog.getByRole('button', { name: '高级设置' }).click()
  await expect(dialog.getByRole('spinbutton', { name: '同时请求数' })).toHaveValue('1')
  await dialog.getByRole('textbox', { name: '模型' }).fill('gpt-5.6-luna')
  await dialog.getByRole('button', { name: '保存配置' }).click()

  await expect(page.getByText('我的 Codex', { exact: true })).toBeVisible()
  await expect(page.getByText('Codex 订阅', { exact: true }).first()).toBeVisible()
  await expect(page.getByText(/gpt-5\.6-luna/)).toBeVisible()
})

test('DeepSeek profile defaults reasoning off and exposes only truthful effort levels', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()
  await page.getByRole('button', { name: '添加 AI 配置' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI 配置' })
  await dialog.getByRole('textbox', { name: '配置名称' }).fill('DeepSeek 翻译')
  await dialog.getByRole('textbox', { name: '服务地址' }).fill('https://api.deepseek.com')
  await dialog.getByRole('textbox', { name: '模型' }).fill('deepseek-v4-flash')
  const reasoning = dialog.getByRole('combobox', { name: '思考模式' })
  await expect(reasoning).toContainText('关闭（翻译推荐）')
  await expect(dialog.getByText(/开启思考可能明显增加等待时间和费用/)).toBeVisible()
  if (process.env.GLYPHSHIFT_REASONING_SCREENSHOT) {
    await page.setViewportSize({ width: 960, height: 640 })
    await page.screenshot({ path: process.env.GLYPHSHIFT_REASONING_SCREENSHOT, fullPage: true })
  }
  await reasoning.click()
  await expect(page.getByRole('option', { name: '低', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: '中', exact: true })).toHaveCount(0)
  await expect(page.getByRole('option', { name: '高', exact: true })).toBeVisible()
  await expect(page.getByRole('option', { name: '最高', exact: true })).toBeVisible()
  await page.keyboard.press('Escape')
  await dialog.getByRole('textbox', { name: 'API Key（密钥）' }).fill('synthetic-secret')
  await dialog.getByRole('button', { name: '保存配置' }).click()

  await expect(page.getByText('DeepSeek 翻译', { exact: true })).toBeVisible()
  await expect(page.getByText(/思考 关闭（翻译推荐）/)).toBeVisible()
  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await page.getByRole('button', { name: '编辑 AI 配置：DeepSeek 翻译' }).click()
  const editor = page.getByRole('dialog', { name: '编辑 AI 配置' })
  await expect(editor.getByRole('combobox', { name: '思考模式' })).toContainText('关闭（翻译推荐）')
  await expect(editor.getByText('删除已保存的凭据', { exact: true })).toHaveCount(0)
  if (process.env.GLYPHSHIFT_PROFILE_EDIT_SCREENSHOT) {
    await page.setViewportSize({ width: 960, height: 640 })
    await page.screenshot({ path: process.env.GLYPHSHIFT_PROFILE_EDIT_SCREENSHOT, fullPage: true })
  }
  await editor.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '删除 AI 配置：DeepSeek 翻译' }).click()
  const deleteDialog = page.getByRole('dialog', { name: '删除 AI 配置？' })
  await expect(deleteDialog).toContainText('将删除“DeepSeek 翻译”及其已保存的 API Key')
  await deleteDialog.getByRole('button', { name: '取消' }).click()
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

  await page.getByRole('button', { name: '测试 AI 配置“本地 Ollama”' }).click()

  await expect(page.getByText('连接成功，可以开始翻译')).toBeVisible()
  await expect(page.getByText(/模型发现|结构化输出/)).toHaveCount(0)
})

test('settings keeps confirmation global while batch size belongs to each AI profile', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置' }).click()

  const aiSection = page.getByRole('region', { name: 'AI 翻译', exact: true })
  await expect(aiSection).toBeVisible()
  await expect(page.getByRole('heading', { name: 'AI 翻译执行' })).toHaveCount(0)
  await expect(page.getByRole('heading', { name: 'AI 翻译', exact: true })).toHaveCount(1)
  const confirmation = aiSection.getByRole('switch', { name: '翻译前询问' })
  await expect(aiSection.getByRole('heading', { name: 'AI 配置' })).toBeVisible()
  await expect(aiSection.getByRole('button', { name: '添加 AI 配置' })).toBeVisible()
  await expect(page.getByRole('spinbutton', { name: '每批最多翻译' })).toHaveCount(0)
  await expect(confirmation).toBeChecked()
  await expect(page.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await confirmation.click()

  await page.getByRole('button', { name: '添加 AI 配置' }).click()
  const dialog = page.getByRole('dialog', { name: '添加 AI 配置' })
  await dialog.getByRole('textbox', { name: '配置名称' }).fill('自定义批次')
  await dialog.getByRole('combobox', { name: 'AI 服务' }).click()
  await page.getByRole('option', { name: 'Ollama（本机）', exact: true }).click()
  await dialog.getByRole('textbox', { name: '模型' }).fill('local-model')
  await dialog.getByRole('button', { name: '高级设置' }).click()
  const items = dialog.getByRole('spinbutton', { name: '每批最多翻译' })
  await expect(items).toHaveValue('50')
  await items.fill('75')
  await dialog.getByRole('spinbutton', { name: '同时请求数' }).fill('3')
  await expect(dialog.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await dialog.getByRole('button', { name: '保存配置' }).click()

  await page.reload()
  await page.getByRole('button', { name: '设置' }).click()
  await expect(page.getByRole('spinbutton', { name: '每批最多翻译' })).toHaveCount(0)
  await expect(page.getByRole('switch', { name: '翻译前询问' })).not.toBeChecked()
  await expect(page.getByRole('spinbutton', { name: '单批输入 Token 预算' })).toHaveCount(0)
  await page.getByRole('button', { name: '编辑 AI 配置：自定义批次' }).click()
  const editor = page.getByRole('dialog', { name: '编辑 AI 配置' })
  await editor.getByRole('button', { name: '高级设置' }).click()
  await expect(editor.getByRole('spinbutton', { name: '每批最多翻译' })).toHaveValue('75')
  await expect(editor.getByRole('spinbutton', { name: '同时请求数' })).toHaveValue('3')
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
  await expect(preflight.getByText('1 条待翻译文本 · 1 批')).toBeVisible()
  await expect(preflight.getByText('约 389 个输入 Token')).toBeVisible()
  await expect(preflight.getByText(/这里只估算发送的原文/)).toBeVisible()
  await expect(preflight.getByText('每批最多 25 条 · 同时处理 1 批')).toBeVisible()
  await expect(preflight.getByText('由 AI 服务决定')).toBeVisible()
  await expect(preflight.getByText('每次请求最多 1 分钟 · 失败后重试 2 次')).toBeVisible()
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
    let started = false
    let cancelled = false
    const job = () => ({
      jobId: 'job-stop', planToken: 'plan-stop', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
      startedAtMs: 1, finishedAtMs: cancelled ? 2 : null, profileName: profile.name,
      protocol: profile.protocol, modelId: profile.modelId,
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
      usage: null, results: [], errors: [], targetDictionaryId: 'dictionary-proof', origin: 'dictionary',
      appliedCount: 0, skippedCount: 0, writebackError: null, dictionaryLocked: !cancelled,
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
        if (command === 'desktop_start_ai_translation') {
          started = true
          return job()
        }
        if (command === 'desktop_ai_translation_tasks') return { current: started ? job() : null, history: [] }
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

  await expect(page.getByRole('tab', { name: /当前任务/ })).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByText('运行中', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('模型已完成 0/258 条')).toBeVisible()
  await expect(page.getByText('当前请求').locator('..')).toContainText('3/3')
  const activeScreenshotPath = process.env.GLYPHSHIFT_ACTIVE_TASK_SCREENSHOT
  if (activeScreenshotPath) {
    await page.setViewportSize({
      width: Number(process.env.GLYPHSHIFT_TASK_SCREENSHOT_WIDTH ?? 1280),
      height: Number(process.env.GLYPHSHIFT_TASK_SCREENSHOT_HEIGHT ?? 720),
    })
    await page.screenshot({ path: activeScreenshotPath, fullPage: true })
  }
  await page.getByRole('button', { name: '批次详情 0/6' }).click()
  await expect(page.getByTestId('ai-batch-1')).toContainText('请求中')
  await expect(page.getByTestId('ai-batch-4')).toContainText('排队')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
  await expect(page.getByText('任务期间该词典保持只读；其他词典和功能不受影响。停止或完成后自动解锁。')).toBeVisible()
  await expect(page.getByRole('textbox', { name: '编辑译文：Pending source' })).toBeDisabled()
  await page.getByRole('button', { name: '查看任务' }).first().click()
  await page.getByRole('button', { name: '停止翻译' }).click()
  await expect(page.getByRole('button', { name: '停止翻译' })).toHaveCount(0, { timeout: 1_000 })
  await expect(page.getByText('已停止', { exact: true })).toBeVisible()
  await expect(page.getByText('词典正在翻译')).toHaveCount(0)
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
  await page.getByRole('menuitem', { name: '预览待翻译内容' }).click()
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
  await expect(progress.getByText('当前请求 1/1 · 最高同时 1 个请求')).toBeVisible()
  await expect(page.getByText(/已补全全部 60 条译文，自动完成 60 批；用时 0:0[4-9]。/)).toBeVisible({ timeout: 10_000 })
})

test('background translation task reports partial batches, writeback, usage, and model history', async ({ page }) => {
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
    let started = false
    const task = {
      jobId: 'job-1', planToken: 'plan-1', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
      startedAtMs: 1_800_000, finishedAtMs: 1_802_500, profileName: profile.name,
      protocol: profile.protocol, modelId: profile.modelId,
      status: 'completed_with_failures', totalCount: 3, completedCount: 2, failedCount: 1,
      totalBatches: 2, finishedBatches: 2, failedBatches: 1,
      batchSize: 2, maxConcurrency: 1, maxRetries: 2, elapsedMs: 2_500, peakConcurrency: 1,
      usage: { inputTokens: 120, cachedInputTokens: 20, outputTokens: 80, reasoningTokens: 70, totalTokens: 200 },
      batches: [
        { batchNumber: 1, itemCount: 2, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 400, lastError: null, usage: { inputTokens: 120, cachedInputTokens: 20, outputTokens: 80, reasoningTokens: 70, totalTokens: 200 } },
        { batchNumber: 2, itemCount: 1, status: 'failed', attemptCount: 3, startedAfterMs: 401, elapsedMs: 2_099, lastError: { category: 'invalid_request', retryable: false, retryAfterMs: null, providerCode: null, requestId: null, httpStatus: 400, safeMessage: 'Request rejected' }, usage: null },
      ],
      results: [], errors: [], targetDictionaryId: 'dictionary-proof', origin: 'dictionary',
      appliedCount: 2, skippedCount: 0, writebackError: null, dictionaryLocked: false,
    }
    const history = [{
      recordId: 'record-1', startedAtMs: task.startedAtMs, finishedAtMs: task.finishedAtMs,
      scopeKind: 'dictionary', profileName: task.profileName, protocol: task.protocol, modelId: task.modelId,
      status: task.status, totalCount: task.totalCount, completedCount: task.completedCount,
      failedCount: task.failedCount, appliedCount: task.appliedCount, skippedCount: task.skippedCount,
      totalBatches: task.totalBatches, finishedBatches: task.finishedBatches,
      failedBatches: task.failedBatches, requestAttempts: 4, retryAttempts: 2, elapsedMs: task.elapsedMs,
      peakConcurrency: task.peakConcurrency, usage: task.usage, batches: task.batches,
    }]
    const internals = {
      invoke: async (command: string, args?: Record<string, any>) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark', confirmAiTranslation: false }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return snapshot
        if (command === 'desktop_dictionary') return snapshot.dictionaryDetails[args?.dictionaryId as string]
        if (command === 'desktop_ai_profiles') return { defaultProfileId: profile.id, profiles: [profile] }
        if (command === 'desktop_plan_ai_translation') {
          return {
            token: 'plan-1', scopeId: 'dictionary:dictionary-proof', snapshotRevision: 3,
            sourceLocale: 'en-US', targetLocale: 'zh-CN',
            candidates: (args?.request?.items ?? []).map((item: { itemId: string; source: string }) => ({ ...item, protectedTokens: [] })), skipped: [],
          }
        }
        if (command === 'desktop_start_ai_translation') {
          started = true
          return task
        }
        if (command === 'desktop_ai_translation_tasks') return { current: started ? task : null, history: started ? history : [] }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { snapshot, profile })
  await page.goto('/')
  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()

  await page.getByRole('button', { name: 'AI 补全' }).click()

  await expect(page.getByRole('heading', { name: '翻译任务' })).toBeVisible()
  await expect(page.getByText('部分完成', { exact: true })).toBeVisible()
  await expect(page.getByTestId('ai-batch-2')).toContainText('失败')
  await expect(page.getByTestId('ai-batch-2')).toContainText('第 3 次请求')
  await expect(page.getByTestId('ai-batch-2')).toContainText('Request rejected')
  await expect(page.getByTestId('ai-batch-1')).toContainText('缓存输入 20')
  await expect(page.getByTestId('ai-batch-1')).toContainText('思考 70')
  await expect(page.getByText(/200\s*Token/)).toBeVisible()
  await expect(page.getByText('已写入').locator('..')).toContainText('2')
  await page.getByRole('tab', { name: '统计' }).click()
  const statistics = page.getByTestId('translation-task-statistics')
  await expect(statistics).toBeVisible()
  await expect(page.getByText('模型统计', { exact: true })).toHaveCount(0)
  await expect(statistics.getByTestId('translation-statistics-table')).toBeVisible()
  await expect.poll(async () => (await statistics.boundingBox())?.width ?? 0).toBeGreaterThan(1100)
  await expect(statistics.getByText('200', { exact: true })).toBeVisible()
  await expect(statistics.getByRole('button', { name: '筛选 AI 配置' })).toBeVisible()
  await expect(statistics.getByRole('button', { name: '显示列' })).toBeVisible()
  await expect(statistics.getByRole('combobox', { name: '每页数量' })).toBeVisible()
  await expect(statistics.getByRole('columnheader', { name: '连接方式', exact: true })).toHaveCount(0)
  await statistics.getByRole('button', { name: '筛选 AI 配置' }).click()
  await page.getByRole('menuitem', { name: '本地 Ollama', exact: true }).click()
  await expect(statistics.getByText('qwen3:8b', { exact: true })).toBeVisible()
  await statistics.getByRole('textbox', { name: '搜索模型或 AI 配置' }).fill('missing-model')
  await expect(statistics.getByText('没有符合条件的模型统计')).toBeVisible()
  await statistics.getByRole('textbox', { name: '搜索模型或 AI 配置' }).fill('qwen3')
  await statistics.getByRole('button', { name: '显示列' }).click()
  await expect(page.getByRole('menuitemcheckbox', { name: '连接方式', exact: true })).toHaveAttribute('aria-checked', 'false')
  await page.keyboard.press('Escape')
  await page.getByRole('tab', { name: /任务列表/ }).click()
  const taskList = page.getByTestId('translation-task-list')
  await expect(taskList).toBeVisible()
  await expect(page.getByText('最近任务', { exact: true })).toHaveCount(0)
  await expect(taskList.getByTestId('translation-history-table')).toBeVisible()
  await expect(taskList.getByRole('button', { name: '筛选任务状态' })).toBeVisible()
  await expect(taskList.getByRole('button', { name: '显示列' })).toBeVisible()
  await expect(taskList.getByRole('combobox', { name: '每页数量' })).toBeVisible()
  await taskList.getByRole('button', { name: '筛选任务状态' }).click()
  await page.getByRole('menuitem', { name: '已完成', exact: true }).click()
  await expect(taskList.getByText('没有符合当前筛选条件的任务。')).toBeVisible()
  await taskList.getByRole('button', { name: '筛选任务状态' }).click()
  await page.getByRole('menuitem', { name: '部分完成', exact: true }).click()
  await page.setViewportSize({ width: 960, height: 640 })
  const detailButton = page.getByRole('button', { name: '查看 qwen3:8b 的任务详情' })
  await expect(detailButton).toBeVisible()
  const pinnedGeometry = await page.evaluate(() => {
    const table = document.querySelector<HTMLElement>('[data-testid="translation-history-table"]')!.getBoundingClientRect()
    const button = document.querySelector<HTMLElement>('[aria-label="查看 qwen3:8b 的任务详情"]')!.getBoundingClientRect()
    return { tableLeft: table.left, tableRight: table.right, buttonLeft: button.left, buttonRight: button.right }
  })
  expect(pinnedGeometry.buttonLeft).toBeGreaterThanOrEqual(pinnedGeometry.tableLeft)
  expect(pinnedGeometry.buttonRight).toBeLessThanOrEqual(pinnedGeometry.tableRight)
  await detailButton.click()
  await expect(page.getByRole('region', { name: 'qwen3:8b 的任务详情' })).toContainText('思考 70')
  const screenshotPath = process.env.GLYPHSHIFT_TASK_SCREENSHOT
  if (screenshotPath) {
    const screenshotTab = process.env.GLYPHSHIFT_TASK_SCREENSHOT_TAB ?? 'current'
    await page.getByRole('tab', {
      name: screenshotTab === 'statistics' ? '统计' : screenshotTab === 'list' ? /任务列表/ : /当前任务/,
    }).click()
    await page.setViewportSize({
      width: Number(process.env.GLYPHSHIFT_TASK_SCREENSHOT_WIDTH ?? 1280),
      height: Number(process.env.GLYPHSHIFT_TASK_SCREENSHOT_HEIGHT ?? 720),
    })
    await page.screenshot({ path: screenshotPath, fullPage: true })
  }
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
        if (command === 'desktop_start_ai_translation') {
          applied = true
          summary = { ...summary, dictionaryRevision: 4, dictionaryEntryCount: 3 }
          return {
          jobId: 'job-1', planToken: 'plan-1', scopeId: 'probe:probe-ai', snapshotRevision: 3,
          startedAtMs: 1, finishedAtMs: 2, profileName: aiProfile.name,
          protocol: aiProfile.protocol, modelId: aiProfile.modelId,
          status: 'completed', totalCount: 1, completedCount: 1, failedCount: 0,
          totalBatches: 1, finishedBatches: 1, failedBatches: 0,
          batchSize: 1, maxConcurrency: 1, maxRetries: 2, elapsedMs: 300, peakConcurrency: 1,
          usage: { inputTokens: 20, cachedInputTokens: 0, outputTokens: 5, reasoningTokens: 0, totalTokens: 25 },
          batches: [{ batchNumber: 1, itemCount: 1, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 300, lastError: null, usage: null }],
          results: [], errors: [], targetDictionaryId: 'dictionary-proof', origin: 'probe',
          appliedCount: 1, skippedCount: 0, writebackError: null, dictionaryLocked: false,
          }
        }
        if (command === 'desktop_ai_translation_tasks') {
          if (!applied) return { current: null, history: [] }
          return {
            current: {
              jobId: 'job-1', planToken: 'plan-1', scopeId: 'probe:probe-ai', snapshotRevision: 3,
              startedAtMs: 1, finishedAtMs: 2, profileName: aiProfile.name,
              protocol: aiProfile.protocol, modelId: aiProfile.modelId,
              status: 'completed', totalCount: 1, completedCount: 1, failedCount: 0,
              totalBatches: 1, finishedBatches: 1, failedBatches: 0,
              batchSize: 1, maxConcurrency: 1, maxRetries: 2, elapsedMs: 300, peakConcurrency: 1,
              usage: { inputTokens: 20, cachedInputTokens: 0, outputTokens: 5, reasoningTokens: 0, totalTokens: 25 },
              batches: [{ batchNumber: 1, itemCount: 1, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 300, lastError: null, usage: null }],
              results: [], errors: [], targetDictionaryId: 'dictionary-proof', origin: 'probe',
              appliedCount: 1, skippedCount: 0, writebackError: null, dictionaryLocked: false,
            },
            history: [],
          }
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

  await expect(page.getByRole('heading', { name: '翻译任务' })).toBeVisible()
  await expect(page.getByText('探针', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('已写入').locator('..')).toContainText('1')
  await expect.poll(() => page.evaluate(() => (
    window as unknown as { __probeAiPlan?: { runId?: string } }
  ).__probeAiPlan)).toEqual(expect.objectContaining({ runId: 'probe-ai' }))
})
