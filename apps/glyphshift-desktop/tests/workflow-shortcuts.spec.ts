import { expect, test } from '@playwright/test'
import { model, storageKey, workflowEditor } from './fixtures/productModel'

test.beforeEach(async ({ page }) => {
  await page.addInitScript(({key, value}) => { if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify(value)) }, {key:storageKey, value:model})
  await page.goto('/')
})

test('workflow shortcut records keys, cancels, clears, and persists in the saved definition', async ({page}) => {
  await page.getByRole('button', {name:'编辑 默认创作工作流', exact:true}).click()
  const editor = workflowEditor(page)
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
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await recorder.click()
  await page.keyboard.press('Backspace')
  await expect(recorder).toContainText('点击设置快捷键')
  await page.getByRole('button', {name:'保存工作流', exact:true}).click()
  await page.reload()
  await page.getByRole('button', {name:'编辑 默认创作工作流', exact:true}).click()
  await expect(recorder).toContainText('点击设置快捷键')
})

test('a shortcut conflict keeps the workflow draft available to correct', async ({page}) => {
  await page.addInitScript(({snapshot}) => {
    (window as any).__TAURI_INTERNALS__ = {invoke: async (command:string) => {
      if(command==='desktop_snapshot') return snapshot
      if(command==='desktop_status') return {apiVersion:32,productVersion:'0.2.3',shellReady:true}
      if(command==='desktop_settings') return {localePreference:'zh-CN',themePreference:'dark'}
      if(command==='desktop_workflow') return snapshot.workflowDetails['workflow-proof']
      if(command==='desktop_update_workflow') throw {schemaVersion:1,code:'workflow.shortcut_conflict',args:{}}
      return null
    }}
  },{snapshot:model})
  await page.reload()
  await page.getByRole('button', {name:'编辑 默认创作工作流',exact:true}).click()
  const editor=workflowEditor(page)
  const recorder=editor.getByRole('button',{name:'全局快捷键',exact:true})
  await recorder.click()
  await page.keyboard.press('Control+Alt+k')
  await page.getByRole('button',{name:'保存工作流',exact:true}).click()
  await expect(editor.getByText(/快捷键已被其他工作流/)).toBeVisible()
  await expect(recorder).toContainText('Ctrl + Alt + K')
  await expect(editor.locator('[data-testid="workflow-basic-tab"] input').first()).toHaveValue('默认创作工作流')
})

test('probe selection can delete a mixed batch containing a running temporary task', async ({page}) => {
  await page.addInitScript(({snapshot}) => {
    const base={softwareId:'software-proof',dictionaryId:'dictionary-proof',adapterIds:['synthetic.ext-text-out'],livePreviewEnabled:false,observationRevision:0,observedCount:0,ignoredCount:0,droppedObservations:0,previewGeneration:0,createdAtMs:1,updatedAtMs:1,dictionaryRevision:1,dictionaryEntryCount:0,runtimeCapability:null}
    let runs=[{...base,id:'regular',name:'普通任务',status:'ready',quickProbe:false},{...base,id:'temporary',name:'临时任务',status:'running',quickProbe:true}]
    ;(window as any).__TAURI_INTERNALS__={invoke:async(command:string,args:any)=>{
      if(command==='desktop_snapshot')return snapshot
      if(command==='desktop_status')return{apiVersion:32,productVersion:'0.2.3',shellReady:true}
      if(command==='desktop_settings')return{localePreference:'zh-CN',themePreference:'dark'}
      if(command==='desktop_probe_runs')return runs
      if(command==='desktop_delete_probe_runs'){(window as any).__deletedProbeIds=args.runIds;runs=[]}
      return null
    }}
  },{snapshot:model})
  await page.reload()
  await page.getByRole('button',{name:'探针',exact:true}).click()
  await page.getByRole('checkbox',{name:'选择 普通任务',exact:true}).check()
  await page.getByRole('checkbox',{name:'选择 临时任务',exact:true}).check()
  const remove=page.getByRole('button',{name:'删除任务',exact:true})
  await expect(remove).toBeEnabled()
  await remove.click()
  const dialog=page.getByRole('dialog')
  await expect(dialog).toContainText('复用或仍被引用')
  await dialog.getByRole('button',{name:'删除探针任务',exact:true}).click()
  await expect.poll(()=>page.evaluate(()=>(window as any).__deletedProbeIds?.length)).toBe(2)
  await expect(page.getByRole('checkbox',{name:'选择 临时任务',exact:true})).toHaveCount(0)
})
