<script setup lang="ts">
defineProps<{
  title: string
  description: string
  icon: string
  titleId: string
}>()
</script>

<template>
  <header class="mb-4 flex min-h-10 items-center gap-3">
    <span
      class="grid h-10 w-10 shrink-0 place-items-center rounded-[7px] border border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--accent-strong)]"
      role="img"
      :aria-label="`${title}图标`"
    >
      <UIcon :name="icon" class="size-5" />
    </span>
    <div class="min-w-0">
      <h1 :id="titleId" class="m-0 truncate text-[20px] font-semibold tracking-[-0.02em]">
        {{ title }}
      </h1>
      <p class="mb-0 mt-1 text-[10px] text-[var(--text-muted)]">
        {{ description }}
      </p>
    </div>
    <div class="ml-auto flex shrink-0 items-center gap-2">
      <slot name="actions" />
    </div>
  </header>
</template>
