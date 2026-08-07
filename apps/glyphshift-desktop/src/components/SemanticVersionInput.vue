<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const model = defineModel<string>({ required: true })
const { t } = useI18n()

function parts() {
  const values = model.value.split('.').slice(0, 3).map(value => Number.parseInt(value, 10))
  return [0, 1, 2].map(index => Number.isFinite(values[index]) ? Math.max(0, values[index] ?? 0) : 0)
}

function updatePart(index: number, value: number | null | undefined) {
  const next = parts()
  next[index] = Math.max(0, Math.trunc(value ?? 0))
  model.value = next.join('.')
}
</script>

<template>
  <div class="grid grid-cols-[1fr_auto_1fr_auto_1fr] items-center gap-2">
    <UInputNumber
      :model-value="parts()[0]"
      :min="0"
      :step="1"
      :aria-label="t('dictionaryEditor.versionMajor')"
      class="w-full"
      @update:model-value="updatePart(0, $event)"
    />
    <span class="text-[var(--text-muted)]">.</span>
    <UInputNumber
      :model-value="parts()[1]"
      :min="0"
      :step="1"
      :aria-label="t('dictionaryEditor.versionMinor')"
      class="w-full"
      @update:model-value="updatePart(1, $event)"
    />
    <span class="text-[var(--text-muted)]">.</span>
    <UInputNumber
      :model-value="parts()[2]"
      :min="0"
      :step="1"
      :aria-label="t('dictionaryEditor.versionPatch')"
      class="w-full"
      @update:model-value="updatePart(2, $event)"
    />
  </div>
</template>
