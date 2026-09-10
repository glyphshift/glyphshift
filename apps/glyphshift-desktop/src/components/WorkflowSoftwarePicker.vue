<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open as openFile } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { useWorkspace } from '../useWorkspace'
import { useRecentSoftware } from '../useRecentSoftware'
import { translateCommandError } from '../commandError'
import type { SoftwareRecord, SoftwarePreflight } from '../model'

const props = defineProps<{ software: SoftwareRecord[] }>()
const visible = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ select: [id: string] }>()
const { t } = useI18n()
const workspace = useWorkspace()
const path = ref('')
const query = ref('')
const mode = ref('recent')
const busy = ref(false)
const loading = ref(false)
const error = ref('')
const running = ref<SoftwarePreflight[]>([])
const history = useRecentSoftware(computed(() => props.software))
const recentIds = history.ids
let request = 0
const normalize = (value: string) => value.trim().replace(/\//g, '\\').toLocaleLowerCase()
const recent = computed(() => recentIds.value.map(id => props.software.find(item => item.id === id))
  .filter((item): item is SoftwareRecord => Boolean(item))
  .filter(item => `${item.name} ${item.executablePath ?? ''}`.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())))
async function clearRecent() {
  if (busy.value) return
  busy.value = true; error.value = ''
  try { await history.clear() } catch (cause) { error.value = translateCommandError(cause) }
  finally { busy.value = false }
}
async function loadRunning() {
  const ticket = ++request
  loading.value = true
  error.value = ''
  try {
    const items = '__TAURI_INTERNALS__' in window ? await invoke<SoftwarePreflight[]>('desktop_running_software_targets') : []
    if (ticket === request) running.value = items ?? []
  } catch (cause) { if (ticket === request) error.value = translateCommandError(cause) }
  finally { if (ticket === request) loading.value = false }
}
watch(visible, (value) => {
  if (value) {
    path.value = ''; query.value = ''; error.value = ''; mode.value = 'recent'
    workspace.clearSoftwarePreflight()
  } else {
    ++request
    if (workspace.softwareCaptureArmed.value) void workspace.cancelSoftwareCapture()
  }
})
watch(mode, value => { if (value === 'running') void loadRunning() })
watch(workspace.softwareCaptureResult, result => {
  if (visible.value && result) path.value = result.executablePath
})
onBeforeUnmount(() => { ++request; if (workspace.softwareCaptureArmed.value) void workspace.cancelSoftwareCapture() })
async function browse() {
  try {
    const selected = await openFile({ directory: false, multiple: false, title: t('software.selectWindowsApp'), filters: [{ name: t('software.windowsApp'), extensions: ['exe'] }] })
    if (typeof selected === 'string') path.value = selected
  } catch (cause) { error.value = translateCommandError(cause) }
}
async function confirm() {
  if (busy.value || !path.value.trim()) return
  busy.value = true; error.value = ''
  const chosen = path.value.trim()
  try {
    if (!await workspace.validateSoftware(chosen)) { error.value = workspace.messages.value.software ?? ''; return }
    const check = workspace.softwarePreflight.value
    if (!check || normalize(check.executablePath) !== normalize(chosen)) return
    let record = props.software.find(item => normalize(item.executablePath ?? '') === normalize(chosen))
    if (!check.canAdd && check.state !== 'already_added') {
      const keys: Record<string, string> = { not_running: 'software.not_running', runtime_unavailable: 'software.runtime_unavailable', self_target: 'software.self_target', unsupported_architecture: 'software.unsupported_architecture' }
      error.value = translateCommandError({ schemaVersion: 1, code: keys[check.state] ?? 'software.invalid_executable', args: {} }); return
    }
    if (!record) {
      if (!await workspace.addSoftware(check.suggestedName, '', chosen)) { error.value = workspace.messages.value.software ?? ''; return }
      record = workspace.model.value.software.find(item => normalize(item.executablePath ?? '') === normalize(chosen))
    }
    if (!record) { error.value = t('workspace.softwareResultMissing'); return }
    await history.remember(record.id)
    await nextTick()
    emit('select', record.id)
    visible.value = false
  } catch (cause) { error.value = translateCommandError(cause) }
  finally { busy.value = false }
}
</script>

