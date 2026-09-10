<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { preferredFonts } from '../settingsCatalogs'
import { useWorkspace } from '../useWorkspace'
import { normalizeFontLanguage, type LanguageFallbackFont } from '../fontFallbacks'

const { t, locale } = useI18n()
const app = useAppSettings()
const workspace = useWorkspace()
const language = ref<string>()
const fontFamily = ref<string>()
const error = ref('')
const saved = ref(false)
const rows = computed(() => app.settings.value.languageFallbackFonts)
const fonts = computed(() => preferredFonts(workspace.model.value.fontFamilies, app.settings.value.favoriteFonts, rows.value.map(row => row.fontFamily)))
function languageLabel(code: string) {
  try { return `${new Intl.DisplayNames([locale.value], { type: 'language' }).of(code)} · ${code}` }
  catch { return code }
}
const languages = computed(() => app.settings.value.translationLanguages.map(code => ({ label: languageLabel(code), value: code.toLowerCase() })))
async function persist(value: LanguageFallbackFont[]) {
  error.value = ''; saved.value = false
  try { await app.setLanguageFallbackFonts(value); saved.value = true; return true }
  catch { error.value = t('fontFallbacks.saveFailed'); return false }
}
async function add() {
  const code = normalizeFontLanguage(language.value ?? '')
  if (!code) { error.value = t('fontFallbacks.invalidLanguage'); return }
  if (rows.value.some(row => row.language === code)) { error.value = t('fontFallbacks.duplicate'); return }
  if (!fontFamily.value || rows.value.length >= 64) return
  if (await persist([...rows.value, { language: code, fontFamily: fontFamily.value }])) {
    language.value = undefined; fontFamily.value = undefined
  }
}
function updateFont(code: string, family: string) {
  void persist(rows.value.map(row => row.language === code ? { ...row, fontFamily: family } : row))
}
</script>

<template>
  <ManagementFormSection :title="t('fontFallbacks.title')" :description="t('fontFallbacks.description')" data-testid="settings-font-fallbacks">
    <p class="type-metadata my-3 text-[var(--text-muted)]">{{ t('fontFallbacks.availability') }}</p>
    <ManagementFormRow v-for="row in rows" :key="row.language" :label="languageLabel(row.language)" control-width="compact">
      <div class="flex items-center gap-2">
        <USelectMenu :model-value="row.fontFamily" :items="fonts" virtualize :aria-label="t('fontFallbacks.fontFor', { language: row.language })" :disabled="app.settingsBusy.value" class="min-w-0 flex-1" @update:model-value="updateFont(row.language, $event)" />
        <UButton icon="i-tabler-x" color="neutral" variant="ghost" :aria-label="t('fontFallbacks.remove', { language: row.language })" :disabled="app.settingsBusy.value" @click="persist(rows.filter(item => item.language !== row.language))" />
      </div>
    </ManagementFormRow>
    <p v-if="!rows.length" class="type-body my-4 text-[var(--text-secondary)]">{{ t('fontFallbacks.empty') }}</p>
    <div class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-end gap-3 py-4 @max-[620px]:grid-cols-1">
      <UFormField :label="t('fontFallbacks.language')">
        <USelectMenu v-model="language" :items="languages" value-key="value" create-item :aria-label="t('fontFallbacks.language')" :placeholder="t('fontFallbacks.languagePlaceholder')" :disabled="app.settingsBusy.value" class="w-full" @create="language = $event">
          <template #create-item-label="{ item }">{{ t('fontFallbacks.useCustom', { code: item }) }}</template>
        </USelectMenu>
      </UFormField>
      <UFormField :label="t('fontFallbacks.font')">
        <USelectMenu v-model="fontFamily" :items="fonts" virtualize :aria-label="t('fontFallbacks.font')" :placeholder="t('fontFallbacks.fontPlaceholder')" :disabled="app.settingsBusy.value" class="w-full" />
      </UFormField>
      <UButton :label="t('fontFallbacks.add')" icon="i-tabler-plus" :disabled="!language || !fontFamily || rows.length >= 64 || app.settingsBusy.value" @click="add" />
    </div>
    <p v-if="error" role="alert" class="type-metadata mb-4 text-error">{{ error }}</p>
    <p v-else-if="saved" role="status" class="type-metadata mb-4 text-[var(--text-muted)]">{{ t('fontFallbacks.saved') }}</p>
  </ManagementFormSection>
</template>
