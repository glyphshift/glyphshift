<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { useWorkspace } from '../useWorkspace'
import { preferredFonts } from '../settingsCatalogs'
const value = defineModel<string[]>({ required: true })
const { t } = useI18n()
const app = useAppSettings()
const workspace = useWorkspace()
const favoritesOnly = ref(false)
const open = ref(false)
const search = ref('')
function select(fonts: string[]) { value.value = fonts.slice(0, 16); open.value = false; search.value = '' }
const options = computed(() => {
  const fonts = preferredFonts(workspace.model.value.fontFamilies, app.settings.value.favoriteFonts, value.value)
  return favoritesOnly.value ? fonts.filter(font => app.settings.value.favoriteFonts.includes(font)) : fonts
})
</script>
<template>
  <USelectMenu v-model:open="open" v-model:search-term="search" :model-value="value" :items="options" multiple virtualize :placeholder="t('workflowTypography.inheritFont')" :search-input="{ placeholder: t('workflows.searchFonts') }" @update:model-value="select">
    <template #content-top>
      <div class="flex gap-1 border-b border-[var(--border)] p-2">
        <UButton size="xs" color="neutral" :variant="favoritesOnly ? 'ghost' : 'soft'" :aria-pressed="!favoritesOnly" :label="t('workflowTypography.allFonts')" @click.stop="favoritesOnly = false" />
        <UButton size="xs" color="neutral" :variant="favoritesOnly ? 'soft' : 'ghost'" icon="i-tabler-star" :aria-pressed="favoritesOnly" :label="t('workflowTypography.favorites')" @click.stop="favoritesOnly = true" />
      </div>
    </template>
    <template #empty>{{ favoritesOnly ? t('workflowTypography.noFavorites') : t('workflows.noFontMatch') }}</template>
  </USelectMenu>
</template>
