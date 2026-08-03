<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  variant?: 'surface' | 'canvas'
}>(), {
  variant: 'surface',
})

const surfaceClass = computed(() => props.variant === 'canvas'
  ? 'min-h-0 min-w-0 flex-1 overflow-hidden bg-[var(--app-bg)]'
  : 'min-h-0 min-w-0 flex-1 overflow-hidden rounded-[8px] border border-[var(--border)] bg-[var(--surface)]')
</script>

<template>
  <div
    data-testid="management-workspace-surface"
    :class="surfaceClass"
  >
    <slot />
  </div>
</template>
