<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { TableColumn } from '@nuxt/ui'
import type { AdapterOption, CaptureCatalogEntry, CaptureResult, CaptureSummary, DictionaryEntry, SoftwareRecord } from '../model'
import ManagementPageHeader from './ManagementPageHeader.vue'

const props = defineProps<{
  software: SoftwareRecord[]
  adapters: AdapterOption[]
  capture: CaptureSummary | null
  result: CaptureResult | null
  busy: boolean
  message: string
}>()

const emit = defineEmits<{
  start: [softwareId: string, adapterIds: string[]]
  stop: []
}>()

const { t } = useI18n()
const softwareId = ref(props.software[0]?.id ?? '')
const adapterIds = ref(props.adapters.filter(adapter => adapter.features.includes('textObserve')).map(adapter => adapter.id))
const resultMode = ref<'catalog' | 'draft'>('catalog')
const query = ref('')
const page = ref(1)
const pageSize = 100

watch(() => props.software, (software) => {
  if (!software.some(item => item.id === softwareId.value)) softwareId.value = software[0]?.id ?? ''
})

const observableAdapters = computed(() => props.adapters.filter(adapter => adapter.features.includes('textObserve')))
const active = computed(() => props.capture?.status === 'active')
const selectedSoftware = computed(() => props.software.find(item => item.id === softwareId.value))
const capturedSoftware = computed(() => props.software.find(item => item.id === props.capture?.softwareId))
const filteredCatalog = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return (props.result?.catalog.entries ?? []).filter(entry => !needle || `${entry.source} ${entry.adapterId}`.toLocaleLowerCase().includes(needle))
})
const filteredDraft = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase()
  return (props.result?.dictionaryDraft.entries ?? []).filter(entry => !needle || entry.source.toLocaleLowerCase().includes(needle))
})
const currentRows = computed(() => resultMode.value === 'catalog' ? filteredCatalog.value : filteredDraft.value)
const pageCount = computed(() => Math.max(1, Math.ceil(currentRows.value.length / pageSize)))
const visibleRows = computed(() => currentRows.value.slice((page.value - 1) * pageSize, page.value * pageSize))
const catalogColumns = computed<TableColumn<CaptureCatalogEntry>[]>(() => [
  { accessorKey: 'source', header: t('capture.columns.source'), meta: { class: { th: 'w-[52%]', td: 'w-[52%]' } } },
  { accessorKey: 'adapterId', header: t('capture.columns.adapter'), meta: { class: { th: 'w-[36%]', td: 'w-[36%]' } } },
  { accessorKey: 'count', header: t('capture.columns.count'), meta: { class: { th: 'w-[12%] text-right', td: 'w-[12%] text-right' } } },
])
const draftColumns = computed<TableColumn<DictionaryEntry>[]>(() => [
  { accessorKey: 'source', header: t('capture.columns.source'), meta: { class: { th: 'w-[52%]', td: 'w-[52%]' } } },
  { accessorKey: 'translation', header: t('capture.columns.translation'), meta: { class: { th: 'w-[48%]', td: 'w-[48%]' } } },
])

watch([query, resultMode], () => { page.value = 1 })

