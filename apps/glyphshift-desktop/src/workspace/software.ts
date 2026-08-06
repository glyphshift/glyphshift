import { invoke } from '@tauri-apps/api/core'
import { presentationError, translateCommandCode } from '../commandError'
import { i18n } from '../i18n'
import type {
  DesktopSnapshot,
  SoftwarePreflight,
  SoftwareQuickCaptureEvent,
  SoftwareRecord,
} from '../model'
import {
  applyDesktopSnapshot,
  connectDesktopBackend,
  errorMessage,
  hasDesktopRuntime,
  model,
  setMessage,
  softwareBusy,
  softwareCaptureArmed,
  softwareCaptureResult,
  softwareCaptureShortcut,
  softwarePreflight,
  softwarePreflightBusy,
} from './state'

export function useSoftwareWorkspace() {
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

  function browserSoftwarePreflight(executablePath: string): SoftwarePreflight {
    const path = executablePath.trim()
    const executableName = path.split(/[\\/]/).pop() || path
    const existing = model.value.software.find(item =>
      item.executablePath?.toLocaleLowerCase() === path.toLocaleLowerCase())
    return {
      executablePath: path,
      executableName,
      suggestedName: executableName.replace(/\.exe$/i, ''),
      architecture: 'x86_64',
      running: true,
      canAdd: !existing,
      state: existing ? 'already_added' : 'ready',
      existingName: existing?.name ?? null,
    }
  }

  function clearSoftwarePreflight() {
    softwarePreflight.value = null
    softwareCaptureResult.value = null
    setMessage('software', '')
  }

  async function validateSoftware(executablePath: string) {
    const path = executablePath.trim()
    if (!path || softwarePreflightBusy.value) return false
    softwarePreflightBusy.value = true
    setMessage('software', '')
    try {
      softwarePreflight.value = hasDesktopRuntime()
        ? await invoke<SoftwarePreflight>('desktop_preflight_software', { executablePath: path })
        : browserSoftwarePreflight(path)
      return true
    }
    catch (error) {
      softwarePreflight.value = null
      setMessage('software', errorMessage(error))
      return false
    }
    finally {
      softwarePreflightBusy.value = false
    }
  }

  async function armSoftwareCapture() {
    setMessage('software', '')
    try {
      if (hasDesktopRuntime()) {
        softwareCaptureShortcut.value = await invoke<string>('desktop_arm_software_capture')
      }
      softwareCaptureArmed.value = true
      return true
    }
    catch (error) {
      setMessage('software', errorMessage(error))
      return false
    }
  }

  async function cancelSoftwareCapture() {
    try {
      if (hasDesktopRuntime()) await invoke('desktop_cancel_software_capture')
    }
    catch (error) {
      setMessage('software', errorMessage(error))
    }
    finally {
      softwareCaptureArmed.value = false
    }
  }

  function handleSoftwareQuickCaptureEvent(event: SoftwareQuickCaptureEvent) {
    softwareCaptureShortcut.value = event.shortcut
    if (event.state === 'armed') {
      softwareCaptureArmed.value = true
      softwareCaptureResult.value = null
      setMessage('software', '')
      return
    }
    if (event.state === 'captured') {
      softwareCaptureArmed.value = false
      softwarePreflight.value = event.preflight
      softwareCaptureResult.value = event.preflight
      setMessage('software', '')
      return
    }
    softwareCaptureArmed.value = true
    setMessage('software', translateCommandCode(event.errorCode))
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
      clearSoftwarePreflight()
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
      setMessage('software', errorMessage(error))
      return false
    }
  }

  async function removeSoftware(ids: string[]) {
    if (softwareBusy.value || !ids.length) return false
    softwareBusy.value = true
    setMessage('software', '')
    let succeeded = true
    for (const id of ids) {
      try {
        if (hasDesktopRuntime()) applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_remove_software', { extensionId: id }))
        else model.value.software = model.value.software.filter(item => item.id !== id)
      }
      catch (error) {
        succeeded = false
        setMessage('software', errorMessage(error))
      }
    }
    softwareBusy.value = false
    return succeeded
  }

  return {
    selectSoftware,
    validateSoftware,
    clearSoftwarePreflight,
    armSoftwareCapture,
    cancelSoftwareCapture,
    handleSoftwareQuickCaptureEvent,
    addSoftware,
    updateSoftware,
    removeSoftware,
  }
}
