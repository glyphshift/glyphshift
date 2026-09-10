import { computed, ref } from 'vue'
import { applyWorkflowRuntime } from './workspace/state'
import { invoke } from '@tauri-apps/api/core'
import { isCommandError, translateCommandError, type CommandError } from './commandError'
import { i18n } from './i18n'
import type { ProbeEntryPage, ProbeExportFormat, ProbeRunSummary } from './model'

export interface ProbeRunQueryInput {
  runId: string
  search: string
  adapterIds: string[]
  hideSkipped?: boolean
  translationFilter: ProbeTranslationFilter
  page: number
  pageSize: number
}

export type ProbeTranslationFilter = 'all' | 'untranslated' | 'translated'

export interface ProbeRunUpdateInput {
  excludedDictionaryIds: string[]
  runId: string
  name: string
  dictionaryId: string
  adapterIds: string[]
  livePreviewEnabled: boolean
}

export interface ProbeDictionarySyncEntry {
  source: string
  translation: string
}

export type ProbeTargetSource
  = { kind: 'library'; softwareId: string }
    | { kind: 'active_process'; executablePath: string }

export type ProbeDictionarySource
  = { kind: 'library'; dictionaryId: string }
    | { kind: 'new'; sourceLocale: string; targetLocale: string }

export interface ProbeCreationInput {
  excludedDictionaryIds: string[]
  target: ProbeTargetSource
  dictionary: ProbeDictionarySource
  name?: string
  adapterIds: string[]
  livePreviewEnabled: boolean
}

export interface ProbeCreationBrowserFallback {
  softwareId?: string
  softwareName: string
  dictionaryId?: string
}

export type ProbeActivityStatus = 'running' | 'paused' | null

const runs = ref<ProbeRunSummary[]>([])
const activityStatus = computed<ProbeActivityStatus>(() => {
  if (runs.value.some(run => run.status === 'running')) return 'running'
  if (runs.value.some(run => run.status === 'paused')) return 'paused'
  return null
})
const selectedRunId = ref(localStorage.getItem('glyphshift.workflow.selectedRecord') ?? '')
const busy = ref(false)
const polling = ref(false)
const summaryRequests = new Map<string, Promise<ProbeRunSummary | null>>()
const message = ref('')
const lastError = ref<CommandError | null>(null)
let connected = false

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function upsert(summary: ProbeRunSummary) {
  if (summary.workflowRuntime) applyWorkflowRuntime(summary.workflowRuntime)
  const index = runs.value.findIndex(item => item.id === summary.id)
  if (index >= 0) runs.value.splice(index, 1, summary)
  else runs.value.push(summary)
  runs.value.sort((left, right) => right.updatedAtMs - left.updatedAtMs || left.name.localeCompare(right.name))
}

function selectRun(id: string) {
  if (selectedRunId.value !== id) clearMessage()
  selectedRunId.value = id
  if (id) localStorage.setItem('glyphshift.workflow.selectedRecord', id)
  else localStorage.removeItem('glyphshift.workflow.selectedRecord')
}

function clearMessage() {
  message.value = ''
  lastError.value = null
}

function reportError(error: unknown, runId?: string) {
  if (runId && selectedRunId.value && selectedRunId.value !== runId) return
  lastError.value = isCommandError(error) ? error : null
  message.value = translateCommandError(error)
}

