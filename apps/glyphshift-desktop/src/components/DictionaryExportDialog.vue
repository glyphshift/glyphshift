<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { save, open as chooseDirectory } from '@tauri-apps/plugin-dialog'
import { dictionaryFormats, exportDictionaryFile } from '../dictionaryTransfer'
import { translateCommandError } from '../commandError'
import ManagementFormModal from './ManagementFormModal.vue'

const { t } = useI18n()
const visible = ref(false)
const format = ref('json')
const dictionaryId = ref('')
const batchIds = ref<string[]>([])
const result = ref('')
const busy = ref(false)
const error = ref('')
function open(id: string) {
  batchIds.value = []
  result.value = ''
  dictionaryId.value = id
  format.value = 'json'
  error.value = ''
  visible.value = true
}
function openBatch(ids: string[]) {
  if (!ids.length || busy.value) return
  open(ids[0]!)
  batchIds.value = [...ids]
}
async function exportOne(id: string, selectedFormat: 'json' | 'csv') {
  if (busy.value) return
  open(id)
  visible.value = false
  format.value = selectedFormat
  await confirm()
}
async function confirm() {
  if (result.value) { visible.value = false; return }
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    if (batchIds.value.length) {
      const folder = await chooseDirectory({ directory: true, multiple: false, title: t('dictionaryExport.chooseFolder') })
      if (typeof folder !== 'string') return
      const root = `${folder.replace(/[\\/]$/, '')}/glyphshift-export-${crypto.randomUUID()}`
      let completed = 0
      for (const id of batchIds.value) {
        try {
          await exportDictionaryFile(id, `${root}/${encodeURIComponent(id)}.${format.value}`)
          completed++
        } catch { /* Report the full batch outcome after trying each selected dictionary. */ }
      }
      result.value = t('dictionaryExport.batchResult', { completed, total: batchIds.value.length, path: root })
      if (completed !== batchIds.value.length) error.value = t('dictionaryExport.partialFailure')
      return
    }
    const path = await save({ title: t('dictionaryExport.title'), defaultPath: `${dictionaryId.value}.${format.value}`, filters: [{ name: format.value.toUpperCase(), extensions: [format.value] }] })
    if (!path) return
    const outputPath = path.toLowerCase().endsWith(`.${format.value}`) ? path : `${path.replace(/\.(json|csv)$/i, '')}.${format.value}`
    await exportDictionaryFile(dictionaryId.value, outputPath)
    visible.value = false
  } catch (cause) { error.value = translateCommandError(cause); visible.value = true }
  finally { busy.value = false }
}
defineExpose({ openBatch, exportOne })
</script>

<template>
  <ManagementFormModal :open="visible" :title="t(batchIds.length ? 'dictionaryExport.batchTitle' : 'dictionaryExport.title')" :confirm-label="t(result ? 'dictionaryExport.close' : 'dictionaryExport.confirm')" :busy="busy" @update:open="visible = $event" @confirm="confirm">
    <UFormField v-if="!result" :label="t('dictionaryExport.format')">
      <USelect v-model="format" :items="dictionaryFormats.filter(item => item.exportable).map(item => ({ label: item.label, value: item.extension }))" value-key="value" :aria-label="t('dictionaryExport.format')" class="w-full" />
    </UFormField>
    <p v-if="!result" class="type-metadata mt-3">{{ t(`dictionaryExport.${format}Hint`) }}</p>
    <p v-if="batchIds.length && !result" class="type-metadata mt-3">{{ t('dictionaryExport.batchHint', { count: batchIds.length }) }}</p>
    <p v-if="result" class="type-metadata mt-3 break-all">{{ result }}</p>
    <UAlert v-if="error" role="alert" color="error" :title="t('dictionaryExport.failed')" :description="error" class="mt-3" />
  </ManagementFormModal>
</template>
