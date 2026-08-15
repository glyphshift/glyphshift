<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  title: string
  description: string
  titleId: string
  backLabel?: string
  contentWidth?: 'full' | 'utility'
  flushAfter?: boolean
}>()

const { t } = useI18n()

defineEmits<{ back: [] }>()
</script>

<template>
  <header
    data-testid="management-detail-header"
    class="-mx-4 -mt-4 flex shrink-0 border-b border-[var(--border)] bg-[var(--titlebar)] px-4"
    :class="[$slots.detail ? 'min-h-20' : 'min-h-16', props.flushAfter ? 'mb-0' : 'mb-4']"
  >
    <div
      :class="props.contentWidth === 'utility'
        ? 'mx-auto w-full'
        : 'w-full'"
      :style="props.contentWidth === 'utility'
        ? {
            maxWidth: 'calc(var(--utility-page-content-width) + var(--utility-page-axis-inset) + var(--utility-page-axis-inset))',
            paddingInline: 'var(--utility-page-axis-inset)',
          }
        : undefined"
    >
      <div
        data-testid="management-detail-header-content"
        class="flex w-full items-center justify-between gap-4"
        :class="$slots.detail ? 'min-h-20' : 'min-h-16'"
      >
        <div class="flex min-w-0 items-center gap-2">
          <UButton
            v-if="backLabel"
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
              <h1 :id="titleId" class="type-page-title m-0 truncate font-semibold tracking-[-0.02em]">{{ title }}</h1>
              <slot name="status" />
            </div>
            <p class="type-metadata m-0 mt-0.5 truncate text-[var(--text-muted)]">{{ description }}</p>
            <div v-if="$slots.detail" class="mt-0.5 min-w-0">
              <slot name="detail" />
            </div>
          </div>
        </div>
        <div v-if="$slots.actions" role="toolbar" :aria-label="t('common.pageActions', { title: props.title })" class="flex shrink-0 items-center gap-2">
          <slot name="actions" />
        </div>
      </div>
    </div>
  </header>
</template>
