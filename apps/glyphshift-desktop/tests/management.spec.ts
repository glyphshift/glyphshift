import { expect, test } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})
test('management table body stays continuous for empty and populated states', async ({ page }) => {
  const emptyModel = JSON.parse(JSON.stringify(model))
  emptyModel.workflows = []
  emptyModel.workflowDetails = {}
  await replaceModel(page, emptyModel)

  const tableBody = page.getByTestId('management-table-body')
  const emptyState = tableBody.locator('[data-slot="empty"] > [data-slot="root"]')
  await expect(page.getByText('还没有工作流')).toBeVisible()
  await expect.poll(async () => {
    const bodyBox = await tableBody.boundingBox()
    const emptyBox = await emptyState.boundingBox()
    if (!bodyBox || !emptyBox) return false
    return Math.abs(emptyBox.y - (bodyBox.y + 32)) <= 1
      && Math.abs((emptyBox.y + emptyBox.height) - (bodyBox.y + bodyBox.height)) <= 1
  }).toBe(true)
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/management-table-empty-continuous.png' })

  await replaceModel(page, model)
  const lastRow = page.getByTestId('management-table-body').locator('tbody > tr').last()
  await expect(lastRow).toBeVisible()
  await expect.poll(() => lastRow.evaluate(element => getComputedStyle(element).borderBottomWidth)).toBe('1px')
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/management-table-populated-continuous.png' })
})

test('software creation requires preflight and supports two-step foreground capture', async ({ page }) => {
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('button', { name: '快速捕获' })).toBeVisible()

  await page.getByRole('button', { name: '新增软件' }).click()
  const dialog = page.getByRole('dialog', { name: '新增软件' })
  await dialog.getByRole('textbox', { name: '软件名称' }).fill('Synthetic Editor')
  await dialog.getByRole('textbox', { name: '程序路径' }).fill('X:\\SyntheticFixtures\\SyntheticEditor.exe')
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeDisabled()
  await expect(dialog.getByText('通过接入检查后才能添加。')).toBeVisible()
  await dialog.getByRole('button', { name: '检查' }).click()
  await expect(dialog.getByText('基础接入条件已通过')).toBeVisible()
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeEnabled()
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
    detail: { state: 'armed', shortcut: 'Ctrl+Shift+F8' },
  })))
  await expect(page.getByRole('alert', { name: '等待选择软件' })).toContainText('Ctrl+Shift+F8')
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('glyphshift:software-quick-capture', {
    detail: {
      state: 'captured',
      shortcut: 'Ctrl+Shift+F8',
      preflight: {
        executablePath: 'X:\\SyntheticFixtures\\CapturedEditor.exe',
        executableName: 'CapturedEditor.exe',
        suggestedName: 'CapturedEditor',
        architecture: 'x86_64',
        running: true,
        canAdd: true,
        state: 'ready',
        existingName: null,
      },
    },
  })))
  await expect(dialog.getByRole('textbox', { name: '软件名称' })).toHaveValue('CapturedEditor')
  await expect(dialog.getByRole('textbox', { name: '程序路径' })).toHaveValue('X:\\SyntheticFixtures\\CapturedEditor.exe')
  await expect(dialog.getByText('基础接入条件已通过')).toBeVisible()
  await dialog.getByRole('button', { name: '取消' }).click()
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(dialog).toBeHidden()
})

test('software rows expose a direct delete action', async ({ page }) => {
  await page.getByRole('button', { name: '软件', exact: true }).click()
  const row = page.getByRole('row').filter({ hasText: 'Vector Studio' })

  await expect(row.getByRole('button', { name: '删除 Vector Studio' })).toBeVisible()
})

test('software direct and batch delete remove unreferenced records', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.software.push({
    ...snapshot.software[0],
    id: 'software-disposable-one',
    name: 'Disposable One',
    executableName: 'DisposableOne.exe',
    executablePath: 'X:\\SyntheticFixtures\\DisposableOne.exe',
  }, {
    ...snapshot.software[0],
    id: 'software-disposable-two',
    name: 'Disposable Two',
    executableName: 'DisposableTwo.exe',
    executablePath: 'X:\\SyntheticFixtures\\DisposableTwo.exe',
  })
  await replaceModel(page, snapshot)
  await page.getByRole('button', { name: '软件', exact: true }).click()

  await page.getByRole('button', { name: '删除 Disposable One' }).click()
  await page.getByRole('dialog', { name: '删除软件' }).getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('Disposable One', { exact: true })).toBeHidden()

  await page.getByRole('checkbox', { name: '选择 Disposable Two' }).click()
  await page.getByRole('button', { name: '批量删除' }).click()
  await page.getByRole('dialog', { name: '删除软件' }).getByRole('button', { name: '确认删除' }).click()
  await expect(page.getByText('Disposable Two', { exact: true })).toBeHidden()
})

