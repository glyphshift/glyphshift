import { useDictionaryWorkspace } from './workspace/dictionaries'
import { useSoftwareWorkspace } from './workspace/software'
import {
  connectDesktopBackend,
  dictionaryCatalog,
  dictionaryCatalogBusy,
  dictionaryCatalogError,
  dictionaryDetail,
  fontRefreshing,
  messages,
  model,
  refreshing,
  softwareBusy,
  softwareCaptureArmed,
  softwareCaptureResult,
  softwareCaptureShortcut,
  softwarePreflight,
  softwarePreflightBusy,
  workflowDetail,
  workspaceBusy,
} from './workspace/state'
import { useWorkflowWorkspace } from './workspace/workflows'

export function useWorkspace() {
  const workflows = useWorkflowWorkspace()
  const dictionaries = useDictionaryWorkspace()
  const software = useSoftwareWorkspace()

  return {
    model,
    dictionaryDetail,
    workflowDetail,
    dictionaryCatalog,
    dictionaryCatalogBusy,
    dictionaryCatalogError,
    softwarePreflight,
    softwarePreflightBusy,
    softwareCaptureArmed,
    softwareCaptureShortcut,
    softwareCaptureResult,
    softwareBusy,
    workspaceBusy,
    refreshing,
    fontRefreshing,
    messages,
    connectDesktopBackend,
    ...workflows,
    ...dictionaries,
    ...software,
  }
}
