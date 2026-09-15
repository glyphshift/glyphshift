<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useToast } from '@nuxt/ui/composables'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { version as appVersion } from '../../package.json'
import glyphshiftIconUrl from '../../src-tauri/icons/icon.svg?url'
import { useAppSettings } from '../appSettings'
import { workflowActivity } from '../workflowLifecycle'

const props = defineProps<{
  current: 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'capture' | 'translation-tasks' | 'help' | 'settings'
}>()
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'software' | 'dictionaries' | 'capture' | 'translation-tasks' | 'help' | 'settings']
  close: []
}>()

const { t } = useI18n()
const toast = useToast()
const appSettings = useAppSettings()
const activityLabel = computed(() => workflowActivity.value.map(([phase, count]) => `${t(`workflowLifecycle.${phase}`)} ${count}`).join(' · '))
const nav = computed(() => [
  { id: 'workflows' as const, label: t('titleBar.workflows'), icon: 'i-tabler-git-branch' },
  { id: 'dictionaries' as const, label: t('titleBar.dictionaries'), icon: 'i-tabler-book-2' },
])
const themeToggleLabel = computed(() => appSettings.effectiveTheme.value === 'dark'
  ? t('titleBar.switchToLight')
  : t('titleBar.switchToDark'))

function toggleTheme() {
  const nextTheme = appSettings.effectiveTheme.value === 'dark' ? 'light' : 'dark'
  void appSettings.setThemePreference(nextTheme).catch(() => {
    toast.add({
      title: t('settings.saveFailed'),
      description: appSettings.settingsError.value || t('errors.unknown'),
      color: 'error',
      icon: 'i-tabler-alert-circle',
    })
  })
}

async function toggleTopmost() {
  try { await appSettings.setAlwaysOnTop(!appSettings.alwaysOnTop.value) }
  catch { toast.add({ title: t('settings.saveFailed'), description: appSettings.settingsError.value, color: 'error' }) }
}

async function native(action: 'minimize' | 'maximize') {
  try {
    const window = getCurrentWindow()
    if (action === 'minimize') await invoke('desktop_minimize_window')
    else await window.toggleMaximize()
  }
  catch {
    if ('__TAURI_INTERNALS__' in window) toast.add({ title: t('errors.settings.windowFailed'), color: 'error' })
  }
}
</script>

<template>
  <header class="flex h-12 shrink-0 select-none items-stretch border-b border-[var(--border)] bg-[var(--titlebar)] text-[var(--text-secondary)]" data-tauri-drag-region>
    <div class="flex items-center gap-2 border-r border-[var(--border)] px-3" data-tauri-drag-region>
      <img
        :src="glyphshiftIconUrl"
        alt=""
        aria-hidden="true"
        data-testid="glyphshift-mark"
        draggable="false"
        class="pointer-events-none size-6 shrink-0"
      >
      <strong class="text-[13px] font-semibold tracking-[-0.015em] text-[var(--text)]">Glyphshift</strong>
      <span class="type-caption tabular-nums text-[var(--text-muted)]">v{{ appVersion }}</span>
    </div>
    <nav class="flex items-stretch" :aria-label="t('titleBar.navigation')">
      <UButton :title="item.label"
        v-for="item in nav"
        :key="item.id"
        color="neutral"
        variant="ghost"
        size="sm"
        :class="[
          'relative h-full min-w-[92px] gap-1.5 rounded-none px-3 text-[11px]',
          current === item.id || (item.id === 'dictionaries' && current === 'dictionary-editor')
            ? 'font-semibold text-[var(--text)] after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-[var(--accent)]'
            : 'text-[var(--text-secondary)]',
        ]"
        :aria-label="item.label"
        :aria-describedby="item.id === 'workflows' && activityLabel ? 'probe-activity-status' : undefined"
        :aria-current="current === item.id || (item.id === 'dictionaries' && current === 'dictionary-editor') ? 'page' : undefined"
        @click="emit('navigate', item.id)"
      >
        <UIcon :name="item.icon" class="size-4 shrink-0" />
        <span>{{ item.label }}</span>
        <UBadge
          v-if="item.id === 'workflows' && activityLabel"
          id="probe-activity-status"
          :color="workflowActivity.some(([phase]) => ['failed', 'stop_failed'].includes(phase)) ? 'error' : workflowActivity.some(([phase]) => phase !== 'running') ? 'warning' : 'success'"
          variant="soft"
          size="sm"
          :label="activityLabel"
          class="type-caption h-4 shrink-0 px-1.5 font-semibold leading-none"
          aria-live="polite"
        />
      </UButton>
    </nav>
    <div class="ml-auto flex items-stretch" data-tauri-drag-region>
      <UButton
        color="neutral"
        variant="ghost"
        :icon="appSettings.effectiveTheme.value === 'dark' ? 'i-tabler-sun' : 'i-tabler-moon'"
        class="h-full w-10 rounded-none"
        :aria-label="themeToggleLabel"
        :title="themeToggleLabel"
        @click="toggleTheme"
      />
      <UButton :title="t('titleBar.help')" color="neutral" variant="ghost" icon="i-tabler-help-circle" class="h-full w-10 rounded-none" :class="current === 'help' ? 'bg-[var(--surface-hover)] text-[var(--text)]' : ''" :aria-label="t('titleBar.help')" :aria-current="current === 'help' ? 'page' : undefined" @click="emit('navigate', 'help')" />
      <UButton :title="t('titleBar.settings')" color="neutral" variant="ghost" icon="i-tabler-settings" class="h-full w-10 rounded-none" :class="current === 'settings' ? 'bg-[var(--surface-hover)] text-[var(--text)]' : ''" :aria-label="t('titleBar.settings')" :aria-current="current === 'settings' ? 'page' : undefined" @click="emit('navigate', 'settings')" />
      <UButton :title="appSettings.alwaysOnTop.value ? t('titleBar.unpin') : t('titleBar.pin')" :aria-label="appSettings.alwaysOnTop.value ? t('titleBar.unpin') : t('titleBar.pin')" :aria-pressed="appSettings.alwaysOnTop.value" :disabled="appSettings.settingsBusy.value" color="neutral" variant="ghost" :icon="appSettings.alwaysOnTop.value ? 'i-tabler-pinned-filled' : 'i-tabler-pin'" class="h-full w-10 rounded-none" :class="appSettings.alwaysOnTop.value ? 'text-[var(--accent-strong)] bg-[var(--accent-soft)]' : ''" @click="toggleTopmost" />
      <UButton :title="t('titleBar.minimize')" color="neutral" variant="ghost" icon="i-tabler-minus" class="h-full w-10 rounded-none" :aria-label="t('titleBar.minimize')" @click="native('minimize')" />
      <UButton :title="t('titleBar.maximize')" color="neutral" variant="ghost" icon="i-tabler-square" class="h-full w-10 rounded-none" :aria-label="t('titleBar.maximize')" @click="native('maximize')" />
      <UButton :title="t('titleBar.close')" color="neutral" variant="ghost" icon="i-tabler-x" class="h-full w-10 rounded-none hover:bg-[var(--danger)] hover:text-white" :aria-label="t('titleBar.close')" @click="emit('close')" />
    </div>
  </header>
</template>
