export type CapabilityState = 'active' | 'ready' | 'candidate' | 'limited' | 'unavailable'
export type ThemeChoice = 'system' | 'light' | 'dark'

export interface CapabilityEvidence {
  state: CapabilityState
  enabled: boolean
  coverage: number
  detail: string
  generation: number | null
}

export interface SoftwareRecord {
  id: string
  name: string
  description: string
  vendor: string
  version: string
  executableName: string
  executablePath: string | null
  monogram: string
  lastUsed: string | null
  locale: string
  connected: boolean
  translation: CapabilityEvidence
  font: CapabilityEvidence
  observe: CapabilityEvidence
  locations: Array<{ id: string; label: string }>
}

export interface DictionarySummary {
  id: string
  name: string
  description: string
  locale: string
  hookTypeId: string | null
  revision: number
  entryCount: number
}

export interface HookTypeOption {
  id: string
  label: string
}

export type DictionaryDefaultFont
  = { kind: 'unchanged' }
    | { kind: 'substitute'; family: string }

export type DictionaryEntryFont
  = { kind: 'inherit' }
    | { kind: 'unchanged' }
    | { kind: 'substitute'; family: string }

export interface DictionaryRuleContext {
  kind: string
  key: string
}

export interface DictionaryRule {
  location: string
  context: DictionaryRuleContext | null
  source: string
  translation: string | null
  font: DictionaryEntryFont
  adapterIds: string[]
}

export interface DictionaryDetail {
  id: string
  name: string
  description: string
  locale: string
  hookTypeId: string | null
  revision: number
  defaultFont: DictionaryDefaultFont
  entries: DictionaryRule[]
}

export interface WorkflowTarget {
  softwareId: string
  dictionaryIds: string[]
}

export interface WorkflowSummary {
  id: string
  name: string
  description: string
  revision: number
  softwareIds: string[]
  dictionaryIds: string[]
  targets: WorkflowTarget[]
}

export interface WorkflowDetail {
  id: string
  name: string
  description: string
  revision: number
  targets: WorkflowTarget[]
}

export interface WorkflowActivation {
  workflowId: string
  revision: number
}

export interface WorkflowTargetRuntime {
  softwareId: string
  discovered: boolean
  active: boolean
  translationRequested: boolean
  fontRequested: boolean
  translationActive: boolean
  fontActive: boolean
  appliedGeneration: number | null
}

export interface WorkflowRuntimeStatus {
  workflowId: string
  targets: WorkflowTargetRuntime[]
  errors: Record<string, string>
}

export interface DesktopSnapshot {
  selectedSoftwareId: string | null
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  workflows: WorkflowSummary[]
  activations: WorkflowActivation[]
  workflowRuntimeStatus: Record<string, WorkflowRuntimeStatus>
  hookTypes: HookTypeOption[]
  fontFamilies: string[]
}

export interface WorkflowCommandResult {
  definition: WorkflowDetail
  activation: { workflowId: string; enabled: boolean }
  runtime: WorkflowRuntimeStatus
}

export interface DesktopModel extends DesktopSnapshot {
  dictionaryDetails: Record<string, DictionaryDetail>
  workflowDetails: Record<string, WorkflowDetail>
  theme: ThemeChoice
  translationSource: string
}

export const STORAGE_KEY = 'glyphshift.workflow-product-model'

export function emptyModel(): DesktopModel {
  return {
    selectedSoftwareId: null,
    software: [],
    dictionaries: [],
    workflows: [],
    activations: [],
    workflowRuntimeStatus: {},
    hookTypes: [],
    fontFamilies: [],
    dictionaryDetails: {},
    workflowDetails: {},
    theme: 'dark',
    translationSource: '',
  }
}
