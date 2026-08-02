<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type { DictionaryDetail, DictionaryEntry, DictionaryMetadata } from '../model'

const props = defineProps<{ detail: DictionaryDetail; busy: boolean }>()
const emit = defineEmits<{ back: []; save: [detail: DictionaryDetail] }>()
const { t } = useI18n()

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

const draft = ref<DictionaryDetail>(clone(props.detail))
const query = ref('')
const editingIndex = ref<number | null>(null)
const editorOpen = ref(false)
const metadataOpen = ref(false)
const metadataDraft = ref<DictionaryMetadata>(clone(props.detail.metadata))
const entry = ref<DictionaryEntry>(emptyEntry())
const pendingRemoval = ref<number[]>([])
const selected = ref(new Set<number>())

watch(() => props.detail, (value) => {
  draft.value = clone(value)
  metadataDraft.value = clone(value.metadata)
}, { deep: true })

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return draft.value.entries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => !needle || `${entry.source} ${entry.translation}`.toLocaleLowerCase().includes(needle))
})
const columns = computed<TableColumn<{ entry: DictionaryEntry; index: number }>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'source', header: t('dictionaryEditor.columns.source'), meta: { class: { th: 'w-[42%]', td: 'w-[42%]' } } },
  { id: 'translation', header: t('dictionaryEditor.columns.translation'), meta: { class: { th: 'w-[42%]', td: 'w-[42%]' } } },
  { id: 'actions', header: t('dictionaryEditor.columns.actions'), meta: { class: { th: 'w-24 text-center', td: 'w-24 text-center' } } },
])

const duplicateSource = computed(() => {
  const source = entry.value.source.trim()
  return Boolean(source) && draft.value.entries.some((candidate, index) => index !== editingIndex.value && candidate.source === source)
})

function emptyEntry(): DictionaryEntry {
  return { source: '', translation: '' }
}

function startCreate() {
  editingIndex.value = null
  entry.value = emptyEntry()
  editorOpen.value = true
}

function startEdit(index: number) {
  editingIndex.value = index
  entry.value = clone(draft.value.entries[index]!)
  editorOpen.value = true
}

function openMetadata() {
  metadataDraft.value = clone(draft.value.metadata)
  metadataOpen.value = true
}

function saveMetadata() {
  const next = clone(metadataDraft.value)
  next.name = next.name.trim()
  next.description = next.description.trim()
  next.releaseVersion = next.releaseVersion.trim()
  next.sourceLocale = next.sourceLocale.trim()
  next.targetLocale = next.targetLocale.trim()
  next.authors = next.authors.map(value => value.trim()).filter(Boolean)
  next.license = next.license?.trim() || null
  next.homepage = next.homepage?.trim() || null
  next.tags = next.tags.map(value => value.trim()).filter(Boolean)
  if (!next.name || !next.releaseVersion || !next.sourceLocale || !next.targetLocale) return
  draft.value.metadata = next
  metadataOpen.value = false
}

function saveEntry() {
  const next = clone(entry.value)
  next.source = next.source.trim()
  next.translation = next.translation.trim()
  if (!next.source || !next.translation || duplicateSource.value) return
  if (editingIndex.value === null) draft.value.entries.push(next)
  else draft.value.entries[editingIndex.value] = next
  editorOpen.value = false
}

function confirmRemoval() {
  const removed = new Set(pendingRemoval.value)
  draft.value.entries = draft.value.entries.filter((_, index) => !removed.has(index))
  selected.value = new Set()
  pendingRemoval.value = []
}

function saveDraft() {
  emit('save', clone(draft.value))
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-title">
    <div class="mb-3 flex min-h-10 items-center justify-between gap-4">
      <div class="flex min-w-0 items-center gap-2">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-arrow-left" :aria-label="t('dictionaryEditor.back')" @click="emit('back')" />
        <div class="min-w-0">
          <h1 id="dictionary-title" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">{{ draft.metadata.name }}</h1>
          <p class="m-0 mt-0.5 text-[10px] text-[var(--text-muted)]">{{ t('dictionaryEditor.metadataLine', { source: draft.metadata.sourceLocale, target: draft.metadata.targetLocale, version: draft.metadata.releaseVersion, revision: draft.revision, count: draft.entries.length }) }}</p>
        </div>
      </div>
      <div class="flex shrink-0 gap-2">
        <UButton color="neutral" variant="ghost" size="sm" icon="i-tabler-settings" :label="t('dictionaryEditor.settings')" @click="openMetadata" />
        <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-plus" :label="t('dictionaryEditor.addRule')" @click="startCreate" />
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-device-floppy" :label="t('dictionaryEditor.saveDictionary')" :loading="busy" :disabled="busy || !draft.metadata.name.trim() || !draft.metadata.sourceLocale.trim() || !draft.metadata.targetLocale.trim() || !draft.metadata.releaseVersion.trim()" @click="saveDraft" />
      </div>
    </div>

    <ManagementTableFrame
      v-model:query="query"
      :page="1"
      :page-size="Math.max(20, filtered.length)"
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
      <UTable :data="filtered" :columns="columns" sticky :ui="{ base: 'min-w-[560px]' }">
        <template #select-header></template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.index)" :aria-label="t('dictionaryEditor.selectRule', { source: row.original.entry.source })" @update:model-value="selected.has(row.original.index) ? selected.delete(row.original.index) : selected.add(row.original.index); selected = new Set(selected)" />
        </template>
        <template #source-cell="{ row }"><div class="truncate font-medium" :title="row.original.entry.source">{{ row.original.entry.source }}</div></template>
        <template #translation-cell="{ row }"><div class="truncate" :title="row.original.entry.translation">{{ row.original.entry.translation }}</div></template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-1">
            <UButton color="neutral" variant="ghost" size="xs" :label="t('dictionaryEditor.editAction')" :aria-label="t('common.editNamed', { name: row.original.entry.source })" @click="startEdit(row.original.index)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.entry.source })" @click="pendingRemoval = [row.original.index]" />
          </div>
        </template>
        <template #empty>
          <UEmpty icon="i-tabler-text-plus" :title="draft.entries.length ? t('dictionaryEditor.noMatch') : t('dictionaryEditor.empty')" :description="t('dictionaryEditor.emptyDescription')" />
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
      @confirm="saveMetadata"
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

    <ManagementFormModal
      :open="editorOpen"
      :title="editingIndex === null ? t('dictionaryEditor.createRule') : t('dictionaryEditor.editRule')"
      :description="t('dictionaryEditor.ruleDescription')"
      :confirm-label="t('dictionaryEditor.saveRule')"
      :confirm-disabled="!entry.source.trim() || !entry.translation.trim() || duplicateSource"
      width="md"
      @update:open="$event || (editorOpen = false)"
      @confirm="saveEntry"
    >
      <div class="space-y-3">
        <UFormField :label="t('dictionaryEditor.source')" :error="duplicateSource ? t('dictionaryEditor.duplicateSource') : undefined" required><UTextarea v-model="entry.source" :rows="2" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaryEditor.translation')" required><UTextarea v-model="entry.translation" :rows="2" class="w-full" /></UFormField>
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
