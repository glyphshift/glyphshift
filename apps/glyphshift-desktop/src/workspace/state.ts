import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { translateCommandError } from '../commandError'
import {
  emptyModel,
  STORAGE_KEY,
  type DesktopModel,
  type DesktopSnapshot,
  type DictionaryCatalogPage,
  type DictionaryDetail,
  type DictionaryInstallationSummary,
  type SoftwarePreflight,
  type WorkflowDetail,
} from '../model'

export function hasDesktopRuntime() {
  return '__TAURI_INTERNALS__' in window
}

export function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

export function errorMessage(error: unknown) {
  return translateCommandError(error)
}

export function unmanagedInstallation(): DictionaryInstallationSummary {
  return {
    state: 'unmanaged',
    installedRelease: null,
    verifiedPublisher: null,
    updateRelease: null,
  }
}

function normalizeDictionaryInstallations(model: DesktopModel): DesktopModel {
  return {
    ...model,
    dictionaries: model.dictionaries.map(dictionary => ({
      ...dictionary,
      installation: dictionary.installation ?? unmanagedInstallation(),
    })),
  }
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
    return normalizeDictionaryInstallations({ ...emptyModel(), ...value })
  }
  catch {
    return emptyModel()
  }
}

export const model = ref<DesktopModel>(readModel())
export const softwareBusy = ref(false)
export const workspaceBusy = ref(false)
export const refreshing = ref(false)
export const fontRefreshing = ref(false)
export const messages = ref<Record<string, string>>({})
export const dictionaryDetail = ref<DictionaryDetail | null>(null)
export const workflowDetail = ref<WorkflowDetail | null>(null)
export const dictionaryCatalog = ref<DictionaryCatalogPage>({ releases: [], nextCursor: null })
export const dictionaryCatalogBusy = ref(false)
export const dictionaryCatalogError = ref('')
export const softwarePreflight = ref<SoftwarePreflight | null>(null)
export const softwarePreflightBusy = ref(false)
export const softwareCaptureArmed = ref(false)
export const softwareCaptureShortcut = ref('Ctrl+Shift+F8')
export const softwareCaptureResult = ref<SoftwarePreflight | null>(null)

watch(model, (value) => {
  if (!hasDesktopRuntime()) localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
}, { deep: true })

export function applyDesktopSnapshot(snapshot: DesktopSnapshot) {
  model.value = normalizeDictionaryInstallations({ ...model.value, ...snapshot })
}

export function setMessage(id: string, message: string) {
  messages.value = { ...messages.value, [id]: message }
}

export async function connectDesktopBackend() {
  if (!hasDesktopRuntime()) return false
  applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_snapshot'))
  return true
}
