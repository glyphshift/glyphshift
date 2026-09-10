<script setup lang="ts">
import LanguageSelect from './LanguageSelect.vue'
import { useI18n } from 'vue-i18n'
import type { DictionaryMetadata } from '../model'
const metadata = defineModel<DictionaryMetadata>({ required: true })
const { t } = useI18n()
</script>

<template>
  <div class="space-y-3">
    <UFormField :label="t('dictionaryEditor.name')" required>
      <UInput v-model="metadata.name" :maxlength="128" class="w-full" />
    </UFormField>
    <div class="grid grid-cols-2 gap-3">
      <UFormField :label="t('dictionaryEditor.sourceLocale')" required><LanguageSelect v-model="metadata.sourceLocale" allow-auto :aria-label="t('dictionaryEditor.sourceLocale')" /></UFormField>
      <UFormField :label="t('dictionaryEditor.targetLocale')" required><LanguageSelect v-model="metadata.targetLocale" :aria-label="t('dictionaryEditor.targetLocale')" /></UFormField>
    </div>
  </div>
</template>
