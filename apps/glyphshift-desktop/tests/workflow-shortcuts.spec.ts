import { expect, test } from '@playwright/test'
import { model, storageKey, workflowEditor } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({key, value}) => { if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value)) }, {key:storageKey, value:model})
  await page.goto('/')
})

test('workflow shortcut records keys, cancels, clears, and persists in the saved definition', async ({page}) => {
  await page.getByRole('button', {name:'编辑 默认创作工作流', exact:true}).click()
  const editor = workflowEditor(page)
  await editor.getByRole('tab', {name:'基础配置', exact:true}).click()
  const recorder = editor.getByRole('button', {name:'全局快捷键', exact:true})
  await expect(recorder).toContainText('点击设置快捷键')
  await recorder.click()
  await expect(recorder).toHaveAttribute('aria-pressed', 'true')
  await expect(recorder).toHaveCSS('border-color', 'rgb(110, 210, 167)')
  await page.screenshot({path:'../../local-test/evidence/workflow-shortcuts/recording.png'})
  await page.setViewportSize({width:960,height:640})
  await expect(recorder).toBeInViewport()
  await page.screenshot({path:'../../local-test/evidence/workflow-shortcuts/recording-compact.png'})
  await page.keyboard.press('Control+Alt+k')
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await recorder.click()
  await page.keyboard.press('Escape')
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await expect(editor).toBeVisible()
  await page.getByRole('button', {name:'保存工作流', exact:true}).click()
  await page.reload()
  await page.getByRole('button', {name:'编辑 默认创作工作流', exact:true}).click()
  await editor.getByRole('tab', {name:'基础配置', exact:true}).click()
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await recorder.click()
  await page.keyboard.press('Backspace')
  await expect(recorder).toContainText('点击设置快捷键')
  await page.getByRole('button', {name:'保存工作流', exact:true}).click()
  await page.reload()
  await page.getByRole('button', {name:'编辑 默认创作工作流', exact:true}).click()
  await editor.getByRole('tab', {name:'基础配置', exact:true}).click()
  await expect(recorder).toContainText('点击设置快捷键')
})

test('a shortcut conflict keeps the workflow draft available to correct', async ({page}) => {
  await page.addInitScript(({snapshot}) => {
    (window as any).__TAURI_INTERNALS__ = {invoke: async (command:string) => {
      if(command==='desktop_snapshot') return snapshot
      if(command==='desktop_status') return {apiVersion:36,productVersion:'0.5.2',shellReady:true}
      if(command==='desktop_settings') return {settingsSchemaVersion:1,safetyNoticeVersion:1,onboardingVersion:1,localePreference:'zh-CN',themePreference:'dark'}
      if(command==='desktop_workflow') return snapshot.workflowDetails['workflow-proof']
      if(command==='desktop_update_workflow') throw {schemaVersion:1,code:'workflow.shortcut_conflict',args:{}}
      return null
    }}
  },{snapshot:model})
  await page.reload()
  await page.getByRole('button', {name:'编辑 默认创作工作流',exact:true}).click()
  const editor=workflowEditor(page)
  await editor.getByRole('tab',{name:'基础配置',exact:true}).click()
  const recorder=editor.getByRole('button',{name:'全局快捷键',exact:true})
  await recorder.click()
  await page.keyboard.press('Control+Alt+k')
  await page.getByRole('button',{name:'保存工作流',exact:true}).click()
  await expect(editor.getByRole('alert')).toContainText('快捷键已被其他工作流')
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await expect(editor.locator('[data-testid="workflow-basic-tab"] input').first()).toHaveValue('默认创作工作流')
})
