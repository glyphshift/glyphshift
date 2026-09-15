import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

async function expectFrameAround(page: import('@playwright/test').Page, selector: string, dialog = false) {
  await page.evaluate(async () => {
    await Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity)
      .map(animation => animation.finished.catch(() => undefined)))
  })
  await expect.poll(() => page.evaluate(({ selector, dialog }) => {
    const element = document.querySelector(selector)!
    const target = (dialog ? element.closest('[role="dialog"]')! : element).getBoundingClientRect()
    const frame = document.querySelector('[data-testid="first-run-tour-spotlight"]')!.getBoundingClientRect()
    return Math.abs(frame.left - Math.max(2, target.left - 8)) < 1
      && Math.abs(frame.top - Math.max(2, target.top - 8)) < 1
      && Math.abs(frame.right - Math.min(innerWidth - 2, target.right + 8)) < 1
      && Math.abs(frame.bottom - Math.min(innerHeight - 2, target.bottom + 8)) < 1
  }, { selector, dialog })).toBe(true)
}

async function resetFirstRun(page: import('@playwright/test').Page, localePreference: 'zh-CN' | 'en-US' = 'zh-CN') {
  await page.goto('/')
  await page.evaluate(locale => {
    localStorage.setItem('glyphshift.app-settings.v1', JSON.stringify({
      settingsSchemaVersion: 1,
      safetyNoticeVersion: 0,
      onboardingVersion: 0,
      localePreference: locale,
      themePreference: 'light',
    }))
  }, localePreference)
  await page.reload()
}

async function finishSettingsTour(page: import('@playwright/test').Page, testInfo?: import('@playwright/test').TestInfo) {
  const guide = page.getByTestId('first-run-guide')
  await guide.getByRole('button', { name: '下一步' }).click()
  for (const [heading, selector] of [
    ['部分软件需要管理员权限', '[data-tour="settings-admin"]'],
    ['配置 AI 翻译', '.tour-settings-ai'],
    ['设置文字处理', '.tour-settings-rules'],
    ['管理最近选过的软件', '.tour-settings-software'],
    ['准备常用字体', '.tour-settings-fonts'],
    ['选择常用翻译语言', '.tour-settings-languages'],
  ]) {
    await expect(guide.getByRole('heading', { name: heading, exact: true })).toBeVisible()
    await expectFrameAround(page, selector!)
    if (testInfo && ['部分软件需要管理员权限', '配置 AI 翻译', '设置文字处理'].includes(heading!)) {
      await page.screenshot({ path: testInfo.outputPath(`settings-${heading}.png`) })
    }
    await guide.getByRole('button', { name: '下一步' }).click()
  }
}

test('first run leads with normal compatibility and advances the tour through real controls', async ({ page }, testInfo) => {
  await resetFirstRun(page)

  const safety = page.getByRole('dialog', { name: '开始前了解一下兼容性' })
  await expect(safety).toBeVisible()
  await expect(safety.getByText('大部分场景可以放心使用', { exact: true })).toBeVisible()
  await expect(safety.getByText('少数场景需要留意', { exact: true })).toBeVisible()
  await expect(safety.getByText('极少数兼容问题可能造成卡顿', { exact: true })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(safety).toBeVisible()
  await safety.getByRole('button', { name: '了解，继续' }).click()

  const guide = page.getByTestId('first-run-guide')
  await expect(guide).toBeVisible()
  await expect(guide.getByRole('heading', { name: '用真实操作完成第一次翻译' })).toBeVisible()
  await expect.poll(() => guide.evaluate(element => {
    const box = element.getBoundingClientRect()
    return Math.abs((box.top + box.height / 2) / innerHeight - 0.4) < 0.01
  })).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('intro.png') })
  await finishSettingsTour(page, testInfo)

  await expect(page.getByTestId('first-run-tour-spotlight')).toBeVisible()
  await expect(guide.getByRole('heading', { name: '创建第一个工作流' })).toBeVisible()
  await expect(guide).toContainText('点击高亮位置继续')

  await expect.poll(() => page.evaluate(() => {
    const cursor = document.querySelector('.first-run-tour-cursor')!
    const card = document.querySelector('[data-testid="first-run-guide"]')!.parentElement!
    return Number(getComputedStyle(cursor).zIndex) > Number(getComputedStyle(card).zIndex)
  })).toBe(true)

  await page.locator('[data-tour="workflow-create"]').click()
  await expect(page.getByRole('heading', { name: '选择要翻译的软件' })).toBeVisible()
  await expect(page.locator('[data-tour="workflow-software-picker"]')).toBeVisible()
  await expect(page.getByTestId('first-run-guide')).toContainText('完成当前操作后自动继续')
  await expect.poll(() => page.evaluate(() => {
    const target = document.querySelector('[data-tour="workflow-software-picker"]')!.closest('[role="dialog"]')!.getBoundingClientRect()
    const frame = document.querySelector('[data-testid="first-run-tour-spotlight"]')!.getBoundingClientRect()
    return frame.left <= target.left && frame.top <= target.top && frame.right >= target.right && frame.bottom >= target.bottom
  })).toBe(true)
})

