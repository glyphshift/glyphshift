<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useToast } from '@nuxt/ui/composables'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppSettings } from '../appSettings'

defineProps<{
  current: 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'capture' | 'help' | 'settings'
}>()
const emit = defineEmits<{
  navigate: [view: 'workflows' | 'software' | 'dictionaries' | 'capture' | 'help' | 'settings']
  translate: []
  close: []
}>()

const { t } = useI18n()
const toast = useToast()
const appSettings = useAppSettings()
const nav = computed(() => [
  { id: 'workflows' as const, label: t('titleBar.workflows'), icon: 'i-tabler-git-branch' },
  { id: 'software' as const, label: t('titleBar.software'), icon: 'i-tabler-library' },
  { id: 'dictionaries' as const, label: t('titleBar.dictionaries'), icon: 'i-tabler-book-2' },
  { id: 'capture' as const, label: t('titleBar.capture'), icon: 'i-tabler-radar' },
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

async function native(action: 'minimize' | 'maximize') {
  try {
    const window = getCurrentWindow()
    if (action === 'minimize') await window.minimize()
    else await window.toggleMaximize()
  }
  catch {
    // Browser previews do not expose native window controls.
  }
}
</script>

<template>
  <header class="flex h-12 shrink-0 select-none items-stretch border-b border-[var(--border)] bg-[var(--titlebar)] text-[var(--text-secondary)]" data-tauri-drag-region>
    <div class="flex items-center gap-2 border-r border-[var(--border)] px-3" data-tauri-drag-region>
      <span class="grid h-6 w-6 place-items-center rounded-[5px] bg-[var(--accent)] text-[10px] font-bold text-[var(--accent-foreground)]">G</span>
      <strong class="text-[13px] font-semibold tracking-[-0.015em] text-[var(--text)]">Glyphshift</strong>
      <span class="text-[9px] text-[var(--text-muted)]">v0.2</span>
    </div>
    <nav class="flex items-stretch" :aria-label="t('titleBar.navigation')">
      <UButton
        v-for="item in nav"
        :key="item.id"
        color="neutral"
        variant="ghost"
        size="sm"
        :icon="item.icon"
        :label="item.label"
        :class="[
          'relative h-full min-w-[92px] rounded-none px-3 text-[11px]',
          current === item.id || (item.id === 'dictionaries' && current === 'dictionary-editor')
            ? 'font-semibold text-[var(--text)] after:absolute after:inset-x-2 after:bottom-0 after:h-0.5 after:bg-[var(--accent)]'
            : 'text-[var(--text-secondary)]',
        ]"
        :aria-current="current === item.id || (item.id === 'dictionaries' && current === 'dictionary-editor') ? 'page' : undefined"
        @click="emit('navigate', item.id)"
      />
    </nav>
    <div class="ml-auto flex items-stretch" data-tauri-drag-region>
      <UButton
        color="primary"
        variant="ghost"
        size="sm"
        icon="i-tabler-language"
        :label="t('titleBar.translate')"
        class="h-full rounded-none border-l border-[var(--border)] px-3 text-[11px] font-semibold"
        :aria-label="t('titleBar.translate')"
        @click="emit('translate')"
      />
      <UButton
        color="neutral"
        variant="ghost"
        :icon="appSettings.effectiveTheme.value === 'dark' ? 'i-tabler-sun' : 'i-tabler-moon'"
        class="h-full w-10 rounded-none"
        :aria-label="themeToggleLabel"
        :title="themeToggleLabel"
        @click="toggleTheme"
      />
      <UButton color="neutral" variant="ghost" icon="i-tabler-help-circle" class="h-full w-10 rounded-none" :class="current === 'help' ? 'bg-[var(--surface-hover)] text-[var(--text)]' : ''" :aria-label="t('titleBar.help')" :aria-current="current === 'help' ? 'page' : undefined" @click="emit('navigate', 'help')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-settings" class="h-full w-10 rounded-none" :class="current === 'settings' ? 'bg-[var(--surface-hover)] text-[var(--text)]' : ''" :aria-label="t('titleBar.settings')" :aria-current="current === 'settings' ? 'page' : undefined" @click="emit('navigate', 'settings')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-minus" class="h-full w-10 rounded-none" :aria-label="t('titleBar.minimize')" @click="native('minimize')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-square" class="h-full w-10 rounded-none" :aria-label="t('titleBar.maximize')" @click="native('maximize')" />
      <UButton color="neutral" variant="ghost" icon="i-tabler-x" class="h-full w-10 rounded-none hover:bg-[var(--danger)] hover:text-white" :aria-label="t('titleBar.close')" @click="emit('close')" />
    </div>
  </header>
</template>
