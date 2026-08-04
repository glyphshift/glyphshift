<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { AdapterOption } from '../model'

const props = withDefaults(defineProps<{
  adapters: AdapterOption[]
  modelValue: string[]
  disabled?: boolean
}>(), {
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string[]]
}>()

const { t } = useI18n()
const groups = computed(() => [{
  id: 'realtime',
  label: t('capture.realtimeAdapters'),
  hint: t('capture.realtimeAdaptersHint'),
  adapters: props.adapters.filter(adapter => adapter.features.includes('textReplace')),
}, {
  id: 'collection',
  label: t('capture.collectionAdapters'),
  hint: t('capture.collectionAdaptersHint'),
  adapters: props.adapters.filter(adapter => !adapter.features.includes('textReplace')),
}].filter(group => group.adapters.length))

function toggle(adapterId: string, checked: boolean | 'indeterminate') {
  if (props.disabled) return
  const selected = new Set(props.modelValue)
  if (checked === true) selected.add(adapterId)
  else selected.delete(adapterId)
  emit('update:modelValue', props.adapters.map(adapter => adapter.id).filter(id => selected.has(id)))
}
</script>

<template>
  <div class="rounded-[6px] border border-[var(--border)]">
    <section v-for="(group, groupIndex) in groups" :key="group.id" :class="groupIndex ? 'border-t border-[var(--border)]' : ''">
      <div class="bg-[var(--surface-subtle)] px-3 py-2">
        <div class="flex items-center justify-between gap-3">
          <strong class="text-[10px] font-semibold text-[var(--text)]">{{ group.label }}</strong>
          <span class="tabular-nums text-[9px] text-[var(--text-muted)]">{{ group.adapters.length }}</span>
        </div>
        <p class="mt-0.5 mb-0 text-[9px] leading-4 text-[var(--text-muted)]">{{ group.hint }}</p>
      </div>
      <label
        v-for="adapter in group.adapters"
        :key="adapter.id"
        class="flex min-h-10 items-center gap-2 border-t border-[var(--border)] px-3"
        :class="disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer hover:bg-[var(--surface-hover)]'"
      >
        <UCheckbox :model-value="modelValue.includes(adapter.id)" :disabled="disabled" :aria-label="adapter.name" @update:model-value="toggle(adapter.id, $event)" />
        <span class="min-w-0 flex-1 truncate text-[10px] font-medium">{{ adapter.name }}</span>
        <span class="max-w-32 truncate text-[9px] text-[var(--text-muted)]">{{ adapter.technologies.join(' · ') }}</span>
      </label>
    </section>
  </div>
</template>
