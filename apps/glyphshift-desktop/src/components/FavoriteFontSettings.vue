<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { useWorkspace } from '../useWorkspace'
import { recommendedFavoriteFonts } from '../settingsCatalogs'
const { t } = useI18n()
const app = useAppSettings()
const workspace = useWorkspace()
const selected = ref<string>()
const error = ref('')
const available = computed(() => workspace.model.value.fontFamilies.filter(font => !app.settings.value.favoriteFonts.includes(font)))
const recommended = computed(() => recommendedFavoriteFonts(workspace.model.value.fontFamilies, app.settings.value.favoriteFonts))
async function save(fonts: string[]) {
  error.value = ''
  try { await app.setFavoriteFonts(fonts); selected.value = undefined } catch { error.value = t('settingsManager.saveFailed') }
}
async function refresh() {
  error.value = ''
  if (!await workspace.refreshFontFamilies()) error.value = t('settingsManager.refreshFailed')
}
</script>
<template>
  <ManagementFormSection :title="t('settingsManager.favoriteTitle')" :description="t('settingsManager.favoriteHint')" data-testid="settings-favorite-fonts">
    <template #actions>
      <div class="flex flex-wrap items-center justify-end gap-2">
      <UButton :label="t('settingsManager.addRecommendedFonts')" icon="i-tabler-star" color="neutral" variant="outline" size="sm" :disabled="app.settingsBusy.value || recommended.length === app.settings.value.favoriteFonts.length" @click="save(recommended)" />
      <UButton :label="t('workflows.refreshFonts')" icon="i-tabler-refresh" color="neutral" variant="outline" size="sm" :loading="workspace.fontRefreshing.value" @click="refresh" />
      </div>
    </template>
    <div class="flex flex-wrap gap-2 py-4">
      <div v-for="font in app.settings.value.favoriteFonts" :key="font" class="flex max-w-full items-center gap-1 rounded border border-[var(--border)] pl-2">
        <UIcon name="i-tabler-star" class="size-4 shrink-0 text-[var(--accent-strong)]" /><span class="type-label truncate" :title="font">{{ font }}</span>
        <UButton :aria-label="t('settingsManager.removeFavorite', { font })" icon="i-tabler-x" color="neutral" variant="ghost" :disabled="app.settingsBusy.value" @click="save(app.settings.value.favoriteFonts.filter(value => value !== font))" />
      </div>
      <p v-if="!app.settings.value.favoriteFonts.length" class="type-metadata m-0 text-[var(--text-muted)]">{{ t('settingsManager.noFavorites') }}</p>
    </div>
    <div class="flex items-center gap-3 border-t border-[var(--border)] py-4">
      <USelectMenu v-model="selected" :items="available" virtualize :aria-label="t('settingsManager.searchFont')" :placeholder="t('settingsManager.searchFont')" :disabled="app.settingsBusy.value" class="min-w-0 flex-1" />
      <UButton :label="t('settingsManager.addFavorite')" icon="i-tabler-star" :disabled="!selected || app.settingsBusy.value || app.settings.value.favoriteFonts.length >= 64" @click="selected && save([...app.settings.value.favoriteFonts, selected])" />
    </div>
    <p v-if="error" role="alert" class="type-metadata mb-3 text-error">{{ error }}</p>
  </ManagementFormSection>
</template>
