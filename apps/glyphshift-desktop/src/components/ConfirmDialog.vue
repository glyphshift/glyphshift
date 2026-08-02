<script setup lang="ts">
import { useI18n } from 'vue-i18n'

withDefaults(defineProps<{
  open: boolean
  title: string
  description: string
  confirmLabel?: string
  busy?: boolean
}>(), {
  confirmLabel: '',
  busy: false,
})

const { t } = useI18n()

const emit = defineEmits<{
  'update:open': [value: boolean]
  'confirm': []
}>()
</script>

<template>
  <UModal
    :open="open"
    :title="title"
    :description="description"
    :dismissible="!busy"
    :ui="{
      content: 'max-w-[400px]',
      header: 'min-h-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'mt-1 text-[11px] leading-5 text-[var(--text-secondary)]',
      footer: 'px-5 py-4',
    }"
    @update:open="emit('update:open', $event)"
  >
    <template #footer>
      <UButton
        color="neutral"
        variant="outline"
        size="sm"
        :label="t('common.cancel')"
        class="ml-auto"
        :disabled="busy"
        @click="emit('update:open', false)"
      />
      <UButton
        color="error"
        variant="soft"
        size="sm"
        :label="confirmLabel || t('common.confirmDelete')"
        :loading="busy"
        @click="emit('confirm')"
      />
    </template>
  </UModal>
</template>
