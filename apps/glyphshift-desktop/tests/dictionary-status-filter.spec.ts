import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  const snapshot = structuredClone(model)
  snapshot.dictionaryDetails['dictionary-proof'].entries = Array.from({ length: 125 }, (_, index) => ({
    source: `Entry ${index}`, translation: index < 60 ? `Translated ${index}` : index === 60 ? '   ' : '',
  }))
  snapshot.dictionaries[0].entryCount = 125
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: snapshot })
  await page.goto('/')
  await page.getByRole('button', { name: '字典', exact: true }).click()
  await page.getByRole('button', { name: '编辑 界面基础词典' }).click()
})

test('dictionary translation filter combines with search, pages all matches and clears hidden selections', async ({ page }, testInfo) => {
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await page.getByRole('checkbox', { name: '选择词条 Entry 50', exact: true }).check()
  const choose = async (name: string) => {
    await page.getByRole('button', { name: '按翻译状态筛选', exact: true }).click()
    await page.getByRole('menuitem', { name, exact: true }).click()
  }
  await choose('未翻译')
  await expect(page.getByRole('textbox', { name: '编辑原文：Entry 60', exact: true })).toBeVisible()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(50)
  await expect(page.getByText('显示 1–50，共 65 条词条', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: '删除所选', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(15)
  await page.getByRole('textbox', { name: '搜索字典词条' }).fill('Entry 60')
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(1)
  await choose('已翻译')
  await expect(page.getByRole('textbox', { name: /^编辑原文：/ })).toHaveCount(0)
  await page.getByRole('textbox', { name: '搜索字典词条' }).clear()
  await expect(page.getByText('显示 1–50，共 60 条词条', { exact: true })).toBeVisible()
  await choose('全部条目')
  await expect(page.getByText('显示 1–50，共 125 条词条', { exact: true })).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('dictionary-filter.png') })
})

test('active dictionary remains readable and filterable without the large lock banner', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 960, height: 720 })
  await page.evaluate(async () => {
    const modulePath = '/src/useAiTranslation.ts'
    const { useAiTranslation } = await import(/* @vite-ignore */ modulePath)
    useAiTranslation().currentJob.value = {
      jobId: 'fixture-running', scopeId: 'dictionary:dictionary-proof', targetDictionaryId: 'dictionary-proof',
      dictionaryLocked: true, status: 'running', totalCount: 125, completedCount: 50, failedCount: 0,
      totalBatches: 3, finishedBatches: 1, failedBatches: 0, maxConcurrency: 1, elapsedMs: 1000,
      batches: [
        { batchNumber: 1, itemCount: 50, status: 'completed', attemptCount: 1, startedAfterMs: 0, elapsedMs: 500, lastError: null, usage: null },
        { batchNumber: 2, itemCount: 50, status: 'running', attemptCount: 1, startedAfterMs: 500, elapsedMs: 500, lastError: null, usage: null },
      ], results: [], errors: [],
    }
  })
  await expect(page.getByText('字典正在翻译', { exact: true })).toHaveCount(1)
  await expect(page.getByText(/AI 正在写这个字典/)).toHaveCount(0)
  await expect(page.getByRole('textbox', { name: '编辑译文：Entry 0', exact: true })).toBeDisabled()
  await page.getByRole('button', { name: '按翻译状态筛选', exact: true }).click()
  await page.getByRole('menuitem', { name: '未翻译', exact: true }).click()
  await expect(page.getByRole('textbox', { name: '编辑原文：Entry 60', exact: true })).toBeInViewport()
  await page.screenshot({ path: testInfo.outputPath('dictionary-translating.png') })
})
