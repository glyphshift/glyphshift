import { ref } from 'vue'
import { applyWorkflowRuntime } from './workspace/state'
import { invoke } from '@tauri-apps/api/core'
import { useAiTranslation } from './useAiTranslation'
import { useAppSettings } from './appSettings'
import type { ProbeRunSummary } from './model'

interface AutoRun { workflowId?: string; profileId: string; nextCheck: number; jobId?: string; checkedContent?: string; pendingContent?: string }
const runs = ref<Record<string, AutoRun>>({})
const stopped = ref<Record<string, boolean>>({})
const queued = ref<string[]>([])
const MIN_CHECK_MS = 250
let timer: ReturnType<typeof setTimeout> | undefined
let checking = false
let listening = false

// Session-only: navigation keeps automation alive; restarting the app never starts paid work.
export function useProbeAutoComplete() {
  const ai = useAiTranslation()
  const settings = useAppSettings()
  const intervalMs = () => Math.max(MIN_CHECK_MS, settings.settings.value.autoCompleteIntervalSeconds * 1000)
  function enqueue(runId: string) { if (!queued.value.includes(runId)) queued.value.push(runId) }
  function stop(runId: string) {
    delete runs.value[runId]
    queued.value = queued.value.filter(id => id !== runId)
  }
  async function stopWorkflow(workflowId: string) {
    for (const [runId, run] of Object.entries(runs.value)) {
      if (run.workflowId !== workflowId) continue
      stop(runId)
      if (run.jobId) {
        try { await invoke('desktop_cancel_ai_translation', { jobId: run.jobId }) }
        catch { stopped.value[runId] = true }
      }
    }
  }
  if (!listening) {
    listening = true
    window.addEventListener('glyphshift:workflow-stop', event => { void stopWorkflow((event as CustomEvent<string>).detail) })
  }
  function isRunning(summary: ProbeRunSummary) {
    return summary.workflowRuntime?.lifecycle ? summary.workflowRuntime.lifecycle.phase === 'running' : summary.status === 'running'
  }
  async function tick() {
    if (checking) return
    checking = true
    try {
      await ai.connectTaskMonitor()
      // Coalesce changes while another task is running; never retain stale translation plans.
      for (const [runId, run] of Object.entries(runs.value)) {
        const current = () => runs.value[runId] === run
        try {
          const summary = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
          if (!current()) continue
          if (summary.workflowRuntime) applyWorkflowRuntime(summary.workflowRuntime)
          run.workflowId ??= summary.workflowId ?? undefined
          if (summary.workflowRuntime?.lifecycle?.enabled === false && run.workflowId) { await stopWorkflow(run.workflowId); continue }
          if (!ai.profiles.value.some(profile => profile.id === run.profileId)) { stop(runId); continue }
          if (!isRunning(summary)) {
            queued.value = queued.value.filter(id => id !== runId)
            // Keep the current round and the selection while the target is closed.
            if (!summary.workflowId) stop(runId)
            continue
          }
          const job = ai.taskCenter.value.current
          if (run.jobId && job?.jobId === run.jobId && ['interrupted', 'cancelled', 'completed_with_failures', 'cancelling'].includes(job.status)) {
            stopped.value[runId] = true
            stop(runId)
            continue
          }
          if (run.jobId) {
            if (!job || job.jobId !== run.jobId || job.writebackError) { stopped.value[runId] = true; stop(runId); continue }
            if (job.status === 'completed') run.jobId = undefined
          }
          if (Date.now() < run.nextCheck) continue
          run.nextCheck = Date.now() + intervalMs()
          const content = JSON.stringify([settings.settings.value.textFilterPolicy, summary.observedCount, summary.ignoredCount, summary.dictionaryRevision, summary.exclusionRevisions])
          if (run.checkedContent === content) continue
          run.pendingContent = content
          enqueue(runId)
        } catch {
          if (current()) { stopped.value[runId] = true; stop(runId) }
        }
      }
      // One global task owns writeback; its batches use the selected profile's concurrency.
      while (queued.value.length && !ai.taskRunning.value && !ai.busy.value) {
        const runId = queued.value.shift()!
        const run = runs.value[runId]
        if (!run) continue
        const current = () => runs.value[runId] === run
        try {
          const content = run.pendingContent
          const plan = await ai.planProbe(runId, run.profileId)
          if (!current()) continue
          if (!plan.candidates.length) { run.checkedContent = content; continue }
          if (ai.taskRunning.value || ai.busy.value) { enqueue(runId); break }
          const latest = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
          if (!current() || !isRunning(latest)) { if (current() && !latest.workflowId) stop(runId); continue }
          const task = await ai.startBackgroundPlan(plan, run.profileId)
          if (!current()) {
            if (task.scopeId === `probe:${runId}`) await invoke('desktop_cancel_ai_translation', { jobId: task.jobId })
            continue
          }
          if (current()) {
            run.checkedContent = undefined
            if (task.scopeId === `probe:${runId}`) run.jobId = task.jobId
            else enqueue(runId)
          }
        } catch {
          if (current()) { stopped.value[runId] = true; stop(runId) }
        }
      }
    } catch {
      for (const id of Object.keys(runs.value)) { stopped.value[id] = true; stop(id) }
    } finally {
      checking = false
      timer = undefined
      schedule()
    }
  }
  function schedule() {
    if (!timer && !checking && Object.keys(runs.value).length) timer = setTimeout(() => void tick(), Math.min(intervalMs(), 1000))
  }
  function start(runId: string, profileId: string, workflowId?: string) {
    stop(runId)
    stopped.value[runId] = false
    runs.value[runId] = { profileId, workflowId, nextCheck: 0 }
    if (settings.settings.value.autoCompleteIntervalSeconds === 0 && !checking) {
      if (timer) clearTimeout(timer)
      timer = undefined
      void tick()
    } else schedule()
  }
  return { runs, stopped, queued, start, stop }
}
