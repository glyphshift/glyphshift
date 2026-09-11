<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { LibrarySort } from '../useLibrarySort'
const props = defineProps<{ modelValue: LibrarySort; options: { value: LibrarySort; label: string }[] }>()
const emit = defineEmits<{ 'update:modelValue': [value: LibrarySort] }>()
const { t } = useI18n()
const items = computed(() => props.options.map(option => ({
  label: option.label,
  icon: option.value === props.modelValue ? 'i-tabler-check' : undefined,
  onSelect: () => emit('update:modelValue', option.value),
})))
</script>
<template>
  <UDropdownMenu :items="items" :content="{ align: 'end' }">
    <UButton color="neutral" variant="outline" size="sm" icon="i-tabler-arrows-sort" trailing-icon="i-tabler-chevron-down" :aria-label="t('librarySort.label')" :label="options.find(option => option.value === modelValue)?.label" />
  </UDropdownMenu>
</template>
