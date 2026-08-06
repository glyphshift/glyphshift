import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('one-shot text translation moves from setup through shortcut to a dictionary result', async ({ page }) => {
  await page.getByRole('button', { name: '取词翻译' }).click()
  const dialog = page.getByRole('dialog', { name: '取词翻译' })

  await expect(dialog.getByText('一次请求，只读取光标位置')).toBeVisible()
  await expect(dialog.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(dialog.getByText('界面基础词典', { exact: true })).toBeVisible()
  await dialog.getByRole('button', { name: '开始取词' }).click()

  await expect(dialog.getByTestId('interactive-translation-armed')).toContainText('Ctrl+Shift+F9')
  await page.keyboard.press('Control+Shift+F9')
  await expect(dialog.getByTestId('interactive-translation-result')).toBeVisible()
  await expect(dialog.getByText('Open', { exact: true })).toBeVisible()
  await expect(dialog.getByText('打开', { exact: true })).toBeVisible()
  await expect(dialog.getByText('UIA 结构化取词')).toBeVisible()
  await expect(dialog.getByText('探针', { exact: true })).toHaveCount(0)
})

test('a dictionary miss preserves original text and explains the unavailable provider', async ({ page }) => {
  await page.getByRole('button', { name: '取词翻译' }).click()
  const dialog = page.getByRole('dialog', { name: '取词翻译' })
  await dialog.getByRole('button', { name: '开始取词' }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:interactive-translation', {
    detail: {
      state: 'presented',
      shortcut: 'Ctrl+Shift+F9',
      result: {
        partial: true,
        blocks: [{
          source: 'Open',
          anchors: [{ left: 10, top: 20, right: 80, bottom: 44 }],
          granularity: 'control',
          provenance: 'structured',
          translationState: 'missing',
        }],
      },
    },
  })))

  await expect(dialog.getByText('Open', { exact: true })).toBeVisible()
  await expect(dialog.getByText('词典中没有匹配译文')).toBeVisible()
  await expect(dialog.getByText('当前版本尚未配置在线翻译服务', { exact: false })).toBeVisible()
})

test('UIA no-text exposes an explicit bounded OCR retry and labels the visual result', async ({ page }) => {
  await page.getByRole('button', { name: '取词翻译' }).click()
  const dialog = page.getByRole('dialog', { name: '取词翻译' })
  await dialog.getByRole('button', { name: '开始取词' }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:interactive-translation', {
    detail: {
      state: 'failed',
      shortcut: 'Ctrl+Shift+F9',
      error: { schemaVersion: 1, code: 'acquisition.no_text', args: {} },
    },
  })))

  await expect(dialog.getByRole('button', { name: '尝试 OCR' })).toBeVisible()
  await dialog.getByRole('button', { name: '尝试 OCR' }).click()
  await expect(dialog.getByTestId('interactive-translation-armed')).toContainText('只截取这次授权窗口')
  await page.keyboard.press('Control+Shift+F9')

  await expect(dialog.getByTestId('interactive-translation-result')).toBeVisible()
  await expect(dialog.getByText('OCR 视觉取词')).toBeVisible()
})

test('foreground mismatch remains actionable in compact English layout', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('glyphshift.app-settings.v1', JSON.stringify({
    settingsSchemaVersion: 1,
    localePreference: 'en-US',
    themePreference: 'light',
    launchAtStartup: false,
    closeBehavior: 'minimize',
    launchElevated: false,
  })))
  await page.setViewportSize({ width: 960, height: 640 })
  await page.reload()
  await page.getByRole('button', { name: 'Translate text' }).click()
  const dialog = page.getByRole('dialog', { name: 'Text translation' })
  await dialog.getByRole('button', { name: 'Start text request' }).click()
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:interactive-translation', {
    detail: {
      state: 'failed',
      shortcut: 'Ctrl+Shift+F9',
      error: { schemaVersion: 1, code: 'interactive_translation.foreground_mismatch', args: {} },
    },
  })))

  await expect(dialog.getByRole('alert')).toContainText('foreground application is not the selected software')
  await expect(dialog.getByRole('button', { name: 'Retry' })).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Try OCR' })).toHaveCount(0)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/interactive-translation-compact-en.png' })
})
