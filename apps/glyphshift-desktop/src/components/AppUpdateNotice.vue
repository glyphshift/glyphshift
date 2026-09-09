<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'
import { useAppUpdate } from '../useAppUpdate'
const { t } = useI18n()
const appSettings = useAppSettings()
const update = useAppUpdate()
onMounted(() => { if ('__TAURI_INTERNALS__' in window) void update.check(false) })
watch(() => appSettings.settings.value.checkUpdatesOnStartup, enabled => {
  if (!enabled) update.cancelAutomatic()
  else if ('__TAURI_INTERNALS__' in window) void update.check(false)
})
</script>

<template>
  <div v-if="update.visible.value" role="status" class="flex shrink-0 items-center gap-3 border-b border-[var(--border)] bg-[var(--surface)] px-4 py-2">
    <UIcon name="i-tabler-download" class="size-4 shrink-0 text-[var(--accent-strong)]" />
    <span class="type-label">{{ t('updates.available', { version: update.release.value?.version }) }}</span>
    <UPopover v-if="update.release.value?.releaseNotes">
      <UButton variant="link" size="xs" :label="t('updates.notes')" />
      <template #content><p class="m-0 max-h-64 max-w-96 overflow-auto whitespace-pre-wrap break-words p-4 text-sm">{{ update.release.value?.releaseNotes }}</p></template>
    </UPopover>
    <span v-if="update.openFailed.value" class="type-caption text-[var(--text-secondary)]">{{ t('updates.openFailed') }}</span>
    <UButton class="ml-auto shrink-0" size="xs" :label="t('updates.download')" :loading="update.opening.value" @click="update.download" />
    <UButton icon="i-tabler-x" color="neutral" variant="ghost" size="xs" :aria-label="t('updates.dismiss')" :title="t('updates.dismiss')" @click="update.dismiss" />
  </div>
</template>
