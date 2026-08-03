import { computed, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { presentationError, translateCommandError } from './commandError'
import { i18n } from './i18n'
import {
  emptyModel,
  STORAGE_KEY,
  type DesktopModel,
  type DesktopSnapshot,
  type DictionaryDetail,
  type DictionaryMetadata,
  type SoftwareRecord,
  type WorkflowCommandResult,
  type WorkflowDetail,
  type WorkflowRuntimeStatus,
  type WorkflowTarget,
} from './model'

function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function errorMessage(error: unknown) {
  return translateCommandError(error)
}

function readModel(): DesktopModel {
  const raw = localStorage.getItem(STORAGE_KEY)
  if (!raw) return emptyModel()
  try {
    const value = JSON.parse(raw) as DesktopModel
    if (!Array.isArray(value.software)
      || !Array.isArray(value.workflows)
      || !Array.isArray(value.dictionaries)
      || !Array.isArray(value.adapters)) return emptyModel()
    return { ...emptyModel(), ...value }
  }
  catch {
    return emptyModel()
  }
}

const model = ref<DesktopModel>(readModel())
const softwareBusy = ref(false)
const workspaceBusy = ref(false)
const refreshing = ref(false)
const messages = ref<Record<string, string>>({})
const dictionaryDetail = ref<DictionaryDetail | null>(null)
const workflowDetail = ref<WorkflowDetail | null>(null)

watch(model, (value) => {
  if (!hasDesktopRuntime()) localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
}, { deep: true })

export function useWorkspace() {
  const activationIds = computed(() => new Set(model.value.activations.map(item => item.workflowId)))

  function applyDesktopSnapshot(snapshot: DesktopSnapshot) {
    model.value = { ...model.value, ...snapshot }
  }

  function setMessage(id: string, message: string) {
    messages.value = { ...messages.value, [id]: message }
  }

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

  async function connectDesktopBackend() {
    if (!hasDesktopRuntime()) return false
    applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_snapshot'))
    return true
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

  async function loadDictionary(id: string) {
    try {
      dictionaryDetail.value = hasDesktopRuntime()
        ? await invoke<DictionaryDetail>('desktop_dictionary', { dictionaryId: id })
        : model.value.dictionaryDetails[id] ?? null
      return dictionaryDetail.value
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

  async function saveDictionary(detail: DictionaryDetail) {
    workspaceBusy.value = true
    try {
      const snapshot = hasDesktopRuntime()
        ? await invoke<DesktopSnapshot>('desktop_update_dictionary', { edit: {
            metadata: detail.metadata,
            baseRevision: detail.revision,
            entries: detail.entries,
          } })
        : localSaveDictionary(detail)
      applyDesktopSnapshot(snapshot)
      await loadDictionary(detail.metadata.id)
      return true
    }
    catch (error) {
      setMessage(detail.metadata.id, errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  async function createDictionary(metadata: Omit<DictionaryMetadata, 'id'>) {
    if (workspaceBusy.value) return false
    workspaceBusy.value = true
    setMessage('dictionaries', '')
    const id = `dictionary-${crypto.randomUUID()}`
    const detail: DictionaryDetail = { metadata: { id, ...metadata }, revision: 1, entries: [] }
    try {
      if (hasDesktopRuntime()) {
        applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_create_dictionary', { create: {
          metadata: detail.metadata,
          entries: [],
        } }))
      }
      else {
        model.value.dictionaryDetails[id] = detail
        model.value.dictionaries = [...model.value.dictionaries, {
          metadata: detail.metadata,
          revision: 1,
          entryCount: 0,
        }].sort((left, right) => left.metadata.name.localeCompare(right.metadata.name))
      }
      return true
    }
    catch (error) {
      setMessage('dictionaries', errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  async function removeDictionaries(ids: string[]) {
    if (workspaceBusy.value || !ids.length) return false
    workspaceBusy.value = true
    setMessage('dictionaries', '')
    try {
      if (hasDesktopRuntime()) {
        applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_delete_dictionaries', { dictionaryIds: ids }))
      }
      else {
        if (ids.some(id => model.value.workflows.some(workflow => workflow.dictionaryIds.includes(id)))) {
          throw presentationError(i18n.global.t('workspace.dictionaryReferenced'))
        }
        const removed = new Set(ids)
        model.value.dictionaries = model.value.dictionaries.filter(item => !removed.has(item.metadata.id))
        for (const id of ids) delete model.value.dictionaryDetails[id]
      }
      return true
    }
    catch (error) {
      setMessage('dictionaries', errorMessage(error))
      return false
    }
    finally {
      workspaceBusy.value = false
    }
  }

  function localSaveDictionary(detail: DictionaryDetail): DesktopSnapshot {
    const next = { ...clone(detail), revision: detail.revision + 1 }
    model.value.dictionaryDetails[next.metadata.id] = next
    model.value.dictionaries = model.value.dictionaries.map(item => item.metadata.id === next.metadata.id ? {
      metadata: next.metadata,
      revision: next.revision,
      entryCount: next.entries.length,
    } : item)
    return model.value
  }

  async function selectSoftware(id: string) {
    model.value.selectedSoftwareId = id
    if (!hasDesktopRuntime()) return
    try {
      applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_select_software', { extensionId: id }))
    }
    catch {
      await connectDesktopBackend()
    }
  }

  async function addSoftware(displayName: string, description: string, executablePath: string) {
    setMessage('software', '')
    if (softwareBusy.value) return false
    const name = displayName.trim()
    const path = executablePath.trim()
    if (!name || !path) return false
    softwareBusy.value = true
    try {
      if (hasDesktopRuntime()) {
        const existingIds = new Set(model.value.software.map(item => item.id))
        const added = await invoke<DesktopSnapshot>('desktop_add_software', { executablePath: path })
        const created = added.software.find(item => !existingIds.has(item.id))
        applyDesktopSnapshot(added)
        if (!created) throw presentationError(i18n.global.t('workspace.softwareResultMissing'))
        if (created.name !== name || description.trim()) {
          applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_update_software', {
            extensionId: created.id,
            displayName: name,
            description: description.trim(),
            executablePath: path,
          }))
        }
      }
      else {
        const executableName = path.split(/[\\/]/).pop() || path
        const record: SoftwareRecord = {
          id: `software-${crypto.randomUUID()}`,
          name,
          description: description.trim(),
          vendor: 'Glyphshift',
          version: '—',
          executableName,
          executablePath: path,
          monogram: name.slice(0, 2).toLocaleUpperCase(),
          lastUsed: null,
          locale: 'zh-CN',
          connected: false,
          translation: { state: 'unavailable', enabled: false, coverage: 0, detail: 'capability.text-unavailable', generation: null },
          font: { state: 'unavailable', enabled: false, coverage: 0, detail: 'capability.font-unavailable', generation: null },
          observe: { state: 'unavailable', enabled: false, coverage: 0, detail: 'capability.runtime-unavailable', generation: null },
        }
        model.value.software = [...model.value.software, record].sort((left, right) => left.name.localeCompare(right.name))
      }
      return true
    }
    catch (error) {
      setMessage('software', errorMessage(error))
      return false
    }
    finally {
      softwareBusy.value = false
    }
  }

  async function updateSoftware(id: string, displayName: string, description: string, executablePath: string) {
    const software = model.value.software.find(item => item.id === id)
    if (!software) return false
    try {
      if (hasDesktopRuntime()) {
        applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_update_software', {
          extensionId: id,
          displayName: displayName.trim(),
          description: description.trim(),
          executablePath: executablePath.trim(),
        }))
      }
      else {
        software.name = displayName.trim()
        software.description = description.trim()
        software.executablePath = executablePath.trim()
        software.executableName = executablePath.split(/[\\/]/).pop() || software.executableName
      }
      return true
    }
    catch (error) {
      setMessage(id, errorMessage(error))
      return false
    }
  }

  async function removeSoftware(ids: string[]) {
    for (const id of ids) {
      try {
        if (hasDesktopRuntime()) applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_remove_software', { extensionId: id }))
        else model.value.software = model.value.software.filter(item => item.id !== id)
      }
      catch (error) {
        setMessage(id, errorMessage(error))
      }
    }
  }

  function runtimeStatus(id: string): WorkflowRuntimeStatus | undefined {
    return model.value.workflowRuntimeStatus[id]
  }

  return {
    model,
    activationIds,
    dictionaryDetail,
    workflowDetail,
    softwareBusy,
    workspaceBusy,
    refreshing,
    messages,
    connectDesktopBackend,
    setWorkflowEnabled,
    refreshWorkflows,
    loadWorkflow,
    loadDictionary,
    createWorkflow,
    copyWorkflow,
    removeWorkflows,
    setWorkflowsEnabled,
    saveWorkflow,
    saveDictionary,
    createDictionary,
    removeDictionaries,
    selectSoftware,
    addSoftware,
    updateSoftware,
    removeSoftware,
    runtimeStatus,
  }
}
