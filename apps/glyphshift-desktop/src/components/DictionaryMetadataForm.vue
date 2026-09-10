<script setup lang="ts">
import LanguageSelect from './LanguageSelect.vue'
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { DictionaryMetadata } from '../model'

const metadata = defineModel<DictionaryMetadata>({ required: true })
const { t } = useI18n()
const moreOpen = ref(false)
</script>

<template>
  <div class="space-y-3">
    <UFormField :label="t('dictionaryEditor.name')" required>
      <UInput v-model="metadata.name" :maxlength="128" class="w-full" />
    </UFormField>
    <UFormField :label="t('dictionaryEditor.description')">
      <UTextarea v-model="metadata.description" :maxlength="512" :rows="2" autoresize class="w-full" />
    </UFormField>
    <UFormField :label="t('dictionaryEditor.tags')" :hint="t('dictionaryEditor.tagsHint')">
      <UInputTags
        v-model="metadata.tags"
        :add-on-blur="true"
        :add-on-paste="true"
        :duplicate="false"
        :placeholder="t('dictionaryEditor.tagsPlaceholder')"
        class="w-full"
      />
    </UFormField>
    <div class="grid grid-cols-2 gap-3">
      <UFormField :label="t('dictionaryEditor.sourceLocale')" required>
        <LanguageSelect v-model="metadata.sourceLocale" allow-auto :aria-label="t('dictionaryEditor.sourceLocale')" />
      </UFormField>
      <UFormField :label="t('dictionaryEditor.targetLocale')" required>
        <LanguageSelect v-model="metadata.targetLocale" :aria-label="t('dictionaryEditor.targetLocale')" />
      </UFormField>
    </div>

    <UCollapsible v-model:open="moreOpen" class="rounded-[var(--radius-control)] border border-[var(--border)] bg-[var(--surface-subtle)]">
      <UButton
        color="neutral"
        variant="ghost"
        size="sm"
        class="w-full justify-between rounded-[var(--radius-control)] px-3"
        :label="t('dictionaryEditor.moreMetadata')"
        :trailing-icon="moreOpen ? 'i-tabler-chevron-up' : 'i-tabler-chevron-down'"
      />
      <template #content>
        <div class="space-y-3 border-t border-[var(--border)] p-3">
          <UFormField :label="t('dictionaryEditor.releaseVersion')" required>
            <SemanticVersionInput v-model="metadata.releaseVersion" />
          </UFormField>
          <UFormField :label="t('dictionaryEditor.authors')" :hint="t('dictionaryEditor.authorsHint')">
            <UInputTags
              v-model="metadata.authors"
              :add-on-blur="true"
              :add-on-paste="true"
              :duplicate="false"
              :placeholder="t('dictionaryEditor.authorsPlaceholder')"
              class="w-full"
            />
          </UFormField>
          <UFormField :label="t('dictionaryEditor.license')">
            <UInput
              :model-value="metadata.license ?? ''"
              class="w-full"
              @update:model-value="metadata.license = String($event)"
            />
          </UFormField>
          <UFormField :label="t('dictionaryEditor.homepage')">
            <UInput
              :model-value="metadata.homepage ?? ''"
              type="url"
              class="w-full"
              @update:model-value="metadata.homepage = String($event)"
            />
          </UFormField>
        </div>
      </template>
    </UCollapsible>
  </div>
</template>
