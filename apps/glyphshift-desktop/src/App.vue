<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { en, zh_cn } from '@nuxt/ui/locale'
import { useI18n } from 'vue-i18n'
import DictionaryLibrary from './components/DictionaryLibrary.vue'
import DictionaryProof from './components/DictionaryProof.vue'
import CaptureView from './components/CaptureView.vue'
import HelpView from './components/HelpView.vue'
import SettingsView from './components/SettingsView.vue'
import SoftwareTable from './components/SoftwareTable.vue'
import TitleBar from './components/TitleBar.vue'
import TranslationTasksView from './components/TranslationTasksView.vue'
import WorkflowTable from './components/WorkflowTable.vue'
import { useAppSettings } from './appSettings'
import { useAiTranslation } from './useAiTranslation'
import type { SoftwareQuickCaptureEvent, WorkflowCommandResult, WorkflowDetail, WorkflowTarget } from './model'
import { translateCommandError, type CommandError } from './commandError'
import { useProbeRuns } from './useProbeRuns'
import { useWorkspace } from './useWorkspace'

type View = 'workflows' | 'software' | 'dictionaries' | 'dictionary-editor' | 'capture' | 'translation-tasks' | 'help' | 'settings'
type NavigableView = Exclude<View, 'dictionary-editor'>
const desktopApiVersion = 32

const { t } = useI18n()
const appSettings = useAppSettings()
const workspace = useWorkspace()
const probe = useProbeRuns()
const ai = useAiTranslation()
const view = ref<View>('workflows')
const editorDirty = ref(false)
const pendingExit = ref<NavigableView | 'close' | null>(null)
const discardOpen = computed(() => pendingExit.value !== null)
const closingActiveTask = computed(() => pendingExit.value === 'close' && ai.taskRunning.value)
const shellCompatibilityErrorKey = ref('')
const quickProbeCaptureActive = ref(false)
const mainContent = ref<HTMLElement | null>(null)
const shellCompatibilityError = computed(() => shellCompatibilityErrorKey.value ? t(shellCompatibilityErrorKey.value) : '')
const nuxtLocale = computed(() => appSettings.effectiveLocale.value === 'en-US' ? en : zh_cn)
const translationTaskProgress = computed(() => {
  const task = ai.currentJob.value
  return task && ai.taskRunning.value ? `${task.finishedBatches}/${task.totalBatches}` : ''
})
let unlistenSoftwareCapture: UnlistenFn | null = null
let unlistenWorkflowShortcut: UnlistenFn | null = null
let unlistenWindowClose: UnlistenFn | null = null

async function openWorkflow(id: string) {
  await workspace.loadWorkflow(id)
}

async function openDictionary(id: string) {
  if (await workspace.loadDictionary(id)) {
    editorDirty.value = false
    view.value = 'dictionary-editor'
  }
}

function requestNavigation(next: NavigableView) {
  if (next === view.value) return
  if (editorDirty.value) {
    pendingExit.value = next
    return
  }
  editorDirty.value = false
  view.value = next
}

async function closeWindow() {
  try {
    await getCurrentWindow().close()
  }
  catch {
    // Browser previews do not expose native window controls.
  }
}

async function minimizeWindow() {
  try {
    await getCurrentWindow().minimize()
  }
  catch {
    // Browser previews do not expose native window controls.
  }
}

function requestWindowClose() {
  if (appSettings.closeBehavior.value === 'minimize') {
    void minimizeWindow()
    return
  }
  if (editorDirty.value || ai.taskRunning.value) {
    pendingExit.value = 'close'
    return
  }
  void closeWindow()
}

async function connectWindowCloseBehavior() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    unlistenWindowClose = await getCurrentWindow().onCloseRequested(event => {
      if (appSettings.closeBehavior.value === 'quit' && !editorDirty.value && !ai.taskRunning.value) return
      event.preventDefault()
      requestWindowClose()
    })
  }
  catch {
    // The title-bar close action remains available if the native listener is unavailable.
  }
}

function cancelDiscard() {
  pendingExit.value = null
}

