import { invoke } from '@tauri-apps/api/core'
import { presentationError } from '../commandError'
import { i18n } from '../i18n'
import type {
  DesktopSnapshot,
  DictionaryCatalogInstallRequest,
  DictionaryCatalogPage,
  DictionaryCatalogQueryRequest,
  DictionaryDetail,
  DictionaryMetadata,
} from '../model'
import {
  applyDesktopSnapshot,
  clone,
  dictionaryCatalog,
  dictionaryCatalogBusy,
  dictionaryCatalogError,
  dictionaryDetail,
  errorMessage,
  hasDesktopRuntime,
  model,
  setMessage,
  unmanagedInstallation,
  workspaceBusy,
} from './state'

export function useDictionaryWorkspace() {
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
          installation: unmanagedInstallation(),
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

  async function importDictionary(inputPath: string) {
    if (workspaceBusy.value || !hasDesktopRuntime()) return false
    workspaceBusy.value = true
    setMessage('dictionaries', '')
    try {
      applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_import_dictionary', { inputPath }))
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

  async function exportDictionary(dictionaryId: string, outputPath: string) {
    if (workspaceBusy.value || !hasDesktopRuntime()) return false
    workspaceBusy.value = true
    setMessage('dictionaries', '')
    try {
      await invoke('desktop_export_dictionary', { dictionaryId, outputPath })
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

  async function queryDictionaryCatalog(request: DictionaryCatalogQueryRequest) {
    if (dictionaryCatalogBusy.value) return false
    dictionaryCatalogBusy.value = true
    dictionaryCatalogError.value = ''
    try {
      if (!hasDesktopRuntime()) {
        dictionaryCatalog.value = { releases: [], nextCursor: null }
        dictionaryCatalogError.value = i18n.global.t('errors.dictionary.catalogUnavailable')
        return false
      }
      dictionaryCatalog.value = await invoke<DictionaryCatalogPage>('desktop_query_dictionary_catalog', { request })
      return true
    }
    catch (error) {
      dictionaryCatalog.value = { releases: [], nextCursor: null }
      dictionaryCatalogError.value = errorMessage(error)
      return false
    }
    finally {
      dictionaryCatalogBusy.value = false
    }
  }

  async function installDictionaryRelease(request: DictionaryCatalogInstallRequest) {
    if (dictionaryCatalogBusy.value || workspaceBusy.value) return false
    dictionaryCatalogBusy.value = true
    workspaceBusy.value = true
    dictionaryCatalogError.value = ''
    try {
      if (!hasDesktopRuntime()) {
        dictionaryCatalogError.value = i18n.global.t('errors.dictionary.catalogUnavailable')
        return false
      }
      applyDesktopSnapshot(await invoke<DesktopSnapshot>('desktop_install_dictionary_release', { request }))
      return true
    }
    catch (error) {
      dictionaryCatalogError.value = errorMessage(error)
      return false
    }
    finally {
      dictionaryCatalogBusy.value = false
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
      installation: item.installation ?? unmanagedInstallation(),
    } : item)
    return model.value
  }

  return {
    loadDictionary,
    saveDictionary,
    createDictionary,
    importDictionary,
    exportDictionary,
    queryDictionaryCatalog,
    installDictionaryRelease,
    removeDictionaries,
  }
}
