<script setup lang="ts">
import { useI18n } from 'vue-i18n'
defineProps<{ label: string; text: string }>()
const { t } = useI18n()
</script>
<template>
  <UPopover mode="hover">
    <UButton icon="i-tabler-help-circle" color="neutral" variant="ghost" size="xs" class="shrink-0" :aria-label="t('common.fieldHelp', { label })" />
    <template #content><p class="m-0 max-w-80 p-3 text-xs font-normal leading-5 text-[var(--text-secondary)]">{{ text }}</p></template>
  </UPopover>
</template>
