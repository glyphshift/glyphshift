<script setup lang="ts">
defineProps<{
  title: string
  description: string
  titleId: string
  backLabel: string
}>()

defineEmits<{ back: [] }>()
</script>

<template>
  <header
    data-testid="management-detail-header"
    class="-mx-4 -mt-4 mb-4 flex h-16 shrink-0 items-center justify-between gap-4 border-b border-[var(--border)] bg-[var(--titlebar)] px-4"
  >
    <div class="flex min-w-0 items-center gap-2">
      <UButton
        color="neutral"
        variant="ghost"
        size="sm"
        icon="i-tabler-arrow-left"
        :aria-label="backLabel"
        :title="backLabel"
        class="-ml-1 h-8 w-8 shrink-0 rounded-[6px]"
        @click="$emit('back')"
      />
      <div class="min-w-0">
        <div class="flex min-w-0 items-center gap-2">
          <h1 :id="titleId" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">{{ title }}</h1>
          <slot name="status" />
        </div>
        <p class="m-0 mt-0.5 truncate text-[10px] text-[var(--text-muted)]">{{ description }}</p>
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <slot name="actions" />
    </div>
  </header>
</template>
