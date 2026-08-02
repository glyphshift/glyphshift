<script setup lang="ts">
import { computed } from 'vue'
import type { DropdownMenuItem } from '@nuxt/ui/components/DropdownMenu.vue'

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
}>(), {
  filterLabel: '',
  filterAriaLabel: '',
  filterValue: '',
  filterOptions: () => [],
  columnsLabel: '显示列',
  columnOptions: () => [],
  selectedCount: 0,
  selectedLabel: '项',
})

const emit = defineEmits<{
  'update:query': [value: string]
  'update:filterValue': [value: string]
  'update:page': [value: number]
  'update:pageSize': [value: number]
  'toggleColumn': [key: string, visible: boolean]
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
    label: '恢复默认列',
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
    <div class="flex shrink-0 gap-2 border-b border-[var(--border)] p-3">
      <UInput
        :model-value="query"
        icon="i-tabler-search"
        size="sm"
        class="min-w-0 flex-1"
        :placeholder="searchPlaceholder"
        :aria-label="searchLabel"
        @update:model-value="emit('update:query', String($event ?? ''))"
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
          :aria-label="filterAriaLabel || `筛选${itemLabel}`"
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
          :label="columnsLabel"
          :aria-label="columnsLabel"
        />
      </UDropdownMenu>

      <slot name="toolbar-actions" />
    </div>

    <div
      v-if="selectedCount"
      class="flex h-11 shrink-0 items-center gap-2 border-b border-[var(--border)] bg-[var(--surface-subtle)] px-3 text-[10px]"
    >
      <strong>{{ selectedCount }} {{ selectedLabel }}已选择</strong>
      <div class="ml-auto flex items-center gap-2">
        <slot name="bulk-actions" />
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-hidden">
      <slot />
    </div>

    <footer class="flex h-14 shrink-0 items-center border-t border-[var(--border)] px-3 text-[10px] text-[var(--text-muted)]">
      <span>显示 {{ rangeStart }}–{{ rangeEnd }}，共 {{ total }} {{ itemLabel }}</span>
      <div class="ml-auto flex items-center gap-3">
        <UPagination
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
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevrons-left" aria-label="首页" />
          </template>
          <template #prev>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevron-left" aria-label="上一页" />
          </template>
          <template #next>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevron-right" aria-label="下一页" />
          </template>
          <template #last>
            <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-chevrons-right" aria-label="末页" />
          </template>
        </UPagination>
        <span class="whitespace-nowrap">每页</span>
        <USelect
          :model-value="pageSize"
          :items="pageSizeOptions"
          value-key="value"
          label-key="label"
          size="sm"
          color="neutral"
          variant="outline"
          class="w-24"
          aria-label="每页数量"
          @update:model-value="updatePageSize"
        />
      </div>
    </footer>
  </div>
</template>
