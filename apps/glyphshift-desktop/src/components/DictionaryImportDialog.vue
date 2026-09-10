<script setup lang="ts">
import LanguageSelect from './LanguageSelect.vue'
import { computed, ref } from 'vue'
import DictionaryImportMetadata from './DictionaryImportMetadata.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import type { DictionaryMetadata } from '../model'
import { translateCommandError } from '../commandError'
import { dictionaryFormats, previewImportFiles, emptyImportMetadata as emptyMetadata, type ImportFormat, type ImportFile } from '../dictionaryTransfer'
import { combineImportEntries, type DictionaryImportData } from '../dictionaryImport'

const props = defineProps<{ existing?: boolean; apply: (data: DictionaryImportData) => Promise<boolean> }>()
const { t } = useI18n()
const visible = ref(false)
const busy = ref(false)
const error = ref('')
const files = ref<ImportFile[]>([])
const layout = ref('separate')
const priority = ref<'first' | 'last'>('first')
const sourceLocale = ref('')
const targetLocale = ref('')
const completed = ref(0)
const entries = computed(() => combineImportEntries(files.value.map(file => file.entries), priority.value))
const metadata = ref<DictionaryMetadata>(emptyMetadata(''))
const mode = ref<DictionaryImportData['mode']>('overwrite')
const confirmedReplace = ref(false)
const modes = computed(() => ['append', 'overwrite', 'replace'].map(value => ({ value, label: t(`dictionaryImport.modes.${value}`) })))
const separate = computed(() => !props.existing && files.value.length > 1 && layout.value === 'separate')
const outputs = computed(() => separate.value ? files.value : [{ metadata: metadata.value, entries: entries.value }])
const valid = computed(() => files.value.length > 0 && (props.existing
  ? mode.value !== 'replace' || confirmedReplace.value
  : outputs.value.every(({ metadata: m }) => m.name.trim() && m.sourceLocale.trim() && m.targetLocale.trim() && m.releaseVersion.trim())))
function applyLanguages() {
  for (const file of files.value.slice(completed.value)) {
    if (sourceLocale.value.trim()) file.metadata.sourceLocale = sourceLocale.value.trim()
    if (targetLocale.value.trim()) file.metadata.targetLocale = targetLocale.value.trim()
  }
}
async function choose(format?: ImportFormat) {
  if (busy.value) return
  error.value = ''
  busy.value = true
  try {
    const paths = await open({ multiple: true, filters: [{ name: format?.toUpperCase() ?? 'JSON / CSV / SRT', extensions: format ? [format] : dictionaryFormats.filter(item => item.importable).map(item => item.extension) }] })
    if (!paths) return
    const pending = await previewImportFiles(typeof paths === 'string' ? [paths] : paths)
    if (!pending.length) return
    files.value = pending
    metadata.value = structuredClone(pending[0]!.metadata)
    layout.value = 'separate'
    priority.value = 'first'
    sourceLocale.value = ''
    targetLocale.value = ''
    completed.value = 0
    mode.value = 'overwrite'
    confirmedReplace.value = false
    visible.value = true
  } catch (cause) { error.value = cause instanceof Error ? cause.message : translateCommandError(cause) }
  finally { busy.value = false }
}
async function confirm() {
  if (busy.value || !valid.value) return
  busy.value = true
  error.value = ''
  try {
    // Advance only after a successful write, so a retry never recreates completed dictionaries.
    for (; completed.value < outputs.value.length; completed.value++) {
      const item = outputs.value[completed.value]!
      if (!await props.apply({ entries: item.entries, metadata: item.metadata, mode: mode.value })) {
        error.value = `${item.metadata.name}: ${t('dictionaryImport.saveFailed')}`
        return
      }
    }
    visible.value = false
  } catch (cause) { error.value = cause instanceof Error ? cause.message : translateCommandError(cause) }
  finally { busy.value = false }
}
defineExpose({ choose })
</script>

<template>
  <UAlert v-if="error && !visible" role="alert" color="error" :title="t('dictionaryImport.failed')" :description="error" class="mb-3" />
  <ManagementFormModal :open="visible" :title="t(existing ? 'dictionaryImport.entriesTitle' : 'dictionaryImport.createTitle')" :confirm-label="t('dictionaryImport.confirm')" :confirm-disabled="!valid" :busy="busy" @update:open="visible = $event" @confirm="confirm">
    <p class="type-metadata mb-4">{{ t('dictionaryImport.count', { count: entries.length }) }}</p>
    <template v-if="existing">
      <UFormField :label="t('dictionaryImport.mode')">
        <USelect v-model="mode" :items="modes" value-key="value" :aria-label="t('dictionaryImport.mode')" class="w-full" @update:model-value="confirmedReplace = false" />
      </UFormField>
      <p class="type-metadata mt-3">{{ t(`dictionaryImport.hints.${mode}`) }}</p>
      <UCheckbox v-if="mode === 'replace'" v-model="confirmedReplace" :label="t('dictionaryImport.replaceConfirm')" class="mt-3" />
      <p class="type-metadata mt-3 text-[var(--text-muted)]">{{ t('dictionaryImport.draftHint') }}</p>
    </template>
    <fieldset :disabled="busy" class="space-y-4">
      <UFormField v-if="!existing && files.length > 1" :label="t('dictionaryImport.layout')">
        <USelect v-model="layout" :disabled="completed > 0" :items="['separate', 'merge'].map(value => ({ value, label: t(`dictionaryImport.${value}`) }))" class="w-full" />
      </UFormField>
      <template v-if="files.length > 1">
        <ol class="type-metadata space-y-1"><li v-for="(file, index) in files" :key="index">{{ index + 1 }}. {{ file.name }}</li></ol>
        <UFormField v-if="!separate" :label="t('dictionaryImport.priority')">
          <USelect v-model="priority" :items="['first', 'last'].map(value => ({ value, label: t(`dictionaryImport.${value}`) }))" class="w-full" />
          <p class="type-metadata mt-2">{{ t('dictionaryImport.emptyHint') }}</p>
        </UFormField>
      </template>
      <template v-if="separate">
        <div class="grid grid-cols-2 gap-3">
          <UFormField :label="t('dictionaryEditor.sourceLocale')"><LanguageSelect v-model="sourceLocale" allow-auto :aria-label="t('dictionaryEditor.sourceLocale')" /></UFormField>
          <UFormField :label="t('dictionaryEditor.targetLocale')"><LanguageSelect v-model="targetLocale" :aria-label="t('dictionaryEditor.targetLocale')" /></UFormField>
        </div>
        <UButton :label="t('dictionaryImport.applyLanguages')" @click="applyLanguages" />
        <fieldset v-for="(file, index) in files" :key="index" :disabled="index < completed" class="rounded border border-[var(--border)] p-3">
          <p class="type-metadata mb-3">{{ file.name }} · {{ file.entries.length }}</p>
          <DictionaryImportMetadata v-model="file.metadata" />
        </fieldset>
      </template>
      <DictionaryImportMetadata v-else-if="!existing" v-model="metadata" />
    </fieldset>
    <p v-if="completed" class="type-metadata mt-3">{{ t('dictionaryImport.completed', { count: completed, total: outputs.length }) }}</p>
    <UAlert v-if="error" role="alert" color="error" :title="t('dictionaryImport.failed')" :description="error" class="mt-3" />
  </ManagementFormModal>
</template>