function confirmDiscard() {
  const destination = pendingExit.value
  pendingExit.value = null
  editorDirty.value = false
  if (destination === 'close') void closeWindow()
  else if (destination) view.value = destination
}

function guardBrowserExit(event: BeforeUnloadEvent) {
  if (!editorDirty.value) return
  event.preventDefault()
  event.returnValue = ''
}

function focusMainContent() {
  mainContent.value?.focus({ preventScroll: true })
}

function handleShellShortcut(event: KeyboardEvent) {
  if (!event.altKey || event.ctrlKey || event.metaKey || event.code !== 'KeyM' || event.defaultPrevented) return
  event.preventDefault()
  focusMainContent()
}

async function createWorkflow(name: string, description: string, targets: WorkflowTarget[], globalShortcut: string, done: (saved: boolean) => void) {
  done(await workspace.createWorkflow(name, description, targets, globalShortcut))
}

async function saveWorkflow(detail: WorkflowDetail, done: (saved: boolean) => void) {
  const saved = await workspace.saveWorkflow(detail)
  done(saved)
}

async function connectDesktopShell() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    const status = await invoke<{ shellReady: boolean; apiVersion: number }>('desktop_status')
    if (status.shellReady && status.apiVersion !== desktopApiVersion) {
      shellCompatibilityErrorKey.value = 'app.desktopApiChanged'
      return
    }
    shellCompatibilityErrorKey.value = ''
    if (!status.shellReady || !await workspace.connectDesktopBackend()) {
      shellCompatibilityErrorKey.value = 'app.desktopUnavailable'
      return
    }
    await probe.connect()
  }
  catch {
    shellCompatibilityErrorKey.value = 'app.desktopUnavailable'
  }
}

function receiveSoftwareQuickCapture(event: SoftwareQuickCaptureEvent) {
  const requestedForQuickProbe = quickProbeCaptureActive.value
  workspace.handleSoftwareQuickCaptureEvent(event)
  if (requestedForQuickProbe) {
    if (event.state === 'captured') quickProbeCaptureActive.value = false
    requestNavigation('capture')
  }
  else requestNavigation('software')
}

async function armQuickProbeCapture() {
  quickProbeCaptureActive.value = await workspace.armSoftwareCapture()
}

async function cancelQuickProbeCapture() {
  quickProbeCaptureActive.value = false
  await workspace.cancelSoftwareCapture()
}

async function refreshWorkspaceAfterQuickProbe() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    await workspace.connectDesktopBackend()
  }
  catch {
    shellCompatibilityErrorKey.value = 'app.desktopUnavailable'
  }
}

function receiveBrowserSoftwareQuickCapture(event: Event) {
  receiveSoftwareQuickCapture((event as CustomEvent<SoftwareQuickCaptureEvent>).detail)
}

async function connectSoftwareQuickCaptureEvents() {
  window.addEventListener('glyphshift:software-quick-capture', receiveBrowserSoftwareQuickCapture)
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    unlistenSoftwareCapture = await listen<SoftwareQuickCaptureEvent>('software-quick-capture', event => {
      receiveSoftwareQuickCapture(event.payload)
    })
  }
  catch {
    // The explicit Quick capture action still reports shortcut registration failures.
  }
}

async function connectWorkflowShortcuts() {
  if (!('__TAURI_INTERNALS__' in window)) return
  try {
    unlistenWorkflowShortcut = await listen<{ workflowId: string; result: WorkflowCommandResult | null; error: CommandError | null }>('workflow-shortcut', event => {
      const { workflowId, result, error } = event.payload
      if (result) workspace.applyWorkflowResult(result)
      workspace.messages.value = { ...workspace.messages.value, [workflowId]: error ? translateCommandError(error) : '' }
    })
    const errors = await invoke<Record<string, CommandError>>('desktop_workflow_shortcut_errors')
    for (const [id, error] of Object.entries(errors ?? {})) {
      workspace.messages.value = { ...workspace.messages.value, [id]: translateCommandError(error) }
    }
  } catch { /* Shortcuts are available only in the native desktop. */ }
}

