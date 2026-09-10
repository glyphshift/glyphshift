<script setup lang="ts">
import { latestRequestQueue } from '../latestRequestQueue'
import { invoke } from '@tauri-apps/api/core'
import { useAppSettings } from '../appSettings'
import { skipReason } from '../textFilters'

import DictionaryExportDialog from './DictionaryExportDialog.vue'
import DictionaryImportDialog from './DictionaryImportDialog.vue'
import { mergeDictionaryEntries, type DictionaryImportData } from '../dictionaryImport'
import { mergeDictionaryDraft } from '../dictionaryDraft'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { TableColumn, TableRow } from '@nuxt/ui/components/Table.vue'
import type { DropdownMenuItem } from '@nuxt/ui'
import { useI18n } from 'vue-i18n'
import type { DictionaryDetail, DictionaryEntry, DictionaryMetadata } from '../model'
import { useAiTranslation, type AiTranslationPlan } from '../useAiTranslation'
import { usePageEscape } from '../usePageEscape'
import AiTranslationPreflight from './AiTranslationPreflight.vue'
import AiTranslationProgress from './AiTranslationProgress.vue'

interface DictionaryTableRow {
  entry: DictionaryEntry
  index: number
  kind: 'entry' | 'new'
}

const props = defineProps<{ detail: DictionaryDetail; busy: boolean }>()
const emit = defineEmits<{
  back: []
  save: [detail: DictionaryDetail]
  'configure-ai': []
  'open-ai-tasks': []
  'dirty-change': [dirty: boolean]
}>()
const { t } = useI18n()
const ai = useAiTranslation()
const exportDialog = ref<InstanceType<typeof DictionaryExportDialog>>()
const importDialog = ref<InstanceType<typeof DictionaryImportDialog>>()
async function applyImport(data: DictionaryImportData) {
  if (dictionaryLocked.value || props.busy || !commitNewEntry()) return false
  draft.value.entries = mergeDictionaryEntries(draft.value.entries, data.entries, data.mode)
  selected.value = new Set()
  query.value = ''
  translationFilter.value = 'all'
  return true
}

const dictionaryActions = computed<DropdownMenuItem[][]>(() => [[
  { label: t('dictionaryEditor.settings'), icon: 'i-tabler-settings', disabled: dictionaryLocked.value, onSelect: openMetadata },
], [
  ...(['json', 'csv', 'srt'] as const).map(format => ({ label: t(format === 'json' ? 'capture.importJson' : format === 'csv' ? 'capture.importCsv' : 'capture.importSrt'), icon: 'i-tabler-file-import', disabled: dictionaryLocked.value || props.busy, onSelect: () => void importDialog.value?.choose(format) })),
], [
  ...(['json', 'csv'] as const).map(format => ({ label: t(format === 'json' ? 'capture.exportJson' : 'capture.exportCsv'), icon: 'i-tabler-file-export', disabled: props.busy || hasUnsavedChanges.value, onSelect: () => void exportDialog.value?.exportOne(props.detail.metadata.id, format) })),
], [
  { label: t('dictionaryEditor.clearDictionary'), icon: 'i-tabler-trash', color: 'error', disabled: dictionaryLocked.value || props.busy || !draft.value.entries.length, onSelect: () => { clearOpen.value = true } },
]])

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function emptyEntry(): DictionaryEntry {
  return { source: '', translation: '' }
}

