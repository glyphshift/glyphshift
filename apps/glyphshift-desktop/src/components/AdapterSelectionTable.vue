<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui/components/Table.vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AdapterOption } from '../model'
import { adapterName, adapterSummary } from '../adapterPresentation'

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
  { id: 'adapter', header: t('workflows.adapterTable.adapter') },
])
const selectableAdapters = computed(() => props.adapters.filter(adapter => adapter.availableForNewUsage !== false))
const selectedCount = computed(() => selectableAdapters.value.filter(adapter => props.modelValue.includes(adapter.id)).length)
const allSelectionState = computed<boolean | 'indeterminate'>(() => {
  if (!selectableAdapters.value.length || !selectedCount.value) return false
  return selectedCount.value === selectableAdapters.value.length ? true : 'indeterminate'
})

function orderedSelection(selected: Set<string>) {
  return props.adapters.map(adapter => adapter.id).filter(id => selected.has(id))
}

function toggle(adapterId: string, checked: boolean | 'indeterminate') {
  const adapter = props.adapters.find(candidate => candidate.id === adapterId)
  if (checked === true && adapter?.availableForNewUsage === false) return
  const selected = new Set(props.modelValue)
  if (checked === true) selected.add(adapterId)
  else selected.delete(adapterId)
  emit('update:modelValue', orderedSelection(selected))
}

function toggleAll(checked: boolean | 'indeterminate') {
  if (checked !== true) {
    emit('update:modelValue', [])
    return
  }
  const selected = new Set(props.modelValue.filter(id => props.adapters.some(adapter => adapter.id === id && adapter.availableForNewUsage === false)))
  selectableAdapters.value.forEach(adapter => selected.add(adapter.id))
  emit('update:modelValue', orderedSelection(selected))
}

function toggleAllFromHeader(event: MouseEvent) {
  if (event.target instanceof Element && event.target.closest('button')) return
  toggleAll(allSelectionState.value !== true)
}

</script>

<template>
  <div data-testid="workflow-adapter-table" class="overflow-x-auto rounded-[6px] border border-[var(--border)] bg-[var(--surface)]">
    <UTable :data="adapters" :columns="columns" :ui="{ base: 'min-w-[520px] table-fixed' }">
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
          :aria-label="t('workflows.adapterTable.selectNamed', { name: adapterName(row.original, t) })"
          :disabled="row.original.availableForNewUsage === false && !modelValue.includes(row.original.id)"
          @update:model-value="toggle(row.original.id, $event)"
        />
      </template>
      <template #adapter-cell="{ row }">
        <div class="min-w-0">
          <div class="flex min-w-0 items-center gap-2">
            <div class="type-label min-w-0 truncate font-semibold text-[var(--text)]">{{ adapterName(row.original, t) }}</div>
            <UBadge v-if="row.original.availableForNewUsage === false" color="warning" variant="soft" size="sm" :label="t('workflows.adapterTable.unavailable')" />
          </div>
          <div class="type-metadata mt-0.5 line-clamp-2 leading-4 text-[var(--text-muted)]">{{ adapterSummary(row.original, t) }}</div>
          <div v-if="row.original.restartRequired" class="type-metadata mt-1 text-[var(--warning)]">{{ t('workflows.adapterTable.restartRequired') }}</div>
        </div>
      </template>
    </UTable>
  </div>
</template>
