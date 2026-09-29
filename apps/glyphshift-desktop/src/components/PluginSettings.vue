<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { translateCommandError, type CommandError } from '../commandError'
import { hasDesktopRuntime } from '../workspace/state'
import ConfirmDialog from './ConfirmDialog.vue'

interface Plugin {
  packageId: string
  version: string
  sha256: string
  architectures: string[]
  adapters: string[]
  selected: boolean
  loaded: boolean
  problem: CommandError | null
}
interface Snapshot { packages: Plugin[]; restartRequired: boolean; runtimeReady: boolean }
const { t } = useI18n()
const desktop = hasDesktopRuntime()
const snapshot = ref<Snapshot | null>(null)
const busy = ref(false)
const error = ref('')
const notice = ref('')
const prepared = ref<Plugin | null>(null)
const selection = ref<Plugin | null>(null)
let disposed = false

function label(plugin: Plugin) {
  const names: Record<string, string> = { qt: 'Qt', gtk3: 'GTK3', unity: 'Unity', raylib: 'Raylib', sidefx: 'SideFX', monogame: 'MonoGame', renpy: "Ren’Py", web: 'Web / Chromium', 'rpgmaker-mv': 'RPGMaker MV', tyranoscript: 'TyranoScript', vgui: 'VGUI', catsystem2: 'CatSystem2', kirikiri: 'KiriKiri' }
  const key = plugin.packageId.replace(/^glyphshift-adapter-/, '')
  return names[key] ?? key
}
async function run(action: () => Promise<void>) {
  if (busy.value || !desktop) return
  busy.value = true
  error.value = ''
  notice.value = ''
  try { await action() } catch (cause) { error.value = translateCommandError(cause) }
  finally { busy.value = false }
}
async function refresh() {
  await run(async () => { snapshot.value = await invoke<Snapshot>('desktop_plugins') })
}
async function choose() {
  await run(async () => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: 'Glyphshift Plugin', extensions: ['gsp'] }] })
    if (typeof path !== 'string' || disposed) return
    const result = await invoke<Plugin>('desktop_prepare_plugin', { path })
    if (!disposed) prepared.value = result
    else await invoke('desktop_cancel_plugin_install')
  })
}
function dismissPreview(value: boolean) {
  if (value || busy.value) return
  prepared.value = null
  void invoke('desktop_cancel_plugin_install').catch(() => {})
}
async function install() {
  const candidate = prepared.value
  if (!candidate) return
  await run(async () => {
    try {
      snapshot.value = await invoke<Snapshot>('desktop_install_plugin', { approvedSha256: candidate.sha256 })
      notice.value = t('plugins.installed')
    } finally { prepared.value = null }
  })
}
async function changeSelection() {
  const candidate = selection.value
  if (!candidate) return
  await run(async () => {
    try {
      snapshot.value = await invoke<Snapshot>('desktop_select_plugin', { sha256: candidate.sha256, enabled: !candidate.selected })
    } finally { selection.value = null }
  })
}
onMounted(refresh)
onBeforeUnmount(() => {
  disposed = true
  if (desktop && prepared.value) void invoke('desktop_cancel_plugin_install').catch(() => {})
})
</script>