const saved = ref<DictionaryDetail>(clone(props.detail))
const draft = ref<DictionaryDetail>(clone(props.detail))
const externalConflicts = ref<string[]>([])
const aiSaveRequired = ref(false)
const selected = ref(new Set<number>())
const textSettings = useAppSettings()
const hideSkipped = ref(localStorage.getItem('glyphshift.dictionary.hide-skipped') === 'true')
const hiddenSources = ref(new Set<string>())
const filterError = ref('')
let filterRequest = 0
const sourceTexts = computed(() => hideSkipped.value ? draft.value.entries.map(entry => entry.source) : [])
const filterQueue = latestRequestQueue(async () => {
  const request = filterRequest
  const sources = [...sourceTexts.value]
  const policy = textSettings.settings.value.textFilterPolicy
  if (!hideSkipped.value) return
  try {
    const mask = '__TAURI_INTERNALS__' in window
      ? await invoke<boolean[]>('desktop_filter_dictionary_sources', { sources })
      : sources.map(source => Boolean(skipReason({ itemId: '', source, translation: null, ignored: false }, policy)))
    if (request === filterRequest) hiddenSources.value = new Set(sources.filter((_, index) => mask[index]))
  } catch { if (request === filterRequest) { hiddenSources.value = new Set(); filterError.value = t('textFilters.failed') } }
})
watch([sourceTexts, () => textSettings.settings.value.textFilterPolicy, hideSkipped], () => {
  filterRequest += 1
  selected.value = new Set()
  filterError.value = ''
  localStorage.setItem('glyphshift.dictionary.hide-skipped', String(hideSkipped.value))
  if (!hideSkipped.value) { hiddenSources.value = new Set(); return }
  void filterQueue.request()
}, { deep: true, immediate: true })
onBeforeUnmount(() => { filterRequest += 1; filterQueue.dispose() })
const query = ref('')
const translationFilter = ref('all')
const translationFilterOptions = computed(() => (['all', 'translated', 'untranslated'] as const).map(value => ({
  value, label: t(`dictionaryEditor.translationFilter.${value}`),
})))
const translationFilterLabel = computed(() => translationFilterOptions.value.find(option => option.value === translationFilter.value)?.label ?? '')
const page = ref(1)
const pageSize = ref(50)
const metadataOpen = ref(false)
const metadataDraft = ref<DictionaryMetadata>(clone(props.detail.metadata))
const newEntry = ref<DictionaryEntry>(emptyEntry())
const pendingRemoval = ref<number[]>([])
const clearOpen = ref(false)
function clearDictionary() {
  if (dictionaryLocked.value || props.busy) return
  draft.value.entries = []
  newEntry.value = emptyEntry()
  selected.value = new Set()
  clearOpen.value = false
}
const allSelected = computed(() => filtered.value.length > 0 && filtered.value.every(row => selected.value.has(row.index)))
const selectionState = computed(() => allSelected.value ? true : filtered.value.some(row => selected.value.has(row.index)) ? 'indeterminate' as const : false)
function toggleAllEntries() {
  if (dictionaryLocked.value || props.busy) return
  const next = new Set(selected.value)
  for (const row of filtered.value) {
    if (allSelected.value) next.delete(row.index)
    else next.add(row.index)
  }
  selected.value = next
}
const selectedProfileId = ref<string | null>(null)
const aiPreviewOpen = ref(false)
const aiPreflightOpen = ref(false)
const aiPlan = ref<AiTranslationPlan | null>(null)
const aiNotice = ref('')
const aiNoticeTone = ref<'success' | 'warning'>('success')
const aiNoticeCancelled = ref(false)
const aiRetryAvailable = ref(false)
const aiNoticeTitle = computed(() => aiNoticeCancelled.value
  ? t('ai.translationCancelled')
  : aiNoticeTone.value === 'warning'
  ? t('ai.partialCompletion')
  : t('ai.translationCompleted'))
const displayedAiJob = computed(() => {
  const job = ai.currentJob.value
  return job?.scopeId === `dictionary:${draft.value.metadata.id}` ? job : null
})
const dictionaryLocked = computed(() => ai.lockedDictionaryId.value === draft.value.metadata.id)

