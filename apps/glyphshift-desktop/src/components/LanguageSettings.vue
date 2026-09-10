<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { normalizeFontLanguage } from '../fontFallbacks'
import { defaultTranslationLanguages, suggestedLanguageCodes, translationLanguageLabel } from '../settingsCatalogs'
const { t, locale } = useI18n()
const app = useAppSettings()
const query = ref('')
const code = ref('')
const error = ref('')
const saved = ref(false)
const suggestions = computed(() => {
  const needle = code.value.trim().toLowerCase()
  if (!needle) return []
  return suggestedLanguageCodes.map(value => ({ value, label: translationLanguageLabel(value, locale.value) }))
    .filter(item => item.label.toLowerCase().includes(needle))
    .slice(0, 8)
})
const rows = computed(() => app.settings.value.translationLanguages.filter(value => translationLanguageLabel(value, locale.value).toLowerCase().includes(query.value.trim().toLowerCase())))
async function save(value: string[]) {
  error.value = ''; saved.value = false
  try { await app.setTranslationLanguages(value); saved.value = true; return true }
  catch { error.value = t('settingsManager.saveFailed'); return false }
}
async function add() {
  const value = normalizeFontLanguage(code.value)
  if (!value) { error.value = t('fontFallbacks.invalidLanguage'); return }
  if (app.settings.value.translationLanguages.includes(value)) { error.value = t('settingsManager.duplicateLanguage'); return }
  if (await save([...app.settings.value.translationLanguages, value])) code.value = ''
}
</script>
<template>
  <ManagementFormSection :title="t('settingsManager.languageTitle')" :description="t('settingsManager.languageHint')" data-testid="settings-languages">
    <template #actions><UButton :label="t('settingsManager.resetLanguages')" :title="t('settingsManager.resetHint')" icon="i-tabler-restore" color="neutral" variant="outline" size="sm" :disabled="app.settingsBusy.value" @click="save(defaultTranslationLanguages())" /></template>
    <UInput v-model="query" icon="i-tabler-search" :placeholder="t('settingsManager.searchLanguage')" :aria-label="t('settingsManager.searchLanguage')" class="my-4 w-full" />
    <ul class="m-0 grid max-h-80 list-none grid-cols-2 gap-x-6 overflow-y-auto p-0 @max-[620px]:grid-cols-1">
      <li v-for="item in rows" :key="item" class="flex items-center justify-between gap-3 border-t border-[var(--border)] py-2">
        <span class="type-label">{{ translationLanguageLabel(item, locale) }}</span>
        <UButton :aria-label="t('settingsManager.removeLanguage', { code: item })" icon="i-tabler-x" color="neutral" variant="ghost" :disabled="app.settingsBusy.value" @click="save(app.settings.value.translationLanguages.filter(value => value !== item))" />
      </li>
    </ul>
    <p v-if="!rows.length" class="type-metadata my-4 text-[var(--text-muted)]">{{ t(query ? 'settingsManager.noMatch' : 'settingsManager.noLanguages') }}</p>
    <form class="flex items-end gap-3 border-t border-[var(--border)] py-4" @submit.prevent="add">
      <UFormField :label="t('settingsManager.languageCode')" class="min-w-0 flex-1"><UInput v-model="code" :aria-label="t('settingsManager.languageCode')" :placeholder="t('settingsManager.languageCodeHint')" :maxlength="63" :disabled="app.settingsBusy.value" class="w-full" /></UFormField>
      <UButton type="submit" :label="t('settingsManager.addLanguage')" icon="i-tabler-plus" :disabled="!code.trim() || app.settingsBusy.value || app.settings.value.translationLanguages.length >= 128" />
    </form>
    <div v-if="suggestions.length" class="mb-3 flex flex-wrap gap-2" :aria-label="t('settingsManager.languageSuggestions')">
      <UButton v-for="item in suggestions" :key="item.value" color="neutral" variant="soft" size="sm" :label="item.label" :disabled="app.settingsBusy.value || app.settings.value.translationLanguages.includes(item.value)" :title="app.settings.value.translationLanguages.includes(item.value) ? t('settingsManager.duplicateLanguage') : item.label" @click="code = item.value; error = ''" />
    </div>
    <p class="type-metadata mb-3 text-[var(--text-muted)]">{{ t('settingsManager.languageSuggestionHint') }}</p>
    <p v-if="error" role="alert" class="type-metadata mb-3 text-error">{{ error }}</p>
    <p v-else-if="saved" role="status" class="type-metadata mb-3 text-[var(--text-muted)]">{{ t('fontFallbacks.saved') }}</p>
  </ManagementFormSection>
</template>
