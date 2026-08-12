<script setup lang="ts">
import { computed } from 'vue'
import type { DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue'
import { useI18n } from 'vue-i18n'

type FilterOption = {
  label: string
  value: string
}

type ColumnOption = {
  key: string
  label: string
  visible: boolean
  defaultVisible?: boolean
}

const props = withDefaults(defineProps<{
  query: string
  searchPlaceholder: string
  searchLabel: string
  filterLabel?: string
  filterAriaLabel?: string
  filterValue?: string
  filterOptions?: FilterOption[]
  columnsLabel?: string
  columnOptions?: ColumnOption[]
  selectedCount?: number
  selectedLabel?: string
  page: number
  pageSize: number
  total: number
  itemLabel: string
  paginationMode?: 'offset' | 'cursor'
  cursorPage?: number
  hasNextPage?: boolean
  footerSummary?: string
}>(), {
  filterLabel: '',
  filterAriaLabel: '',
  filterValue: '',
  filterOptions: () => [],
  columnsLabel: '',
  columnOptions: () => [],
  selectedCount: 0,
  selectedLabel: '',
  paginationMode: 'offset',
  cursorPage: 1,
  hasNextPage: false,
  footerSummary: '',
})

const { t } = useI18n()

const emit = defineEmits<{
  'update:query': [value: string]
  'update:filterValue': [value: string]
  'update:page': [value: number]
  'update:pageSize': [value: number]
  'toggleColumn': [key: string, visible: boolean]
  search: []
  previousPage: []
  nextPage: []
}>()

const filterItems = computed(() => props.filterOptions.map(option => ({
  label: option.label,
  icon: option.value === props.filterValue ? 'i-tabler-check' : undefined,
  onSelect: () => emit('update:filterValue', option.value),
})))

const rangeStart = computed(() => props.total ? (props.page - 1) * props.pageSize + 1 : 0)
const rangeEnd = computed(() => Math.min(props.page * props.pageSize, props.total))
const pageSizeOptions = [
  { label: '20', value: 20 },
  { label: '50', value: 50 },
  { label: '100', value: 100 },
]
const columnItems = computed<DropdownMenuItem[][]>(() => [
  props.columnOptions.map(option => ({
    type: 'checkbox' as const,
    label: option.label,
    checked: option.visible,
    onSelect: event => event.preventDefault(),
    onUpdateChecked: checked => emit('toggleColumn', option.key, checked),
  })),
  [{
    label: t('table.resetColumns'),
    icon: 'i-tabler-restore',
    onSelect: () => props.columnOptions.forEach(option => emit('toggleColumn', option.key, option.defaultVisible ?? true)),
  }],
])

function updatePageSize(value: unknown) {
  const next = Number(value)
  if (!pageSizeOptions.some(option => option.value === next)) return
  emit('update:pageSize', next)
  emit('update:page', 1)
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-[8px] border border-[var(--border)] bg-[var(--surface)]">
    <div role="toolbar" :aria-label="t('table.toolbar')" class="flex shrink-0 gap-2 border-b border-[var(--border)] p-3">
      <UInput
        :model-value="query"
        icon="i-tabler-search"
        size="sm"
        class="min-w-0 flex-1"
        :placeholder="searchPlaceholder"
        :aria-label="searchLabel"
        @update:model-value="emit('update:query', String($event ?? ''))"
        @keyup.enter="emit('search')"
      />

      <UDropdownMenu v-if="filterOptions.length" :items="filterItems" :content="{ align: 'end' }">
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-tabler-filter"
          trailing-icon="i-tabler-chevron-down"
          :label="filterLabel"
          class="min-w-32 justify-between"
          :aria-label="filterAriaLabel || t('table.filterItems', { items: itemLabel })"
        />
      </UDropdownMenu>

      <UDropdownMenu
        v-if="columnOptions.length"
        :items="columnItems"
        :content="{ align: 'end' }"
        :ui="{ content: 'min-w-32' }"
      >
        <UButton
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-tabler-columns-3"
          trailing-icon="i-tabler-chevron-down"
          :label="columnsLabel || t('table.columns')"
          :aria-label="columnsLabel || t('table.columns')"
        />
      </UDropdownMenu>

      <slot name="toolbar-actions" />
    </div>

    <div
      v-if="selectedCount"
      class="type-label flex h-11 shrink-0 items-center gap-2 border-b border-[var(--border)] bg-[var(--surface-subtle)] px-3"
    >
      <strong>{{ t('table.selected', { count: selectedCount, items: selectedLabel || t('table.items') }) }}</strong>
      <div role="toolbar" :aria-label="t('table.bulkToolbar')" class="ml-auto flex items-center gap-2">
        <slot name="bulk-actions" />
      </div>
    </div>

    <div
      data-testid="management-table-body"
      class="relative min-h-0 flex-1 overflow-hidden bg-[var(--surface-inset)] [&_[data-slot=empty]]:!h-0 [&_[data-slot=empty]>[data-slot=root]]:absolute [&_[data-slot=empty]>[data-slot=root]]:inset-x-0 [&_[data-slot=empty]>[data-slot=root]]:top-8 [&_[data-slot=empty]>[data-slot=root]]:bottom-0 [&_[data-slot=empty]>[data-slot=root]]:!h-auto [&_[data-slot=tbody]>[data-slot=tr]:last-child:not(:has([data-slot=empty]))]:border-b [&_[data-slot=tbody]>[data-slot=tr]:last-child:not(:has([data-slot=empty]))]:border-[var(--border)]"
    >
      <slot />
    </div>

    <footer class="type-label flex h-14 shrink-0 items-center border-t border-[var(--border)] px-3 text-[var(--text-muted)]">
      <span>{{ footerSummary || t('table.range', { start: rangeStart, end: rangeEnd, total, items: itemLabel }) }}</span>
      <div class="ml-auto flex items-center gap-3">
        <UPagination
          v-if="paginationMode === 'offset'"
          :page="page"
          :total="total"
          :items-per-page="pageSize"
          :sibling-count="0"
          :show-edges="true"
          :show-controls="true"
          size="sm"
          color="neutral"
          variant="outline"
          active-color="primary"
          active-variant="soft"
          @update:page="emit('update:page', $event)"
        >
          <template #first>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevrons-left" :aria-label="t('table.firstPage')" />
          </template>
          <template #prev>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevron-left" :aria-label="t('table.previousPage')" />
          </template>
          <template #next>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevron-right" :aria-label="t('table.nextPage')" />
          </template>
          <template #last>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevrons-right" :aria-label="t('table.lastPage')" />
          </template>
        </UPagination>
        <div v-else class="flex items-center gap-1">
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-tabler-chevron-left"
            :disabled="cursorPage <= 1"
            :aria-label="t('table.previousPage')"
            @click="emit('previousPage')"
          />
          <span class="type-label min-w-14 text-center text-[var(--text-secondary)]">{{ t('table.pageNumber', { page: cursorPage }) }}</span>
          <UButton
            color="neutral"
            variant="outline"
            size="sm"
            icon="i-tabler-chevron-right"
            :disabled="!hasNextPage"
            :aria-label="t('table.nextPage')"
            @click="emit('nextPage')"
          />
        </div>
        <span class="whitespace-nowrap">{{ t('table.perPage') }}</span>
        <USelect
          :model-value="pageSize"
          :items="pageSizeOptions"
          value-key="value"
          label-key="label"
          size="sm"
          color="neutral"
          variant="outline"
          class="w-24"
          :aria-label="t('table.perPageLabel')"
          @update:model-value="updatePageSize"
        />
      </div>
    </footer>
  </div>
</template>