watch(() => props.detail, (value) => {
  const sameDictionary = value.metadata.id === saved.value.metadata.id
  if (sameDictionary && hasUnsavedChanges.value) {
    const merged = mergeDictionaryDraft(saved.value, draft.value, value)
    draft.value = clone(merged.detail)
    externalConflicts.value = [...new Set([...externalConflicts.value, ...merged.conflicts])]
  } else {
    draft.value = clone(value)
    externalConflicts.value = []
    newEntry.value = emptyEntry()
  }
  saved.value = clone(value)
  if (!metadataOpen.value) metadataDraft.value = clone(draft.value.metadata)
  aiPlan.value = null
  selected.value = new Set()
}, { deep: true })

const filtered = computed<DictionaryTableRow[]>(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return draft.value.entries
    .map((entry, index) => ({ entry, index, kind: 'entry' as const }))
    .filter(({ entry }) => !hideSkipped.value || !hiddenSources.value.has(entry.source))
    .filter(({ entry }) => translationFilter.value === 'all' || (translationFilter.value === 'translated' ? Boolean(entry.translation.trim()) : !entry.translation.trim()))
    .filter(({ entry }) => !needle || `${entry.source} ${entry.translation}`.toLocaleLowerCase().includes(needle))
})

const tableRows = computed<DictionaryTableRow[]>(() => [
  ...filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value),
  { entry: newEntry.value, index: -1, kind: 'new' },
])

watch([query, pageSize, translationFilter], () => { page.value = 1 })
watch(translationFilter, () => { selected.value = new Set() })
watch(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)), count => {
  page.value = Math.min(page.value, count)
})
watch(() => props.detail.metadata.id, () => { page.value = 1; translationFilter.value = 'all' })

const columns = computed<TableColumn<DictionaryTableRow>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'source', header: t('dictionaryEditor.columns.source'), meta: { class: { th: 'w-[43%]', td: 'w-[43%]' } } },
  { id: 'translation', header: t('dictionaryEditor.columns.translation'), meta: { class: { th: 'w-[43%]', td: 'w-[43%]' } } },
  { id: 'actions', header: t('dictionaryEditor.columns.actions'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
])

function normalizedEntry(value: DictionaryEntry): DictionaryEntry {
  return {
    source: value.source.trim(),
    translation: value.translation.trim(),
  }
}

const sourceCounts = computed(() => {
  const counts = new Map<string, number>()
  for (const entry of draft.value.entries) {
    const source = entry.source.trim()
    counts.set(source, (counts.get(source) ?? 0) + 1)
  }
  return counts
})

function sourceIsDuplicate(source: string, excludingIndex: number | null = null) {
  const normalized = source.trim()
  const excluded = excludingIndex !== null && draft.value.entries[excludingIndex]?.source.trim() === normalized ? 1 : 0
  return Boolean(normalized) && (sourceCounts.value.get(normalized) ?? 0) > excluded
}

function entrySourceError(index: number) {
  const source = draft.value.entries[index]?.source.trim() ?? ''
  if (!source) return t('dictionaryEditor.sourceRequired')
  if (sourceIsDuplicate(source, index)) return t('dictionaryEditor.duplicateSource')
  return ''
}