function toggleAdapter(id: string, enabled: boolean | 'indeterminate') {
  if (enabled === true && !adapterIds.value.includes(id)) adapterIds.value.push(id)
  if (enabled !== true) adapterIds.value = adapterIds.value.filter(candidate => candidate !== id)
}
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col overflow-hidden px-4 py-3">
    <ManagementPageHeader title-id="capture-title" icon="i-tabler-radar" :title="t('capture.title')" :description="t('capture.description')">
      <template #actions>
        <UButton
          v-if="active"
          color="error"
          variant="solid"
          size="sm"
          icon="i-tabler-player-stop-filled"
          :label="t('capture.stop')"
          :loading="busy"
          @click="emit('stop')"
        />
        <UButton
          v-else
          color="primary"
          variant="solid"
          size="sm"
          icon="i-tabler-radar"
          :label="t('capture.start')"
          :loading="busy"
          :disabled="!softwareId || !adapterIds.length"
          @click="emit('start', softwareId, adapterIds)"
        />
      </template>
    </ManagementPageHeader>

    <UAlert v-if="message" role="alert" color="error" variant="soft" :title="t('capture.error')" :description="message" class="mb-3" />

    <div class="mb-3 grid shrink-0 grid-cols-[minmax(220px,0.72fr)_minmax(420px,1.28fr)] gap-3">
      <section class="rounded-[6px] border border-[var(--border)] bg-[var(--surface)] p-3" aria-labelledby="capture-target-title">
        <h2 id="capture-target-title" class="m-0 text-[11px] font-semibold">{{ t('capture.target') }}</h2>
        <p class="mb-3 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('capture.targetHint') }}</p>
        <USelect
          v-model="softwareId"
          :items="software.map(item => ({ value: item.id, label: item.name }))"
          value-key="value"
          label-key="label"
          class="w-full"
          :disabled="active"
          :placeholder="t('capture.chooseSoftware')"
        />
        <div v-if="selectedSoftware" class="mt-2 truncate text-[9px] text-[var(--text-muted)]">{{ selectedSoftware.executableName }}</div>
      </section>

      <section class="rounded-[6px] border border-[var(--border)] bg-[var(--surface)] p-3" aria-labelledby="capture-adapters-title">
        <div class="flex items-baseline justify-between gap-3">
          <div>
            <h2 id="capture-adapters-title" class="m-0 text-[11px] font-semibold">{{ t('capture.adapters') }}</h2>
            <p class="mb-2 mt-1 text-[9px] leading-4 text-[var(--text-muted)]">{{ t('capture.adaptersHint') }}</p>
          </div>
          <span class="text-[9px] tabular-nums text-[var(--text-muted)]">{{ t('capture.selectedCount', { count: adapterIds.length }) }}</span>
        </div>
        <div v-if="observableAdapters.length" class="grid grid-cols-2 gap-1.5">
          <label v-for="adapter in observableAdapters" :key="adapter.id" class="flex min-h-10 items-start gap-2 rounded-[5px] border border-[var(--border)] px-2 py-1.5 hover:bg-[var(--surface-hover)]">
            <UCheckbox :model-value="adapterIds.includes(adapter.id)" :disabled="active" class="mt-0.5" @update:model-value="toggleAdapter(adapter.id, $event)" />
            <span class="min-w-0">
              <strong class="block truncate text-[10px]">{{ adapter.name }}</strong>
              <span class="block truncate text-[9px] text-[var(--text-muted)]" :title="adapter.technicalTarget">{{ adapter.technicalTarget }}</span>
            </span>
          </label>
        </div>
        <UEmpty v-else icon="i-tabler-plug-off" :title="t('capture.noAdapters')" :description="t('capture.noAdaptersHint')" size="sm" />
      </section>
    </div>

    <section v-if="active" class="grid min-h-0 flex-1 place-items-center rounded-[6px] border border-[var(--border)] bg-[var(--surface)]">
      <div class="max-w-[520px] text-center">
        <span class="mx-auto mb-3 grid h-12 w-12 place-items-center rounded-full bg-[color-mix(in_srgb,var(--accent)_14%,transparent)] text-[var(--accent)]"><UIcon name="i-tabler-radar" class="h-6 w-6 animate-pulse" /></span>
        <h2 class="m-0 text-[13px] font-semibold">{{ t('capture.listening', { software: capturedSoftware?.name ?? capture?.softwareId }) }}</h2>
        <p class="mx-auto mb-0 mt-2 max-w-[460px] text-[10px] leading-5 text-[var(--text-muted)]">{{ t('capture.listeningHint') }}</p>
      </div>
    </section>

    <section v-else-if="capture?.status === 'completed'" class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[6px] border border-[var(--border)] bg-[var(--surface)]">
      <div class="flex shrink-0 items-center gap-2 border-b border-[var(--border)] px-3 py-2">
        <UButton size="xs" color="neutral" :variant="resultMode === 'catalog' ? 'solid' : 'ghost'" :label="t('capture.catalog')" @click="resultMode = 'catalog'" />
        <UButton size="xs" color="neutral" :variant="resultMode === 'draft' ? 'solid' : 'ghost'" :label="t('capture.draft')" @click="resultMode = 'draft'" />
        <span class="ml-1 text-[9px] text-[var(--text-muted)]">{{ t('capture.resultSummary', { entries: capture.entryCount, dropped: capture.droppedObservations }) }}</span>
        <UInput v-model="query" icon="i-tabler-search" size="sm" class="ml-auto w-64" :placeholder="t('capture.search')" />
      </div>
      <div class="min-h-0 flex-1 overflow-auto">
        <UTable
          v-if="currentRows.length && resultMode === 'catalog'"
          :data="visibleRows as CaptureCatalogEntry[]"
          :columns="catalogColumns"
          :ui="{ base: 'table-fixed', thead: 'sticky top-0 z-10 bg-[var(--surface-subtle)]' }"
        >
          <template #source-cell="{ row }"><div class="truncate font-medium" :title="row.original.source">{{ row.original.source }}</div></template>
          <template #adapterId-cell="{ row }"><div class="truncate font-mono text-[9px] text-[var(--text-secondary)]" :title="row.original.adapterId">{{ row.original.adapterId }}</div></template>
          <template #count-cell="{ row }"><div class="text-right tabular-nums">{{ row.original.count }}</div></template>
        </UTable>
        <UTable
          v-else-if="currentRows.length"
          :data="visibleRows as DictionaryEntry[]"
          :columns="draftColumns"
          :ui="{ base: 'table-fixed', thead: 'sticky top-0 z-10 bg-[var(--surface-subtle)]' }"
        >
          <template #source-cell="{ row }"><div class="truncate font-medium" :title="row.original.source">{{ row.original.source }}</div></template>
          <template #translation-cell><span class="text-[var(--text-muted)]">{{ t('capture.pendingTranslation') }}</span></template>
        </UTable>
        <UEmpty v-if="!currentRows.length" icon="i-tabler-radar-off" :title="t('capture.emptyResult')" :description="t('capture.emptyResultHint')" />
      </div>
      <div v-if="currentRows.length" class="flex shrink-0 items-center justify-between border-t border-[var(--border)] px-3 py-2 text-[9px] text-[var(--text-muted)]">
        <span>{{ t('capture.visibleCount', { count: currentRows.length }) }}</span>
        <UPagination v-model:page="page" :items-per-page="pageSize" :total="currentRows.length" size="xs" />
      </div>
    </section>

    <section v-else class="grid min-h-0 flex-1 place-items-center rounded-[6px] border border-dashed border-[var(--border)] bg-[var(--surface)]">
      <UEmpty icon="i-tabler-radar" :title="t('capture.empty')" :description="t('capture.emptyHint')" />
    </section>
  </section>
</template>
