<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { useI18n } from 'vue-i18n'
import type { DictionaryMetadata, DictionarySummary } from '../model'

const props = defineProps<{
  items: DictionarySummary[]
  busy: boolean
  messages: Record<string, string>
}>()
const emit = defineEmits<{
  open: [id: string]
  create: [metadata: Omit<DictionaryMetadata, 'id'>]
  remove: [ids: string[]]
}>()
const { t } = useI18n()

const query = ref('')
const page = ref(1)
const pageSize = ref(20)
const selected = ref(new Set<string>())
const creating = ref(false)
const pendingRemoval = ref<DictionarySummary[]>([])
const name = ref('')
const description = ref('')
const sourceLocale = ref('en-US')
const targetLocale = ref('zh-CN')
const releaseVersion = ref('0.1.0')
const authors = ref('')
const license = ref('')
const homepage = ref('')
const tags = ref('')

const filtered = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return props.items.filter((item) => {
    const metadata = item.metadata
    return !needle || `${metadata.name} ${metadata.description} ${metadata.sourceLocale} ${metadata.targetLocale} ${metadata.releaseVersion} ${metadata.tags.join(' ')}`.toLocaleLowerCase().includes(needle)
  })
})
const pageCount = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize.value)))
const pageItems = computed(() => filtered.value.slice((page.value - 1) * pageSize.value, page.value * pageSize.value))
const pageSelected = computed(() => Boolean(pageItems.value.length) && pageItems.value.every(item => selected.value.has(item.metadata.id)))
const tableColumns = computed<TableColumn<DictionarySummary>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-11', td: 'w-11' } } },
  { id: 'dictionary', header: t('dictionaries.columns.dictionary'), meta: { class: { th: 'w-[28%]', td: 'w-[28%]' } } },
  { id: 'languages', header: t('dictionaries.columns.languages'), meta: { class: { th: 'w-40', td: 'w-40' } } },
  { id: 'release', header: t('dictionaries.columns.release'), meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'tags', header: t('dictionaries.columns.tags') },
  { id: 'rules', header: t('dictionaries.columns.rules'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
  { id: 'actions', header: t('dictionaries.columns.actions'), meta: { class: { th: 'w-20 text-center', td: 'w-20 text-center' } } },
])
const removalDescription = computed(() => pendingRemoval.value.length === 1
  ? t('dictionaries.deleteOne', { name: pendingRemoval.value[0]?.metadata.name ?? '' })
  : t('dictionaries.deleteMany', { count: pendingRemoval.value.length }))

watch([query, pageSize], () => { page.value = 1 })

function toggleSelection(id: string) {
  const next = new Set(selected.value)
  next.has(id) ? next.delete(id) : next.add(id)
  selected.value = next
}

function togglePageSelection() {
  const next = new Set(selected.value)
  if (pageSelected.value) pageItems.value.forEach(item => next.delete(item.metadata.id))
  else pageItems.value.forEach(item => next.add(item.metadata.id))
  selected.value = next
}

function resetForm() {
  name.value = ''
  description.value = ''
  sourceLocale.value = 'en-US'
  targetLocale.value = 'zh-CN'
  releaseVersion.value = '0.1.0'
  authors.value = ''
  license.value = ''
  homepage.value = ''
  tags.value = ''
}

function submit() {
  if (!name.value.trim() || !sourceLocale.value.trim() || !targetLocale.value.trim() || !releaseVersion.value.trim()) return
  emit('create', {
    name: name.value.trim(),
    description: description.value.trim(),
    sourceLocale: sourceLocale.value.trim(),
    targetLocale: targetLocale.value.trim(),
    releaseVersion: releaseVersion.value.trim(),
    authors: authors.value.split(',').map(value => value.trim()).filter(Boolean),
    license: license.value.trim() || null,
    homepage: homepage.value.trim() || null,
    tags: tags.value.split(',').map(value => value.trim()).filter(Boolean),
  })
  creating.value = false
  resetForm()
}

function confirmRemoval() {
  const ids = pendingRemoval.value.map(item => item.metadata.id)
  emit('remove', ids)
  selected.value = new Set([...selected.value].filter(id => !ids.includes(id)))
  pendingRemoval.value = []
}
</script>

