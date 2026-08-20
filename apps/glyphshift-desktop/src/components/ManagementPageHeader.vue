<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const props = withDefaults(defineProps<{
  title: string
  description?: string
  icon: string
  titleId: string
}>(), {
  description: undefined,
})

const { t } = useI18n()
</script>

<template>
  <header data-testid="management-page-header" class="mb-4 flex min-h-10 w-full items-center gap-3">
    <span
      data-testid="management-page-header-icon"
      class="grid h-10 w-10 shrink-0 place-items-center rounded-[7px] border border-[var(--border)] bg-[var(--surface-subtle)] text-[var(--accent-strong)]"
      aria-hidden="true"
    >
      <UIcon :name="icon" class="size-5" aria-hidden="true" />
    </span>
    <div class="min-w-0">
      <h1 :id="titleId" class="type-page-title m-0 truncate font-semibold tracking-[-0.02em]">
        {{ title }}
      </h1>
      <p v-if="description" class="type-metadata mb-0 mt-1 text-[var(--text-muted)]">
        {{ description }}
      </p>
    </div>
    <div v-if="$slots.actions" role="toolbar" :aria-label="t('common.pageActions', { title: props.title })" class="ml-auto flex shrink-0 items-center gap-2">
      <slot name="actions" />
    </div>
  </header>
</template>
