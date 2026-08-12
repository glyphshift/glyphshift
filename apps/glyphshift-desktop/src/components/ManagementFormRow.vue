<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  label: string
  description?: string
  icon?: string
  required?: boolean
  controlWidth?: 'compact' | 'fill'
  multiline?: boolean
}>(), {
  description: undefined,
  icon: undefined,
  required: false,
  controlWidth: 'fill',
  multiline: false,
})

const fieldUi = computed(() => ({
  root: props.multiline
    ? '!grid min-h-[84px] grid-cols-[184px_minmax(0,1fr)] items-start gap-8 border-b border-[var(--border)] py-4 last:border-b-0 @max-[620px]:grid-cols-1 @max-[620px]:gap-3'
    : '!grid min-h-[76px] grid-cols-[184px_minmax(0,1fr)] items-center gap-8 border-b border-[var(--border)] py-3 last:border-b-0 @max-[620px]:grid-cols-1 @max-[620px]:items-start @max-[620px]:gap-3',
  wrapper: props.multiline ? 'min-w-0 pt-1.5 @max-[620px]:pt-0' : 'min-w-0',
  label: 'type-label flex items-center gap-2.5 font-semibold text-[var(--text-secondary)]',
  description: props.icon
    ? 'type-metadata mt-1 pl-7 leading-4 text-[var(--text-muted)]'
    : 'type-metadata mt-1 leading-4 text-[var(--text-muted)]',
  container: props.controlWidth === 'compact'
    ? 'min-w-0 w-full max-w-[240px] justify-self-end @max-[620px]:max-w-none @max-[620px]:justify-self-stretch'
    : 'min-w-0 w-full',
}))
</script>

<template>
  <UFormField
    orientation="horizontal"
    :label="label"
    :description="description"
    :required="required"
    :ui="fieldUi"
  >
    <template v-if="icon" #label>
      <UIcon :name="icon" class="size-[18px] shrink-0 text-[var(--accent-strong)]" aria-hidden="true" />
      <span>{{ label }}</span>
    </template>
    <slot />
  </UFormField>
</template>