const newEntryTouched = computed(() => Boolean(newEntry.value.source.trim() || newEntry.value.translation.trim()))
const newSourceError = computed(() => {
  if (!newEntryTouched.value) return ''
  if (!newEntry.value.source.trim()) return t('dictionaryEditor.sourceRequired')
  return sourceIsDuplicate(newEntry.value.source) ? t('dictionaryEditor.duplicateSource') : ''
})
const newEntryValid = computed(() => (
  newEntryTouched.value && !newSourceError.value
))
const metadataValid = computed(() => Boolean(
  draft.value.metadata.name.trim()
  && draft.value.metadata.sourceLocale.trim()
  && draft.value.metadata.targetLocale.trim()
  && draft.value.metadata.releaseVersion.trim(),
))
const entriesValid = computed(() => draft.value.entries.every((_, index) => (
  !entrySourceError(index)
)))
const hasUnsavedChanges = computed(() => (
  JSON.stringify(draft.value) !== JSON.stringify(saved.value) || newEntryTouched.value
))
const canSave = computed(() => (
  hasUnsavedChanges.value
  && !externalConflicts.value.length
  && metadataValid.value
  && entriesValid.value
  && (!newEntryTouched.value || newEntryValid.value)
))
const selectedProfile = computed(() => ai.profiles.value.find(profile => (
  profile.id === (selectedProfileId.value ?? ai.catalog.value.defaultProfileId)
)) ?? ai.defaultProfile.value)
const aiMenuItems = computed<DropdownMenuItem[][]>(() => [
  [{
    label: t('ai.previewCandidates'),
    icon: 'i-tabler-list-check',
    disabled: !selectedProfile.value || ai.busy.value,
    onSelect: () => void previewAiTranslation(),
  }],
  ai.profiles.value.map(profile => ({
    label: t('ai.useProfile', { name: profile.name }),
    icon: selectedProfile.value?.id === profile.id ? 'i-tabler-check' : 'i-tabler-sparkles',
    onSelect: () => { selectedProfileId.value = profile.id },
  })),
  [{
    label: t('ai.manageProfiles'),
    icon: 'i-tabler-settings',
    onSelect: () => emit('configure-ai'),
  }],
])

watch(hasUnsavedChanges, value => {
  if (!value) aiSaveRequired.value = false
  emit('dirty-change', value)
}, { immediate: true })

function openMetadata() {
  metadataDraft.value = clone(draft.value.metadata)
  metadataOpen.value = true
}

function applyMetadata() {
  const next = clone(metadataDraft.value)
  next.name = next.name.trim()
  next.description = next.description.trim()
  next.releaseVersion = next.releaseVersion.trim()
  next.sourceLocale = next.sourceLocale.trim()
  next.targetLocale = next.targetLocale.trim()
  next.authors = next.authors.map(value => value.trim()).filter(Boolean)
  next.license = next.license?.trim() || null
  next.homepage = next.homepage?.trim() || null
  next.tags = [...new Set(next.tags.map(value => value.trim()).filter(Boolean))]
  if (!next.name || !next.releaseVersion || !next.sourceLocale || !next.targetLocale) return
  draft.value.metadata = next
  metadataOpen.value = false
}

function commitNewEntry() {
  if (!newEntryTouched.value) return true
  if (!newEntryValid.value) return false
  draft.value.entries.push(normalizedEntry(newEntry.value))
  newEntry.value = emptyEntry()
  return true
}

function confirmRemoval() {
  const removed = new Set(pendingRemoval.value)
  draft.value.entries = draft.value.entries.filter((_, index) => !removed.has(index))
  selected.value = new Set()
  pendingRemoval.value = []
}

function saveDraft() {
  if (externalConflicts.value.length || dictionaryLocked.value || props.busy) return
  if (!commitNewEntry() || !metadataValid.value || !entriesValid.value) return
  const next = clone(draft.value)
  next.entries = next.entries.map(normalizedEntry)
  draft.value = clone(next)
  emit('save', next)
}

async function prepareAiPlan() {
  aiNotice.value = ''
  aiNoticeCancelled.value = false
  aiRetryAvailable.value = false
  if (!selectedProfile.value) {
    emit('configure-ai')
    return null
  }
  if (!commitNewEntry()) return null
  try {
    const plan = await ai.planDictionary(draft.value, selectedProfile.value.id)
    aiPlan.value = plan
    return plan
  }
  catch {
    return null
  }
}

async function previewAiTranslation() {
  const plan = await prepareAiPlan()
  if (plan) aiPreviewOpen.value = true
}

function dismissAiOutcome() {
  aiNotice.value = ''
  aiNoticeCancelled.value = false
  aiRetryAvailable.value = false
  ai.dismissCurrentJob()
}

function requireSavedAiDraft() {
  aiSaveRequired.value = hasUnsavedChanges.value || externalConflicts.value.length > 0
  return !aiSaveRequired.value
}