<template>
  <ManagementFormModal v-model:open="visible" :title="t('workflows.tabs.software')" width="lg" :busy="busy"
    :confirm-label="t('workflows.confirmSoftware')" :confirm-disabled="!path.trim()" @confirm="confirm">
    <div class="space-y-4">
      <UFormField :label="t('software.executablePath')" required>
        <div class="flex gap-2"><UInput v-model="path" class="min-w-0 flex-1" :aria-label="t('software.executablePath')" :disabled="busy" /><UButton color="neutral" variant="outline" :label="t('software.browse')" :disabled="busy" @click="browse" /></div>
      </UFormField>
      <div class="flex items-center justify-between gap-3">
        <UTabs v-model="mode" :content="false" :items="[{value:'recent', label:t('workflowSoftware.recent')}, {value:'running', label:t('software.runningSoftware')}]" />
        <UButton v-if="mode === 'recent'" color="neutral" variant="ghost" size="sm" icon="i-tabler-trash" :label="t('workflowSoftware.clear')" :title="t('workflowSoftware.clearHint')" :disabled="busy || !recentIds.length" @click="clearRecent" />
        <UButton v-else color="neutral" variant="ghost" icon="i-tabler-refresh" :aria-label="t('software.refreshRunningSoftware')" :title="t('software.refreshRunningSoftware')" :loading="loading" @click="loadRunning" />
      </div>
      <template v-if="mode === 'recent'">
        <UInput v-model="query" class="w-full" icon="i-tabler-search" :placeholder="t('workflows.searchSoftware')" :aria-label="t('workflows.searchSoftware')" />
        <div data-testid="workflow-software-catalog" class="max-h-64 overflow-y-auto rounded-md border border-[var(--border)]">
          <UButton color="neutral" variant="ghost" v-for="item in recent" :key="item.id" :disabled="busy" :aria-pressed="normalize(path) === normalize(item.executablePath ?? '')" class="flex w-full justify-start rounded-none items-center gap-3 border-b border-[var(--border)] px-3 py-3 text-left last:border-b-0 hover:bg-[var(--surface-hover)]" @click="path = item.executablePath ?? ''; error = item.executablePath ? '' : t('errors.software.bindingMissing')">
            <UIcon :name="path && normalize(path) === normalize(item.executablePath ?? '') ? 'i-tabler-circle-dot' : 'i-tabler-circle'" class="size-4 shrink-0" />
            <span class="min-w-0"><strong class="type-label block truncate">{{ item.name }}</strong><span class="type-metadata block truncate text-[var(--text-muted)]" :title="item.executablePath ?? ''">{{ item.executablePath || t('workflowSoftware.missingPath') }}</span></span>
          </UButton>
          <p v-if="!recent.length" class="type-metadata p-5 text-center text-[var(--text-muted)]">{{ t('workflowSoftware.empty') }}</p>
        </div>
      </template>
      <div v-else class="max-h-64 overflow-y-auto rounded-md border border-[var(--border)]">
        <UButton color="neutral" variant="ghost" v-for="item in running" :key="item.executablePath" :disabled="busy" class="block w-full rounded-none border-b border-[var(--border)] px-3 py-3 text-left last:border-b-0 hover:bg-[var(--surface-hover)]" @click="path = item.executablePath"><strong class="type-label block">{{ item.existingName || item.suggestedName }}</strong><span class="type-metadata block truncate text-[var(--text-muted)]">{{ item.executablePath }}</span></UButton>
        <p v-if="!running.length && !loading" class="type-metadata p-5 text-center text-[var(--text-muted)]">{{ t('software.runningSoftwareEmpty') }}</p>
      </div>
      <div class="flex items-center justify-between gap-3">
        <span class="type-metadata text-[var(--text-muted)]">{{ t('workflowSoftware.captureHint', { shortcut: workspace.softwareCaptureShortcut.value }) }}</span>
        <UButton color="neutral" variant="outline" size="sm" :disabled="busy" :icon="workspace.softwareCaptureArmed.value ? 'i-tabler-x' : 'i-tabler-focus-centered'" :label="workspace.softwareCaptureArmed.value ? t('software.cancelCapture') : t('software.quickCapture')" @click="workspace.softwareCaptureArmed.value ? workspace.cancelSoftwareCapture() : workspace.armSoftwareCapture()" />
      </div>
      <p v-if="error || workspace.messages.value.software" role="alert" class="type-metadata text-error">{{ error || workspace.messages.value.software }}</p>
    </div>
  </ManagementFormModal>
</template>