test('the interactive guide can be dismissed permanently and restarted from settings', async ({ page }) => {
  await resetFirstRun(page)
  await page.getByRole('dialog', { name: '开始前了解一下兼容性' })
    .getByRole('button', { name: '了解，继续' })
    .click()

  const guide = page.getByTestId('first-run-guide')
  await guide.getByRole('button', { name: '不再提示' }).click()
  await expect(guide).toHaveCount(0)
  await page.reload()
  await expect(page.getByTestId('first-run-guide')).toHaveCount(0)

  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('button', { name: '重新运行指引', exact: true }).click()

  const reopened = page.getByTestId('first-run-guide')
  await expect(reopened).toBeVisible()
  await expect(reopened.getByRole('heading', { name: '用真实操作完成第一次翻译' })).toBeVisible()
  await expect(page.getByRole('dialog', { name: '开始前了解一下兼容性' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '设置', exact: true })).toHaveAttribute('aria-current', 'page')
})

test('first-run compatibility and interactive tour follow the selected English locale', async ({ page }) => {
  await resetFirstRun(page, 'en-US')
  const safety = page.getByRole('dialog', { name: 'A quick note about compatibility' })
  await expect(safety).toBeVisible()
  await expect(safety.getByText('Most applications can be used normally', { exact: true })).toBeVisible()
  await safety.getByRole('button', { name: 'Got it, continue' }).click()

  const guide = page.getByTestId('first-run-guide')
  await expect(guide).toBeVisible()
  await expect(guide.getByRole('heading', { name: 'Complete your first translation in the real UI' })).toBeVisible()
  await expect(guide.getByRole('button', { name: "Don't show again" })).toBeVisible()
})

test('acknowledged disclaimer remains accessible from settings without restarting the tour', async ({ page }, testInfo) => {
  await page.goto('/')
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('button', { name: '查看兼容性与免责声明', exact: true }).click()
  const safety = page.getByRole('dialog', { name: '开始前了解一下兼容性' })
  await expect(safety).toBeVisible()
  await expect(safety).toContainText('继续表示你了解兼容性会因软件、版本和界面技术而异。')
  await page.screenshot({ path: testInfo.outputPath('settings-disclaimer.png') })
  await safety.getByRole('button', { name: '了解，继续' }).click()
  await expect(safety).toHaveCount(0)
  await expect(page.getByTestId('first-run-guide')).toHaveCount(0)
  await expect(page.getByRole('button', { name: '查看兼容性与免责声明', exact: true })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('dialog', { name: '开始前了解一下兼容性' })).toHaveCount(0)
})

for (const width of [960, 1440]) {
  test(`tour frames track software dialogs and dictionary settings at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 })
    await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
    await resetFirstRun(page)
    await page.getByRole('button', { name: '了解，继续' }).click()
    await finishSettingsTour(page)
    await expectFrameAround(page, '[data-tour="workflow-create"]')
    await page.locator('[data-tour="workflow-create"]').click()
    await expectFrameAround(page, '[data-tour="workflow-software-picker"]', true)
    const software = page.locator('[data-tour="workflow-software-picker"]')
    await software.getByRole('textbox', { name: '程序路径', exact: true }).fill(model.software[0]!.executablePath!)
    await page.getByRole('dialog').getByRole('button', { name: '确认', exact: true }).click()
    await expect(page.getByTestId('first-run-guide')).toContainText('10 / 13')
    await expectFrameAround(page, '.tour-workflow-dictionary-tab')
    await page.screenshot({ path: testInfo.outputPath('dictionary-settings.png') })
    await page.locator('.tour-workflow-dictionary-tab').click()
    await expect(page.getByTestId('first-run-guide')).toContainText('11 / 13')
    await expectFrameAround(page, '[data-tour="workflow-dictionary-picker"]', true)
    await expect.poll(() => page.getByTestId('first-run-guide').evaluate(element => {
      const box = element.getBoundingClientRect()
      const point = document.elementFromPoint(box.left + 20, box.top + 20)
      return element.contains(point) && box.left >= 0 && box.right <= innerWidth
    })).toBe(true)
    await page.screenshot({ path: testInfo.outputPath('dictionary-picker.png') })
    await page.setViewportSize({ width: width === 960 ? 1100 : 1200, height: 800 })
    await expectFrameAround(page, '[data-tour="workflow-dictionary-picker"]', true)
    await page.getByTestId('first-run-guide').getByRole('button', { name: '不再提示' }).click()
    await expect(page.getByTestId('first-run-guide')).toHaveCount(0)
    await expect(page.locator('[data-tour="workflow-dictionary-picker"]')).toBeVisible()
  })
}
