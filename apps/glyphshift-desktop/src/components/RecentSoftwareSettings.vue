<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useWorkspace } from '../useWorkspace'
import { useAppSettings } from '../appSettings'
import { useRecentSoftware } from '../useRecentSoftware'
const { t } = useI18n()
const workspace = useWorkspace()
const app = useAppSettings()
const recent = useRecentSoftware(computed(() => workspace.model.value.software))
const query = ref('')
const error = ref('')
const filtered = computed(() => recent.records.value.filter(item => `${item.name} ${item.executablePath ?? ''}`.toLowerCase().includes(query.value.trim().toLowerCase())))
async function change(action: () => Promise<void>) {
  error.value = ''
  try { await action() } catch { error.value = t('settingsManager.saveFailed') }
}
</script>
<template>
  <ManagementFormSection :title="t('settingsManager.recentTitle')" :description="t('settingsManager.recentHint')" data-testid="settings-recent-software">
    <template #actions><UButton :label="t('settingsManager.clearRecent')" icon="i-tabler-trash" color="neutral" variant="outline" size="sm" :disabled="app.settingsBusy.value || !recent.ids.value.length" @click="change(recent.clear)" /></template>
    <UInput v-model="query" icon="i-tabler-search" :placeholder="t('settingsManager.searchSoftware')" :aria-label="t('settingsManager.searchSoftware')" class="my-4 w-full" />
    <ul class="m-0 list-none p-0">
      <li v-for="item in filtered" :key="item.id" class="flex items-center gap-3 border-t border-[var(--border)] py-3">
        <UIcon name="i-tabler-app-window" class="size-5 shrink-0 text-[var(--text-muted)]" />
        <div class="min-w-0 flex-1"><p class="type-label m-0 font-semibold">{{ item.name }}</p><p class="type-metadata m-0 truncate text-[var(--text-muted)]" :title="item.executablePath ?? undefined">{{ item.executablePath }}</p></div>
        <UButton :aria-label="t('settingsManager.removeRecent', { name: item.name })" icon="i-tabler-x" color="neutral" variant="ghost" :disabled="app.settingsBusy.value" @click="change(() => recent.remove(item.id))" />
      </li>
    </ul>
    <p v-if="!filtered.length" class="type-metadata my-5 text-[var(--text-muted)]">{{ t(query ? 'settingsManager.noMatch' : 'settingsManager.noRecent') }}</p>
    <p v-if="error" role="alert" class="type-metadata my-3 text-error">{{ error }}</p>
  </ManagementFormSection>
</template>