async function runAiTranslation(plan?: AiTranslationPlan | null) {
  if (!requireSavedAiDraft()) return
  const nextPlan = plan ?? await prepareAiPlan()
  if (!nextPlan || !selectedProfile.value) return
  if (!nextPlan.candidates.length) {
    aiPreviewOpen.value = true
    return
  }
  aiPreviewOpen.value = false
  aiPreflightOpen.value = true
}

async function executeAiTranslation() {
  if (!requireSavedAiDraft()) { aiPreflightOpen.value = false; return }
  const nextPlan = aiPlan.value
  if (!nextPlan || !selectedProfile.value) return
  aiPreflightOpen.value = false
  try {
    if (!('__TAURI_INTERNALS__' in window)) {
      const job = await ai.runPlan(nextPlan, selectedProfile.value.id)
      let applied = 0
      for (const result of job.results) {
        const entry = draft.value.entries.find(candidate => (
          candidate.source.trim() === result.source && !candidate.translation.trim()
        ))
        if (!entry) continue
        entry.translation = result.translation
        applied += 1
      }
      if (job.status === 'completed') {
        aiNoticeCancelled.value = false
        aiNoticeTone.value = 'success'
        aiNotice.value = t('ai.dictionaryCompleted', {
          count: applied,
          batches: job.totalBatches,
          elapsed: ai.elapsed.value,
        })
      }
      else if (job.status === 'cancelled') {
        aiNoticeCancelled.value = true
        aiNoticeTone.value = 'warning'
        aiRetryAvailable.value = job.completedCount < job.totalCount
        aiNotice.value = t('ai.translationCancelledNotice', {
          completed: job.completedCount,
          total: job.totalCount,
          elapsed: ai.elapsed.value,
        })
      }
      else {
        aiNoticeCancelled.value = false
        aiNoticeTone.value = 'warning'
        aiRetryAvailable.value = job.completedCount < job.totalCount
        aiNotice.value = t('ai.dictionaryPartial', {
          completed: job.completedCount,
          total: job.totalCount,
          failed: Math.max(job.failedCount, job.totalCount - job.completedCount),
          elapsed: ai.elapsed.value,
        })
      }
      aiPlan.value = null
      return
    }
    await ai.startBackgroundPlan(nextPlan, selectedProfile.value.id)
    aiPlan.value = null
  }
  catch {
    // The composable exposes the localized error below the header.
  }
}

onMounted(() => void ai.connect().catch(() => undefined))

