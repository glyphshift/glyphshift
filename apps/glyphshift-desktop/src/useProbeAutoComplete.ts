import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAiTranslation } from './useAiTranslation'
import { useAppSettings } from './appSettings'
import type { ProbeRunSummary } from './model'

interface AutoRun { profileId: string; nextCheck: number; jobId?: string; checkedContent?: string }
const runs = ref<Record<string, AutoRun>>({})
const stopped = ref<Record<string, boolean>>({})
let timer: ReturnType<typeof setTimeout> | undefined
let checking = false

// Session-only: navigation keeps automation alive; restarting the app never starts paid work.
export function useProbeAutoComplete() {
  const ai = useAiTranslation()
  const settings = useAppSettings()
  function stop(runId: string) { delete runs.value[runId] }
  async function tick() {
    if (checking) return
    checking = true
    try {
      await ai.connectTaskMonitor()
      for (const [runId, run] of Object.entries(runs.value)) {
        const current = () => runs.value[runId] === run
        try {
          const summary = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
          if (!current()) continue
          if (summary.status !== 'running' || !ai.profiles.value.some(profile => profile.id === run.profileId)) { stop(runId); continue }
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
          const interval = settings.settings.value.autoCompleteIntervalSeconds
          if (ai.taskRunning.value || ai.busy.value || (interval > 0 && Date.now() < run.nextCheck)) continue
          const content = JSON.stringify([settings.settings.value.textFilterPolicy, summary.observedCount, summary.ignoredCount, summary.dictionaryRevision, summary.exclusionRevisions])
          if (run.checkedContent === content) continue
          run.nextCheck = Date.now() + interval * 1000
          const plan = await ai.planProbe(runId, run.profileId)
          if (!current()) continue
          if (!plan.candidates.length) { run.checkedContent = content; continue }
          if (ai.taskRunning.value || ai.busy.value) continue
          run.checkedContent = undefined
          const latest = await invoke<ProbeRunSummary>('desktop_probe_run_summary', { runId })
          if (!current() || latest.status !== 'running') { if (current()) stop(runId); continue }
          const task = await ai.startBackgroundPlan(plan, run.profileId)
          if (current() && task.scopeId === `probe:${runId}`) run.jobId = task.jobId
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
    if (!timer && !checking && Object.keys(runs.value).length) timer = setTimeout(() => void tick(), settings.settings.value.autoCompleteIntervalSeconds === 0 ? 250 : 1000)
  }
  function start(runId: string, profileId: string) {
    stopped.value[runId] = false
    runs.value[runId] = { profileId, nextCheck: 0 }
    if (settings.settings.value.autoCompleteIntervalSeconds === 0 && !checking) {
      if (timer) clearTimeout(timer)
      timer = undefined
      void tick()
    } else schedule()
  }
  return { runs, stopped, start, stop }
}