onMounted(() => {
  window.addEventListener('beforeunload', guardBrowserExit)
  window.addEventListener('keydown', handleShellShortcut)
  void connectDesktopShell()
  void connectSoftwareQuickCaptureEvents()
  void connectWorkflowShortcuts()
  void connectWindowCloseBehavior()
  void ai.connectTaskMonitor().catch(() => undefined)
})

watch(() => ai.currentJob.value?.appliedCount, async (value, previous) => {
  if (!value || value === previous || !('__TAURI_INTERNALS__' in window)) return
  await workspace.connectDesktopBackend()
  const dictionaryId = ai.currentJob.value?.targetDictionaryId
  if (dictionaryId && workspace.dictionaryDetail.value?.metadata.id === dictionaryId) {
    await workspace.loadDictionary(dictionaryId)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('beforeunload', guardBrowserExit)
  window.removeEventListener('keydown', handleShellShortcut)
  window.removeEventListener('glyphshift:software-quick-capture', receiveBrowserSoftwareQuickCapture)
  unlistenSoftwareCapture?.()
  unlistenWorkflowShortcut?.()
  unlistenWindowClose?.()
})
</script>

<template>
  <UApp :locale="nuxtLocale">
    <div class="flex h-full min-h-0 flex-col overflow-hidden bg-[var(--app-bg)] text-[var(--text)]">
      <a
        href="#main-content"
        class="fixed left-3 top-2 z-[120] flex h-8 -translate-y-14 items-center rounded-[6px] border border-[var(--border-strong)] bg-[var(--surface)] px-3 type-label font-semibold text-[var(--text)] outline-none focus:translate-y-0 focus:ring-2 focus:ring-[var(--focus)] focus:ring-offset-2 focus:ring-offset-[var(--titlebar)]"
        :aria-label="t('app.skipToMain')"
        aria-keyshortcuts="Alt+M"
        @click.prevent="focusMainContent"
      >
        <span>{{ t('app.skipToMain') }}</span>
        <span class="type-caption ml-2 font-normal text-[var(--text-muted)]" aria-hidden="true">Alt+M</span>
      </a>
      <TitleBar
        :current="view"
        :probe-activity-status="probe.activityStatus.value"
        :translation-task-active="ai.taskRunning.value"
        :translation-task-progress="translationTaskProgress"
        @navigate="requestNavigation"
        @close="requestWindowClose"
      />
      <main id="main-content" ref="mainContent" tabindex="-1" class="flex min-h-0 flex-1 overflow-hidden outline-none" :aria-label="t('app.mainContent')">
      <section v-if="shellCompatibilityError && view !== 'help'" class="grid min-h-0 flex-1 place-items-center bg-[var(--app-bg)] p-6" role="alert">
        <h1 class="sr-only">{{ t('app.desktopReloadTitle') }}</h1>
        <UAlert
          color="warning"
          variant="soft"
          icon="i-tabler-refresh-alert"
          :title="t('app.desktopReloadTitle')"
          :description="shellCompatibilityError"
          class="max-w-[520px]"
        />
      </section>
      <WorkflowTable
        v-else-if="view === 'workflows'"
        :items="workspace.model.value.workflows"
        :software="workspace.model.value.software"
        :dictionaries="workspace.model.value.dictionaries"
        :installed-families="workspace.model.value.fontFamilies"
        :adapters="workspace.model.value.adapters"
        :activation-ids="workspace.activationIds.value"
        :runtime-status="workspace.model.value.workflowRuntimeStatus"
        :busy="workspace.workspaceBusy.value"
        :refreshing="workspace.refreshing.value"
        :font-refreshing="workspace.fontRefreshing.value"
        :messages="workspace.messages.value"
        :editing="workspace.workflowDetail.value"
        @toggle="workspace.setWorkflowEnabled"
        @refresh="workspace.refreshWorkflows"
        @refresh-fonts="workspace.refreshFontFamilies"
        @open="openWorkflow"
        @create="createWorkflow"
        @save="saveWorkflow"
        @close-edit="workspace.workflowDetail.value = null"
        @copy="workspace.copyWorkflow"
        @remove="workspace.removeWorkflows"
        @toggle-many="workspace.setWorkflowsEnabled"
        @navigate="requestNavigation"
        @dirty-change="editorDirty = $event"
      />
      <SoftwareTable
        v-else-if="view === 'software'"
        :items="workspace.model.value.software"
        :busy="workspace.softwareBusy.value"
        :messages="workspace.messages.value"
        :preflight="workspace.softwarePreflight.value"
        :preflight-busy="workspace.softwarePreflightBusy.value"
        :capture-armed="workspace.softwareCaptureArmed.value"
        :capture-shortcut="workspace.softwareCaptureShortcut.value"
        :capture-result="workspace.softwareCaptureResult.value"
        @add="workspace.addSoftware"
        @validate="workspace.validateSoftware"
        @clear-preflight="workspace.clearSoftwarePreflight"
        @arm-capture="workspace.armSoftwareCapture"
        @cancel-capture="workspace.cancelSoftwareCapture"
        @update-software="workspace.updateSoftware"
        @remove="workspace.removeSoftware"
        @dirty-change="editorDirty = $event"
      />
      <DictionaryLibrary
        v-else-if="view === 'dictionaries'"
        :items="workspace.model.value.dictionaries"
        :busy="workspace.workspaceBusy.value"
        :messages="workspace.messages.value"
        :catalog-page="workspace.dictionaryCatalog.value"
        :catalog-busy="workspace.dictionaryCatalogBusy.value"
        :catalog-error="workspace.dictionaryCatalogError.value"
        :presentation-locale="appSettings.effectiveLocale.value"
        :artifact-warnings="workspace.model.value.artifactWarnings"
        @open="openDictionary"
        @create="workspace.createDictionary"
        @import-dictionary="workspace.importDictionary"
        @export-dictionary="workspace.exportDictionary"
        @remove="workspace.removeDictionaries"
        @query-catalog="workspace.queryDictionaryCatalog"
        @install-catalog="workspace.installDictionaryRelease"
      />
      <DictionaryProof
        v-else-if="view === 'dictionary-editor' && workspace.dictionaryDetail.value"
        :detail="workspace.dictionaryDetail.value"
        :busy="workspace.workspaceBusy.value"
        @back="requestNavigation('dictionaries')"
        @save="workspace.saveDictionary"
        @configure-ai="requestNavigation('settings')"
        @open-ai-tasks="requestNavigation('translation-tasks')"
        @dirty-change="editorDirty = $event"
      />
      <CaptureView
        v-else-if="view === 'capture'"
        :software="workspace.model.value.software"
        :dictionaries="workspace.model.value.dictionaries"
        :adapters="workspace.model.value.adapters"
        :capture-armed="quickProbeCaptureActive && workspace.softwareCaptureArmed.value"
        :capture-shortcut="workspace.softwareCaptureShortcut.value"
        :capture-result="workspace.softwareCaptureResult.value"
        :capture-error="workspace.messages.value.software ?? ''"
        @arm-capture="armQuickProbeCapture"
        @cancel-capture="cancelQuickProbeCapture"
        @open-dictionary="openDictionary"
        @workspace-changed="refreshWorkspaceAfterQuickProbe"
        @configure-ai="requestNavigation('settings')"
        @open-ai-tasks="requestNavigation('translation-tasks')"
      />
      <TranslationTasksView v-else-if="view === 'translation-tasks'" />
      <HelpView v-else-if="view === 'help'" :adapters="workspace.model.value.adapters" @navigate="requestNavigation" />
      <SettingsView v-else @navigate="view = $event" />
      </main>
      <ConfirmDialog
        :open="discardOpen"
        :title="closingActiveTask ? t('ai.tasks.quitTitle') : t('common.discardTitle')"
        :description="closingActiveTask ? t('ai.tasks.quitDescription') : t('common.discardDescription')"
        :cancel-label="t('common.continueEditing')"
        :confirm-label="closingActiveTask ? t('ai.tasks.quitConfirm') : t('common.discardChanges')"
        confirm-color="warning"
        @update:open="$event || cancelDiscard()"
        @confirm="confirmDiscard"
      />
    </div>
  </UApp>
</template>