usePageEscape(() => true, () => emit('back'))
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-title">
    <ManagementDetailHeader
      title-id="dictionary-title"
      :title="draft.metadata.name"
      :description="t('dictionaryEditor.metadataLine', { source: draft.metadata.sourceLocale, target: draft.metadata.targetLocale, version: draft.metadata.releaseVersion, count: draft.entries.length })"
      :back-label="t('dictionaryEditor.back')"
      @back="emit('back')"
    >
      <template #status>
        <div class="flex items-center gap-1.5">
          <UBadge v-if="dictionaryLocked" color="warning" variant="subtle" size="sm" icon="i-tabler-lock" :label="t('ai.tasks.dictionaryLocked')" />
          <UBadge v-if="hasUnsavedChanges" color="warning" variant="subtle" size="sm" :label="t('dictionaryEditor.unsaved')" />
        </div>
      </template>
      <template #actions>
        <UDropdownMenu :items="dictionaryActions" :content="{ align: 'end' }">
          <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-dots-vertical" trailing-icon="i-tabler-chevron-down" :label="t('dictionaryExport.actions')" />
        </UDropdownMenu>
        <div class="inline-flex">
          <UButton
            color="primary"
            variant="soft"
            size="sm"
            icon="i-tabler-sparkles"
            :label="dictionaryLocked ? t('ai.tasks.viewCurrent') : selectedProfile ? t('ai.fillUntranslated') : t('ai.configure')"
            :loading="ai.busy.value"
            :disabled="busy"
            class="rounded-r-none"
            @click="dictionaryLocked ? emit('open-ai-tasks') : selectedProfile ? runAiTranslation() : emit('configure-ai')"
          />
          <UDropdownMenu :items="aiMenuItems" :content="{ align: 'end' }">
            <UButton :title="t('ai.translationOptions')" color="primary" variant="soft" size="sm" icon="i-tabler-chevron-down" class="rounded-l-none border-l border-l-[var(--border)]" :aria-label="t('ai.translationOptions')" :disabled="dictionaryLocked || busy || ai.busy.value" />
          </UDropdownMenu>
        </div>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('dictionaryEditor.saveDictionary')" :loading="busy" :disabled="dictionaryLocked || busy || !canSave" @click="saveDraft" />
      </template>
    </ManagementDetailHeader>

    <UAlert v-if="aiSaveRequired" role="status" color="warning" :title="t('ai.saveDraftFirst')" class="mb-3" />
    <UAlert v-if="externalConflicts.length" role="alert" color="warning" :title="t('dictionaryEditor.externalConflictTitle')" :description="t('dictionaryEditor.externalConflictDescription', { items: externalConflicts.join('、') })" class="mb-3">
      <template #actions>
        <UButton :label="t('dictionaryEditor.keepDraftChanges')" @click="externalConflicts = []" />
        <UButton color="neutral" :label="t('dictionaryEditor.useSavedVersion')" @click="draft = clone(saved); metadataDraft = clone(saved.metadata); newEntry = emptyEntry(); externalConflicts = []" />
      </template>
    </UAlert>
    <UAlert v-if="!dictionaryLocked && ai.error.value" role="alert" color="error" variant="soft" :title="t('ai.translationFailed')" :description="ai.error.value" class="mb-3">
      <template #actions><UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :label="t('common.dismissMessage')" @click="ai.clearError()" /></template>
    </UAlert>
    <UAlert v-else-if="!dictionaryLocked && aiNotice" role="status" :color="aiNoticeTone" variant="soft" icon="i-tabler-sparkles" :title="aiNoticeTitle" :description="aiNotice" class="mb-3">
      <template #actions>
        <div class="flex items-center gap-1.5">
          <UButton v-if="aiRetryAvailable" color="primary" variant="soft" size="xs" :label="t('ai.retryRemaining')" @click="runAiTranslation()" />
          <UButton color="neutral" variant="ghost" size="xs" :label="t('ai.dismissOutcome')" @click="dismissAiOutcome" />
        </div>
      </template>
    </UAlert>
    <AiTranslationProgress
      v-if="displayedAiJob"
      :job="displayedAiJob"
      :elapsed="ai.elapsed.value"
      @cancel="ai.cancelCurrentJob"
      @dismiss="ai.dismissCurrentJob"
    />

    <p v-if="filterError" role="alert" class="text-error">{{ filterError }}</p>
    <ManagementTableFrame
      v-model:query="query"
      v-model:filter-value="translationFilter"
      :filter-options="translationFilterOptions"
      :filter-label="translationFilterLabel"
      :filter-aria-label="t('dictionaryEditor.translationFilterLabel')"
      v-model:page="page"
      v-model:page-size="pageSize"
      :page-sizes="[50, 100, 200]"
      :search-placeholder="t('dictionaryEditor.searchPlaceholder')"
      :search-label="t('dictionaryEditor.searchLabel')"
      :selected-count="selected.size"
      :selected-label="t('dictionaryEditor.itemLabel')"
      :total="filtered.length"
      :item-label="t('dictionaryEditor.itemLabel')"
    >
      <template #toolbar-actions>
        <UCheckbox v-model="hideSkipped" :label="t('textFilters.hide')" />
        <FieldHelp :label="t('textFilters.hide')" :text="t('textFilters.hideHint')" />
      </template>
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('dictionaryEditor.deleteSelected')" @click="pendingRemoval = [...selected]" />
      </template>
      <UTable
        :data="tableRows"
        :columns="columns"
        sticky
        :meta="{ class: { tr: (row: TableRow<DictionaryTableRow>) => row.original.kind === 'new' ? 'bg-[var(--surface-subtle)]' : '' } }"
        :ui="{ root: 'h-full overflow-auto [scrollbar-gutter:stable]', base: 'min-w-[620px]' }"
      >
        <template #select-header><UCheckbox :model-value="selectionState" :disabled="dictionaryLocked || busy || !filtered.length" :aria-label="t('dictionaryEditor.selectAllMatches')" :title="t('dictionaryEditor.selectAllHint')" @update:model-value="toggleAllEntries" /></template>
        <template #select-cell="{ row }">
          <UIcon v-if="row.original.kind === 'new'" name="i-tabler-plus" class="mx-auto block size-4 text-[var(--text-muted)]" />
          <UCheckbox
            v-else
            :disabled="dictionaryLocked"
            :model-value="selected.has(row.original.index)"
            :aria-label="t('dictionaryEditor.selectRule', { source: row.original.entry.source })"
            @update:model-value="selected.has(row.original.index) ? selected.delete(row.original.index) : selected.add(row.original.index); selected = new Set(selected)"
          />
        </template>
        <template #source-cell="{ row }">
          <div class="min-w-0">
            <UInput
              v-if="row.original.kind === 'new'"
              v-model="newEntry.source"
              color="neutral"
              variant="none"
              size="sm"
              :aria-label="t('dictionaryEditor.newSourceLabel')"
              :placeholder="t('dictionaryEditor.newSourcePlaceholder')"
              :ui="{ base: 'px-0' }"
              class="w-full"
              :disabled="dictionaryLocked"
            />
            <UInput
              v-else
              v-model="row.original.entry.source"
              :color="entrySourceError(row.original.index) ? 'error' : 'neutral'"
              variant="none"
              size="sm"
              :aria-label="t('dictionaryEditor.entrySourceLabel', { source: row.original.entry.source || row.original.index + 1 })"
              :aria-invalid="Boolean(entrySourceError(row.original.index))"
              :ui="{ base: 'px-0' }"
              class="w-full"
              :disabled="dictionaryLocked"
            />
            <p v-if="row.original.kind === 'new' ? newSourceError : entrySourceError(row.original.index)" class="type-metadata m-0 leading-4 text-[var(--danger)]">
              {{ row.original.kind === 'new' ? newSourceError : entrySourceError(row.original.index) }}
            </p>
          </div>
        </template>
        <template #translation-cell="{ row }">
          <div class="min-w-0">
            <UInput
              v-if="row.original.kind === 'new'"
              v-model="newEntry.translation"
              color="neutral"
              variant="none"
              size="sm"
              :aria-label="t('dictionaryEditor.newTranslationLabel')"
              :placeholder="t('dictionaryEditor.newTranslationPlaceholder')"
              :ui="{ base: 'px-0' }"
              class="w-full"
              :disabled="dictionaryLocked"
              @keydown.enter.prevent="commitNewEntry"
            />
            <UInput
              v-else
              v-model="row.original.entry.translation"
              color="neutral"
              variant="none"
              size="sm"
              :aria-label="t('dictionaryEditor.entryTranslationLabel', { source: row.original.entry.source || row.original.index + 1 })"
              :ui="{ base: 'px-0' }"
              class="w-full"
              :disabled="dictionaryLocked"
            />
          </div>
        </template>
        <template #actions-cell="{ row }">
          <span v-if="row.original.kind === 'new'" class="type-metadata text-[var(--text-muted)]">{{ t('dictionaryEditor.pressEnterToAdd') }}</span>
          <UButton :title="t('common.deleteNamed', { name: row.original.entry.source })"
            v-else
            color="error"
            variant="ghost"
            size="xs"
            icon="i-tabler-trash"
            :aria-label="t('common.deleteNamed', { name: row.original.entry.source })"
            :disabled="dictionaryLocked"
            @click="pendingRemoval = [row.original.index]"
          />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="metadataOpen"
      :title="t('dictionaryEditor.settingsTitle')"
      :description="t('dictionaryEditor.settingsDescription')"
      :confirm-label="t('dictionaryEditor.applySettings')"
      :confirm-disabled="!metadataDraft.name.trim() || !metadataDraft.sourceLocale.trim() || !metadataDraft.targetLocale.trim() || !metadataDraft.releaseVersion.trim()"
      width="md"
      @update:open="$event || (metadataOpen = false)"
      @confirm="applyMetadata"
    >
      <DictionaryMetadataForm v-model="metadataDraft" />
    </ManagementFormModal>

    <ManagementFormModal
      :open="aiPreviewOpen"
      :title="t('ai.previewTitle')"
      :description="aiPlan ? t('ai.previewSummary', { eligible: aiPlan.candidates.length, skipped: aiPlan.skipped.length }) : ''"
      :confirm-label="t('ai.translateCount', { count: aiPlan?.candidates.length ?? 0 })"
      :confirm-disabled="!aiPlan?.candidates.length"
      :busy="ai.busy.value"
      width="md"
      @update:open="aiPreviewOpen = $event"
      @confirm="runAiTranslation(aiPlan)"
    >
      <p v-if="aiPlan?.candidates.length && selectedProfile" class="type-metadata mb-3 mt-0 rounded-md bg-[var(--surface-subtle)] px-3 py-2 leading-4 text-[var(--text-muted)]">
        {{ t('ai.previewBatchHint', { previewed: Math.min(aiPlan.candidates.length, 20), total: aiPlan.candidates.length, items: selectedProfile.maxItemsPerRequest, batches: Math.ceil(aiPlan.candidates.length / selectedProfile.maxItemsPerRequest), concurrency: selectedProfile.maxConcurrency }) }}
      </p>
      <div v-if="aiPlan?.candidates.length" class="space-y-1">
        <div v-for="candidate in aiPlan.candidates.slice(0, 20)" :key="candidate.itemId" class="flex items-center gap-2 border-b border-[var(--border)] py-2 last:border-b-0">
          <UIcon name="i-tabler-arrow-right" class="size-4 shrink-0 text-[var(--accent-strong)]" aria-hidden="true" />
          <span class="min-w-0 flex-1 truncate text-[11px] text-[var(--text)]">{{ candidate.source }}</span>
        </div>
      </div>
      <UEmpty v-else icon="i-tabler-check" :title="t('ai.nothingToTranslate')" :description="t('ai.nothingToTranslateHint')" />
      <p v-if="aiPlan?.skipped.length" class="type-metadata mb-0 mt-3 leading-4 text-[var(--text-muted)]">{{ t('ai.skippedHint', { count: aiPlan.skipped.length }) }}</p>
    </ManagementFormModal>

    <AiTranslationPreflight
      v-model:open="aiPreflightOpen"
      :plan="aiPlan"
      :profile="selectedProfile"
      @proceed="executeAiTranslation"
    />

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('dictionaryEditor.deleteTitle')"
      :description="t('dictionaryEditor.deleteDescription', { count: pendingRemoval.length })"
      :confirm-label="t('dictionaryEditor.deleteConfirm')"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
    <ConfirmDialog
      v-model:open="clearOpen"
      :title="t('dictionaryEditor.clearDictionary')"
      :description="t('dictionaryEditor.clearDescription', { count: draft.entries.length })"
      :confirm-label="t('dictionaryEditor.clearConfirm')"
      :busy="dictionaryLocked || busy"
      @confirm="clearDictionary"
    />
    <DictionaryExportDialog ref="exportDialog" />
    <DictionaryImportDialog ref="importDialog" existing :apply="applyImport" />
  </section>
</template>