<template>
  <ManagementFormSection :title="t('plugins.title')" :description="t('plugins.description')" data-testid="plugin-settings">
    <template #actions>
      <div class="flex flex-wrap gap-2">
        <UButton color="neutral" variant="ghost" icon="i-tabler-refresh" :label="t('plugins.refresh')" :disabled="busy || !desktop" @click="refresh" />
        <UButton icon="i-tabler-package-import" :label="t('plugins.installLocal')" :disabled="busy || !desktop" @click="choose" />
      </div>
    </template>
    <div class="min-w-0 space-y-3 p-4" :aria-busy="busy">
      <p v-if="!desktop" class="type-body text-[var(--text-secondary)]">{{ t('plugins.desktopOnly') }}</p>
      <p v-if="busy" role="status" class="type-body text-[var(--text-secondary)]">{{ t('plugins.working') }}</p>
      <UAlert v-if="error" role="alert" color="error" variant="soft" :title="error" />
      <UAlert v-if="notice" role="status" color="success" variant="soft" :title="notice" />
      <UAlert v-if="snapshot?.restartRequired" data-testid="plugin-restart" role="status" color="warning" variant="soft" :title="t('plugins.restartTitle')" :description="t('plugins.restartHint')" />
      <UAlert v-if="snapshot && !snapshot.runtimeReady" color="warning" variant="soft" :title="t('plugins.runtimeUnavailable')" />
      <p v-if="snapshot && !snapshot.packages.length" class="type-body py-6 text-center text-[var(--text-secondary)]">{{ t('plugins.empty') }}</p>
      <ul v-if="snapshot" class="space-y-3">
        <li v-for="plugin in snapshot.packages" :key="plugin.sha256" class="min-w-0 rounded-lg border border-[var(--border)] p-4" :data-testid="`plugin-${plugin.packageId}-${plugin.version}`">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="type-label break-words">{{ label(plugin) }}</h3>
                <span class="type-caption text-[var(--text-secondary)]">{{ plugin.version }}</span>
                <UBadge v-if="plugin.loaded" color="success" variant="subtle">{{ t('plugins.loaded') }}</UBadge>
                <UBadge :color="plugin.selected ? 'primary' : 'neutral'" variant="subtle">{{ t(plugin.selected ? 'plugins.selected' : 'plugins.unselected') }}</UBadge>
              </div>
              <p class="type-caption mt-1 break-all text-[var(--text-secondary)]">{{ plugin.packageId }} · {{ plugin.architectures.join(' / ') }}</p>
            </div>
            <UButton size="sm" color="neutral" variant="outline" :label="t(plugin.selected ? 'plugins.disable' : 'plugins.enable')" :disabled="busy || (!!plugin.problem && !plugin.selected)" @click="selection = plugin" />
          </div>
          <p v-if="plugin.problem" role="alert" class="type-body mt-2 text-red-500">{{ translateCommandError(plugin.problem) }}</p>
          <ul class="type-caption mt-2 space-y-1 text-[var(--text-secondary)]"><li v-for="name in plugin.adapters" :key="name" class="break-words">{{ name }}</li></ul>
          <details class="type-caption mt-2 text-[var(--text-secondary)]"><summary class="cursor-pointer">SHA-256</summary><p class="mt-1 select-text break-all font-mono">{{ plugin.sha256 }}</p></details>
        </li>
      </ul>
    </div>
  </ManagementFormSection>

  <UModal :open="!!prepared" :title="t('plugins.previewTitle')" :description="t('plugins.previewHint')" :dismissible="!busy" @update:open="dismissPreview">
    <template #body>
      <div v-if="prepared" class="min-w-0 space-y-3">
        <p class="font-semibold">{{ label(prepared) }} {{ prepared.version }}</p>
        <p class="type-caption break-all">{{ prepared.packageId }} · {{ prepared.architectures.join(' / ') }}</p>
        <ul class="type-body space-y-1"><li v-for="name in prepared.adapters" :key="name">{{ name }}</li></ul>
        <details class="type-caption"><summary class="cursor-pointer">SHA-256</summary><p class="mt-2 select-text break-all font-mono">{{ prepared.sha256 }}</p></details>
      </div>
    </template>
    <template #footer>
      <div class="ml-auto flex gap-2">
        <UButton color="neutral" variant="outline" :label="t('common.cancel')" :disabled="busy" @click="dismissPreview(false)" />
        <UButton :label="t('plugins.confirmInstall')" :loading="busy" :disabled="busy" @click="install" />
      </div>
    </template>
  </UModal>
  <ConfirmDialog :open="!!selection" :title="t(selection?.selected ? 'plugins.disableTitle' : 'plugins.enableTitle')"
    :description="t(selection?.selected ? 'plugins.disableHint' : 'plugins.enableHint', { name: selection ? label(selection) : '' })"
    :confirm-label="t(selection?.selected ? 'plugins.disable' : 'plugins.enable')" confirm-color="primary" :busy="busy"
    @update:open="value => { if (!value && !busy) selection = null }" @confirm="changeSelection" />
</template>
