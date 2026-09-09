<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import type { AiFilterPolicy } from '../textFilters'
const { t } = useI18n()
const app = useAppSettings()
const draft = ref<AiFilterPolicy>(JSON.parse(JSON.stringify(app.settings.value.textFilterPolicy)))
const patterns = ref(draft.value.excludedPatterns.join('\n'))
const error = ref('')
watch(() => app.settings.value.textFilterPolicy, policy => {
  draft.value = JSON.parse(JSON.stringify(policy)); patterns.value = policy.excludedPatterns.join('\n')
})
const options = computed(() => [
  ['skipPureNumbersOrSymbols', 'ai.filterPureNumbers'], ['skipNumericMeasurements', 'ai.filterMeasurements'],
  ['skipSingleCharacter', 'ai.filterSingleCharacter'], ['skipTextContainingDigits', 'ai.filterContainingDigits'],
  ['skipUrls', 'textFilters.urls'], ['skipFilePaths', 'textFilters.paths'],
  ['skipEmails', 'textFilters.emails'], ['skipShortcuts', 'textFilters.shortcuts'],
] as const)
async function save() {
  error.value = ''
  const policy = { ...draft.value, maxSourceChars: Number(draft.value.maxSourceChars) > 0 ? Number(draft.value.maxSourceChars) : null,
    excludedPatterns: patterns.value.split('\n').map(line => line.trim()).filter(Boolean) }
  try {
    if (!('__TAURI_INTERNALS__' in window)) for (const pattern of policy.excludedPatterns) new RegExp(pattern)
    await app.setTextFilterPolicy(policy)
  } catch { error.value = t('textFilters.invalid') }
}
</script>
<template>
  <ManagementFormSection :title="t('textFilters.title')" :description="t('textFilters.description')" data-testid="settings-text-filters">
    <template #actions><UButton :label="t('textFilters.save')" size="sm" :loading="app.settingsBusy.value" @click="save" /></template>
    <ManagementFormRow v-for="[key, label] in options" :key="key" :label="t(label)" control-width="compact">
      <div class="flex justify-end"><USwitch v-model="draft[key]" :aria-label="t(label)" :disabled="app.settingsBusy.value" /></div>
    </ManagementFormRow>
    <ManagementFormRow :label="t('ai.maxSourceChars')" :description="t('ai.unlimited')" control-width="compact">
      <UInput v-model.number="draft.maxSourceChars" type="number" min="1" :aria-label="t('ai.maxSourceChars')" class="w-full" />
    </ManagementFormRow>
    <ManagementFormRow :label="t('ai.excludedPatterns')" :help="t('ai.excludedPatternsHint')" multiline>
      <UTextarea v-model="patterns" :aria-label="t('ai.excludedPatterns')" :rows="3" class="w-full" />
    </ManagementFormRow>
    <p v-if="error" role="alert" class="text-error px-4">{{ error }}</p>
  </ManagementFormSection>
</template>