<template>
  <section class="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--app-bg)] p-4" aria-labelledby="dictionary-library-title">
    <ManagementPageHeader
      title-id="dictionary-library-title"
      :title="t('dictionaries.title')"
      :description="t('dictionaries.description')"
      icon="i-tabler-language"
    >
      <template #actions>
        <UButton color="primary" variant="solid" size="sm" icon="i-tabler-plus" :label="t('dictionaries.create')" :disabled="busy" @click="creating = true" />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="messages.dictionaries" role="alert" color="error" variant="soft" :title="t('dictionaries.error')" :description="messages.dictionaries" class="mb-3" />

    <ManagementTableFrame
      v-model:query="query"
      v-model:page="page"
      v-model:page-size="pageSize"
      :search-placeholder="t('dictionaries.searchPlaceholder')"
      :search-label="t('dictionaries.searchLabel')"
      :selected-count="selected.size"
      :selected-label="t('dictionaries.itemLabel')"
      :total="filtered.length"
      :item-label="t('dictionaries.itemLabel')"
    >
      <template #bulk-actions>
        <UButton color="error" variant="soft" size="sm" icon="i-tabler-trash" :label="t('dictionaries.bulkDelete')" :disabled="busy" @click="pendingRemoval = items.filter(item => selected.has(item.metadata.id))" />
      </template>

      <UTable :data="pageItems" :columns="tableColumns" sticky :ui="{ base: 'min-w-[860px]' }">
        <template #select-header>
          <UCheckbox :model-value="pageSelected" :aria-label="t('dictionaries.selectPage')" @update:model-value="togglePageSelection" />
        </template>
        <template #select-cell="{ row }">
          <UCheckbox :model-value="selected.has(row.original.metadata.id)" :aria-label="t('common.selectNamed', { name: row.original.metadata.name })" @update:model-value="toggleSelection(row.original.metadata.id)" />
        </template>
        <template #dictionary-cell="{ row }">
          <UButton color="neutral" variant="link" class="block min-w-0 max-w-full justify-start p-0 text-left" @click="emit('open', row.original.metadata.id)">
            <span class="block truncate font-semibold text-[var(--text)]">{{ row.original.metadata.name }}</span>
            <span class="mt-0.5 block truncate text-[9px] text-[var(--text-muted)]">{{ row.original.metadata.description || t('common.noDescription') }}</span>
          </UButton>
        </template>
        <template #languages-cell="{ row }">
          <span>{{ row.original.metadata.sourceLocale }}</span>
          <UIcon name="i-tabler-arrow-right" class="mx-1 align-[-2px] text-[var(--text-muted)]" />
          <span>{{ row.original.metadata.targetLocale }}</span>
        </template>
        <template #release-cell="{ row }">
          <div>v{{ row.original.metadata.releaseVersion }}</div>
          <div class="mt-0.5 text-[9px] text-[var(--text-muted)]">{{ t('dictionaries.localRevision', { revision: row.original.revision }) }}</div>
        </template>
        <template #tags-cell="{ row }">
          <div class="flex flex-wrap gap-1">
            <UBadge v-for="tag in row.original.metadata.tags.slice(0, 3)" :key="tag" color="neutral" variant="soft" size="sm" :label="tag" />
            <span v-if="!row.original.metadata.tags.length" class="text-[var(--text-muted)]">—</span>
          </div>
        </template>
        <template #rules-cell="{ row }">{{ row.original.entryCount }}</template>
        <template #actions-cell="{ row }">
          <div class="flex justify-center gap-0.5">
            <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-edit" :aria-label="t('common.editNamed', { name: row.original.metadata.name })" @click="emit('open', row.original.metadata.id)" />
            <UButton color="error" variant="ghost" size="xs" icon="i-tabler-trash" :aria-label="t('common.deleteNamed', { name: row.original.metadata.name })" :disabled="busy" @click="pendingRemoval = [row.original]" />
          </div>
        </template>
        <template #empty>
          <UEmpty icon="i-tabler-language" :title="items.length ? t('dictionaries.noMatch') : t('dictionaries.empty')" :description="items.length ? t('dictionaries.adjustSearch') : t('dictionaries.emptyDescription')" />
        </template>
      </UTable>
    </ManagementTableFrame>

    <ManagementFormModal
      :open="creating"
      :title="t('dictionaries.create')"
      :description="t('dictionaries.createDescription')"
      :confirm-label="t('dictionaries.createConfirm')"
      :confirm-disabled="busy || !name.trim() || !sourceLocale.trim() || !targetLocale.trim() || !releaseVersion.trim()"
      :busy="busy"
      width="lg"
      @update:open="$event || (creating = false)"
      @confirm="submit"
    >
      <div class="grid grid-cols-2 gap-3">
        <UFormField :label="t('dictionaries.name')" required class="col-span-2"><UInput v-model="name" :maxlength="128" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.descriptionField')" class="col-span-2"><UTextarea v-model="description" :maxlength="512" :rows="2" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.sourceLocale')" required><UInput v-model="sourceLocale" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.targetLocale')" required><UInput v-model="targetLocale" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.releaseVersion')" required><UInput v-model="releaseVersion" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.authors')" :hint="t('dictionaries.authorsHint')"><UInput v-model="authors" class="w-full" /></UFormField>
        <UFormField :label="t('dictionaries.license')"><UInput v-model="license" class="w-full" :placeholder="t('dictionaries.licensePlaceholder')" /></UFormField>
        <UFormField :label="t('dictionaries.homepage')"><UInput v-model="homepage" class="w-full" placeholder="https://…" /></UFormField>
        <UFormField :label="t('dictionaries.tags')" :hint="t('dictionaries.tagsHint')" class="col-span-2"><UInput v-model="tags" class="w-full" /></UFormField>
      </div>
    </ManagementFormModal>

    <ConfirmDialog
      :open="Boolean(pendingRemoval.length)"
      :title="t('dictionaries.deleteTitle')"
      :description="removalDescription"
      :busy="busy"
      @update:open="$event || (pendingRemoval = [])"
      @confirm="confirmRemoval"
    />
  </section>
</template>
