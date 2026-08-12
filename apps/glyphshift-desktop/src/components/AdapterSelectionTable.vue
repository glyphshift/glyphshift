<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AdapterOption } from '../model'

const props = defineProps<{
  adapters: AdapterOption[]
  modelValue: string[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string[]]
}>()

const { t } = useI18n()
const columns = computed<TableColumn<AdapterOption>[]>(() => [
  { id: 'select', header: '', meta: { class: { th: 'w-24', td: 'w-24' } } },
  { id: 'adapter', header: t('workflows.adapterTable.adapter'), meta: { class: { th: 'w-[48%]', td: 'w-[48%]' } } },
  { id: 'platform', header: t('workflows.adapterTable.platform'), meta: { class: { th: 'w-28', td: 'w-28' } } },
  { id: 'technology', header: t('workflows.adapterTable.technology'), meta: { class: { th: 'w-44', td: 'w-44' } } },
])
const selectedCount = computed(() => props.adapters.filter(adapter => props.modelValue.includes(adapter.id)).length)
const allSelectionState = computed<boolean | 'indeterminate'>(() => {
  if (!selectedCount.value) return false
  return selectedCount.value === props.adapters.length ? true : 'indeterminate'
})

function orderedSelection(selected: Set<string>) {
  return props.adapters.map(adapter => adapter.id).filter(id => selected.has(id))
}

function toggle(adapterId: string, checked: boolean | 'indeterminate') {
  const selected = new Set(props.modelValue)
  if (checked === true) selected.add(adapterId)
  else selected.delete(adapterId)
  emit('update:modelValue', orderedSelection(selected))
}

function toggleAll(checked: boolean | 'indeterminate') {
  emit('update:modelValue', checked === true ? props.adapters.map(adapter => adapter.id) : [])
}

function toggleAllFromHeader(event: MouseEvent) {
  if (event.target instanceof Element && event.target.closest('button')) return
  toggleAll(allSelectionState.value !== true)
}

function platformLabel(value: string) {
  if (value === 'windows') return 'Windows'
  if (value === 'macos') return 'macOS'
  return value
}
</script>

<template>
  <div data-testid="workflow-adapter-table" class="overflow-x-auto rounded-[6px] border border-[var(--border)] bg-[var(--surface)]">
    <UTable :data="adapters" :columns="columns" :ui="{ base: 'min-w-[720px] table-fixed' }">
      <template #select-header>
        <div class="-m-2 flex min-h-8 cursor-pointer items-center gap-2 p-2" @click="toggleAllFromHeader">
          <UCheckbox
            data-testid="workflow-adapter-select-all"
            :model-value="allSelectionState"
            :aria-label="t('workflows.adapterTable.toggleAll')"
            @update:model-value="toggleAll"
          />
          <span class="type-label whitespace-nowrap font-medium">{{ t(allSelectionState === true ? 'workflows.adapterTable.clearAll' : 'workflows.adapterTable.selectAll') }}</span>
        </div>
      </template>
      <template #select-cell="{ row }">
        <UCheckbox
          :model-value="modelValue.includes(row.original.id)"
          :aria-label="t('workflows.adapterTable.selectNamed', { name: row.original.name })"
          @update:model-value="toggle(row.original.id, $event)"
        />
      </template>
      <template #adapter-cell="{ row }">
        <div class="min-w-0">
          <div class="type-label truncate font-semibold text-[var(--text)]">{{ row.original.name }}</div>
          <div class="type-metadata mt-0.5 line-clamp-2 leading-4 text-[var(--text-muted)]">{{ row.original.summary }}</div>
        </div>
      </template>
      <template #platform-cell="{ row }">
        <div class="flex flex-wrap gap-1">
          <UBadge v-for="platform in row.original.platforms" :key="platform" color="neutral" variant="soft" size="sm" :label="platformLabel(platform)" />
          <span v-if="!row.original.platforms.length" class="type-metadata text-[var(--text-muted)]">{{ t('workflows.crossPlatform') }}</span>
        </div>
      </template>
      <template #technology-cell="{ row }">
        <div class="flex flex-wrap gap-1">
          <UBadge v-for="technology in row.original.technologies" :key="technology" color="neutral" variant="outline" size="sm" :label="technology" />
          <span v-if="!row.original.technologies.length" class="type-metadata text-[var(--text-muted)]">{{ t('workflows.otherTechnology') }}</span>
        </div>
      </template>
    </UTable>
  </div>
</template>
