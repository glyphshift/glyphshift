import { computed, ref } from 'vue'
import type { WorkflowLifecycle, WorkflowRuntimeStatus } from './model'
import { model } from './workspace/state'

export const workflowOperations = ref<Record<string, 'connecting' | 'stopping' | 'unknown' | undefined>>({})
export function projectLifecycle(runtime: WorkflowRuntimeStatus | undefined, enabled: boolean): WorkflowLifecycle {
  if (runtime?.lifecycle) return runtime.lifecycle
  const active = runtime?.targets.some(target => target.active) ?? false
  const failed = Object.values(runtime?.errors ?? {}).some(error => error.code !== 'runtime.target_not_found')
  return { phase: enabled ? (failed ? 'failed' : active ? 'running' : 'waiting') : (active || failed ? 'stop_failed' : 'stopped'), enabled, collectNewSources: true, checkedAtMs: runtime?.checkedAtMs ?? 0, revision: runtime?.revision ?? 0 }
}
export function workflowLifecycle(id: string): WorkflowLifecycle {
  const state = projectLifecycle(model.value.workflowRuntimeStatus[id], model.value.activations.some(item => item.workflowId === id))
  const operation = workflowOperations.value[id]
  return operation ? { ...state, phase: operation } : state
}
export const workflowActivity = computed(() => {
  const counts: Partial<Record<WorkflowLifecycle['phase'], number>> = {}
  for (const workflow of model.value.workflows) {
    const phase = workflowLifecycle(workflow.id).phase
    if (phase !== 'stopped') counts[phase] = (counts[phase] ?? 0) + 1
  }
  return Object.entries(counts) as [WorkflowLifecycle['phase'], number][]
})

export function workflowStateColor(phase: WorkflowLifecycle['phase']): 'success' | 'error' | 'neutral' | 'warning' {
  return phase === 'running' ? 'success' : ['failed', 'stop_failed'].includes(phase) ? 'error' : phase === 'stopped' ? 'neutral' : 'warning'
}