test('software batch delete keeps a rejected record and explains why', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 24 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_remove_software') {
          throw {
            schemaVersion: 1,
            code: 'software.referenced',
            args: { workflowCount: 1, probeCount: 0 },
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await page.getByRole('checkbox', { name: '选择 Vector Studio' }).click()
  await page.getByRole('button', { name: '批量删除' }).click()
  const confirmation = page.getByRole('dialog', { name: '删除软件' })
  await confirmation.getByRole('button', { name: '确认删除' }).click()

  await expect(page.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(page.getByRole('alert')).toContainText('仍被 1 个工作流使用')
})

test('workflow names a stopped software and exposes its actionable Runtime error', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  snapshot.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: false, active: false,
        translationRequested: true, fontRequested: true,
        translationActive: false, fontActive: false, appliedGeneration: null,
      }],
      errors: {
        'software-proof': {
          schemaVersion: 1,
          code: 'runtime.target_not_found',
          args: {},
        },
      },
    },
  }
  await replaceModel(page, snapshot)

  await expect(page.getByText('需要处理', { exact: true })).toHaveCount(0)
  const stoppedStatus = page.getByRole('button', { name: '软件未启动', exact: true })
  await expect(stoppedStatus).toBeVisible()
  await stoppedStatus.click()
  const details = page.getByTestId('workflow-runtime-issues')
  await expect(details.getByText('Vector Studio', { exact: true })).toBeVisible()
  await expect(details.getByText('没有找到与该程序路径匹配的运行实例；请先启动这个版本的软件。')).toBeVisible()
  await expect(details.getByRole('button', { name: '刷新状态', exact: true })).toBeVisible()
  await page.screenshot({ path: '../../target/local-test/evidence/desktop-screens/workflow-runtime-stopped.png' })
})

test('workflow keeps permission failures distinct from a stopped software', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  snapshot.activations = [{ workflowId: 'workflow-proof', revision: 5 }]
  snapshot.workflowRuntimeStatus = {
    'workflow-proof': {
      workflowId: 'workflow-proof',
      targets: [{
        softwareId: 'software-proof', discovered: true, active: false,
        translationRequested: true, fontRequested: true,
        translationActive: false, fontActive: false, appliedGeneration: null,
      }],
      errors: {
        'software-proof': {
          schemaVersion: 1,
          code: 'runtime.target_access_failed',
          args: {},
        },
      },
    },
  }
  await replaceModel(page, snapshot)

  const failedStatus = page.getByRole('button', { name: '权限不匹配', exact: true })
  await expect(failedStatus).toBeVisible()
  await failedStatus.click()
  await expect(page.getByTestId('workflow-runtime-issues').getByText('无法写入目标软件。它可能已经退出，或正以管理员权限运行。请确认软件仍在运行；若权限更高，请在设置中开启“始终以管理员身份启动”。')).toBeVisible()
})

test('navigation keeps fonts inside workflow targets instead of a separate asset page', async ({ page }) => {
  await expect(page.getByRole('heading', { name: '工作流' })).toBeVisible()
  await expect(page.getByText('ExtTextOutW', { exact: true })).toBeVisible()
  await expect(page.getByText('桌面服务已连接')).toHaveCount(0)
  await expect(page.getByText('本地预览')).toHaveCount(0)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await expect(page.getByRole('heading', { name: '词典' })).toBeVisible()
  await expect(page.getByText('en-US')).toBeVisible()
  await expect(page.getByText('v1.2.0')).toBeVisible()

  await expect(page.getByRole('button', { name: '字体', exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: '工作流', exact: true }).click()
  await expect(page.getByText('字体策略：Synthetic Sans')).toBeVisible()
})
