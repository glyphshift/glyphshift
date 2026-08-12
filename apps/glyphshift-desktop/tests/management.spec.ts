import { expect, test, type Page } from '@playwright/test'
import { model, replaceModel, storageKey } from './fixtures/productModel'

async function visibleHeadingLevels(page: Page) {
  return page.locator('h1, h2, h3, h4, h5, h6').evaluateAll(elements => elements
    .filter((element) => {
      const html = element as HTMLElement
      const style = getComputedStyle(html)
      return html.getClientRects().length > 0 && style.display !== 'none' && style.visibility !== 'hidden'
    })
    .map(element => ({
      level: Number(element.tagName.slice(1)),
      text: element.textContent?.trim() ?? '',
    })))
}

function expectNoHeadingJumps(headings: Array<{ level: number; text: string }>) {
  expect(headings[0]?.level).toBe(1)
  for (let index = 1; index < headings.length; index += 1) {
    expect(headings[index]!.level, `heading jump before “${headings[index]!.text}”: ${JSON.stringify(headings)}`)
      .toBeLessThanOrEqual(headings[index - 1]!.level + 1)
  }
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: model })
  await page.goto('/')
})

test('compact workflow table keeps object identity and actions in view', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 640 })

  const table = page.getByTestId('workflow-management-table')
  const row = table.getByRole('row').filter({ hasText: '默认创作工作流' })
  const identity = row.locator('.management-table-identity-cell')
  const actions = row.locator('.management-table-actions-cell')

  await expect(table).toBeVisible()
  await expect(identity).toBeVisible()
  await expect(actions).toBeVisible()
  await expect.poll(() => table.evaluate((element) => {
    element.scrollLeft = element.scrollWidth
    return element.scrollWidth > element.clientWidth
  })).toBe(true)

  await expect.poll(async () => {
    const tableBox = await table.boundingBox()
    const identityBox = await identity.boundingBox()
    const actionsBox = await actions.boundingBox()
    if (!tableBox || !identityBox || !actionsBox) return false
    return identityBox.x >= tableBox.x
      && identityBox.x + identityBox.width <= tableBox.x + tableBox.width
      && actionsBox.x >= tableBox.x
      && actionsBox.x + actionsBox.width <= tableBox.x + tableBox.width
  }).toBe(true)

  await table.focus()
  await expect(table).toBeFocused()
  const moreActions = actions.getByRole('button', { name: '更多操作：默认创作工作流' })
  await moreActions.click()
  await expect(page.getByRole('menuitem', { name: '复制 默认创作工作流' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '删除 默认创作工作流' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(moreActions).toBeFocused()
})

test('shell offers a discoverable main-content shortcut and named toolbars', async ({ page }) => {
  const skipLink = page.getByRole('link', { name: /跳到主要内容/ })
  const main = page.getByRole('main', { name: '主要内容' })

  await page.keyboard.press('Tab')
  await expect(skipLink).toBeFocused()
  await expect(skipLink).toBeVisible()
  await page.keyboard.press('Enter')
  await expect(main).toBeFocused()
  await page.keyboard.press('Tab')
  await expect.poll(() => main.evaluate(element => element.contains(document.activeElement))).toBe(true)

  await page.getByRole('button', { name: '切换到浅色主题' }).focus()
  await page.keyboard.press('Alt+m')
  await expect(main).toBeFocused()
  await expect(page.getByRole('toolbar', { name: '工作流页面操作' })).toBeVisible()
  await expect(page.getByRole('toolbar', { name: '列表工具栏' })).toBeVisible()

  await page.getByRole('checkbox', { name: '选择 默认创作工作流' }).click()
  await expect(page.getByRole('toolbar', { name: '批量操作' })).toBeVisible()
})

test('workflow sections and Help keep a continuous visible heading outline', async ({ page }) => {
  await page.getByRole('button', { name: '编辑 默认创作工作流' }).click()
  await expect(page.getByTestId('workflow-editor')).toBeVisible()
  expectNoHeadingJumps(await visibleHeadingLevels(page))

  for (const tab of ['软件与拦截', '翻译词典', '字体策略']) {
    await page.getByRole('tab', { name: tab }).click()
    expectNoHeadingJumps(await visibleHeadingLevels(page))
  }

  await page.getByRole('button', { name: '帮助' }).click()
  expectNoHeadingJumps(await visibleHeadingLevels(page))
})

test('functional copy follows the semantic desktop type ramp', async ({ page }) => {
  const typeRamp = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement)
    return {
      caption: style.getPropertyValue('--type-caption').trim(),
      label: style.getPropertyValue('--type-label').trim(),
      metadata: style.getPropertyValue('--type-metadata').trim(),
      body: style.getPropertyValue('--type-body').trim(),
      sectionTitle: style.getPropertyValue('--type-section-title').trim(),
      pageTitle: style.getPropertyValue('--type-page-title').trim(),
    }
  })

  expect(typeRamp).toEqual({
    caption: '10px',
    label: '11px',
    metadata: '11px',
    body: '12px',
    sectionTitle: '13px',
    pageTitle: '20px',
  })

  const description = page.getByText('按软件目标组合拦截方式、有序词典与字体策略，并持续维持运行期望。')
  await expect(description).toBeVisible()
  await expect.poll(() => description.evaluate(element => getComputedStyle(element).fontSize)).toBe('11px')
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
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/management-table-empty-continuous.png' })

  await replaceModel(page, model)
  const lastRow = page.getByTestId('management-table-body').locator('tbody > tr').last()
  await expect(lastRow).toBeVisible()
  await expect.poll(() => lastRow.evaluate(element => getComputedStyle(element).borderBottomWidth)).toBe('1px')
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/management-table-populated-continuous.png' })
})

