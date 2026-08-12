<script setup lang="ts">
import { useI18n } from 'vue-i18n'

defineProps<{ families: string[] }>()

const emit = defineEmits<{
  move: [family: string, offset: number]
  remove: [family: string]
}>()

const { t } = useI18n()
</script>

<template>
  <section
    data-testid="workflow-selected-fonts"
    class="space-y-2 rounded-[6px] border border-[var(--border)] bg-[var(--surface-subtle)] px-3 py-2.5"
    :aria-label="t('workflows.selectedFontsLabel')"
  >
    <div class="min-w-0">
      <h3 class="type-label m-0 font-semibold" aria-live="polite">
        {{ t('workflows.selectedFonts', { count: families.length }) }}
      </h3>
      <p class="type-metadata m-0 mt-0.5 leading-4 text-[var(--text-muted)]">
        {{ t('workflows.selectedFontsHint') }}
      </p>
    </div>

    <div v-if="families.length" class="flex flex-wrap gap-1.5">
      <div
        v-for="(family, index) in families"
        :key="family"
        :data-selected-font-family="family"
        class="inline-flex min-w-0 max-w-full items-center gap-1 rounded-[6px] border border-[var(--border)] bg-[var(--surface)] py-0.5 pl-1.5 pr-0.5"
      >
        <span class="type-caption grid size-5 shrink-0 place-items-center rounded-[4px] bg-[var(--surface-inset)] font-semibold tabular-nums text-[var(--text-secondary)]">
          {{ index + 1 }}
        </span>
        <span class="type-label max-w-44 truncate font-medium" :title="family">{{ family }}</span>
        <div class="ml-0.5 flex shrink-0 items-center">
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-left" :disabled="index === 0" :aria-label="t('workflows.raiseFont', { name: family })" @click="emit('move', family, -1)" />
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-chevron-right" :disabled="index === families.length - 1" :aria-label="t('workflows.lowerFont', { name: family })" @click="emit('move', family, 1)" />
          <UButton color="neutral" variant="ghost" size="xs" icon="i-tabler-x" :aria-label="t('workflows.removeSelectedFont', { name: family })" @click="emit('remove', family)" />
        </div>
      </div>
    </div>
    <p v-else class="type-metadata m-0 leading-4 text-[var(--text-muted)]">
      {{ t('workflows.noSelectedFonts') }}
    </p>
  </section>
</template>
