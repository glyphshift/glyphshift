<script setup lang="ts">
import DismissibleAlert from './DismissibleAlert.vue'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { canonicalLanguageCode, translationLanguageLabel } from '../settingsCatalogs'
import { normalizeFontLanguage } from '../fontFallbacks'
defineOptions({ inheritAttrs: false })
defineProps<{ allowAuto?: boolean }>()
const value = defineModel<string>({ required: true })
const { t, locale } = useI18n()
const app = useAppSettings()
const error = ref(false)
const open = ref(false)
const search = ref('')
const items = computed(() => [...new Set([
  ...app.settings.value.translationLanguages.map(code => code === normalizeFontLanguage(value.value) ? value.value : canonicalLanguageCode(code)),
  ...(value.value ? [value.value] : []),
])].map(code => ({ label: translationLanguageLabel(code, locale.value), value: code })))
function create(code: string) {
  const normalized = normalizeFontLanguage(code)
  error.value = !normalized
  if (normalized) {
    value.value = code.trim()
    open.value = false
    search.value = ''
  }
}
</script>
<template>
  <USelectMenu v-model:open="open" v-model:search-term="search" v-bind="$attrs" :model-value="value || undefined" :items="allowAuto ? [{ label: t('settingsManager.autoLanguage'), value: 'auto' }, ...items.filter(item => item.value !== 'auto')] : items" value-key="value" create-item :placeholder="t('fontFallbacks.languagePlaceholder')" class="w-full" @update:model-value="value = $event ?? ''; error = false" @create="create">
    <template #create-item-label="{ item }">{{ t('fontFallbacks.useCustom', { code: item }) }}</template>
  </USelectMenu>
  <DismissibleAlert v-if="error" role="alert" color="error" variant="soft" :description="t('fontFallbacks.invalidLanguage')" :dismiss-key="modelValue" class="mt-1" />
</template>
