<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  open: boolean
  title: string
  description?: string
  confirmLabel: string
  confirmDisabled?: boolean
  busy?: boolean
  width?: 'sm' | 'md' | 'lg' | 'xl'
}>(), {
  description: '',
  confirmDisabled: false,
  busy: false,
  width: 'md',
})

const emit = defineEmits<{
  'update:open': [value: boolean]
  'confirm': []
}>()
const { t } = useI18n()

const widthClass = computed(() => ({
  sm: 'max-w-[400px]',
  md: 'max-w-[480px]',
  lg: 'max-w-[640px]',
  xl: 'max-w-[760px]',
})[props.width])
</script>

<template>
  <UModal
    :open="open"
    :title="title"
    :description="description"
    :dismissible="!busy"
    :ui="{
      content: widthClass,
      header: 'min-h-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'mt-1 text-[10px] leading-4',
      body: 'px-5 py-4',
      footer: 'px-5 py-4',
    }"
    @update:open="emit('update:open', $event)"
  >
    <template #body>
      <slot />
    </template>
    <template #footer>
      <slot name="footer-leading" />
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
        color="primary"
        variant="solid"
        size="sm"
        :label="confirmLabel"
        :loading="busy"
        :disabled="confirmDisabled"
        @click="emit('confirm')"
      />
    </template>
  </UModal>
</template>
