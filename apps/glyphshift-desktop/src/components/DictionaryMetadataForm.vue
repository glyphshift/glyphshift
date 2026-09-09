<script setup lang="ts">
import FieldHelp from './FieldHelp.vue'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useWorkspace } from '../useWorkspace'
import type { DictionaryMetadata } from '../model'

const metadata = defineModel<DictionaryMetadata>({ required: true })
const { t } = useI18n()
const moreOpen = ref(false)
const workspace = useWorkspace()
const fontOptions = computed(() => [...new Set([...(metadata.value.fontFamilies ?? []), ...workspace.model.value.fontFamilies])])
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
        <UInput v-model="metadata.sourceLocale" class="w-full" />
      </UFormField>
      <UFormField :label="t('dictionaryEditor.targetLocale')" required>
        <UInput v-model="metadata.targetLocale" class="w-full" />
      </UFormField>
    </div>

    <UFormField :label="t('dictionaryEditor.fontFamilies')">
      <template #label><span class="inline-flex items-center gap-1">{{ t('dictionaryEditor.fontFamilies') }}<FieldHelp :label="t('dictionaryEditor.fontFamilies')" :text="t('dictionaryEditor.fontFamiliesHint')" /></span></template>
      <USelectMenu :model-value="metadata.fontFamilies ?? []" :items="fontOptions" multiple virtualize class="w-full" :aria-label="t('dictionaryEditor.fontFamilies')" :placeholder="t('dictionaryEditor.fontFamiliesPlaceholder')" :search-input="{ placeholder: t('workflows.searchFonts') }" @update:model-value="metadata.fontFamilies = $event.slice(0, 16)" />
    </UFormField>

    <UFormField :label="t('dictionaryEditor.fontScale')">
      <template #label><span class="inline-flex items-center gap-1">{{ t('dictionaryEditor.fontScale') }}<FieldHelp :label="t('dictionaryEditor.fontScale')" :text="t('dictionaryEditor.fontScaleHint')" /></span></template>
      <div class="flex items-center gap-3">
        <USwitch :model-value="metadata.fontScalePercent != null" :aria-label="t('dictionaryEditor.fontScaleOverride')" @update:model-value="metadata.fontScalePercent = $event ? 100 : null" />
        <UInput v-if="metadata.fontScalePercent != null" v-model.number="metadata.fontScalePercent" type="number" :min="50" :max="200" :step="5" :aria-label="t('dictionaryEditor.fontScale')" class="w-28" />
        <span v-if="metadata.fontScalePercent != null">%</span>
        <span v-else class="type-metadata text-[var(--text-muted)]">{{ t('dictionaryEditor.fontScaleInherit') }}</span>
      </div>
    </UFormField>

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
