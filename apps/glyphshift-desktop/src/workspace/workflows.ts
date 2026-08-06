import { computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { presentationError } from '../commandError'
import { i18n } from '../i18n'
import type {
  DesktopSnapshot,
  WorkflowCommandResult,
  WorkflowDetail,
  WorkflowRuntimeStatus,
  WorkflowTarget,
} from '../model'
import {
  applyDesktopSnapshot,
  clone,
  errorMessage,
  fontRefreshing,
  hasDesktopRuntime,
  model,
  refreshing,
  setMessage,
  workflowDetail,
  workspaceBusy,
} from './state'

export function useWorkflowWorkspace() {
  const activationIds = computed(() => new Set(model.value.activations.map(item => item.workflowId)))

  function applyWorkflowResult(result: WorkflowCommandResult) {
    const activation = model.value.activations.filter(item => item.workflowId !== result.activation.workflowId)
    if (result.activation.enabled) activation.push({
      workflowId: result.activation.workflowId,
      revision: result.definition.revision,
    })
    model.value.activations = activation.sort((left, right) => left.workflowId.localeCompare(right.workflowId))
    model.value.workflowRuntimeStatus = {
      ...model.value.workflowRuntimeStatus,
      [result.runtime.workflowId]: result.runtime,
    }
  }

  async function setWorkflowEnabled(id: string, enabled: boolean, replaceConflicts = false) {
    if (workspaceBusy.value) return false
    workspaceBusy.value = true
    setMessage(id, '')
    try {
      if (hasDesktopRuntime()) {
        const command = enabled ? 'desktop_enable_workflow' : 'desktop_disable_workflow'
        const args = enabled ? { workflowId: id, replaceConflicts } : { workflowId: id }
        applyWorkflowResult(await invoke<WorkflowCommandResult>(command, args))
      }
      else {
        const workflow = model.value.workflows.find(item => item.id === id)
        if (!workflow) return false
        const activations = model.value.activations.filter(item => item.workflowId !== id)
        if (enabled) activations.push({ workflowId: id, revision: workflow.revision })
        model.value.activations = activations
        model.value.workflowRuntimeStatus[id] = {
          workflowId: id,
          errors: {},
          targets: workflow.targets.map(target => ({
            softwareId: target.softwareId,
            discovered: false,
            active: false,
            translationRequested: enabled && target.dictionaryIds.length > 0,
            fontRequested: enabled && Boolean(target.fontPolicy),
            translationActive: false,
            fontActive: false,
            appliedGeneration: null,
          })),
        }
      }
      return true
    }
    catch (error) {
      setMessage(id, errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  async function refreshWorkflows() {
    if (refreshing.value) return false
    refreshing.value = true
    try {
      if (hasDesktopRuntime()) applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_refresh_workflows'))
      return true
    }
    catch (error) {
      setMessage('workflows', errorMessage(error))
      return false
    }
    finally {
      refreshing.value = false
    }
  }

  async function refreshFontFamilies() {
    if (fontRefreshing.value) return false
    fontRefreshing.value = true
    setMessage('workflows', '')
    try {
      if (hasDesktopRuntime()) {
        applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_refresh_font_families'))
      }
      return true
    }
    catch (error) {
      setMessage('workflows', errorMessage(error))
      return false
    }
    finally {
      fontRefreshing.value = false
    }
  }

  async function loadWorkflow(id: string) {
    try {
      workflowDetail.value = hasDesktopRuntime()
        ? await invoke<WorkflowDetail>('desktop_workflow', { workflowId: id })
        : model.value.workflowDetails[id] ?? null
      return workflowDetail.value
    }
    catch (error) {
      setMessage(id, errorMessage(error))
      return null
    }
  }

  async function saveWorkflow(detail: WorkflowDetail) {
    workspaceBusy.value = true
    try {
      const snapshot = hasDesktopRuntime()
        ? await invoke<DesktopSnapshot>('desktop_update_workflow', { edit: {
            id: detail.id,
            name: detail.name,
            description: detail.description,
            baseRevision: detail.revision,
            targets: detail.targets,
          } })
        : localSaveWorkflow(detail)
      applyDesktopSnapshot(snapshot)
      await loadWorkflow(detail.id)
      return true
    }
    catch (error) {
      setMessage(detail.id, errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  async function createWorkflow(name: string, description: string, targets: WorkflowTarget[]) {
    if (workspaceBusy.value) return false
    workspaceBusy.value = true
    setMessage('workflows', '')
    const id = `workflow-${crypto.randomUUID()}`
    try {
      const snapshot = hasDesktopRuntime()
        ? await invoke<DesktopSnapshot>('desktop_create_workflow', { create: { id, name, description, targets } })
        : localCreateWorkflow({ id, name, description, revision: 1, targets })
      applyDesktopSnapshot(snapshot)
      return true
    }
    catch (error) {
      setMessage('workflows', errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  function localCreateWorkflow(detail: WorkflowDetail): DesktopSnapshot {
    model.value.workflowDetails[detail.id] = detail
    model.value.workflows = [...model.value.workflows, {
      ...detail,
      softwareIds: detail.targets.map(target => target.softwareId),
      dictionaryIds: [...new Set(detail.targets.flatMap(target => target.dictionaryIds))].sort(),
      targets: clone(detail.targets),
    }].sort((left, right) => left.id.localeCompare(right.id))
    model.value.workflowRuntimeStatus[detail.id] = {
      workflowId: detail.id,
      targets: detail.targets.map(target => ({
        softwareId: target.softwareId,
        discovered: false,
        active: false,
        translationRequested: false,
        fontRequested: false,
        translationActive: false,
        fontActive: false,
        appliedGeneration: null,
      })),
      errors: {},
    }
    return model.value
  }

  async function copyWorkflow(sourceId: string) {
    if (workspaceBusy.value) return false
    const source = model.value.workflows.find(item => item.id === sourceId)
    if (!source) return false
    workspaceBusy.value = true
    setMessage('workflows', '')
    const id = `workflow-${crypto.randomUUID()}`
    const name = `${source.name} ${i18n.global.t('workspace.copySuffix')}`
    try {
      const snapshot = hasDesktopRuntime()
        ? await invoke<DesktopSnapshot>('desktop_copy_workflow', { sourceWorkflowId: sourceId, newWorkflowId: id, name })
        : localCopyWorkflow(sourceId, id, name)
      applyDesktopSnapshot(snapshot)
      return true
    }
    catch (error) {
      setMessage('workflows', errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  function localCopyWorkflow(sourceId: string, id: string, name: string): DesktopSnapshot {
    const source = model.value.workflowDetails[sourceId]
    if (!source) throw presentationError(i18n.global.t('workspace.missingCopySource'))
    return localCreateWorkflow({ ...clone(source), id, name, revision: 1 })
  }

  async function removeWorkflows(ids: string[]) {
    if (workspaceBusy.value || !ids.length) return false
    workspaceBusy.value = true
    setMessage('workflows', '')
    try {
      if (hasDesktopRuntime()) {
        applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_delete_workflows', { workflowIds: ids }))
      }
      else {
        const enabled = new Set(model.value.activations.map(item => item.workflowId))
        if (ids.some(id => enabled.has(id))) throw presentationError(i18n.global.t('workspace.disableBeforeDelete'))
        const removed = new Set(ids)
        model.value.workflows = model.value.workflows.filter(item => !removed.has(item.id))
        for (const id of ids) {
          delete model.value.workflowDetails[id]
          delete model.value.workflowRuntimeStatus[id]
        }
      }
      return true
    }
    catch (error) {
      setMessage('workflows', errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  async function setWorkflowsEnabled(ids: string[], enabled: boolean) {
    let succeeded = true
    for (const id of ids) if (!await setWorkflowEnabled(id, enabled)) succeeded = false
    return succeeded
  }

  function localSaveWorkflow(detail: WorkflowDetail): DesktopSnapshot {
    const next = { ...clone(detail), revision: detail.revision + 1 }
    model.value.workflowDetails[next.id] = next
    model.value.workflows = model.value.workflows.map(item => item.id === next.id ? {
      ...next,
      softwareIds: next.targets.map(target => target.softwareId),
      dictionaryIds: [...new Set(next.targets.flatMap(target => target.dictionaryIds))].sort(),
    } : item)
    return model.value
  }

  function runtimeStatus(id: string): WorkflowRuntimeStatus | undefined {
    return model.value.workflowRuntimeStatus[id]
  }

  return {
    activationIds,
    setWorkflowEnabled,
    refreshWorkflows,
    refreshFontFamilies,
    loadWorkflow,
    createWorkflow,
    copyWorkflow,
    removeWorkflows,
    setWorkflowsEnabled,
    saveWorkflow,
    runtimeStatus,
  }
}
