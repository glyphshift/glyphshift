<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { TableColumn, TableRow } from '@nuxt/ui/components/Table.vue'
import type { DropdownMenuItem } from '@nuxt/ui'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
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
  'dirty-change': [dirty: boolean]
}>()
const { t } = useI18n()
const ai = useAiTranslation()
const appSettings = useAppSettings()

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function emptyEntry(): DictionaryEntry {
  return { source: '', translation: '' }
}

const saved = ref<DictionaryDetail>(clone(props.detail))
const draft = ref<DictionaryDetail>(clone(props.detail))
const query = ref('')
const metadataOpen = ref(false)
const metadataDraft = ref<DictionaryMetadata>(clone(props.detail.metadata))
const newEntry = ref<DictionaryEntry>(emptyEntry())
const pendingRemoval = ref<number[]>([])
const selected = ref(new Set<number>())
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

watch(() => props.detail, (value) => {
  saved.value = clone(value)
  draft.value = clone(value)
  metadataDraft.value = clone(value.metadata)
  newEntry.value = emptyEntry()
  selected.value = new Set()
}, { deep: true })

const filtered = computed<DictionaryTableRow[]>(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return draft.value.entries
    .map((entry, index) => ({ entry, index, kind: 'entry' as const }))
    .filter(({ entry }) => !needle || `${entry.source} ${entry.translation}`.toLocaleLowerCase().includes(needle))
})

const tableRows = computed<DictionaryTableRow[]>(() => [
  ...filtered.value,
  { entry: newEntry.value, index: -1, kind: 'new' },
])

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

function sourceIsDuplicate(source: string, excludingIndex: number | null = null) {
  const normalized = source.trim()
  return Boolean(normalized) && draft.value.entries.some((candidate, index) => (
    index !== excludingIndex && candidate.source.trim() === normalized
  ))
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

watch(hasUnsavedChanges, value => emit('dirty-change', value), { immediate: true })

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

async function runAiTranslation(plan?: AiTranslationPlan | null, alreadyConfirmed = false) {
  const nextPlan = plan ?? await prepareAiPlan()
  if (!nextPlan || !selectedProfile.value) return
  if (!nextPlan.candidates.length) {
    aiPreviewOpen.value = true
    return
  }
  aiPreviewOpen.value = false
  if (appSettings.confirmAiTranslation.value && !alreadyConfirmed) {
    aiPreflightOpen.value = true
    return
  }
  await executeAiTranslation()
}

async function executeAiTranslation() {
  const nextPlan = aiPlan.value
  if (!nextPlan || !selectedProfile.value) return
  aiPreflightOpen.value = false
  try {
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
      :description="t('dictionaryEditor.metadataLine', { source: draft.metadata.sourceLocale, target: draft.metadata.targetLocale, version: draft.metadata.releaseVersion, revision: draft.revision, count: draft.entries.length })"
      :back-label="t('dictionaryEditor.back')"
      @back="emit('back')"
    >
      <template #status><UBadge v-if="hasUnsavedChanges" color="warning" variant="subtle" size="sm" :label="t('dictionaryEditor.unsaved')" /></template>
      <template #actions>
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-settings" :label="t('dictionaryEditor.settings')" @click="openMetadata" />
        <div class="inline-flex">
          <UButton
            color="primary"
            variant="soft"
            size="sm"
            icon="i-tabler-sparkles"
            :label="selectedProfile ? t('ai.fillUntranslated') : t('ai.configure')"
            :loading="ai.busy.value"
            :disabled="busy"
            class="rounded-r-none"
            @click="selectedProfile ? runAiTranslation() : emit('configure-ai')"
          />
          <UDropdownMenu :items="aiMenuItems" :content="{ align: 'end' }">
            <UButton color="primary" variant="soft" size="sm" icon="i-tabler-chevron-down" class="rounded-l-none border-l border-l-[var(--border)]" :aria-label="t('ai.translationOptions')" :disabled="busy || ai.busy.value" />
          </UDropdownMenu>
        </div>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('dictionaryEditor.saveDictionary')" :loading="busy" :disabled="busy || !canSave" @click="saveDraft" />
      </template>
    </ManagementDetailHeader>

    <UAlert v-if="ai.error.value" role="alert" color="error" variant="soft" :title="t('ai.translationFailed')" :description="ai.error.value" class="mb-3" />
    <UAlert v-else-if="aiNotice" role="status" :color="aiNoticeTone" variant="soft" icon="i-tabler-sparkles" :title="aiNoticeTitle" :description="aiNotice" class="mb-3">
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

    <ManagementTableFrame
      v-model:query="query"
      :page="1"
      :page-size="Math.max(20, tableRows.length)"
      :search-placeholder="t('dictionaryEditor.searchPlaceholder')"
      :search-label="t('dictionaryEditor.searchLabel')"
      :selected-count="selected.size"
      :selected-label="t('dictionaryEditor.itemLabel')"
      :total="filtered.length"
      :item-label="t('dictionaryEditor.itemLabel')"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('dictionaryEditor.deleteSelected')" @click="pendingRemoval = [...selected]" />
      </template>
      <UTable
        :data="tableRows"
        :columns="columns"
        sticky
        :meta="{ class: { tr: (row: TableRow<DictionaryTableRow>) => row.original.kind === 'new' ? 'bg-[var(--surface-subtle)]' : '' } }"
        :ui="{ base: 'min-w-[620px]' }"
      >
        <template #select-header></template>
        <template #select-cell="{ row }">
          <UIcon v-if="row.original.kind === 'new'" name="i-tabler-plus" class="mx-auto block size-4 text-[var(--text-muted)]" />
          <UCheckbox
            v-else
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
            />
          </div>
        </template>
        <template #actions-cell="{ row }">
          <span v-if="row.original.kind === 'new'" class="type-metadata text-[var(--text-muted)]">{{ t('dictionaryEditor.pressEnterToAdd') }}</span>
          <UButton
            v-else
            color="error"
            variant="ghost"
            size="xs"
            icon="i-tabler-trash"
            :aria-label="t('common.deleteNamed', { name: row.original.entry.source })"
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
      @confirm="runAiTranslation(aiPlan, true)"
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
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
  </section>
</template>
