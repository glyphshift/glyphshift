import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { translateCommandError } from './commandError'
import { i18n } from './i18n'
import type { ProbeEntryPage, ProbeExportFormat, ProbeRunSummary } from './model'

export interface ProbeRunQueryInput {
  runId: string
  search: string
  adapterIds: string[]
  page: number
  pageSize: number
}

export interface ProbeRunUpdateInput {
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
    | { kind: 'temporary'; targetLocale: string }

export interface ProbeCreationInput {
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

export interface QuickProbeCleanupResult {
  software: 'removed' | 'reused' | 'retained'
  dictionary: 'removed' | 'reused' | 'retained'
}

const runs = ref<ProbeRunSummary[]>([])
const selectedRunId = ref(localStorage.getItem('glyphshift.probe.selectedRun') ?? '')
const busy = ref(false)
const polling = ref(false)
const message = ref('')
let connected = false

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function upsert(summary: ProbeRunSummary) {
  const index = runs.value.findIndex(item => item.id === summary.id)
  if (index >= 0) runs.value.splice(index, 1, summary)
  else runs.value.push(summary)
  runs.value.sort((left, right) => right.updatedAtMs - left.updatedAtMs || left.name.localeCompare(right.name))
}

function selectRun(id: string) {
  selectedRunId.value = id
  if (id) localStorage.setItem('glyphshift.probe.selectedRun', id)
  else localStorage.removeItem('glyphshift.probe.selectedRun')
}

function clearMessage() {
  message.value = ''
}

export function useProbeRuns() {
  const selectedRun = computed(() => runs.value.find(item => item.id === selectedRunId.value) ?? null)

  async function connect() {
    if (connected) return
    connected = true
    message.value = ''
    if (hasDesktopRuntime()) {
      try {
        runs.value = await invoke<ProbeRunSummary[]>('desktop_probe_runs')
      }
      catch (error) {
        message.value = translateCommandError(error)
      }
    }
    if (!runs.value.some(item => item.id === selectedRunId.value)) selectRun('')
  }

  async function compatibleAdapters(softwareId: string) {
    if (!hasDesktopRuntime()) return null
    try {
      return await invoke<string[]>('desktop_compatible_probe_adapters', { softwareId })
    }
    catch (error) {
      message.value = translateCommandError(error)
      return []
    }
  }

  async function createFromSources(input: ProbeCreationInput, fallback: ProbeCreationBrowserFallback) {
    if (busy.value) return null
    busy.value = true
    message.value = ''
    try {
      const now = Date.now()
      const ownsSoftware = input.target.kind === 'active_process' && !fallback.softwareId
      const ownsDictionary = input.dictionary.kind === 'temporary'
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
            quickProbe: ownsSoftware || ownsDictionary,
          }
      upsert(summary)
      selectRun(summary.id)
      return summary
    }
    catch (error) {
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function retainQuickProbe(runId: string) {
    if (busy.value) return null
    const current = runs.value.find(run => run.id === runId)
    if (!current) return null
    busy.value = true
    message.value = ''
    try {
      const summary = hasDesktopRuntime()
        ? await invoke<ProbeRunSummary>('desktop_retain_quick_probe', { runId })
        : { ...current, quickProbe: false, updatedAtMs: Date.now() }
      upsert(summary)
      return summary
    }
    catch (error) {
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function cleanupQuickProbe(runId: string) {
    if (busy.value) return null
    busy.value = true
    message.value = ''
    try {
      const result = hasDesktopRuntime()
        ? await invoke<QuickProbeCleanupResult>('desktop_cleanup_quick_probe', { runId })
        : { software: 'reused' as const, dictionary: 'removed' as const }
      runs.value = runs.value.filter(run => run.id !== runId)
      if (selectedRunId.value === runId) selectRun('')
      return result
    }
    catch (error) {
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function remove(runIds: string[]) {
    if (busy.value || !runIds.length) return false
    busy.value = true
    message.value = ''
    try {
      if (hasDesktopRuntime()) await invoke('desktop_delete_probe_runs', { runIds })
      runs.value = runs.value.filter(run => !runIds.includes(run.id))
      if (runIds.includes(selectedRunId.value)) selectRun('')
      return true
    }
    catch (error) {
      message.value = translateCommandError(error)
      return false
    }
    finally {
      busy.value = false
    }
  }

  async function update(input: ProbeRunUpdateInput) {
    if (busy.value) return null
    busy.value = true
    message.value = ''
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
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function clearEntries(runId: string) {
    if (busy.value) return null
    busy.value = true
    message.value = ''
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
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
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

  async function runSummaryCommand(command: string, args: Record<string, unknown>) {
    if (busy.value || !hasDesktopRuntime()) return null
    busy.value = true
    message.value = ''
    try {
      const summary = await invoke<ProbeRunSummary>(command, args)
      upsert(summary)
      return summary
    }
    catch (error) {
      message.value = translateCommandError(error)
      return null
    }
    finally {
      busy.value = false
    }
  }

  async function refreshSummary(runId: string) {
    if (polling.value || !hasDesktopRuntime()) return selectedRun.value
    polling.value = true
    try {
      const summary = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
      upsert(summary)
      return summary
    }
    catch (error) {
      message.value = translateCommandError(error)
      return null
    }
    finally {
      polling.value = false
    }
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
    return invoke<ProbeEntryPage>('desktop_probe_run_entries', { request: input })
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
    message.value = ''
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
      message.value = translateCommandError(error)
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
      message.value = translateCommandError(error)
      return false
    }
  }

  function report(error: unknown) {
    message.value = translateCommandError(error)
  }

  return {
    runs,
    selectedRunId,
    selectedRun,
    busy,
    polling,
    message,
    connect,
    clearMessage,
    selectRun,
    compatibleAdapters,
    createFromSources,
    retainQuickProbe,
    cleanupQuickProbe,
    remove,
    update,
    clearEntries,
    resume,
    setPaused,
    disconnect,
    refreshSummary,
    queryEntries,
    editTranslation,
    syncDictionaryEntries,
    bulk,
    exportRun,
    report,
  }
}
