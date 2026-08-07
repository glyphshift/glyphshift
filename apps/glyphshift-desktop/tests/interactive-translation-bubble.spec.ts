import { expect, test, type Page } from '@playwright/test'

async function presentBubble(page: Page) {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:interactive-translation-bubble', {
    detail: {
      focusBlockIndex: 1,
      result: {
        partial: false,
        blocks: [
          {
            source: 'Settings',
            anchors: [{ left: 20, top: 40, right: 90, bottom: 64 }],
            granularity: 'control',
            provenance: 'structured',
            translation: '设置',
            translationState: 'translated',
            origin: 'dictionary',
          },
          {
            source: 'Start mission',
            anchors: [{ left: 440, top: 280, right: 570, bottom: 320 }],
            granularity: 'line',
            provenance: 'visual',
            translation: '开始任务',
            translationState: 'translated',
            origin: 'dictionary',
          },
        ],
      },
    },
  })))
}

test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 420, height: 210 })
  await page.goto('/?surface=translation-bubble')
})

test('opens on the nearest OCR block and lets the user browse the bounded result', async ({ page }) => {
  await presentBubble(page)
  const bubble = page.getByTestId('interactive-translation-bubble')

  await expect(bubble).toBeVisible()
  await expect(bubble.getByText('OCR', { exact: true })).toBeVisible()
  await expect(bubble.getByText('Start mission', { exact: true })).toBeVisible()
  await expect(bubble.getByText('开始任务', { exact: true })).toBeVisible()
  await expect(bubble.getByText('2/2', { exact: true })).toBeVisible()

  await bubble.getByRole('button', { name: '上一段' }).click()
  await expect(bubble.getByText('Settings', { exact: true })).toBeVisible()
  await expect(bubble.getByText('设置', { exact: true })).toBeVisible()
  await expect(bubble.getByText('UIA', { exact: true })).toBeVisible()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/interactive-translation-bubble.png' })
})

test('shows an honest dictionary miss and dismisses with Escape', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:interactive-translation-bubble', {
    detail: {
      focusBlockIndex: 0,
      result: {
        partial: true,
        blocks: [{
          source: 'Unknown command',
          anchors: [{ left: 20, top: 40, right: 160, bottom: 64 }],
          granularity: 'line',
          provenance: 'visual',
          translationState: 'missing',
        }],
      },
    },
  })))

  const bubble = page.getByTestId('interactive-translation-bubble')
  await expect(bubble.getByText('Unknown command', { exact: true })).toBeVisible()
  await expect(bubble.getByText('词典中没有匹配译文')).toBeVisible()
  await expect(bubble.getByText('部分文字未翻译')).toBeVisible()

  await page.keyboard.press('Escape')
  await expect(bubble).toHaveCount(0)
})
