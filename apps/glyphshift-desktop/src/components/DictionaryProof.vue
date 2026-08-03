<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn, TableRow } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type { DictionaryDetail, DictionaryEntry, DictionaryMetadata } from '../model'

interface DictionaryTableRow {
  entry: DictionaryEntry
  index: number
  kind: 'entry' | 'new'
}

const props = defineProps<{ detail: DictionaryDetail; busy: boolean }>()
const emit = defineEmits<{
  back: []
  save: [detail: DictionaryDetail]
  'dirty-change': [dirty: boolean]
}>()
const { t } = useI18n()

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

function entryTranslationError(index: number) {
  return draft.value.entries[index]?.translation.trim() ? '' : t('dictionaryEditor.translationRequired')
}

const newEntryTouched = computed(() => Boolean(newEntry.value.source.trim() || newEntry.value.translation.trim()))
const newSourceError = computed(() => {
  if (!newEntryTouched.value) return ''
  if (!newEntry.value.source.trim()) return t('dictionaryEditor.sourceRequired')
  return sourceIsDuplicate(newEntry.value.source) ? t('dictionaryEditor.duplicateSource') : ''
})
const newTranslationError = computed(() => (
  newEntryTouched.value && !newEntry.value.translation.trim() ? t('dictionaryEditor.translationRequired') : ''
))
const newEntryValid = computed(() => (
  newEntryTouched.value && !newSourceError.value && !newTranslationError.value
))
const metadataValid = computed(() => Boolean(
  draft.value.metadata.name.trim()
  && draft.value.metadata.sourceLocale.trim()
  && draft.value.metadata.targetLocale.trim()
  && draft.value.metadata.releaseVersion.trim(),
))
const entriesValid = computed(() => draft.value.entries.every((_, index) => (
  !entrySourceError(index) && !entryTranslationError(index)
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
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-title">
    <div class="mb-3 flex min-h-10 items-center justify-between gap-4">
      <div class="flex min-w-0 items-center gap-2">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-arrow-left" :aria-label="t('dictionaryEditor.back')" @click="emit('back')" />
        <div class="min-w-0">
          <div class="flex min-w-0 items-center gap-2">
            <h1 id="dictionary-title" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">{{ draft.metadata.name }}</h1>
            <UBadge v-if="hasUnsavedChanges" color="warning" variant="subtle" size="sm" :label="t('dictionaryEditor.unsaved')" />
          </div>
          <p class="m-0 mt-0.5 text-[10px] text-[var(--text-muted)]">{{ t('dictionaryEditor.metadataLine', { source: draft.metadata.sourceLocale, target: draft.metadata.targetLocale, version: draft.metadata.releaseVersion, revision: draft.revision, count: draft.entries.length }) }}</p>
        </div>
      </div>
      <div class="flex shrink-0 gap-2">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-settings" :label="t('dictionaryEditor.settings')" @click="openMetadata" />
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('dictionaryEditor.saveDictionary')" :loading="busy" :disabled="busy || !canSave" @click="saveDraft" />
      </div>
    </div>

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
            <p v-if="row.original.kind === 'new' ? newSourceError : entrySourceError(row.original.index)" class="m-0 text-[9px] leading-4 text-[var(--danger)]">
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
              :aria-invalid="Boolean(newTranslationError)"
              :ui="{ base: 'px-0' }"
              class="w-full"
              @keydown.enter.prevent="commitNewEntry"
            />
            <UInput
              v-else
              v-model="row.original.entry.translation"
              :color="entryTranslationError(row.original.index) ? 'error' : 'neutral'"
              variant="none"
              size="sm"
              :aria-label="t('dictionaryEditor.entryTranslationLabel', { source: row.original.entry.source || row.original.index + 1 })"
              :aria-invalid="Boolean(entryTranslationError(row.original.index))"
              :ui="{ base: 'px-0' }"
              class="w-full"
            />
            <p v-if="row.original.kind === 'new' ? newTranslationError : entryTranslationError(row.original.index)" class="m-0 text-[9px] leading-4 text-[var(--danger)]">
              {{ row.original.kind === 'new' ? newTranslationError : entryTranslationError(row.original.index) }}
            </p>
          </div>
        </template>
        <template #actions-cell="{ row }">
          <span v-if="row.original.kind === 'new'" class="text-[9px] text-[var(--text-muted)]">{{ t('dictionaryEditor.pressEnterToAdd') }}</span>
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
      <div class="space-y-3">
        <UFormField :label="t('dictionaryEditor.name')" required><UInput v-model="metadataDraft.name" :maxlength="128" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.description')"><UTextarea v-model="metadataDraft.description" :maxlength="512" :rows="2" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.sourceLocale')" required><UInput v-model="metadataDraft.sourceLocale" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.targetLocale')" required><UInput v-model="metadataDraft.targetLocale" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.releaseVersion')" required><UInput v-model="metadataDraft.releaseVersion" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.authors')" :hint="t('dictionaryEditor.commaSeparated')"><UInput :model-value="metadataDraft.authors.join(', ')" class="w-full" @update:model-value="metadataDraft.authors = String($event).split(',').map(value => value.trim()).filter(Boolean)" /></UFormField>
        <UFormField :label="t('dictionaryEditor.license')"><UInput :model-value="metadataDraft.license ?? ''" class="w-full" @update:model-value="metadataDraft.license = String($event)" /></UFormField>
        <UFormField :label="t('dictionaryEditor.homepage')"><UInput :model-value="metadataDraft.homepage ?? ''" class="w-full" @update:model-value="metadataDraft.homepage = String($event)" /></UFormField>
        <UFormField :label="t('dictionaryEditor.tags')" :hint="t('dictionaryEditor.commaSeparated')"><UInput :model-value="metadataDraft.tags.join(', ')" class="w-full" @update:model-value="metadataDraft.tags = String($event).split(',').map(value => value.trim()).filter(Boolean)" /></UFormField>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('dictionaryEditor.deleteTitle')"
      :description="t('dictionaryEditor.deleteDescription', { count: pendingRemoval.length })"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
  </section>
</template>
