<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  open: boolean
  title: string
  description?: string
  confirmLabel: string
  confirmVisible?: boolean
  confirmDisabled?: boolean
  busy?: boolean
  width?: 'sm' | 'md' | 'lg' | 'xl'
  workspace?: boolean
}>(), {
  description: '',
  confirmVisible: true,
  confirmDisabled: false,
  busy: false,
  width: 'md',
  workspace: false,
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
const contentClass = computed(() => `${widthClass.value} flex flex-col ${props.workspace ? 'h-[720px] max-h-[calc(100dvh-32px)]' : 'max-h-[calc(100dvh-32px)]'}`)
const bodyClass = computed(() => props.workspace
  ? 'flex min-h-0 flex-1 flex-col overflow-hidden p-0'
  : 'min-h-0 overflow-y-auto px-5 py-4')
</script>

<template>
  <UModal
    :open="open"
    :title="title"
    :description="description"
    :dismissible="!busy"
    :ui="{
      content: contentClass,
      header: 'min-h-0 shrink-0 px-5 py-4',
      title: 'text-[15px]',
      description: 'type-metadata mt-1 leading-4',
      body: bodyClass,
      footer: 'shrink-0 px-5 py-4',
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
        v-if="confirmVisible"
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
