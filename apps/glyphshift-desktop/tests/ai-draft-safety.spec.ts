import { expect, test } from '@playwright/test'
import { model, storageKey } from './fixtures/productModel'

test('review: preview must not commit an unsaved dictionary deletion', async ({ page }, testInfo) => {
  await page.addInitScript(({ model, storageKey }) => {
    localStorage.setItem(storageKey, JSON.stringify(model))
    localStorage.setItem('glyphshift.ai-profiles.v2', JSON.stringify({defaultProfileId:'synthetic-profile',profiles:[{
      id:'synthetic-profile',name:'Synthetic',protocol:'ollama_chat',baseUrl:'http://127.0.0.1:1/api',modelId:'synthetic',reasoningEffort:'disabled',timeoutMs:30000,maxItemsPerRequest:50,maxConcurrency:1,maxRetries:0,hasCredential:false,credentialRequired:false,
      filterPolicy:{skipPureNumbersOrSymbols:true,skipNumericMeasurements:true,skipSingleCharacter:true,skipTextContainingDigits:false,skipUrls:true,skipEmails:true,skipFilePaths:true,skipShortcuts:true,maxSourceChars:null,excludedPatterns:[]}
    }]}))
  }, {model, storageKey})
  await page.goto('/')
  await page.getByRole('button', {name:'字典',exact:true}).click()
  await page.getByRole('button', {name:'编辑 界面基础词典'}).click()
  const before = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries, storageKey)
  await page.getByRole('row').filter({has:page.getByRole('textbox',{name:'编辑原文：Open',exact:true})}).getByRole('button',{name:/删除/}).click()
  await page.getByRole('dialog').getByRole('button',{name:'删除词条',exact:true}).click()
  expect(await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries, storageKey)).toEqual(before)
  await page.getByRole('button',{name:'AI 翻译选项'}).click()
  await page.getByRole('menuitem',{name:'预览待翻译内容'}).click()
  await expect(page.getByRole('dialog',{name:'AI 翻译预览'})).toBeVisible()
  const after = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries, storageKey)
  await testInfo.attach('persisted-before-after', {body:JSON.stringify({before,after},null,2),contentType:'application/json'})
  await page.screenshot({path:testInfo.outputPath('preview-commits-deletion.png')})
  expect(after, 'Opening an AI preview must leave persistent dictionary entries unchanged').toEqual(before)
  await page.getByRole('dialog',{name:'AI 翻译预览'}).getByRole('button',{name:'关闭',exact:true}).click()
  await page.getByRole('button',{name:'AI 补全',exact:true}).click()
  await expect(page.getByText('先保存字典，再开始 AI 翻译。预览不会保存你的修改。')).toBeVisible()
  await expect(page.getByRole('dialog',{name:'确认 AI 翻译'})).toHaveCount(0)
  expect(await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries, storageKey)).toEqual(before)
})


test('background dictionary updates preserve local edits and new AI results', async ({ page }) => {
  await page.addInitScript(({ model, storageKey }) => localStorage.setItem(storageKey, JSON.stringify(model)), { model, storageKey })
  await page.goto('/')
  await page.getByRole('button', {name:'字典',exact:true}).click()
  await page.getByRole('button', {name:'编辑 界面基础词典'}).click()
  await page.getByRole('textbox',{name:'编辑译文：Open',exact:true}).fill('My manual edit')
  await page.evaluate(async () => {
    const path = '/src/workspace/state.ts'
    const state = await import(/* @vite-ignore */ path)
    const next = JSON.parse(JSON.stringify(state.dictionaryDetail.value))
    next.revision++
    next.entries[1].translation = 'New background result'
    next.entries.push({source:'New source',translation:'New translated result'})
    state.model.value.dictionaryDetails[next.metadata.id] = next
    state.dictionaryDetail.value = next
  })
  await expect(page.getByRole('textbox',{name:'编辑译文：Open',exact:true})).toHaveValue('My manual edit')
  await expect(page.getByRole('textbox',{name:'编辑译文：Save As…',exact:true})).toHaveValue('New background result')
  await expect(page.getByRole('textbox',{name:'编辑译文：New source',exact:true})).toHaveValue('New translated result')
  await page.getByRole('button',{name:'保存字典',exact:true}).click()
  const persisted = await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'], storageKey)
  expect(persisted.entries).toEqual([{source:'Open',translation:'My manual edit'},{source:'Save As…',translation:'New background result'},{source:'New source',translation:'New translated result'}])
  expect(persisted.revision).toBe(5)
})

test('conflicting background updates block saving until an explicit choice', async ({ page }) => {
  await page.addInitScript(({ model, storageKey }) => localStorage.setItem(storageKey, JSON.stringify(model)), { model, storageKey })
  await page.goto('/')
  await page.getByRole('button', {name:'字典',exact:true}).click()
  await page.getByRole('button', {name:'编辑 界面基础词典'}).click()
  await page.getByRole('textbox',{name:'编辑译文：Open',exact:true}).fill('My manual edit')
  await page.evaluate(async () => {
    const path = '/src/workspace/state.ts'
    const state = await import(/* @vite-ignore */ path)
    const next = JSON.parse(JSON.stringify(state.dictionaryDetail.value))
    next.revision++
    next.entries[0].translation = 'Background version'
    state.model.value.dictionaryDetails[next.metadata.id] = next
    state.dictionaryDetail.value = next
  })
  await expect(page.getByText('你的修改和后台更新有冲突')).toBeVisible()
  await expect(page.getByRole('textbox',{name:'编辑译文：Open',exact:true})).toHaveValue('My manual edit')
  await expect(page.getByRole('button',{name:'保存字典',exact:true})).toBeDisabled()
  expect(await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries[0].translation, storageKey)).toBe('Background version')
  await page.getByRole('button',{name:'保留我的修改',exact:true}).click()
  await page.getByRole('button',{name:'保存字典',exact:true}).click()
  expect(await page.evaluate(key => JSON.parse(localStorage.getItem(key)!).dictionaryDetails['dictionary-proof'].entries[0].translation, storageKey)).toBe('My manual edit')
})


test('rebasing a temporarily invalid source edit does not collapse duplicate rows', async ({ page }) => {
  await page.goto('/')
  const result = await page.evaluate(async (base) => {
    const path = '/src/dictionaryDraft.ts'
    const { mergeDictionaryDraft } = await import(/* @vite-ignore */ path)
    const local = structuredClone(base)
    local.entries[1].source = local.entries[0].source
    const remote = structuredClone(base)
    remote.revision++
    remote.entries.push({source:'Background source',translation:'Background result'})
    return mergeDictionaryDraft(base, local, remote)
  }, model.dictionaryDetails['dictionary-proof'])
  expect(result.detail.entries.filter((entry: any) => entry.source === 'Open')).toHaveLength(2)
  expect(result.detail.entries).toContainEqual({source:'Background source',translation:'Background result'})
  expect(result.conflicts).toContain('Open')
})