test('software creation uses one entry and supports preflight plus foreground capture', async ({ page }) => {
  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(page.getByRole('button', { name: '快速捕获' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '新建软件' }).first()).toBeVisible()

  await page.getByRole('button', { name: '新建软件' }).first().click()
  const dialog = page.getByRole('dialog', { name: '新建软件' })
  await dialog.getByRole('textbox', { name: '软件名称' }).fill('Synthetic Editor')
  await dialog.getByRole('textbox', { name: '程序路径' }).fill('X:\\SyntheticFixtures\\SyntheticEditor.exe')
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeDisabled()
  await expect(dialog.getByText('通过接入检查后才能添加。')).toBeVisible()
  await dialog.getByRole('button', { name: '检查' }).click()
  await expect(dialog.getByText('基础接入条件已通过')).toBeVisible()
  await expect(dialog.getByRole('button', { name: '添加软件' })).toBeEnabled()
  await dialog.getByRole('button', { name: '取消' }).click()

  await page.getByRole('button', { name: '新建软件' }).first().click()
  await dialog.getByRole('button', { name: '按键捕获' }).click()
  await expect(dialog.getByText('等待选择软件', { exact: true })).toBeVisible()
  await expect(dialog.getByText(/切换到目标软件，再按 Ctrl\+Shift\+F8/).first()).toBeVisible()
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

test('management tables share independent persisted column controls', async ({ page }) => {
  const columnsButton = page.getByRole('button', { name: '显示列', exact: true })

  await expect(columnsButton).toBeVisible()
  await columnsButton.click()
  await page.getByRole('menuitemcheckbox', { name: '拦截方式', exact: true }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('columnheader', { name: '拦截方式', exact: true })).toHaveCount(0)

  await page.getByRole('button', { name: '软件', exact: true }).click()
  await expect(columnsButton).toBeVisible()
  await expect(page.getByRole('columnheader', { name: '描述', exact: true })).toBeVisible()

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await columnsButton.click()
  await page.getByRole('menuitemcheckbox', { name: '发布', exact: true }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('columnheader', { name: '发布', exact: true })).toHaveCount(0)

  await page.getByRole('button', { name: '探针', exact: true }).click()
  await columnsButton.click()
  await page.getByRole('menuitemcheckbox', { name: '更新时间', exact: true }).click()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('columnheader', { name: '更新时间', exact: true })).toHaveCount(0)

  await page.reload()
  await expect(page.getByRole('columnheader', { name: '拦截方式', exact: true })).toHaveCount(0)
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

test('dictionary delete names every workflow and probe that blocks it', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
        if (command === 'desktop_snapshot') return current
        if (command === 'desktop_delete_dictionaries') {
          throw {
            schemaVersion: 1,
            code: 'dictionary.referenced',
            args: {
              workflowNames: ['默认创作工作流'],
              probeNames: ['界面巡检探针'],
            },
          }
        }
        return null
      },
    }
    ;(window as unknown as { __TAURI_INTERNALS__: typeof internals }).__TAURI_INTERNALS__ = internals
  }, { current: snapshot })
  await replaceModel(page, snapshot)

  await page.getByRole('button', { name: '词典', exact: true }).click()
  await page.getByRole('button', { name: '删除 界面基础词典' }).click()
  await page.getByRole('dialog', { name: '删除词典' }).getByRole('button', { name: '确认删除' }).click()

  const alert = page.getByRole('alert')
  await expect(alert).toContainText('默认创作工作流')
  await expect(alert).toContainText('界面巡检探针')
})

test('software batch delete keeps a rejected record and explains why', async ({ page }) => {
  const snapshot = JSON.parse(JSON.stringify(model))
  await page.addInitScript(({ current }) => {
    const internals = {
      invoke: async (command: string) => {
        if (command === 'desktop_settings') return { settingsSchemaVersion: 1, localePreference: 'zh-CN', themePreference: 'dark' }
        if (command === 'desktop_status') return { shellReady: true, productVersion: '0.2.0', apiVersion: 30 }
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
  await page.screenshot({ path: '../../local-test/evidence/desktop-screens/workflow-runtime-stopped.png' })
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
