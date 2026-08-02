import type { CommandError } from './commandError'

export type CapabilityState = 'active' | 'ready' | 'candidate' | 'limited' | 'unavailable'
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

export interface DictionaryMetadata {
  id: string
  releaseVersion: string
  name: string
  description: string
  sourceLocale: string
  targetLocale: string
  authors: string[]
  license: string | null
  homepage: string | null
  tags: string[]
}

export interface DictionarySummary {
  metadata: DictionaryMetadata
  revision: number
  entryCount: number
}

export interface DictionaryEntry {
  source: string
  translation: string
}

export interface DictionaryDetail {
  metadata: DictionaryMetadata
  revision: number
  entries: DictionaryEntry[]
}

export interface FontProfileMetadata {
  id: string
  name: string
  description: string
}

export interface FontProfileSummary {
  metadata: FontProfileMetadata
  revision: number
  families: string[]
  resolvedFamily: string | null
}

export interface FontProfileDetail extends FontProfileSummary {}

export interface AdapterOption {
  id: string
  name: string
  version: string
  summary: string
  platforms: string[]
  technologies: string[]
  features: string[]
  technicalTarget: string
  configuration: 'none'
}

export interface CaptureSummary {
  sessionId: string
  softwareId: string
  adapterIds: string[]
  status: 'active' | 'completed' | 'failed'
  entryCount: number
  droppedObservations: number
}

export interface CaptureCatalogEntry {
  source: string
  adapterId: string
  count: number
  firstSeenMs: number
  lastSeenMs: number
}

export interface CaptureResult {
  catalog: {
    schema: 'glyphshift.capture-catalog/1'
    sessionId: string
    startedAtMs: number
    stoppedAtMs: number
    droppedObservations: number
    entries: CaptureCatalogEntry[]
  }
  dictionaryDraft: {
    schema: 'glyphshift.dictionary-draft/1'
    sourceSessionId: string
    entries: DictionaryEntry[]
  }
}

export interface WorkflowAdapterPlan {
  strategy: 'parallel'
  adapterIds: string[]
}

export type FontProfileScope
  = { kind: 'all' }
    | { kind: 'locations'; locationIds: string[] }

export interface FontProfileBinding {
  fontProfileId: string
  scope: FontProfileScope
}

export interface WorkflowTarget {
  softwareId: string
  adapterPlan: WorkflowAdapterPlan
  dictionaryIds: string[]
  fontBindings: FontProfileBinding[]
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
  errors: Record<string, CommandError>
}

export interface DesktopSnapshot {
  selectedSoftwareId: string | null
  software: SoftwareRecord[]
  dictionaries: DictionarySummary[]
  fontProfiles: FontProfileSummary[]
  workflows: WorkflowSummary[]
  activations: WorkflowActivation[]
  workflowRuntimeStatus: Record<string, WorkflowRuntimeStatus>
  adapters: AdapterOption[]
  fontFamilies: string[]
  capture: CaptureSummary | null
}

export interface WorkflowCommandResult {
  definition: WorkflowDetail
  activation: { workflowId: string; enabled: boolean }
  runtime: WorkflowRuntimeStatus
}

export interface DesktopModel extends DesktopSnapshot {
  dictionaryDetails: Record<string, DictionaryDetail>
  fontProfileDetails: Record<string, FontProfileDetail>
  workflowDetails: Record<string, WorkflowDetail>
}

export const STORAGE_KEY = 'glyphshift.composable-product-model.v2'

export function emptyModel(): DesktopModel {
  return {
    selectedSoftwareId: null,
    software: [],
    dictionaries: [],
    fontProfiles: [],
    workflows: [],
    activations: [],
    workflowRuntimeStatus: {},
    adapters: [],
    fontFamilies: [],
    capture: null,
    dictionaryDetails: {},
    fontProfileDetails: {},
    workflowDetails: {},
  }
}