export function useProbeRuns() {
  const selectedRun = computed(() => runs.value.find(item => item.id === selectedRunId.value) ?? null)

  async function connect() {
    if (connected) return
    connected = true
    clearMessage()
    if (hasDesktopRuntime()) {
      try {
        const loaded = await invoke<ProbeRunSummary[]>('desktop_probe_runs')
        runs.value = Array.isArray(loaded) ? loaded : []
      }
      catch (error) {
        reportError(error)
      }
    }
    if (!runs.value.some(item => item.id === selectedRunId.value)) selectRun('')
  }

  async function compatibleAdapters(softwareId: string, runId?: string) {
    if (!hasDesktopRuntime()) return null
    try {
      return await invoke<string[]>('desktop_compatible_probe_adapters', { softwareId })
    }
    catch (error) {
      reportError(error, runId)
      return []
    }
  }

  async function createFromSources(input: ProbeCreationInput, fallback: ProbeCreationBrowserFallback) {
    if (busy.value) return null
    busy.value = true
    clearMessage()
    try {
      const now = Date.now()
      const summary = hasDesktopRuntime()
        ? await invoke<ProbeRunSummary>('desktop_create_probe_from_sources', { request: input })
        : {
            id: `quick-probe-${crypto.randomUUID()}`,
            name: input.name?.trim() || `${fallback.softwareName} ${i18n.global.t('capture.quickProbe.browserRunSuffix')}`,
            softwareId: fallback.softwareId ?? `quick-software-${crypto.randomUUID()}`,
            dictionaryId: fallback.dictionaryId ?? `quick-dictionary-${crypto.randomUUID()}`,
            adapterIds: [],
            status: 'running' as const,
            livePreviewEnabled: false,
            observationRevision: 0,
            observedCount: 0,
            ignoredCount: 0,
            droppedObservations: 0,
            previewGeneration: 0,
            createdAtMs: now,
            updatedAtMs: now,
            dictionaryRevision: 1,
            dictionaryEntryCount: 0,
            runtimeCapability: null,
            quickProbe: false,
            excludedDictionaryIds: input.excludedDictionaryIds,
          }
      upsert(summary)
      selectRun(summary.id)
      return summary
    }
    catch (error) {
      reportError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function remove(runIds: string[]) {
    if (busy.value || !runIds.length) return false
    busy.value = true
    clearMessage()
    try {
      if (hasDesktopRuntime()) await invoke('desktop_delete_probe_runs', { runIds })
      runs.value = runs.value.filter(run => !runIds.includes(run.id))
      if (runIds.includes(selectedRunId.value)) selectRun('')
      return true
    }
    catch (error) {
      if (hasDesktopRuntime()) {
        try { runs.value = await invoke<ProbeRunSummary[]>('desktop_probe_runs') } catch { /* Keep the original deletion error. */ }
      }
      reportError(error)
      return false
    }
    finally {
      busy.value = false
    }
  }

  async function update(input: ProbeRunUpdateInput) {
    if (busy.value) return null
    busy.value = true
    clearMessage()
    try {
      const summary = hasDesktopRuntime()
        ? await invoke<ProbeRunSummary>('desktop_update_probe_run', { request: input })
        : (() => {
            const current = runs.value.find(run => run.id === input.runId)
            return current
              ? {
                  ...current,
                  name: input.name,
                  dictionaryId: input.dictionaryId,
                  adapterIds: [...input.adapterIds],
                  livePreviewEnabled: input.livePreviewEnabled,
                  updatedAtMs: Date.now(),
                }
              : null
          })()
      if (!summary) return null
      upsert(summary)
      return summary
    }
    catch (error) {
      reportError(error, input.runId)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function clearEntries(runId: string) {
    if (busy.value) return null
    busy.value = true
    clearMessage()
    try {
      const summary = hasDesktopRuntime()
        ? await invoke<ProbeRunSummary>('desktop_clear_probe_run_entries', { runId })
        : (() => {
            const current = runs.value.find(run => run.id === runId)
            return current
              ? {
                  ...current,
                  observationRevision: current.observationRevision + 1,
                  observedCount: 0,
                  ignoredCount: 0,
                  droppedObservations: 0,
                  dictionaryRevision: current.dictionaryRevision + (current.dictionaryEntryCount ? 1 : 0),
                  dictionaryEntryCount: 0,
                  updatedAtMs: Date.now(),
                }
              : null
          })()
      if (!summary) return null
      upsert(summary)
      return summary
    }
    catch (error) {
      reportError(error, runId)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function openWorkflow(workflowId: string) {
    const run = await runSummaryCommand('desktop_workflow_collection', { workflowId })
    if (run) selectRun(run.id)
    return run
  }

  async function resume(runId: string) {
    return runSummaryCommand('desktop_resume_probe_run', { runId })
  }

  async function setPaused(runId: string, paused: boolean) {
    return runSummaryCommand('desktop_set_probe_run_paused', { runId, paused })
  }

  async function disconnect(runId: string) {
    return runSummaryCommand('desktop_disconnect_probe_run', { runId })
  }

  async function importEntries(runId: string, inputPath: string, format: 'json' | 'csv', mode: string) {
    return runSummaryCommand('desktop_import_probe_entries', { request: { runId, inputPath, format, mode } })
  }

  async function refreshText(runId: string) {
    return runSummaryCommand('desktop_refresh_probe_text', { runId })
  }

  async function runSummaryCommand(command: string, args: Record<string, unknown>) {
    if (busy.value || !hasDesktopRuntime()) return null
    busy.value = true
    clearMessage()
    try {
      const summary = await invoke<ProbeRunSummary>(command, args)
      upsert(summary)
      return summary
    }
    catch (error) {
      const contextualError = command === 'desktop_refresh_probe_text' && isCommandError(error)
        ? { ...error, args: { ...error.args, action: 'refreshText' } } : error
      reportError(contextualError, typeof args.runId === 'string' ? args.runId : undefined)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function refreshSummary(runId: string) {
    if (!hasDesktopRuntime()) return runs.value.find(run => run.id === runId) ?? null
    const existing = summaryRequests.get(runId)
    if (existing) return existing
    polling.value = true
    const request = (async () => {
      try {
        const summary = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
        upsert(summary)
        return summary
      }
      catch (error) { reportError(error, runId); return null }
      finally {
        summaryRequests.delete(runId)
        polling.value = summaryRequests.size > 0
      }
    })()
    summaryRequests.set(runId, request)
    return request
  }

  async function queryEntries(input: ProbeRunQueryInput): Promise<ProbeEntryPage> {
    if (!hasDesktopRuntime()) {
      return {
        observationRevision: 0,
        dictionaryRevision: selectedRun.value?.dictionaryRevision ?? 1,
        page: input.page,
        pageSize: input.pageSize,
        total: 0,
        rows: [],
      }
    }
    return invoke<ProbeEntryPage>('desktop_probe_run_entries', { request: input, hideSkipped: input.hideSkipped ?? false })
  }

  async function editTranslation(runId: string, source: string, translation: string) {
    if (!hasDesktopRuntime()) return selectedRun.value
    const summary = await invoke<ProbeRunSummary>('desktop_edit_probe_translation', {
      request: { runId, source, translation },
    })
    upsert(summary)
    return summary
  }

  async function syncDictionaryEntries(runId: string, entries: ProbeDictionarySyncEntry[]) {
    if (busy.value || !entries.length) return null
    busy.value = true
    clearMessage()
    try {
      const summary = hasDesktopRuntime()
        ? await invoke<ProbeRunSummary>('desktop_sync_probe_dictionary_entries', {
            request: { runId, entries },
          })
        : (() => {
            const current = runs.value.find(run => run.id === runId)
            return current
              ? {
                  ...current,
                  dictionaryRevision: current.dictionaryRevision + 1,
                  dictionaryEntryCount: current.dictionaryEntryCount + entries.length,
                  updatedAtMs: Date.now(),
                }
              : null
          })()
      if (!summary) return null
      upsert(summary)
      return summary
    }
    catch (error) {
      reportError(error, runId)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function bulk(
    runId: string,
    sources: string[],
    action: 'ignore' | 'restore' | 'clear_translations',
  ) {
    if (!hasDesktopRuntime()) return selectedRun.value
    const summary = await invoke<ProbeRunSummary>('desktop_bulk_probe_entries', {
      request: { runId, sources, action },
    })
    upsert(summary)
    return summary
  }

  async function exportRun(runId: string, format: ProbeExportFormat, outputPath: string) {
    if (!hasDesktopRuntime()) return false
    try {
      await invoke('desktop_export_probe_run', { request: { runId, format, outputPath } })
      return true
    }
    catch (error) {
      reportError(error, runId)
      return false
    }
  }

  function report(error: unknown, runId?: string) {
    reportError(error, runId)
  }

  return {
    openWorkflow,
    runs,
    activityStatus,
    selectedRunId,
    selectedRun,
    busy,
    polling,
    message,
    lastError,
    connect,
    clearMessage,
    selectRun,
    compatibleAdapters,
    createFromSources,
    remove,
    update,
    clearEntries,
    resume,
    setPaused,
    disconnect,
    refreshText,
    refreshSummary,
    queryEntries,
    editTranslation,
    syncDictionaryEntries,
    bulk,
    exportRun,
    importEntries,
    report,
  }
}
