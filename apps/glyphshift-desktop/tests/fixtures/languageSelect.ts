import type { Locator, Page } from '@playwright/test'

export async function selectLanguage(page: Page, trigger: Locator, code: string) {
  await trigger.click()
  await page.getByRole('combobox').filter({ visible: true }).last().fill(code)
  await page.getByRole('option').first().click()
}
