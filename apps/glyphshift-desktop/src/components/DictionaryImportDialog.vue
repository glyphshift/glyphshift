<script setup lang="ts">
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import type { DictionaryEntry, DictionaryMetadata } from '../model'
import { translateCommandError } from '../commandError'
import type { DictionaryImportData } from '../dictionaryImport'

const props = defineProps<{ existing?: boolean; apply: (data: DictionaryImportData) => Promise<boolean> }>()
const { t } = useI18n()
const visible = ref(false)
const busy = ref(false)
const error = ref('')
const entries = ref<DictionaryEntry[]>([])
const metadata = ref<DictionaryMetadata>({ id: '', name: '', description: '', sourceLocale: '', targetLocale: '', releaseVersion: '0.1.0', authors: [], tags: [], license: null, homepage: null })
const mode = ref<DictionaryImportData['mode']>('overwrite')
const confirmedReplace = ref(false)
const modes = computed(() => ['append', 'overwrite', 'replace'].map(value => ({ value, label: t(`dictionaryImport.modes.${value}`) })))
const valid = computed(() => props.existing
  ? mode.value !== 'replace' || confirmedReplace.value
  : Boolean(metadata.value.name.trim() && metadata.value.sourceLocale.trim() && metadata.value.targetLocale.trim() && metadata.value.releaseVersion.trim()))

async function choose() {
  if (busy.value) return
  error.value = ''
  busy.value = true
  try {
    const inputPath = await open({ multiple: false, filters: [{ name: 'JSON / CSV', extensions: ['json', 'csv'] }] })
    if (typeof inputPath !== 'string') return
    const preview = await invoke<{ entries: DictionaryEntry[]; metadata: DictionaryMetadata | null }>('desktop_preview_dictionary_import', { inputPath })
    entries.value = preview.entries
    metadata.value = preview.metadata ?? { id: '', name: inputPath.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, '') ?? '', description: '', sourceLocale: '', targetLocale: '', releaseVersion: '0.1.0', authors: [], tags: [], license: null, homepage: null }
    mode.value = 'overwrite'
    confirmedReplace.value = false
    visible.value = true
  } catch (cause) { error.value = translateCommandError(cause) }
  finally { busy.value = false }
}
async function confirm() {
  if (busy.value || !valid.value) return
  busy.value = true
  error.value = ''
  try {
    if (await props.apply({ entries: entries.value, metadata: metadata.value, mode: mode.value })) visible.value = false
    else error.value = t('dictionaryImport.saveFailed')
  } catch (cause) { error.value = translateCommandError(cause) }
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
    <DictionaryMetadataForm v-else v-model="metadata" />
    <UAlert v-if="error" role="alert" color="error" :title="t('dictionaryImport.failed')" :description="error" class="mt-3" />
  </ManagementFormModal>
</template>
